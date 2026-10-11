<script setup lang="ts">
// 添加实例窗口（独立窗口）
// 四种模式：从头新建 / 导入压缩包 / 添加文件夹 / 在线实例
//
// 拆分：数据源与联动在 composables/（版本列表、加载器、文件树），
// 局部 UI 如模式切换、分组框、三个弹窗、浮动进度都在 parts/，四个模式表单在 modes/。
import { computed, nextTick, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import DropOverlay from "../../components/ui/DropOverlay.vue";
import ModeTabs from "./parts/ModeTabs.vue";
import GroupCombo from "./parts/GroupCombo.vue";
import NameConflictModal from "./parts/NameConflictModal.vue";
import ContinueModal from "./parts/ContinueModal.vue";
import PackProgressModal from "./parts/PackProgressModal.vue";
import NewMode from "./modes/NewMode.vue";
import ArchiveMode from "./modes/ArchiveMode.vue";
import FolderMode from "./modes/FolderMode.vue";
import OnlineMode from "./modes/OnlineMode.vue";
import { useAddVersions } from "./composables/useAddVersions";
import { useLoaders, NO_VERSION_LOADERS } from "./composables/useLoaders";
import { useFileTree } from "./composables/useFileTree";
import { useUnlisteners } from "../../composables/useUnlisteners";
import { useFileDrop } from "../../composables/useFileDrop";
import type { FileDropPayload } from "../../composables/useFileDrop";
import { buildTree, type FileNode } from "../../lib/fileTree";
import {
  api,
  onCloseBlocked,
  onAddNameConflict,
  onAddPackProgress,
  answerNameConflict,
} from "../../lib/api";
import { showToast } from "../../lib/toast";
import { KEYS, emitChange, writeRaw } from "../../lib/storage";
import { usePageActive } from "../../lib/pageActive";
import { t, tErr } from "../../lib/i18n";
import { dropFromUrl, isTauri, multiWindow, openWindow, windowParams } from "../windowManager";
import type { WindowDropParam } from "../windowManager";
import { commands } from "../../lib/bindings";
import type { FolderInstanceDto, GroupDto, PackProgressDto } from "../../lib/bindings";
import { ADD_MODES, isModpackPackType, type AddMode } from "./types";

const emit = defineEmits<{
  /** 请求关闭本窗口 */
  (e: "close"): void;
}>();

// ================= 模式与通用字段 =================

const addMode = ref<AddMode>("new");
const newName = ref("");
const newVersion = ref("");
const addGroup = ref("");
const loaderPath = ref("");
const creating = ref(false);
const addError = ref("");

/** 校验失败的字段：底部提示 + 该字段红边、自动聚焦 */
type ErrorField = "" | "name" | "version" | "archive" | "folder" | "url";
const addErrorField = ref<ErrorField>("");

/** 关窗中：在途请求的结果一律丢弃（同时用于停止版本轮询） */
let leaving = false;
const isLeaving = () => leaving;

/** 创建按钮文案随模式变化 */
const BUTTON_KEYS: Record<AddMode, string> = {
  new: "add.btnNew",
  archive: "add.btnArchive",
  folder: "add.btnFolder",
  online: "add.btnOnline",
};
const createLabel = computed(() => {
  if (creating.value) return t("add.creating");
  // 压缩包模式选了整合包类型：这一步实际是"安装整合包"，走的是下载整合包那套安装任务
  // （见后端 add_import_archive），按钮上直说，别让用户以为只是解开压缩包
  if (addMode.value === "archive" && isModpackPackType(addPackType.value)) {
    return t("add.installPack");
  }
  return t(BUTTON_KEYS[addMode.value]);
});

// ================= 数据源 =================

const {
  versions,
  versionTypes,
  packTypes,
  verTypes,
  verLoading,
  filteredVersions,
  fetchOptionLists,
  loadVersions,
  startPolling,
  refreshVersions,
} = useAddVersions(isLeaving);

const {
  loaders,
  loader,
  loaderVersion,
  loaderVersions,
  loaderLoading,
  loaderVerLoading,
  queryStep,
  queryTotal,
  refreshSupportLoaders,
  refreshLoaderVersions,
  dropPending,
  syncCloseGuard,
} = useLoaders(newVersion, isLeaving);

// ================= 分组 =================

const groups = ref<GroupDto[]>([]);
const groupOpen = ref(false);

// ================= 实例名（自动填写） =================

/** 用户手动改过名字后不再自动填写 */
let nameEdited = false;

/** 用户在名字输入框手动输入：非空则停止自动填写，清空则恢复 */
function onNameInput(e: Event) {
  const value = (e.target as HTMLInputElement).value;
  nameEdited = value.trim().length > 0;
  newName.value = value;
}

/**
 * 自动实例名：`游戏版本-加载器-加载器版本`（如 26.2-Fabric-0.19.3）；
 * 原版只有游戏版本，自定义 / 未拉到加载器版本时省略对应段
 */
function autoName() {
  if (addMode.value !== "new" || nameEdited) return;
  const parts = [newVersion.value];
  if (newVersion.value && loader.value !== "normal") {
    parts.push(t(`add.loader.${loader.value}`));
    if (loaderVersion.value) parts.push(loaderVersion.value);
  }
  newName.value = parts.filter(Boolean).join("-");
}

// 版本 / 加载器 / 加载器版本变化时刷新自动名；切换加载器时会先无版本号，列表到达后补全
watch([newVersion, loader, loaderVersion], autoName);

// ================= 模式二：导入压缩包（路径框 + 内容树） =================

const addPackType = ref("curseforge");
const addArchivePath = ref("");
const archiveInput = ref<HTMLInputElement | null>(null);

const {
  tree: archiveTree,
  checked: archiveChecked,
  expanded: archiveExpanded,
  reset: resetArchive,
  setAll: setAllArchive,
  toggleFile: onArchiveToggleFile,
  toggleDir: onArchiveToggleDir,
  toggleExpand: onArchiveToggleExpand,
  unselectedKeys: archiveUnselected,
} = useFileTree();

function onArchivePick(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  applyArchivePath(`C:\\Users\\demo\\Downloads\\${file.name}`);
}

/** 应用压缩包路径并生成内容树（默认全部选中、不展开目录） */
async function applyArchivePath(path: string) {
  addArchivePath.value = path;
  // 浏览器回退（无后端）：mock 内容树 + 按文件名识别
  if (!isTauri()) {
    resetArchive(buildTree(MOCK_ARCHIVE_FILES), true);
    detectMockPack(path);
    return;
  }
  // 读条目 + 识别类型都是磁盘活，期间给个滚动进度条并挡住模式切换
  archiveScanning.value = true;
  try {
    // 读取压缩包真实条目（目录以 / 结尾，buildTree 直接解析）
    try {
      const list = await commands.add.listArchive(path);
      if (isLeaving()) return;
      resetArchive(buildTree(list), true);
    } catch {
      if (isLeaving()) return;
      resetArchive();
      addError.value = t("add.archiveReadFail");
      return;
    }
    // 识别整合包类型，自动填入类型和实例名（用户改过名字则不动）
    try {
      const detected = await api.addDetectArchive(path);
      if (isLeaving()) return;
      addPackType.value = detected.packType;
      if (!nameEdited && detected.name) {
        newName.value = detected.name;
      }
    } catch {
      // 识别失败保持手动选择
    }
  } finally {
    archiveScanning.value = false;
  }
}

/** 浏览器回退的压缩包识别：mock 树里有 manifest.json 视为 CurseForge，名字取文件名 */
function detectMockPack(path: string) {
  if (!MOCK_ARCHIVE_FILES.includes("manifest.json")) return;
  addPackType.value = "curseforge";
  const stem = path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "";
  if (!nameEdited && stem) {
    newName.value = stem;
  }
}

/** 选择压缩包：Tauri 用系统文件对话框，浏览器回退到文件输入 */
async function pickArchive() {
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("add.archive"),
      multiple: false,
      filters: [{ name: "Modpack", extensions: ["zip", "mrpack"] }],
    });
    if (typeof picked === "string") applyArchivePath(picked);
    return;
  }
  archiveInput.value?.click();
}

/** 压缩包读取 / 类型识别中：整合包条目多、要解析 manifest，会明显耗时 */
const archiveScanning = ref(false);

// ================= 模式三：添加文件夹，路径框 + 实例列表 / 内容树，懒加载 =================

const addFolderPath = ref("");
const folderInput = ref<HTMLInputElement | null>(null);

/** 扫描出的可导入实例；空表示目录本身就是实例目录，走内容树那条路 */
const folderFound = ref<FolderInstanceDto[]>([]);
/** 已勾选要导入的实例路径 */
const folderPicked = ref<Set<string>>(new Set());
const folderScanning = ref(false);
/** 扫描失败的原因：扫描命令报错时显示出来，别静默当成"没扫到" */
const folderScanError = ref("");
/** 待确认"是否允许读取"的目录（非空 = 确认框打开着） */
const scanAsk = ref("");

const {
  tree: folderTree,
  checked: folderChecked,
  expanded: folderExpanded,
  reset: resetFolder,
  setAll: setAllFolder,
  toggleFile: onFolderToggleFile,
  toggleDir: onFolderToggleDir,
  toggleExpand: onFolderToggleExpand,
} = useFileTree();

function onFolderPick(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  const folder = file.webkitRelativePath.split("/")[0];
  addFolderPath.value = `C:\\Users\\demo\\.minecraft\\versions\\${folder}`;
  // 浏览器回退：用 mock 内容树（默认不勾选、不展开）
  resetFolder(buildTree(MOCK_FOLDER_FILES));
}

/**
 * 选择文件夹：Tauri 用系统目录对话框
 *
 * 选完先弹确认框问"是否允许读取这个目录"，允许了才扫（见 confirmScan）。
 * 手动输入 / 粘贴路径不走这里，用路径框右边的「扫描目录」按钮自己触发。
 */
async function pickFolder() {
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("add.folder"),
      directory: true,
      multiple: false,
    });
    if (typeof picked === "string") {
      addFolderPath.value = picked;
      scanAsk.value = picked;
    }
    return;
  }
  folderInput.value?.click();
}

/** 确认框里点了"允许"：真去扫 */
function confirmScan() {
  const path = scanAsk.value;
  scanAsk.value = "";
  if (path) void scanFolder();
}

/**
 * 扫描选中的目录，找出其中可导入的实例（默认全选）
 *
 * 没扫到实例说明这个目录**本身就是实例目录**，退回"整目录当一个实例"的内容树。
 */
async function scanFolder() {
  const path = addFolderPath.value.trim();
  if (!path) return;
  folderScanning.value = true;
  folderScanError.value = "";
  try {
    const found = await api.addScanFolder(path);
    // 扫描期间路径又被改了：这次结果作废，别把它贴到新路径上
    if (addFolderPath.value.trim() !== path) return;
    folderFound.value = found;
    folderPicked.value = new Set(found.map((item) => item.path));
    if (!found.length) await loadFolderTree(path);
  } catch (e) {
    if (addFolderPath.value.trim() !== path) return;
    folderFound.value = [];
    folderPicked.value = new Set();
    // 扫描失败要说出来：以前是静默当"没扫到"，界面看起来像功能没生效
    folderScanError.value = tErr(e);
  } finally {
    folderScanning.value = false;
  }
}

/** 勾选 / 取消某个扫到的实例 */
function toggleFoundInstance(path: string, on: boolean) {
  const next = new Set(folderPicked.value);
  if (on) next.add(path);
  else next.delete(path);
  folderPicked.value = next;
}

function setAllFoundInstances(on: boolean) {
  folderPicked.value = on ? new Set(folderFound.value.map((item) => item.path)) : new Set();
}

// ================= 拖拽导入：主窗口拖入的文件 / 文件夹转到这里 =================

/** 拖入的路径是否已经应用过：多窗口模式下 URL 上的参数会一直留着，只能消费一次 */
let dropApplied = false;

/** 按拖入路径进对应模式：压缩包交给 applyArchivePath，目录走文件夹模式并填好地址 */
async function applyDrop(drop: WindowDropParam) {
  if (drop.kind === "folder") {
    addMode.value = "folder";
    addFolderPath.value = drop.path;
    // 与 pickFolder 一致：先问"是否允许读取这个目录"，允许了才扫
    scanAsk.value = drop.path;
    return;
  }
  addMode.value = "archive";
  await applyArchivePath(drop.path);
}

/**
 * 消费"主窗口拖入的路径"（参数从哪读见 windowManager 的 targetDrop）
 *
 * - 多窗口：参数挂在 URL 上，会一直留着，靠 `dropApplied` 只认一次；
 *   再拖一次会走 window_reopen_window 重载页面，组件重建、标志自然复位
 * - 单窗口：参数在 `windowParams` 里，读完立刻清掉，所以"再拖一次"能重新触发。
 */
async function consumeDrop() {
  if (!multiWindow.value) {
    const drop = windowParams.value.drop;
    if (!drop) return;
    windowParams.value = { ...windowParams.value, drop: null };
    await applyDrop(drop);
    return;
  }
  if (dropApplied) return;
  const drop = dropFromUrl();
  if (!drop) return;
  dropApplied = true;
  await applyDrop(drop);
}

// 单窗口模式下已经停在本页时不会再触发 onActivated，靠参数变化接住
watch(
  () => windowParams.value.drop,
  () => void consumeDrop(),
);

/**
 * 本窗口自己也接拖入：拖进来的整合包 / 文件夹直接进对应模式
 *
 * 与主窗口那条路共用 [`applyDrop`] —— 那边是"带着路径把本窗口开起来"，
 * 这边是"窗口已经开着，直接拖进来"，最后落到同一个模式切换上。
 */
async function onFilesDropped(payload: FileDropPayload) {
  // 正在创建 / 扫描 / 读压缩包：模式切换本身就禁用着（busy），
  // 这里也不能把在跑的流程丢在半路（archiveScanning 这类状态是共用的）
  if (busy.value) {
    showToast(t("add.dropBusy"));
    return;
  }
  // 本窗口只认文件（整合包 / 文件夹）：网页卡片那种文本是账户窗口的事
  const first = payload.paths[0];
  if (!first) return;
  if (payload.paths.length > 1) showToast(t("drop.onlyFirst"));
  let probe: { kind: string; url: string };
  try {
    probe = await commands.windows.probeDrop(first);
  } catch (e) {
    showToast(tErr(e));
    return;
  }
  if (probe.kind !== "folder" && probe.kind !== "archive") {
    showToast(t("drop.notModpack"));
    return;
  }
  await applyDrop({ path: first, kind: probe.kind });
}

const { fileDragOver } = useFileDrop(onFilesDropped, {
  // 只接文件拖入：拖文本进来不该亮遮罩，悬停阶段也只有类型清单可看
  accept: (payload) => payload.types.includes("Files"),
});

/** 文件夹模式扫到了实例：此时实例名由各自的目录名决定，「实例名称」那个输入框无意义 */
const folderScanned = computed(() => addMode.value === "folder" && folderFound.value.length > 0);

/**
 * 有在途的耗时操作（创建中 / 扫描目录中 / 读压缩包中）
 *
 * 期间禁用模式切换与创建按钮：切换会把正在跑的流程丢在半路，
 * 创建则还没拿到扫描结果（比如剩下的实例），都不该点得动。
 */
const busy = computed(() => creating.value || folderScanning.value || archiveScanning.value);

/** 列出目录直接内容为树节点（目录标记 lazy，展开时再加载） */
async function listDirNodes(dirPath: string, rel: string): Promise<FileNode[]> {
  const entries = await commands.add.listDir(dirPath);
  return entries.map((en) => {
    const key = rel ? `${rel}/${en.name}` : en.name;
    const node: FileNode = { key, name: en.name, isDir: en.isDir, children: [] };
    if (en.isDir) node.lazy = true;
    return node;
  });
}

/** 加载文件夹根目录内容树（默认不展开、不勾选） */
async function loadFolderTree(rootPath: string) {
  try {
    const nodes = await listDirNodes(rootPath, "");
    if (isLeaving()) return;
    resetFolder(nodes);
  } catch {
    if (isLeaving()) return;
    resetFolder();
    addError.value = t("add.folderReadFail");
  }
}

/** 懒加载：展开目录时读取其子节点 */
async function onFolderLazyLoad(node: FileNode) {
  node.lazy = false;
  const dirPath = folderPathOf(node.key);
  try {
    node.children = await listDirNodes(dirPath, node.key);
  } catch {
    node.children = [];
  }
}

/** 相对 key → 完整路径 */
function folderPathOf(rel: string): string {
  const root = addFolderPath.value.replace(/[\\/]+$/, "");
  return `${root}\\${rel.replace(/\//g, "\\")}`;
}

// ================= 模式四：在线实例 =================

const addUrl = ref("");

// ================= mock 文件列表（仅浏览器预览用） =================

const MOCK_ARCHIVE_FILES = [
  "manifest.json",
  "modlist.html",
  "minecraft/",
  "minecraft/mods/",
  "minecraft/mods/jei-1.20.1-15.2.0.27.jar",
  "minecraft/mods/sodium-fabric-0.5.8.jar",
  "minecraft/config/",
  "minecraft/config/jei.toml",
  "overrides/",
];
const MOCK_FOLDER_FILES = [
  "options.txt",
  "servers.dat",
  "config/",
  "config/mouse-tweaks.json",
  "mods/",
  "mods/example-mod-1.0.0.jar",
  "resourcepacks/",
  "saves/",
  "saves/新世界/level.dat",
  "logs/",
];

// ================= 字段校验 =================

const nameInput = ref<HTMLInputElement | null>(null);

/** 记一条字段级错误：底部提示 + 该字段高亮（名称还会自动聚焦） */
function failField(field: Exclude<ErrorField, "">, msg: string) {
  addErrorField.value = field;
  addError.value = msg;
}

/** 字段内容变了就把它的错误清掉 */
function clearFieldError(field: Exclude<ErrorField, "">) {
  if (addErrorField.value !== field) return;
  addErrorField.value = "";
  addError.value = "";
}

watch(newName, () => clearFieldError("name"));
watch(newVersion, () => clearFieldError("version"));
watch(addArchivePath, () => clearFieldError("archive"));
/**
 * 路径一变就把上一次的扫描结果清掉
 *
 * **不自动扫描**：选目录走"确认框 → 允许了再扫"（pickFolder → scanAsk → confirmScan），
 * 手输 / 粘贴路径用路径框右边的「扫描目录」按钮触发。扫描要读磁盘，不该在用户
 * 还在敲路径的时候就自己跑起来。
 */
watch(addFolderPath, () => {
  clearFieldError("folder");
  folderFound.value = [];
  folderPicked.value = new Set();
  folderScanError.value = "";
});
watch(addUrl, () => clearFieldError("url"));

// 名称错误：聚焦输入框；版本 / 路径的聚焦交给各模式组件自己处理
watch(addErrorField, async (field) => {
  if (field !== "name") return;
  await nextTick();
  nameInput.value?.focus();
});

// ================= 创建 =================

/**
 * 本页面是否在前台
 *
 * 单窗口模式下 App.vue 把窗口组件 KeepAlive 缓存，切走只是藏起来 —— 安装却在后台继续跑，
 * 异步回来时（装完 / 后端要重名答复）必须先问一句"这个上下文还在不在"：
 * 弹窗由 BaseModal 自己收起来（见 lib/pageActive.ts），而**关窗动作**得在这里挡住
 * —— 页面已切走时 `emit('close')` 会被 App.vue 当成"关掉当前窗口"，当前页是主页面，
 * 那就是退出启动器。
 */
const pageActive = usePageActive();

async function create() {
  // 防重复提交：按钮 disabled 之外，Enter / 连续点击也在这里挡住
  if (creating.value || nameConflict.value || askContinue.value || packProgress.value) return;

  // 文件夹模式扫到了实例时：每个实例用自己目录的名字，不用用户填实例名
  const scanned = folderScanned.value;
  const name = newName.value.trim();
  if (!scanned && !name) return failField("name", t("add.nameEmpty"));
  if (addMode.value === "new" && !newVersion.value) {
    return failField("version", t("add.versionEmpty"));
  }
  if (addMode.value === "archive" && !addArchivePath.value.trim()) {
    return failField("archive", t("add.archiveEmpty"));
  }
  if (addMode.value === "folder" && !addFolderPath.value.trim()) {
    return failField("folder", t("add.folderEmpty"));
  }
  if (scanned && !folderPicked.value.size) {
    return failField("folder", t("add.folderPickNone"));
  }
  if (addMode.value === "online" && !addUrl.value.trim()) {
    return failField("url", t("add.urlEmpty"));
  }

  const group = addGroup.value;
  addError.value = "";
  addErrorField.value = "";
  creating.value = true;
  try {
    let uuid: string;
    // 分组按 uuid 传给后端：手输的组名可能已有、也可能要现建一个
    const groupId = await api.resolveGroupId(group);
    // 询问"是否继续添加"时显示的名字：单个导入用实例名，批量用数量
    let doneName = name;
    if (addMode.value === "new") {
      uuid = await api.addCreateNew(
        name,
        newVersion.value,
        loader.value,
        loader.value === "custom" ? loaderPath.value || null : loaderVersion.value || null,
        groupId,
      );
    } else if (addMode.value === "archive") {
      // 文件树未勾选的文件按压缩包内条目名排除（全选时传 null）
      const unselected = archiveUnselected();
      uuid = await api.addImportArchive(
        addArchivePath.value.trim(),
        addPackType.value,
        name,
        groupId,
        unselected.length ? unselected : null,
      );
    } else if (addMode.value === "folder") {
      if (scanned) {
        // 逐个导入：每个扫到的实例各建一个，名字取它自己的目录名
        const targets = folderFound.value.filter((item) => folderPicked.value.has(item.path));
        uuid = "";
        for (const item of targets) {
          uuid = await api.addImportFolder(item.path, item.name, groupId);
        }
        doneName = t("add.folderImported", { n: targets.length });
      } else {
        uuid = await api.addImportFolder(addFolderPath.value.trim(), name, groupId);
      }
    } else {
      uuid = await api.addImportUrl(addUrl.value.trim(), name, groupId);
    }
    if (isLeaving()) return;
    // 通知主窗口选中新实例（后端已发 instance-change 刷新列表），然后询问是否继续添加
    writeRaw(KEYS.addedInstance, uuid);
    emitChange(KEYS.addedInstance, uuid);
    addedName.value = doneName;
    askContinue.value = true;
  } catch (e) {
    packProgress.value = null;
    addError.value = t("add.createFail", { msg: tErr(e) });
  } finally {
    creating.value = false;
  }
}

// ================= 整合包安装进度 =================

/** 安装进度（add-pack-progress 事件驱动，null = 未在安装） */
const packProgress = ref<PackProgressDto | null>(null);

// ================= 创建成功后的继续添加询问 =================

/** 刚创建成功的实例名（弹窗显示用） */
const addedName = ref("");
/** 创建成功后弹出「是否继续添加」询问 */
const askContinue = ref(false);

/** 继续添加：清空一次性表单（保留模式 / 版本列表等数据源），回到干净表单 */
function continueAdding() {
  askContinue.value = false;
  addError.value = "";
  addErrorField.value = "";
  newName.value = "";
  nameEdited = false;
  loaderPath.value = "";
  loaderVersion.value = "";
  addArchivePath.value = "";
  resetArchive();
  addFolderPath.value = "";
  resetFolder();
  folderFound.value = [];
  folderPicked.value = new Set();
  addUrl.value = "";
}

/** 不继续：丢掉本窗口的查询状态（加载器支持列表 / 加载器版本）并直接关窗 */
async function finishAdding() {
  askContinue.value = false;
  // 页面已经切走：没有"本窗口"可关（见 pageActive 的说明），本次添加到此为止即可
  if (!pageActive.value) return;
  leaving = true;
  // 查询在途时后端有关闭保护（CloseRequested 被拒），先解除再关，否则窗关不掉
  dropPending();
  await api.setCloseGuard(false).catch(() => { });
  emit("close");
}

// ================= 实例重名确认 =================

/** 当前弹出的重名确认（kind：overwrite 覆盖 / rename 自动改名；null = 无） */
const nameConflict = ref<{ id: number; kind: string; name: string } | null>(null);

/** 答复后端创建流程；关闭弹窗视为拒绝，避免创建流程挂起 */
async function answerConflict(answer: boolean) {
  const cur = nameConflict.value;
  nameConflict.value = null;
  if (cur) {
    await answerNameConflict(cur.id, answer).catch(() => { });
  }
}

// ================= Esc：先收浮层，再关窗 =================

function onKeyDown(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  // 1) 分组下拉开着：先收下拉
  if (groupOpen.value) {
    groupOpen.value = false;
    return;
  }
  // 2) 有未决弹窗 / 正在创建：不关窗，否则会丢掉后端等着的答复或半个创建流程
  if (nameConflict.value || askContinue.value || packProgress.value || creating.value) return;
  // 3) 其余情况等同点标题栏的关闭按钮（查询在途时后端会拦下并提示）
  emit("close");
}

// ================= 初始化 =================

const { track } = useUnlisteners();

/** 键盘监听：单窗口模式下窗口被 KeepAlive 缓存，切走时 onUnmounted 不会跑 ——
 *  必须自己在停用 / 启用之间装卸，否则在别的窗口按 Esc 会被当成"关闭本窗口" */
function bindKeys() {
  document.addEventListener("keydown", onKeyDown);
}

function unbindKeys() {
  document.removeEventListener("keydown", onKeyDown);
}

// 单窗口模式：切走卸掉键盘监听，切回再装上；onActivated 首次挂载也会跑，重复 add 同一函数无副作用。
//
// 这里刻意**不**动 `leaving`：切走只是"藏起来"，在途的重名确认 / 进度事件还得让用户能答上
// （回到本窗口时弹窗还在），置 leaving 会把它们吞掉、把安装流程卡住。
//
// 但**关闭保护必须松手**：那两个查询在后台继续跑，而守卫是按窗口 uuid 挂在后端上的。
// 用户在别的页面（比如下载整合包）关窗口时，不该因为这里还有个查询在跑就被拦下、
// 弹一句"正在获取数据"。回到本页时下面的 watch 会按当时状态重新判定。
//
// 模型（取消令牌 / 关闭保护 / 重名应答通道）也跟着页面的启停走：
// 挂载时建、切走时释放，别让它常驻占内存（见 api.ensureWindowModel / dropWindowModel）。
// **创建任务在途时不释放** —— 取消令牌就在模型里，丢了就没法取消，回到本页也会拿到一份空模型。
onActivated(() => {
  bindKeys();
  // 切回来时把模型补回来（切走时释放掉了），再按当时状态恢复守卫
  void api.ensureWindowModel("add").catch(() => { });
  syncCloseGuard();
  // 单窗口模式下"带着拖入路径跳回本页"走这里
  void consumeDrop();
});
onDeactivated(() => {
  unbindKeys();
  void api.setCloseGuard(false).catch(() => { });
  if (creating.value || packProgress.value) return;
  void api.dropWindowModel("add").catch(() => { });
});

onMounted(async () => {
  bindKeys();
  // 先把本页的模型准备好：创建 / 取消 / 关闭保护 / 重名确认都依赖它。
  // 多窗口模式下真实窗口由后端建窗时已经建好，这里是幂等的
  await api.ensureWindowModel("add").catch(() => { });
  // 关闭被拒绝（查询数据期间后端拒关）：弹提示说明原因
  track(onCloseBlocked(() => showToast(t("add.closeBlocked"))));
  // 实例重名确认（后端创建流程暂停等待答复）
  track(
    onAddNameConflict((e) => {
      nameConflict.value = e;
      // 页面被切走时弹窗要等用户回来才看得见，而后端正停在这一步等答复
      // （安装看着像卡住了）—— 先在当前页面上提示一句，让人知道要回去确认
      if (!pageActive.value) showToast(t("add.conflictAway"));
    }),
  );
  // 整合包安装进度（压缩包 / 网址 / 在线整合包安装共用）
  track(
    onAddPackProgress((e) => {
      packProgress.value = e;
      if (e.state === "done") {
        setTimeout(() => {
          packProgress.value = null;
        }, 600);
      }
    }),
  );

  try {
    // 默认分组（空白名字）不进下拉：输入框留空即默认分组
    groups.value = (await api.getGroups()).filter((g) => g.name.trim());
  } catch {
    groups.value = [];
  }
  // 版本列表与下拉数据源各拉一次；核心尚未就绪时轮询等待（关窗即停）
  await Promise.all([loadVersions(), fetchOptionLists()]);
  if (!versions.value.length) startPolling();
  // 主窗口拖入的路径：多窗口模式下这次挂载就是为它来的（参数挂在 URL 上）
  await consumeDrop();
});

onUnmounted(() => {
  unbindKeys();
  // 直接点标题栏关窗（不走 finishAdding）时也要停止轮询、丢弃在途结果
  leaving = true;
});
</script>

<template>
  <WindowFrame :title="t('add.title')" body-gutter @close="$emit('close')">
    <!-- 内容区：撑满窗口，底部按钮靠 margin-top:auto 钉在右下角 -->
    <div class="add-body">
      <ModeTabs v-model="addMode" :tabs="ADD_MODES" :disabled="busy" />

      <!-- 实例名称 + 分组 -->
      <div class="add-card">
        <div class="field-row-2">
          <div class="add-field">
            <label class="field-label" for="add-name">
              {{ t("add.name") }} <span v-if="!folderScanned" class="req">*</span>
            </label>
            <input id="add-name" ref="nameInput" :value="newName" class="field-input"
              :class="{ 'is-invalid': addErrorField === 'name' }"
              :placeholder="folderScanned ? t('add.folderNameHint') : t('add.namePlaceholder')"
              :disabled="folderScanned" spellcheck="false" autocomplete="off" @input="onNameInput"
              @keydown.enter="create" />
            <!-- 扫到实例时逐个导入，名字各自取目录名，这里填了也没用 -->
            <p v-if="folderScanned" class="field-hint">{{ t("add.folderNameHint") }}</p>
          </div>
          <div class="add-field">
            <label class="field-label">{{ t("add.group") }}</label>
            <GroupCombo v-model="addGroup" v-model:open="groupOpen" :groups="groups" />
          </div>
        </div>
      </div>

      <!-- 模式内容（各模式组件见 modes/ 目录） -->
      <div class="add-card add-card-content">
        <NewMode v-if="addMode === 'new'" :versions="filteredVersions" :versions-loaded="versions.length > 0"
          :ver-loading="verLoading" :ver-types="verTypes" :version-types="versionTypes" :new-version="newVersion"
          :loader="loader" :loaders="loaders" :loader-loading="loaderLoading" :query-step="queryStep"
          :query-total="queryTotal" :loader-version="loaderVersion" :loader-versions="loaderVersions"
          :loader-ver-loading="loaderVerLoading" :no-loader-version="NO_VERSION_LOADERS.includes(loader)"
          :loader-path="loaderPath" :invalid-version="addErrorField === 'version'" @refresh-versions="refreshVersions"
          @refresh-loaders="refreshSupportLoaders" @refresh-loader-versions="refreshLoaderVersions"
          @update:ver-types="verTypes = $event" @update:new-version="newVersion = $event"
          @update:loader="loader = $event" @update:loader-version="loaderVersion = $event"
          @update:loader-path="loaderPath = $event" />
        <ArchiveMode v-else-if="addMode === 'archive'" :path="addArchivePath" :tree="archiveTree"
          :checked="archiveChecked" :expanded="archiveExpanded" :pack-types="packTypes" :pack-type="addPackType"
          :scanning="archiveScanning" :invalid="addErrorField === 'archive'" @update:path="addArchivePath = $event"
          @pick="pickArchive" @toggle-file="onArchiveToggleFile" @toggle-dir="onArchiveToggleDir"
          @toggle-expand="onArchiveToggleExpand" @set-all="setAllArchive" @update:pack-type="addPackType = $event" />
        <FolderMode v-else-if="addMode === 'folder'" :path="addFolderPath" :tree="folderTree" :checked="folderChecked"
          :expanded="folderExpanded" :found="folderFound" :picked="folderPicked" :scanning="folderScanning"
          :scan-error="folderScanError" :invalid="addErrorField === 'folder'" @update:path="addFolderPath = $event"
          @pick="pickFolder" @rescan="scanFolder" @toggle-instance="toggleFoundInstance"
          @set-all-instances="setAllFoundInstances" @toggle-file="onFolderToggleFile" @toggle-dir="onFolderToggleDir"
          @toggle-expand="onFolderToggleExpand" @lazy-load="onFolderLazyLoad" @set-all="setAllFolder" />
        <OnlineMode v-else :url="addUrl" :invalid="addErrorField === 'url'" @update:url="addUrl = $event" />
      </div>

      <p v-if="addError" class="error-text" role="alert">{{ addError }}</p>

      <div class="modal-actions">
        <!-- 整合包入口：浏览 / 安装与安装进度都在「下载整合包」窗口 -->
        <BaseButton class="modpack-entry" @click="openWindow('add_modpack')">
          {{ t("add.downloadModpack") }}
        </BaseButton>
        <BaseButton variant="primary" :disabled="busy" @click="create">
          {{ createLabel }}
        </BaseButton>
      </div>
    </div>

    <!-- 浏览器回退的文件 / 文件夹选择器（Tauri 下走系统对话框，不使用） -->
    <input ref="archiveInput" type="file" accept=".zip,.mrpack" class="hidden-input" @change="onArchivePick" />
    <input ref="folderInput" type="file" webkitdirectory class="hidden-input" @change="onFolderPick" />

    <!-- 实例重名确认：后端创建流程暂停等待答复，关闭视为拒绝 -->
    <NameConflictModal v-if="nameConflict" :kind="nameConflict.kind" :name="nameConflict.name"
      @answer="answerConflict" />

    <!-- 选完目录的二次确认：读目录 = 读磁盘，先问一句再扫 -->
    <BaseModal v-if="scanAsk" :title="t('add.folderAskTitle')" :closable="false" @close="scanAsk = ''">
      <p class="confirm-text">{{ t("add.folderAskText", { path: scanAsk }) }}</p>
      <div class="modal-actions">
        <BaseButton variant="accent" @click="scanAsk = ''">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmScan">{{ t("add.folderAllow") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 创建成功：询问是否继续添加 -->
    <ContinueModal v-if="askContinue" :name="addedName" @continue="continueAdding" @finish="finishAdding" />

    <!-- 整合包安装进度 -->
    <PackProgressModal v-if="packProgress" :progress="packProgress" />

    <!-- 加载器查询进度不再浮在窗口正上方：两条查询都改成就地显示在各自字段下面了，
         见 NewMode 里的 LoaderQueryProgress -->

    <!-- 拖入整合包 / 文件夹时的遮罩（本窗口只接文件，见 accept） -->
    <DropOverlay v-if="fileDragOver" :title="t('add.dropTitle')" />
  </WindowFrame>
</template>

<style scoped>
/* 内容区撑满窗口高度，底部按钮靠 margin-top:auto 钉在右下角（跟随窗口大小） */
.add-body {
  display: flex;
  flex-direction: column;
  min-height: 100%;
}

.add-body>.modal-actions {
  margin-top: auto;
}

/* 整合包入口靠左，与右侧的创建按钮分开；绿色软底，和侧边栏「添加分组」同色系。
   加 .ui-btn 提高优先级，稳定压过 BaseButton 的 .v-plain 底色 */
.ui-btn.modpack-entry {
  margin-right: auto;
  border: 1px solid color-mix(in srgb, var(--green) 42%, transparent);
  background: color-mix(in srgb, var(--green) 12%, transparent);
  color: var(--green);
}

.ui-btn.modpack-entry:hover {
  background: color-mix(in srgb, var(--green) 20%, transparent);
  color: var(--green);
}

/* 内容卡片 */
.add-card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 16px 18px;
  margin-bottom: 14px;
}

/* 模式内容卡片：四个模式保持同一高度，切换时不跳动 */
.add-card-content {
  min-height: 180px;
}

/* 确认弹窗正文（如"是否允许读取目录 xxx"）：路径可能很长，允许换行 */
.confirm-text {
  margin: 0;
  font-size: 13px;
  color: var(--text);
  line-height: 1.6;
  word-break: break-all;
}
</style>
