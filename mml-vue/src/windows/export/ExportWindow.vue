<script setup lang="ts">
// 实例导出窗口：完整 M²L 实例包与通用整合包。
//
// 目标实例按优先级取：openWindow 带入的目标参数 → `export-focus` 事件 → 实例下拉。
// 目标参数单窗口模式走 windowParams，多窗口走 URL uuid；
// `export-focus` 是窗口已存在时再次打开，由壳层推送。
// 打包在 Rust 侧后台任务执行，即 export_run，进度经 export-progress 事件上报。
//
// 勾选分两块：**模组**（`mods` 目录，来自 export_get_info）与**文件**（游戏目录下的懒加载树）。
// 每一条都能填下载地址：Modrinth 格式下填了地址的写进 index 的下载清单，没填的与目录一起打进
// overrides；CurseForge 的清单格式里只有项目号 / 文件号，地址那一块在那个格式下不作数。
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import IconTabs from "../../components/ui/IconTabs.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import HighlightText from "../../components/ui/HighlightText.vue";
import FilePicker from "./parts/FilePicker.vue";
import { isPickedPath } from "./picked";
import InfoExportButton from "./parts/InfoExportButton.vue";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { api, onCloseBlocked, onExportFocus, onExportProgress } from "../../lib/api";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { currentKind, isTauri, multiWindow, targetUuid, windowParams } from "../windowManager";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { useInstanceChange } from "../../composables/useInstanceChange";
import {
  commands,
  type CustomHomeInfoDto,
  type CustomUiInfoDto,
  type ExportEntryDto,
  type ExportInfoDto,
  type ExportProgressDto,
  type GuiConfigDto,
  type InstanceInfoDto,
} from "../../lib/bindings";

/** 请求关闭本窗口 */
const emit = defineEmits<{ (e: "close"): void }>();

/** 本窗口标签：关闭被拦截事件只认发给自己的那一条 */
const myLabel = getCurrentWindow().label;

/** 页签 id：模组 / 文件 / 整合包信息 */
type ExportTab = "mods" | "files" | "meta";

const inTauri = isTauri();

const instances = ref<InstanceInfoDto[]>([]);
const currentUuid = ref("");
const info = ref<ExportInfoDto | null>(null);
const loadingInfo = ref(false);
let infoRequest = 0;

// ---------------- 导出配置 ----------------

/** 导出格式 id */
type PackId = "curseforge" | "modrinth" | "mmc" | "hmcl" | "mml" | "server";

/** 服务器包窗口与实例导出共用一个组件：这个 kind 下锁死成服务器包，不出现格式页签 */
const serverPack = computed(() => currentKind.value === "server_pack");

const pack = ref<PackId>(serverPack.value ? "server" : "mml");
const includeLauncher = ref(false);

/** 服务器包的三组选择：配置 → 自定义内容 → 实例 */
const serverOptions = reactive({
  config: true,
  guiConfig: true,
  page: false,
  css: false,
  home: false,
});
/** 已安装的整页主题 / 自定义样式 */
const customUi = ref<CustomUiInfoDto | null>(null);
/** 已安装的自定义主页面 */
const customHome = ref<CustomHomeInfoDto | null>(null);
/** GUI 配置，导出前检查实例 / 登录方式锁定用 */
const guiConfig = ref<GuiConfigDto | null>(null);
/** 没锁实例或登录方式时的询问弹窗 */
const lockWarnOpen = ref(false);
/** 服务器包多选的实例 uuid，按实例列表顺序 */
const serverPicked = ref<string[]>([]);

/** 读已安装的自定义主题 / 样式 / 主页面，服务器包窗口用 */
async function loadCustomAssets() {
  if (!serverPack.value) return;
  const [ui, home, gui] = await Promise.all([
    commands.customUi.status().catch(() => null),
    commands.customHome.status().catch(() => null),
    commands.windows.getGuiConfig().catch(() => null),
  ]);
  customUi.value = ui;
  customHome.value = home;
  guiConfig.value = gui;
}

// 界面配置不导出时，自定义内容没有开关可落，顺手清掉选择
watch(
  () => serverOptions.guiConfig,
  (on) => {
    if (!on) {
      serverOptions.page = false;
      serverOptions.css = false;
      serverOptions.home = false;
    }
  },
);
const packName = ref("");
const author = ref("");
const version = ref("1.0.0");
const summary = ref("");

/** 勾选的模组：相对游戏目录的路径 → 条目 */
const pickedMods = ref(new Map<string, ExportEntryDto>());
/** 勾选的文件与目录 */
const pickedFiles = ref(new Map<string, ExportEntryDto>());
/** 取消勾选的路径：父目录勾着时单独去掉某一条 */
const unpickedFiles = ref(new Set<string>());
/** 因为模组已勾选而自动取消的文件：取消勾选模组时按这份表还原 */
const autoExcluded = ref(new Set<string>());
/** 路径 → 下载地址；空串表示没填，按 overrides 打包 */
const urls = ref(new Map<string, string>());
/** 路径 → 项目号 / 文件号；模组页可改，改了以后端为准 */
const modIds = ref(new Map<string, { modId: string; fileId: string }>());
/** 模组搜索词 */
const modKeyword = ref("");

/** 当前选项卡：整合包信息 / 模组 / 文件 */
const tab = ref<ExportTab>("meta");

/** 模组页只给能写下载清单的格式：整包格式的模组随文件一起打进去，不需要单独挑 */
const showModsTab = computed(() => pack.value === "curseforge" || pack.value === "modrinth");

/** 实际展示的页签：「直接压缩」只列实例文件；整包格式没有模组页，落到整合包信息 */
const shownTab = computed<ExportTab>(() => {
  if (pack.value === "mml") return "files";
  if (tab.value === "mods" && !showModsTab.value) return "meta";
  return tab.value;
});

/** 格式选项：两种通用清单格式，加两种整包实例格式（供别的启动器导入） */
const packOptions: Array<{ value: PackId; label: string }> = [
  { value: "mml", label: t("winExport.directZip") },
  { value: "curseforge", label: "CurseForge" },
  { value: "modrinth", label: "Modrinth (.mrpack)" },
  { value: "mmc", label: "MMC" },
  { value: "hmcl", label: "HMCL" },
];

/** 分段控件给出的是 string，收口成 PackId */
function onPackChange(value: string) {
  const option = packOptions.find((x) => x.value === value);
  if (option) pack.value = option.value;
}

/** 压缩档位：轻量最快，固实最紧 */
type ZipLevel = "fast" | "normal" | "solid";

const zipOptions: Array<{ value: ZipLevel; label: string }> = [
  { value: "fast", label: t("winExport.zipFast") },
  { value: "normal", label: t("winExport.zipNormal") },
  { value: "solid", label: t("winExport.zipSolid") },
];

const zipLevel = ref<ZipLevel>("fast");

/** 压缩档位切换：分段控件给 string，收口成档位 */
function onZipLevelChange(value: string) {
  const option = zipOptions.find((x) => x.value === value);
  if (option) zipLevel.value = option.value;
}

/** 存盘对话框里的格式名；服务器包不在页签里，单独取名字 */
const packLabel = computed(() =>
  pack.value === "server"
    ? t("winExport.serverPack")
    : (packOptions.find((x) => x.value === pack.value)?.label ?? "Modpack"),
);

/** 导出文件名后缀（按格式） */
const fileExt = computed(() => (pack.value === "modrinth" ? "mrpack" : "zip"));

/** 只有 Modrinth 的清单带下载地址，其余格式整包打包 */
const withUrls = computed(() => pack.value === "modrinth");

/** 自定义加载器只在两种整包格式下会丢：它们的包内没有这个组件位，其余格式原样带着 */
const customLoaderLost = computed(
  () => !!info.value?.customLoader && (pack.value === "mmc" || pack.value === "hmcl"),
);

/** 实例目录下的游戏目录名；「直接压缩」以实例目录为根，需要它来展开子项 */
const GAME_DIR = ".minecraft";

/** 默认跳过的目录名：运行期产物，打进包里没用还占体积；其余默认全勾 */
const DEFAULT_SKIP = new Set([
  "logs",
  "crash-reports",
  "cache",
  ".cache",
  "temp",
  "backup",
  "backups",
]);

/** 文件树的根：直接压缩看实例目录，其余格式看游戏目录 */
const fileBase = computed<"game" | "instance">(() => (pack.value === "mml" ? "instance" : "game"));

/** 直接压缩时把 .minecraft 展开：默认勾的是它的子项，不展开看不见 */
const fileExpand = computed(() => (pack.value === "mml" ? [GAME_DIR] : []));

/**
 * 直接压缩时实例根上的条目是否必选
 *
 * 实例根目录里除 `.minecraft` 之外的东西属于实例本身，少一份导入后就不是同一个实例，
 * 所以固定勾上、不给取消；运行期目录走默认跳过那一套，不强制。
 *
 * - `path`: 相对实例根目录的路径
 *
 * # 返回值
 *
 * 必选返回 `true`；在 `.minecraft` 里、属于运行期目录、非直接压缩格式返回 `false`
 */
function isLockedFile(path: string): boolean {
  if (pack.value !== "mml") return false;
  const [top] = path.split("/");
  return top !== GAME_DIR && !DEFAULT_SKIP.has(top);
}

/**
 * 项目号 / 文件号算不算这个格式认可的在线来源
 *
 * CurseForge 的清单里只有数字编号，别的写法（Modrinth 的 slug）写进去也下载不到，
 * 按不是在线模组处理，让它随 mods 目录打进包。
 *
 * - `modId`: 项目号
 * - `fileId`: 文件号
 *
 * # 返回值
 *
 * 两个编号都成立返回 `true`
 */
function isOnlineId(modId: string, fileId: string): boolean {
  if (!modId || !fileId) return false;
  if (pack.value !== "curseforge") return true;
  return /^\d+$/.test(modId) && /^\d+$/.test(fileId) && Number(modId) > 0 && Number(fileId) > 0;
}

/** 某条模组当前（以改过的编号为准）算不算在线模组 */
function isOnlineMod(path: string): boolean {
  const ids = modIds.value.get(path);
  return !!ids && isOnlineId(ids.modId, ids.fileId);
}

/** CurseForge 下这一条是不是因为编号不是数字而落在线外 */
function cfIdInvalid(path: string): boolean {
  return pack.value === "curseforge" && !isOnlineMod(path);
}

/** 有在线来源的模组：模组页只列这些，标签上的分母也用它 */
const onlineMods = computed(() => (info.value?.mods ?? []).filter((x) => isOnlineMod(x.path)));

/** 有在线来源的模组数 */
const onlineModCount = computed(() => onlineMods.value.length);

/**
 * 过滤后的模组列表
 *
 * 编号不成立的也列出来（灰着不能勾），好让人看见并改对；它们的文件照常打进包。
 */
const mods = computed(() => {
  const list = (info.value?.mods ?? []).filter(
    (x) => (!!x.modId && !!x.fileId) || isOnlineMod(x.path),
  );
  const keyword = modKeyword.value.trim().toLowerCase();
  if (!keyword) return list;
  return list.filter((x) => x.path.toLowerCase().includes(keyword));
});

/** 勾选项里能填地址的：模组与单个文件，目录不算 */
const urlRows = computed(() =>
  [...pickedMods.value.values(), ...pickedFiles.value.values()].filter((x) => !x.dir),
);

/** 文件树的勾选表按路径判在不在，交给子组件只看键 */
const pickedFilePaths = computed(() => new Map([...pickedFiles.value].map(([path]) => [path, true])));

/** 文件页的说明：三种去向（实例目录 / overrides / 整包实例）各一句 */
const fileHint = computed(() => {
  if (pack.value === "mml") return t("winExport.dirHintZip");
  if (showModsTab.value) return t("winExport.dirHint");
  return t("winExport.dirHintPack");
});

/** 选项卡标签：整合包信息在最前，两个勾选区把已选数量挂在标签尾巴上 */
const tabOptions = computed<
  Array<{ id: ExportTab; labelKey: string; icon: string; suffix?: string }>
>(() => {
  const options: Array<{ id: ExportTab; labelKey: string; icon: string; suffix?: string }> = [
    { id: "meta", labelKey: "winExport.tabMeta", icon: "box" },
  ];
  if (showModsTab.value) {
    options.push({
      id: "mods",
      labelKey: "winExport.tabMods",
      icon: "cube",
      suffix: `${pickedMods.value.size}/${onlineModCount.value}`,
    });
  }
  options.push({
    id: "files",
    labelKey: "winExport.tabFiles",
    icon: "folder",
    suffix: `${pickedFiles.value.size}`,
  });
  return options;
});

// ---------------- 实例切换 ----------------

/** 拉选中实例的导出信息，实例名作为包名默认值 */
async function loadInfo() {
  const request = ++infoRequest;
  const uuid = currentUuid.value;
  info.value = null;
  pickedMods.value = new Map();
  pickedFiles.value = new Map();
  unpickedFiles.value = new Set();
  autoExcluded.value = new Set();
  urls.value = new Map();
  modIds.value = new Map();
  if (!uuid) { loadingInfo.value = false; return; }
  loadingInfo.value = true;
  try {
    const data = await api.getExportInfo(uuid);
    if (request !== infoRequest || uuid !== currentUuid.value) return;
    info.value = data;
    // 元数据默认值跟实例走（用户可改）
    packName.value = data.name;
    summary.value = "";
    await applyDefaults(data, uuid, request);
  } catch (e) {
    if (request !== infoRequest || uuid !== currentUuid.value) return;
    showToast(tErr(e));
    info.value = null;
  } finally {
    if (request === infoRequest) loadingInfo.value = false;
  }
}

/** 勾选状态的初始值：能走在线来源的模组与文件全勾（运行期目录除外），下载地址按来源预填 */
async function applyDefaults(data: ExportInfoDto, uuid: string, request: number) {
  pickedFiles.value = new Map();
  urls.value = new Map();

  for (const mod of data.mods) {
    const ids = { modId: mod.modId ?? "", fileId: mod.fileId ?? "" };
    if (mod.url) urls.value.set(mod.path, mod.url);
    modIds.value.set(mod.path, ids);
  }
  pickedMods.value = seedModPicks();

  if (request !== infoRequest) return;
  await reseedFilePicks(uuid);
}

/**
 * 按当前格式挑出能走下载清单的模组
 *
 * 只有 CurseForge / Modrinth 有清单；整包格式（直接压缩、MMC、HMCL）里模组就是普通文件，
 * 由文件树决定，这里不给勾选。
 *
 * # 返回值
 *
 * 路径 → 模组条目；不是清单格式时为空表
 */
function seedModPicks(): Map<string, ExportEntryDto> {
  const picked = new Map<string, ExportEntryDto>();
  if (!showModsTab.value) return picked;
  for (const mod of info.value?.mods ?? []) {
    const ids = modIds.value.get(mod.path) ?? { modId: mod.modId ?? "", fileId: mod.fileId ?? "" };
    // 只勾编号成立的模组；其余随 mods 目录打进包
    if (isOnlineId(ids.modId, ids.fileId)) picked.set(mod.path, mod);
  }
  return picked;
}

/** 文件默认勾选的请求序号：换实例 / 换格式时，旧的异步结果直接丢掉 */
let picksSeq = 0;

/**
 * 按当前格式铺文件树的默认勾选
 *
 * 先全勾、再把运行期目录单独取消：直接压缩以实例目录为根，`.minecraft` 整目录勾上，
 * 里面的内容靠继承一起算上；其余格式以游戏目录为根，根下逐项勾。
 *
 * - `uuid`: 目标实例
 */
async function reseedFilePicks(uuid: string) {
  const seq = ++picksSeq;
  const picked = new Map<string, ExportEntryDto>();
  const unpicked = new Set<string>();
  try {
    if (pack.value === "mml") {
      // 实例根上全勾，运行期目录随后单独排除；`.minecraft` 里面的内容靠继承算上
      for (const entry of await api.listExportInstanceDir(uuid, "")) picked.set(entry.path, entry);
      for (const name of DEFAULT_SKIP) {
        unpicked.add(name);
        unpicked.add(`${GAME_DIR}/${name}`);
      }
    } else {
      for (const entry of await api.listExportDir(uuid, "")) {
        if (!DEFAULT_SKIP.has(entry.path)) picked.set(entry.path, entry);
      }
    }
    if (seq !== picksSeq || uuid !== currentUuid.value) return;
    pickedFiles.value = picked;
    unpickedFiles.value = unpicked;
    syncModExclusions();
  } catch (e) {
    if (seq === picksSeq && uuid === currentUuid.value) showToast(tErr(e));
  }
}

// 切到整包格式时模组页会消失，把当前页签挪回整合包信息
watch(showModsTab, (value) => {
  if (!value && tab.value === "mods") tab.value = "meta";
});

// 两种格式的文件根不同（实例目录 / 游戏目录），切过去时旧的路径不能带过去，重新铺默认
watch(pack, () => {
  pickedFiles.value = new Map();
  // 模组勾选跟着格式重算：整包格式没有清单，全部按文件打包
  pickedMods.value = seedModPicks();
  if (currentUuid.value) void reseedFilePicks(currentUuid.value);
});

/**
 * 模组勾选联动文件树：勾了模组就走清单下载，同名文件不能再打进 overrides
 *
 * - `path`: 模组在游戏目录下的路径
 * - `excluded`: 这次是排除文件（勾上模组）还是放回文件（取消勾选）
 */
function setModExcluded(path: string, excluded: boolean) {
  if (!showModsTab.value) return;
  if (excluded) {
    // 记下来的只是"这次改的"：本来就手动画掉的不算，取消勾选时不该被还原
    if (isPickedPath(path, pickedFiles.value, unpickedFiles.value)) autoExcluded.value.add(path);
    unpickedFiles.value.add(path);
    pickedFiles.value.delete(path);
  } else if (autoExcluded.value.delete(path)) {
    unpickedFiles.value.delete(path);
  }
}

/** 换实例 / 换格式后重新铺一遍联动：已勾的在线模组对应的文件要重新排除 */
function syncModExclusions() {
  autoExcluded.value = new Set();
  if (!showModsTab.value) return;
  for (const path of pickedMods.value.keys()) {
    if (!isOnlineMod(path)) continue;
    autoExcluded.value.add(path);
    unpickedFiles.value.add(path);
    pickedFiles.value.delete(path);
  }
}

/** 勾选 / 取消一个模组；编号不成立的不给勾，它按本地模组打包 */
function toggleMod(entry: ExportEntryDto) {
  if (!isOnlineMod(entry.path)) return;
  if (pickedMods.value.has(entry.path)) {
    pickedMods.value.delete(entry.path);
    setModExcluded(entry.path, false);
  } else {
    pickedMods.value.set(entry.path, entry);
    setModExcluded(entry.path, true);
    // 有在线来源的顺手把地址填上，用户可改可清
    if (entry.url && !urls.value.has(entry.path)) urls.value.set(entry.path, entry.url);
  }
}

/** 勾选 / 取消一个文件或目录；取消记成排除项，父目录勾着也能单独去掉，必选条目不动 */
function toggleFile(entry: ExportEntryDto) {
  if (isLockedFile(entry.path)) return;
  if (isPickedPath(entry.path, pickedFiles.value, unpickedFiles.value)) {
    pickedFiles.value.delete(entry.path);
    unpickedFiles.value.add(entry.path);
  } else {
    // 手动把它勾回来：同一个模组不能既走清单又从文件里打一遍，把模组也跟着取消
    if (autoExcluded.value.delete(entry.path)) pickedMods.value.delete(entry.path);
    unpickedFiles.value.delete(entry.path);
    pickedFiles.value.set(entry.path, entry);
  }
}

/** 全选 / 全不选当前过滤出来的模组 */
function pickAllMods(all: boolean) {
  for (const mod of mods.value) {
    // 编号不成立的不能进清单，全选时跳过
    if (all && !isOnlineMod(mod.path)) continue;
    if (all) {
      pickedMods.value.set(mod.path, mod);
    } else {
      pickedMods.value.delete(mod.path);
    }
    setModExcluded(mod.path, all);
  }
}

/** 读某条模组的项目号或文件号 */
function modIdOf(path: string, key: "modId" | "fileId"): string {
  return modIds.value.get(path)?.[key] ?? "";
}

/** 改某条模组的项目号或文件号；导出时以这里的值为准 */
function setModId(path: string, key: "modId" | "fileId", value: string) {
  // CurseForge 只认纯数字，粘进来的别的字符直接丢掉
  const next = pack.value === "curseforge" ? value.replace(/\D/g, "") : value.trim();
  const current = modIds.value.get(path) ?? { modId: "", fileId: "" };
  modIds.value.set(path, { ...current, [key]: next });
}

// 编号改到不成立（清空，或者换成 CurseForge 后不是数字）：不再是在线模组，
// 勾选撤掉、文件回到包里，否则这份模组会从清单和压缩包两边一起消失
watch(
  [pack, modIds],
  () => {
    for (const path of [...pickedMods.value.keys()]) {
      if (isOnlineMod(path)) continue;
      pickedMods.value.delete(path);
      setModExcluded(path, false);
    }
  },
  { deep: true },
);

/** 填 / 清某条的下载地址 */
function setUrl(path: string, value: string) {
  if (value) {
    urls.value.set(path, value);
  } else {
    urls.value.delete(path);
  }
}

watch(currentUuid, loadInfo);

/** 切换服务器包里的一个实例；至少保留一个，当前实例被取消时顺延到第一个 */
function toggleServerInstance(uuid: string) {
  if (busy.value) return;
  const picked = serverPicked.value.includes(uuid)
    ? serverPicked.value.filter((item) => item !== uuid)
    : [...serverPicked.value, uuid];
  if (!picked.length) return;
  // 统一按实例列表顺序存，勾选顺序与导出顺序都跟列表一致
  serverPicked.value = instances.value
    .filter((item) => picked.includes(item.uuid))
    .map((item) => item.uuid);
  if (!serverPicked.value.includes(currentUuid.value)) currentUuid.value = serverPicked.value[0];
}

/** 把某个实例设为当前实例；服务器包里同时保证它在多选里 */
function focusInstance(uuid: string) {
  currentUuid.value = uuid;
  if (!serverPack.value || serverPicked.value.includes(uuid)) return;
  serverPicked.value = instances.value
    .filter((item) => item.uuid === uuid || serverPicked.value.includes(item.uuid))
    .map((item) => item.uuid);
}

/** 列表刷新后同步多选：去掉已删除的实例，至少留一个 */
function syncServerPicked() {
  const available = new Set(instances.value.map((item) => item.uuid));
  const picked = serverPicked.value.filter((uuid) => available.has(uuid));
  if (!picked.length && instances.value.length) picked.push(instances.value[0].uuid);
  serverPicked.value = instances.value
    .filter((item) => picked.includes(item.uuid))
    .map((item) => item.uuid);
  if (!serverPicked.value.includes(currentUuid.value)) {
    currentUuid.value = serverPicked.value[0] ?? "";
  }
}

/** 这个实例是不是唯一勾选的那个：唯一的不能取消，至少留一个 */
function isOnlyPicked(uuid: string): boolean {
  return serverPicked.value.length === 1 && serverPicked.value.includes(uuid);
}

/** 加载器显示名，走 add.loader.* 的既有映射 */
function loaderLabel(id: string): string {
  return t(`add.loader.${id}`);
}

// ---------------- 导出执行 ----------------

const progress = ref<ExportProgressDto | null>(null);
const running = computed(() => progress.value?.state === "running");
const preparing = ref(false);
const busy = computed(() => running.value || preparing.value);

/** 进度弹窗标题：跟着状态走 */
const progressTitle = computed(() => {
  if (progress.value?.state === "done") return t("winExport.progressDoneTitle");
  if (progress.value?.state === "failed") return t("winExport.progressFailedTitle");
  return t("winExport.progressTitle");
});

/** 进度百分比：仅在 running 且 total 有效时有意义 */
const percent = computed(() => {
  const p = progress.value;
  if (!p || p.total <= 0) return 0;
  return Math.min(100, Math.round((p.now / p.total) * 100));
});

/** 勾选表 → 后端要的入参：路径 + 下载地址 */
function picksOf(picked: Map<string, ExportEntryDto>) {
  return [...picked.keys()].map((path) => ({
    path,
    url: urls.value.get(path) ?? "",
    modId: modIds.value.get(path)?.modId || null,
    fileId: modIds.value.get(path)?.fileId || null,
    exclude: false,
  }));
}

/**
 * 文件页的勾选 → 后端入参
 *
 * 勾上的在前，取消掉的作为排除项跟在后面。文件页的条目一律不带在线来源：下载清单只由
 * 模组页决定；已按模组勾上的也不再报排除，否则后端只会看到它被取消，清单和包两边都落空。
 */
function filePicks() {
  const picks = [...pickedFiles.value.keys()].map((path) => ({
    path,
    url: "",
    modId: null,
    fileId: null,
    exclude: false,
  }));
  for (const path of unpickedFiles.value) {
    if (pickedMods.value.has(path)) continue;
    picks.push({ path, url: "", modId: null, fileId: null, exclude: true });
  }
  return picks;
}

/** 导出界面配置时，如果实例或登录方式没锁，先问一句 */
function needsLockWarn() {
  const c = guiConfig.value?.client;
  if (!c) return false;
  if (!c.loginLockOn || !c.lockInstance.trim()) return true;
  // 实例锁定只能锁一个：包里不止一个实例，或锁的不是包里的实例时，玩家仍能切走
  return serverPicked.value.length > 1 || !serverPicked.value.includes(c.lockInstance);
}

/** 询问后继续导出 */
function continueExport() {
  lockWarnOpen.value = false;
  void startExport(true);
}

/** 选保存位置后交给 Rust 侧后台导出，进度走 export-progress 事件 */
async function startExport(confirmed = false) {
  if (serverPack.value && !guiConfig.value) await loadCustomAssets();
  if (!confirmed && serverPack.value && serverOptions.guiConfig && needsLockWarn()) {
    lockWarnOpen.value = true;
    return;
  }
  if (!currentUuid.value || busy.value || loadingInfo.value || !info.value) return;
  if (serverPack.value && !serverPicked.value.length) return;
  if (!inTauri) return;
  const uuid = currentUuid.value;
  const config = {
    pack: pack.value,
    compress: zipLevel.value,
    includeLauncher: pack.value === "server" && !!info.value.canBundleLauncher && includeLauncher.value,
    mods: pack.value === "mml" ? [] : picksOf(pickedMods.value), files: filePicks(),
    name: packName.value, author: author.value, version: version.value, summary: summary.value,
    instances: serverPack.value ? [...serverPicked.value] : [],
    server: {
      config: serverOptions.config,
      guiConfig: serverOptions.guiConfig,
      page: serverOptions.page,
      css: serverOptions.css,
      home: serverOptions.home,
    },
  };
  const warnCustomLoader = customLoaderLost.value;
  const dialogOptions = {
    title: t("winExport.pickSave"),
    defaultPath: `${packName.value || info.value?.name || "modpack"}.${fileExt.value}`,
    filters: [
      {
        name: packLabel.value,
        extensions: [fileExt.value],
      },
    ],
  };
  preparing.value = true;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const picked = await save(dialogOptions);
    if (!picked) return;
    if (warnCustomLoader) showToast(t("winExport.customLoaderWarn"), 4000);
    progress.value = { state: "running", now: 0, total: 0, text: "", error: null, file: picked };
    await api.runExport(uuid, { ...config, file: picked });
  } catch (e) {
    showToast(tErr(e));
    progress.value = null;
  } finally {
    preparing.value = false;
  }
}

/** 导出中点了关窗：问一句要不要取消 */
const confirmClose = ref(false);

/** 关窗：正在导出时先问，别把后台任务晾在那儿 */
function askClose() {
  if (running.value) {
    confirmClose.value = true;
    return;
  }
  emit("close");
}

/** 确认取消：等任务收手再关；导出在这期间自己跑完了就只关窗 */
async function cancelAndClose() {
  confirmClose.value = false;
  try {
    await api.cancelExport();
  } catch (e) {
    showToast(tErr(e));
  }
  emit("close");
}

// 问着的时候导出自己结束了：不用再答，直接关
watch(running, (value) => {
  if (!value && confirmClose.value) {
    confirmClose.value = false;
    emit("close");
  }
});

// ---------------- 事件 ----------------

let unlistenProgress: UnlistenFn | null = null;
let unlistenFocus: UnlistenFn | null = null;
let unlistenCloseBlocked: UnlistenFn | null = null;
/** 窗口是否已经销毁：订阅是异步挂上的，挂好之前窗口可能已经关了 */
let gone = false;

/** 定位到 openWindow 带入的目标实例；没有或不存在就保持下拉默认值 */
function applyTarget() {
  const uuid = targetUuid();
  if (uuid && instances.value.some((i) => i.uuid === uuid)) {
    focusInstance(uuid);
  }
}

/** 重新拉实例列表并定位目标实例（首次挂载、切回本窗口、实例被增删改都用） */
async function syncInstances() {
  try {
    instances.value = await api.getInstances();
  } catch (e) {
    console.error("[ExportWindow] 初始化失败", e);
    return;
  }
  // 当前实例可能在别的窗口里被删掉了：先清空，下面再落到第一个
  const lost = !!currentUuid.value && !instances.value.some((i) => i.uuid === currentUuid.value);
  if (lost) currentUuid.value = "";
  // 服务器包已经选过实例时不再按打开参数覆盖，避免切回窗口时把手动取消的实例又勾回来
  if (!serverPack.value || !serverPicked.value.length) applyTarget();
  // 服务器包多选：没有目标参数时默认带上第一个，表单才不会只剩一句提示
  if (serverPack.value) syncServerPicked();
  // 单个实例导出：被删的那个让位给第一个；一个都不剩就停在空态
  else if (lost && !currentUuid.value && instances.value.length) focusInstance(instances.value[0].uuid);
}

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载，自己补一次列表与定位
useWindowRefresh(() => {
  // 上一轮的结果弹窗别留到下一次打开；还在跑就让它继续显示进度
  if (!running.value) progress.value = null;
  // 设置窗口可能刚改过自定义内容与锁定，切回来重读一次
  if (serverPack.value) void loadCustomAssets();
  return syncInstances();
});

// 实例在别的窗口里被增删改（删掉的那个）→ 下拉与多选跟着变
useInstanceChange(syncInstances);

// 已经停在本窗口时又被打开：openWindow 只更新参数、不换组件，跟着参数换实例
watch(() => windowParams.value.uuid, applyTarget);

onMounted(async () => {
  await syncInstances();
  void loadCustomAssets();

  const progressOff = await onExportProgress((p) => {
    progress.value = p;
  });

  // 窗口已存在时再次打开：壳层推送新的目标实例
  const focusOff = await onExportFocus((uuid) => {
    if (instances.value.some((i) => i.uuid === uuid)) {
      focusInstance(uuid);
    }
  });

  // 多窗口：原生关窗（Alt+F4 / 任务栏）由 Rust 拦下并发 close-blocked。
  // 单窗口模式下页面是在主窗口里切换，关页面走标题栏 ✕，看到的是同一条 askClose
  const closeOff = multiWindow.value
    ? await onCloseBlocked((label) => {
        if (label && label !== myLabel) return;
        confirmClose.value = true;
      })
    : null;

  // 订阅回来之前窗口可能已经关了：当场退订，别留悬空监听
  if (gone) {
    progressOff();
    focusOff();
    closeOff?.();
    return;
  }
  unlistenProgress = progressOff;
  unlistenFocus = focusOff;
  unlistenCloseBlocked = closeOff;
});

onBeforeUnmount(() => {
  gone = true;
  unlistenProgress?.();
  unlistenProgress = null;
  unlistenFocus?.();
  unlistenFocus = null;
  unlistenCloseBlocked?.();
  unlistenCloseBlocked = null;
});
</script>

<template>
  <WindowFrame body-fill :title="t(serverPack ? 'winTitle.serverPack' : 'winExport.title')" @close="askClose">
    <!-- 实例选择与导出按钮放进标题栏：不占内容区高度，也不随选项卡滚动 -->
    <template #head-right>
      <select v-if="!serverPack" v-model="currentUuid" class="field-select head-select" :disabled="busy">
        <!-- 标题栏里位置窄，占位用短文案；内容区的空态另有一句更长的提示 -->
        <option value="" disabled>{{ t("winExport.instance") }}</option>
        <option v-for="i in instances" :key="i.uuid" :value="i.uuid">{{ i.name }}</option>
      </select>
      <InfoExportButton v-if="!serverPack" :uuid="currentUuid" :name="info?.name ?? ''"
        :disabled="busy || loadingInfo || !info" />
      <BaseButton size="sm" variant="primary" class="icon-btn-head" :disabled="busy || loadingInfo || !info"
        v-tip="running ? t('winExport.exporting') : t('winExport.exportBtn')"
        :aria-label="running ? t('winExport.exporting') : t('winExport.exportBtn')" @click="startExport()">
        <GlyphIcon name="export" :size="15" :weight="1.8" />
      </BaseButton>
    </template>

    <div class="export-window">
      <template v-if="info">
        <!-- 服务器包：配置 → 自定义内容 → 实例 -->
        <template v-if="serverPack">
          <!-- 配置：跟包一起带走的启动器设置，不带就保留玩家原来的 -->
          <section class="pack-card">
            <header class="pack-card-head">
              <span class="pack-card-title">{{ t("winExport.serverConfig") }}</span>
              <span class="pack-card-hint">{{ t("winExport.serverConfigHint") }}</span>
            </header>
            <label class="check-row">
              <input v-model="serverOptions.config" type="checkbox" :disabled="busy" />
              {{ t("winExport.serverConfigCore") }}
            </label>
            <label class="check-row">
              <input v-model="serverOptions.guiConfig" type="checkbox" :disabled="busy" />
              {{ t("winExport.serverConfigGui") }}
            </label>
          </section>

          <!-- 自定义内容：依赖界面配置，没导入的置灰 -->
          <section class="pack-card">
            <header class="pack-card-head">
              <span class="pack-card-title">{{ t("winExport.serverCustom") }}</span>
              <span class="pack-card-hint">
                {{ serverOptions.guiConfig ? t("winExport.serverCustomHint") : t("winExport.serverCustomNeedsGui") }}
              </span>
            </header>
            <label class="check-row" :class="{ 'is-disabled': !serverOptions.guiConfig || !customUi?.pageInstalled }">
              <input v-model="serverOptions.page" type="checkbox"
                :disabled="busy || !serverOptions.guiConfig || !customUi?.pageInstalled" />
              {{ t("winExport.serverCustomPage") }}
            </label>
            <label class="check-row" :class="{ 'is-disabled': !serverOptions.guiConfig || !customUi?.css.length }">
              <input v-model="serverOptions.css" type="checkbox"
                :disabled="busy || !serverOptions.guiConfig || !customUi?.css.length" />
              {{ t("winExport.serverCustomCss") }}
            </label>
            <label class="check-row" :class="{ 'is-disabled': !serverOptions.guiConfig || !customHome?.installed }">
              <input v-model="serverOptions.home" type="checkbox"
                :disabled="busy || !serverOptions.guiConfig || !customHome?.installed" />
              {{ t("winExport.serverCustomHome") }}
            </label>
          </section>

          <!-- 实例：可多选，每个实例在包里各占一份 -->
          <section class="pack-card">
            <header class="pack-card-head">
              <span class="pack-card-title">{{ t("winExport.serverInstance") }}</span>
              <span class="pack-card-hint">
                {{ t("winExport.serverInstanceHint", { count: serverPicked.length }) }}
              </span>
            </header>
            <div class="instance-list">
              <label v-for="i in instances" :key="i.uuid" class="instance-row"
                :class="{ 'is-on': serverPicked.includes(i.uuid) }"
                :title="isOnlyPicked(i.uuid) ? t('winExport.serverInstanceKeepOne') : undefined">
                <input type="checkbox" :checked="serverPicked.includes(i.uuid)"
                  :disabled="busy || isOnlyPicked(i.uuid)" @change="toggleServerInstance(i.uuid)" />
                <span class="instance-name" :title="i.name">{{ i.name }}</span>
                <span v-if="i.running" class="instance-tag">{{ t("launch.running") }}</span>
                <span class="instance-meta">{{ i.version }} · {{ loaderLabel(i.loader) }}</span>
              </label>
            </div>
            <label v-if="info.canBundleLauncher" class="check-row">
              <input v-model="includeLauncher" type="checkbox" :disabled="busy" />
              {{ t("winExport.includeLauncher") }}
            </label>
          </section>

          <div class="row">
            <label class="field-label" v-tip="t('winExport.zipLevelTip')">{{ t("winExport.zipLevel") }}</label>
            <SegmentedTabs :options="zipOptions" :model-value="zipLevel" :disabled="busy"
              @update:model-value="onZipLevelChange" />
          </div>
        </template>

        <template v-else>
        <!-- 格式用分段页签：一览可选格式，切换不用展开下拉 -->
        <div class="row">
          <label class="field-label">{{ t("winExport.format") }}</label>
          <SegmentedTabs :options="packOptions" :model-value="pack" :disabled="busy"
            @update:model-value="onPackChange" />
        </div>

        <!-- 压缩等级：对整份导出生效，直接压缩与其它格式都走它 -->
        <div class="row">
          <label class="field-label" v-tip="t('winExport.zipLevelTip')">{{ t("winExport.zipLevel") }}</label>
          <SegmentedTabs :options="zipOptions" :model-value="zipLevel" :disabled="busy"
            @update:model-value="onZipLevelChange" />
        </div>

        <!-- 这层限制在选择格式时就说清楚，别让用户以为包里带了自定义加载器 -->
        <p v-if="customLoaderLost" class="warn-hint">{{ t("winExport.customLoaderWarn") }}</p>

        <!-- 选项卡：与「添加实例」窗口同一套图标页签 -->
        <p v-if="pack === 'mml'" class="sec-hint">{{ t("winExport.nativeHint") }}</p>
        <IconTabs v-else v-model="tab" :tabs="tabOptions" />

        <div class="tab-body">
          <!-- 整合包信息：写进清单的元数据，配着上面的格式一起看 -->
          <template v-if="shownTab === 'meta'">
            <p class="sec-hint">
              {{
                showModsTab
                  ? t("winExport.summaryLine", {
                      mods: pickedMods.size,
                      files: pickedFiles.size,
                      urls: urlRows.filter((x) => (urls.get(x.path) ?? "").length > 0).length,
                    })
                  : t("winExport.summaryFiles", { files: pickedFiles.size })
              }}
            </p>
            <div class="grid-2">
              <div>
                <label class="field-label">{{ t("winExport.metaName") }}</label>
                <input v-model="packName" class="field-input" spellcheck="false" />
              </div>
              <div>
                <label class="field-label">{{ t("winExport.metaAuthor") }}</label>
                <input v-model="author" class="field-input" spellcheck="false" />
              </div>
              <div>
                <label class="field-label">{{ t("winExport.metaVersion") }}</label>
                <input v-model="version" class="field-input" spellcheck="false" />
              </div>
              <div>
                <label class="field-label">{{ t("winExport.metaSummary") }}</label>
                <input v-model="summary" class="field-input" spellcheck="false" />
              </div>
            </div>
          </template>

          <!-- 模组 -->
          <template v-else-if="shownTab === 'mods'">
            <div class="sec-bar">
              <input v-model="modKeyword" class="field-input search" spellcheck="false"
                :placeholder="t('winExport.searchMods')" />
              <BaseButton size="sm" @click="pickAllMods(true)">{{ t("winExport.selectAll") }}</BaseButton>
              <BaseButton size="sm" @click="pickAllMods(false)">{{ t("winExport.selectNone") }}</BaseButton>
            </div>
            <!-- 表格列跟着格式走：只有 Modrinth 的清单带下载地址，多一列它就够宽 -->
            <div class="mod-table" :class="{ 'with-url': withUrls }">
              <div class="mod-head">
                <span />
                <span>{{ t("winExport.modFile") }}</span>
                <span>{{ t("winExport.projectId") }}</span>
                <span>{{ t("winExport.fileId") }}</span>
                <span v-if="withUrls">{{ t("winExport.downloadUrl") }}</span>
              </div>
              <p v-if="!mods.length" class="empty-tip">{{ t("winExport.noOnlineMods") }}</p>
              <div v-for="mod in mods" :key="mod.path" class="mod-line"
                :class="{ 'is-local': !isOnlineMod(mod.path) }">
                <input type="checkbox" :checked="pickedMods.has(mod.path)"
                  :disabled="!isOnlineMod(mod.path)"
                  :title="cfIdInvalid(mod.path) ? t('winExport.idNotNumeric') : undefined"
                  @change="toggleMod(mod)" />
                <span class="mod-name-cell">
                  <span class="mod-name" :title="mod.path"><HighlightText :text="mod.path.split('/').pop() ?? ''" :query="modKeyword" /></span>
                  <!-- 编号不成立的在 CF 里点不动：就地标出来，别让人以为是漏掉了 -->
                  <span v-if="cfIdInvalid(mod.path)" class="mod-tag" v-tip="t('winExport.idNotNumeric')">
                    {{ t("winExport.notCfMod") }}
                  </span>
                </span>
                <input class="field-input" spellcheck="false" :value="modIdOf(mod.path, 'modId')"
                  @input="setModId(mod.path, 'modId', ($event.target as HTMLInputElement).value)" />
                <input class="field-input" spellcheck="false" :value="modIdOf(mod.path, 'fileId')"
                  @input="setModId(mod.path, 'fileId', ($event.target as HTMLInputElement).value)" />
                <input v-if="withUrls" class="field-input" spellcheck="false"
                  :value="urls.get(mod.path) ?? ''" :placeholder="t('winExport.urlPlaceholder')"
                  @input="setUrl(mod.path, ($event.target as HTMLInputElement).value)" />
              </div>
            </div>
          </template>

          <!-- 文件：只有一棵树，勾选即打包，取消勾选就排除 -->
          <template v-else>
            <div class="files-pane">
              <p class="sec-hint">{{ fileHint }}</p>
              <FilePicker :uuid="currentUuid" :picked="pickedFilePaths" :unpicked="unpickedFiles"
                :base="fileBase" :expand="fileExpand" :locked="isLockedFile" @toggle="toggleFile" />
            </div>
          </template>
        </div>
        </template>
      </template>
      <div v-else-if="loadingInfo" class="hint-full">{{ t("winExport.loading") }}</div>
      <div v-else class="hint-full">{{ t("winExport.pickInstance") }}</div>
    </div>

    <!-- 服务器包：实例 / 登录方式没锁时先问一句，避免玩家拿到包后还能随意切换 -->
    <BaseModal v-if="lockWarnOpen" :title="t('winExport.lockWarnTitle')" @close="lockWarnOpen = false">
      <p class="modal-text">{{ t("winExport.lockWarnText") }}</p>
      <div class="modal-actions">
        <BaseButton @click="lockWarnOpen = false">{{ t("winExport.lockWarnCancel") }}</BaseButton>
        <BaseButton variant="primary" @click="continueExport">{{ t("winExport.lockWarnContinue") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 导出进度：任务在后台跑，进度靠弹窗盖住整页，免得被选项卡切走看不见 -->
    <BaseModal v-if="progress && !confirmClose" :title="progressTitle" :width="460" :closable="false"
      @close="progress = null">
      <!-- 只有跑着的时候才有一行当前文件；结束后下面那行已写明保存位置，不必重复 -->
      <p v-if="running" class="prog-file" :title="progress.text">
        {{ progress.text || t("winExport.progressPreparing") }}
      </p>
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: percent + '%' }" />
      </div>
      <p v-if="progress.state === 'running'" class="prog-count">
        {{ progress.total > 0 ? t("winExport.progressCount", { now: progress.now, total: progress.total }) : percent + "%" }}
      </p>
      <p v-else-if="progress.state === 'done'" class="prog-note">
        {{ t("winExport.progressSaved", { file: progress.file }) }}
      </p>
      <p v-else class="error-text">{{ progress.error || t("winExport.failed") }}</p>
      <div class="prog-actions">
        <BaseButton v-if="!running" size="sm" variant="primary" @click="progress = null">
          {{ t("winExport.close") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- 导出中关窗：确认后取消导出再关 -->
    <BaseModal v-if="confirmClose" :title="t('winExport.closeRunningTitle')" :closable="false"
      @close="confirmClose = false">
      <p class="delete-tip">{{ t("winExport.closeRunningConfirm") }}</p>
      <div class="prog-actions">
        <BaseButton @click="confirmClose = false">{{ t("winExport.keepExporting") }}</BaseButton>
        <BaseButton variant="danger" @click="cancelAndClose">{{ t("winExport.cancelExport") }}</BaseButton>
      </div>
    </BaseModal>
  </WindowFrame>
</template>

<style scoped>
/* 四周留白交给 .frame-body（上下 22 / 左右 26）：这里**不能再加一份 padding**，
   否则顶部会变成 36px、左右 42px，比别的窗口鼓一圈。
   内容自己滚，所以滚动条槽位仍留给这个容器 */
.export-window {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  min-height: 0;
  box-sizing: border-box;
  overflow-y: auto;
  /* 两侧都留滚动条槽：只留一侧时右侧会凭空多出一截，和左边不一样宽 */
  scrollbar-gutter: stable both-edges;
  /* 见 styles/scrollbar.css */
}

/* 页签自带的下外边距是给「添加实例」窗口的表单留的，这一页用的是 flex gap，
   不取消就会变成 12 + 14 = 26px */
.export-window :deep(.tabs) {
  margin-bottom: 0;
}

/* 这一排的通用标准在 styles/titlebar.css；这里只留导出窗口自己的例外：
   实例名可能很长，比标准下拉宽一档，好让名字完整显示、别把导出按钮挤出去 */
.head-select {
  min-width: 260px;
  max-width: 340px;
}

/* 导出格式：标签与选择器同一行。选择器是 inline-flex，只包住自己的选项内容，
   不会被拉着撑满整行 */
.row {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* .field-label 默认是块级、上下都带外边距（12 / 6），并排时会把这一行顶歪 */
.row .field-label {
  margin: 0;
}

/* 服务器包里的勾选项 */
.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
  cursor: pointer;
}

.check-row.is-disabled {
  color: var(--text-dim);
  cursor: default;
}

/* 服务器包的三段：配置 / 自定义内容 / 实例，每段一张卡片 */
.pack-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
}

.pack-card-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-wrap: wrap;
}

.pack-card-title {
  font-size: 13px;
  font-weight: 600;
}

.pack-card-hint {
  font-size: 11.5px;
  color: var(--text-dim);
}

/* 实例多选：超过一屏自己滚，行高按列表项，不按控件 32px */
.instance-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 220px;
  overflow-y: auto;
  margin: 0 -4px;
}

.instance-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 5px 8px;
  border-radius: 8px;
  cursor: pointer;
}

.instance-row:hover {
  background: var(--bg-hover);
}

.instance-row.is-on {
  background: var(--accent-soft);
}

.instance-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
}

.instance-meta {
  margin-left: auto;
  flex-shrink: 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

.instance-tag {
  flex-shrink: 0;
  padding: 0 6px;
  border-radius: 999px;
  background: var(--bg-hover);
  color: var(--text-dim);
  font-size: 10.5px;
  line-height: 16px;
}

.grid-2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 14px;
}

/* 选项卡内容：吃掉剩余高度，内部各自滚 */
.tab-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-height: 0;
}

.sec-hint {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

/* 会丢东西的提示：与设置页里那些"改了要重启"的提示同一档观感 */
.warn-hint {
  margin: 0;
  font-size: var(--ctl-font);
  color: var(--yellow);
}

.sec-bar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.sec-bar .search {
  flex: 1;
  min-width: 0;
}

/* 模组表：表头吸顶、内容多时自己滚，别把窗口撑长。列宽走同一套 grid，表头与行才对得齐 */
.mod-table {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-solid, var(--bg-card));
}

.mod-head,
.mod-line {
  display: grid;
  grid-template-columns: 22px minmax(0, 1fr) 120px 120px;
  align-items: center;
  gap: 10px;
  padding: 4px 10px;
}

/* Modrinth 多一列下载地址：地址比编号长，给它更宽的弹性列 */
.mod-table.with-url .mod-head,
.mod-table.with-url .mod-line {
  grid-template-columns: 22px minmax(0, 1fr) 110px 110px minmax(0, 1.4fr);
}

.mod-head {
  position: sticky;
  top: 0;
  z-index: 1;
  min-height: 30px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text-dim);
  font-size: 11.5px;
}

.mod-line {
  min-height: 34px;
  border-bottom: 1px solid var(--border);
  font-size: 12.5px;
}

.mod-line:last-child {
  border-bottom: none;
}

.mod-line:hover {
  background: var(--bg-hover);
}

/* 表格里一行只有这么高：收紧内边距与行高，别按常规控件的 32px 把行撑开 */
.mod-line .field-input {
  padding: 2px 8px;
  min-height: 24px;
  height: 24px;
  line-height: 18px;
}

/* 文件名旁边还能挂一个标签，超出部分截断 */
.mod-name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.mod-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 编号不成立的行：不是在线模组，勾不动，压暗一档 */
.mod-line.is-local .mod-name {
  color: var(--text-dim);
}

.mod-tag {
  flex-shrink: 0;
  padding: 0 6px;
  border-radius: 999px;
  background: var(--bg-hover);
  color: var(--text-dim);
  font-size: 10.5px;
  line-height: 16px;
}

/* 文件页：只剩一棵树，占满整宽 */
.files-pane {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-height: 0;
}

/* 进度弹窗：当前文件一行、进度条一行、结果另起一行 */
.prog-file {
  margin: 0 0 10px;
  font-size: 12px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.prog-count {
  margin: 8px 0 0;
  font-size: 11.5px;
  color: var(--text-dim);
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.prog-note {
  margin: 10px 0 0;
  font-size: 12.5px;
  color: var(--text-dim);
  word-break: break-all;
}

.prog-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 16px;
}

.progress-track {
  height: 8px;
  border-radius: 999px;
  background: var(--bg-raised);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.3s ease;
}

.hint-full {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  font-size: 13px;
}
</style>
