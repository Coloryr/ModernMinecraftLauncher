<script setup lang="ts">
// 配置文件窗口：左侧是可搜索的文件树，右侧按类型给四种视图
//
// 左栏与主窗口的侧栏是同一套皮肤：--bg-side 底、右侧描边、贴窗口的上 / 下 / 左缘；
// 目录当"分组标题"、文件当"实例行"，两种行同高一档 --row-h: 24px。
// 窗口因此走 body-flush，右栏只有卡片类视图留 22/26 外留白。
//
// 还没选文件时右栏是一个**禁用的空编辑器**，不是一句提示 —— 打开就是编辑器窗口的样子。
//
// - 文本（json / json5 / toml / yaml / properties / xml / js / snbt / 纯文本）→ Monaco 编辑（行号 + 着色）+ 保存
// - NBT（.dat / .nbt / .rio）→ NBT 树，树上的标量就地改再保存
// - 区域文件（.mca）→ 区块表
// - 图片（png / jpg / gif / webp）→ 直接显示
// - 其余（压缩包与别的二进制）→ 二进制视图（一页 4 KiB 的十六进制）
//
// 来源两条：实例自己的 options.txt 与 `config` / `kubejs` / `local`，或某个存档目录下的文件。
// 实例是**窗口身份的一部分**：多窗口模式下一个实例一个窗口、打开时就锁定，单窗口模式按
// 跳转参数，都没有时退回主窗口选中的那个；
// 来源（空串 = 实例本身，否则是存档目录名）会一起交给后端，文件路径一律**相对该来源**，
// 树根因此就是所选来源，存档来源下不会再套一层 `saves`。
//
// 保存两条：文本走**行级差异**（lib/textDiff），只回传改了的那几行；NBT 只回传改了的那几处
// 标量（改动登记在 NbtTree 里）。两条都按打开时的 SHA1 先核对磁盘上还是不是那一份。
// 标题栏跟日志窗口同一套：左边窗口名，右边实例 chip + 来源下拉，操作留在工具条。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import {
  api,
  deleteConfigFile,
  findConfigChunk,
  getConfigTreeWidth,
  listConfigFiles,
  listSaves,
  openConfigFileDir,
  readConfigFile,
  setConfigTreeWidth,
  writeConfigFile,
} from "../../lib/api";
import { loadGuiConfig } from "../../lib/guiConfig";
import { buildTree, type FileNode } from "../../lib/fileTree";
import { applyHunks, diffLines, joinLines, splitLines } from "../../lib/textDiff";
import { familyInstanceFromLabel, multiWindow, targetSaveDir, targetUuid, windowParams } from "../windowManager";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import HighlightText from "../../components/ui/HighlightText.vue";
import SearchBox from "../../components/ui/SearchBox.vue";
import InstanceChip from "../../components/ui/InstanceChip.vue";
import MonacoText from "../../components/ui/MonacoText.vue";
import HexView from "./parts/HexView.vue";
import ImageView from "./parts/ImageView.vue";
import McaTable from "./parts/McaTable.vue";
import NbtTree from "./parts/NbtTree.vue";
import { clearCleanSessions, closeSessions } from "./parts/nbtSession";
import RowMenu, { type RowMenuItem } from "../resource/parts/RowMenu.vue";
import McaFindModal from "./parts/McaFindModal.vue";
import type { FileItemDto, SaveItemDto } from "../../lib/bindings";

/** 请求关闭本窗口 */
const emit = defineEmits<{ (e: "close"): void }>();

/** 文本类文件：能编能存；其余走只读的结构 / 二进制视图 */
const TEXT_TYPES = ["json", "json5", "toml", "yaml", "properties", "xml", "javascript", "snbt", "text"];

/**
 * 文件类型 → Monaco 语言 id
 *
 * 没有对应语言包的一律退回纯文本。两处近似：
 * toml 用仓库里自己注册的那份着色器，见 lib/monacoToml；
 * snbt 长得像 JSON 但键不带引号，借 javascript 的着色器只上色、不做 JSON 校验，免得满屏红线
 */
const LANG_OF_TYPE: Record<string, string> = {
  json: "json",
  json5: "json",
  toml: "toml",
  yaml: "yaml",
  properties: "ini",
  xml: "xml",
  javascript: "javascript",
  snbt: "javascript",
  text: "plaintext",
};

/** 当前实例 */
const instanceUuid = ref("");
const instanceName = ref("");
/** 存档列表，"看某个存档里的文件"用 */
const worlds = ref<SaveItemDto[]>([]);
/** 当前来源：空串 = 实例本身，否则是存档目录名 */
const source = ref("");
const files = ref<FileItemDto[]>([]);
const selected = ref<FileItemDto | null>(null);
const loading = ref(false);
/** 只允许最近一次读请求回写当前编辑上下文。 */
let requestGeneration = 0;

/** 文本内容与读进来的原文：两者不同就是"未保存" */
const text = ref("");
const origin = ref("");
/**
 * 打开时那一份全文的 SHA1
 *
 * 保存走的是行级差异（见 lib/textDiff），差异只对得上"打开时那一份"；
 * 把它交给后端核对磁盘，变了就拒绝保存，免得冲掉别处（如资源管理改 options.txt）
 * 的改动。
 */
const originSha1 = ref("");
const saving = ref(false);
/** 删除期间锁定编辑上下文，避免完成后的刷新丢掉新输入。 */
const deleting = ref(false);

/** NBT 视图里有没有未保存的改动，由 NbtTree 报上来 */
const nbtDirty = ref(false);
/** 正在保存 NBT */
const nbtSaving = ref(false);
/** NBT 视图实例，保存时要调它的 `save` */
const nbtRef = ref<InstanceType<typeof NbtTree> | null>(null);

const dirty = computed(() => text.value !== origin.value);
const isText = computed(() => !!selected.value && TEXT_TYPES.includes(selected.value.fileType));
const isNbt = computed(() => selected.value?.fileType === "nbt");
/** 当前视图能不能保存：文本与 NBT 都是可改的 */
const canSave = computed(() => isText.value || isNbt.value);
/** 当前视图有没有未保存的改动 */
const unsaved = computed(() => (isText.value ? dirty.value : nbtDirty.value));
/** 正忙着落盘：保存键在这期间不能再点 */
const busy = computed(() => saving.value || nbtSaving.value || deleting.value);

const pendingDiscard = ref<((discard: boolean) => void) | null>(null);

/** 清空文本前确认；保存中不允许切换，避免提交尚未完成就丢掉编辑上下文。 */
async function allowTextLoss(): Promise<boolean> {
  if (busy.value || pendingDiscard.value) return false;
  if (!isText.value || !dirty.value) return true;
  return new Promise((resolve) => { pendingDiscard.value = resolve; });
}

function answerDiscard(discard: boolean) {
  if (discard && busy.value) return;
  const resolve = pendingDiscard.value;
  pendingDiscard.value = null;
  resolve?.(discard);
}

/** 当前文件该用哪种 Monaco 语言 */
function langOf(file: FileItemDto): string {
  return LANG_OF_TYPE[file.fileType] ?? "plaintext";
}

// ---------- 文件树与搜索 ----------

/** 标题栏上的当前实例；与日志窗口一样只给名字，不给版本 */
const instanceChip = computed(() => (instanceName.value ? { name: instanceName.value } : null));

/** 搜索关键字，空串 = 不过滤 */
const query = ref("");
/** 已展开的目录 key */
const expanded = ref<Set<string>>(new Set());

/** 路径 → 文件条目：树节点只带路径，打开文件与取类型都要回查 */
const fileByPath = computed(() => new Map(files.value.map((f) => [f.name, f])));

/** 每层目录在前、文件在后，各自按名字排；`buildTree` 只按插入顺序给 */
function sortDirsFirst(nodes: FileNode[]) {
  nodes.sort((a, b) => (a.isDir === b.isDir ? a.name.localeCompare(b.name) : a.isDir ? -1 : 1));
  for (const n of nodes) {
    if (n.isDir) sortDirsFirst(n.children);
  }
}

/** 文件树；多层路径由 buildTree 合并出目录节点，顺序由这里统一给 */
const tree = computed(() => {
  const nodes = buildTree(files.value.map((f) => f.name));
  sortDirsFirst(nodes);
  return nodes;
});

/** 展开后的一行 */
interface TreeRow {
  /** 完整路径，与 `FileItemDto.name` 一致 */
  key: string;
  /** 显示名，只有最后一段 */
  name: string;
  /** 是否目录 */
  isDir: boolean;
  /** 层级，从 0 起，用来算缩进 */
  depth: number;
}

/** 自身或后代是否命中关键字 */
function subtreeHit(node: FileNode, q: string): boolean {
  if (node.key.toLowerCase().includes(q)) return true;
  return node.children.some((c) => subtreeHit(c, q));
}

/**
 * 把树按展开状态摊平成可见行
 *
 * 有搜索词时不看展开状态：命中的目录一律展开，否则搜到的东西藏在收起的分支里看不见
 */
const rows = computed<TreeRow[]>(() => {
  const q = query.value.trim().toLowerCase();
  const out: TreeRow[] = [];
  const walk = (nodes: FileNode[], depth: number) => {
    for (const n of nodes) {
      if (q && !subtreeHit(n, q)) continue;
      out.push({ key: n.key, name: n.name, isDir: n.isDir, depth });
      if (n.isDir && (q || expanded.value.has(n.key))) walk(n.children, depth + 1);
    }
  };
  walk(tree.value, 0);
  return out;
});

/** 展开 / 收起一个目录；Set 整体换掉才会触发更新 */
function toggleExpand(key: string) {
  const next = new Set(expanded.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expanded.value = next;
}

/** 打开树上的文件：树里只有路径，条目回查后再读 */
function openFile(key: string) {
  const file = fileByPath.value.get(key);
  if (!file) return;
  // 搜索命中的文件可能藏在收起的分支里：点开时把沿途各级目录一并展开，清掉搜索后仍看得见
  const parts = key.split("/");
  const next = new Set(expanded.value);
  for (let i = 1; i < parts.length; i += 1) next.add(parts.slice(0, i).join("/"));
  expanded.value = next;
  void open(file);
}

/** 左栏占位文案：加载中 / 没选实例 / 搜不到 / 这个位置没有文件 */
const emptyTip = computed(() => {
  if (loading.value) return t("config.loading");
  if (!instanceUuid.value) return t("config.noInstance");
  if (query.value) return t("config.searchEmpty");
  return t("config.noFiles");
});

/** 读当前实例与它的存档列表，再列文件 */
async function load() {
  if (!(await allowTextLoss())) return;
  const generation = ++requestGeneration;
  loading.value = true;
  try {
    const list = await api.getInstances();
    // 当前实例：多窗口模式按窗口标签，那是窗口身份的一部分；单窗口 / 浏览器按本次跳转
    // 参数（锁定为空说明是从别的页面返回本页，参数已被清掉），退回主窗口选中的那个
    const locked = familyInstanceFromLabel("config") ?? targetUuid() ?? "";
    const uuid = locked || (await loadGuiConfig())?.mainWindow.selectedInstance || "";
    const found = list.find((i) => i.uuid === uuid) ?? null;
    const nextWorlds = found ? await listSaves(found.uuid) : [];
    if (generation !== requestGeneration) return;
    // 换了实例就把来源退回"实例本身"：原来那个存档名在新实例里没有
    if (found?.uuid !== instanceUuid.value) source.value = "";
    instanceUuid.value = found?.uuid ?? "";
    instanceName.value = found?.name ?? "";
    worlds.value = nextWorlds;

    // "聚焦某个存档"的跳转（资源窗口右键存档 → 修改配置文件），跟存档名对上就套到来源上。
    // 单窗口模式读完就地清掉，别让普通跳转再把它带回来；多窗口模式参数是本窗口 URL 的一
    // 部分，随窗口留着。存档列表里没有的名字套不上，来源保持原样
    const saveDir = targetSaveDir();
    if (saveDir) {
      if (!multiWindow.value) {
        windowParams.value = { ...windowParams.value, saveDir: null };
      }
      if (worlds.value.some((w) => w.dir === saveDir)) {
        source.value = saveDir;
      }
    }
  } catch (e) {
    if (generation !== requestGeneration) return;
    showToast(tErr(e));
    instanceUuid.value = "";
    instanceName.value = "";
    worlds.value = [];
  } finally {
    if (generation === requestGeneration) loading.value = false;
  }
  if (generation === requestGeneration) await loadFiles(true);
}

/** 按当前来源列文件 */
async function loadFiles(confirmed = false) {
  if (!confirmed && !(await allowTextLoss())) return;
  const generation = ++requestGeneration;
  const uuid = instanceUuid.value;
  const from = source.value;
  const current = () => generation === requestGeneration && uuid === instanceUuid.value && from === source.value;
  loading.value = true;
  selected.value = null;
  text.value = "";
  origin.value = "";
  nbtDirty.value = false;
  // 树与搜索都跟着这批文件走：换了来源 / 实例，旧的目录 key 与关键字都没意义了
  expanded.value = new Set();
  query.value = "";
  // 盘上那一份可能已经不是会话里缓存的样子了：没改动的 NBT 会话就地丢掉（连后端的文件
  // 句柄一起关，下次打开重读）。有未保存改动的留着，那是用户还没落盘的东西
  await clearCleanSessions();
  if (!current()) return;
  if (!instanceUuid.value) {
    files.value = [];
    loading.value = false;
    return;
  }

  try {
    // 来源直接交给后端：空串是实例本身，否则是存档目录，路径一律相对该来源
    const list = await listConfigFiles(uuid, from);
    if (!current()) return;
    // 顺序交给文件树统一给（每层目录在前）；这里只拷一份，不和 IPC 返回的数组共享引用
    files.value = [...list];
  } catch (e) {
    if (!current()) return;
    files.value = [];
    showToast(tErr(e));
  } finally {
    if (current()) loading.value = false;
  }
}

/** 选中一个文件：文本类顺手把内容拉下来，其余交给各自的视图组件 */
async function open(file: FileItemDto) {
  if (selected.value?.name === file.name) return true;
  if (!(await allowTextLoss())) return false;
  const generation = ++requestGeneration;
  const uuid = instanceUuid.value;
  const from = source.value;
  const current = () => generation === requestGeneration && uuid === instanceUuid.value && from === source.value;
  selected.value = file;
  text.value = "";
  origin.value = "";
  originSha1.value = "";
  nbtDirty.value = false;
  if (!TEXT_TYPES.includes(file.fileType)) {
    loading.value = false;
    return true;
  }

  loading.value = true;
  try {
    const data = await readConfigFile(uuid, from, file.name);
    if (!current()) return false;
    text.value = data.text;
    origin.value = data.text;
    originSha1.value = data.sha1;
  } catch (e) {
    if (current()) showToast(tErr(e));
  } finally {
    if (current()) loading.value = false;
  }
  return true;
}

/**
 * 保存当前视图：文本走行级差异，NBT 交给树上的登记表
 *
 * 窗口自己不认识 NBT 的改动，只负责把标题栏那一颗保存键分发给对应视图。
 */
async function save() {
  if (busy.value) return;
  if (isNbt.value) {
    const tree = nbtRef.value;
    if (!tree || nbtSaving.value || !nbtDirty.value) return;
    nbtSaving.value = true;
    try {
      await tree.save();
    } finally {
      nbtSaving.value = false;
    }
    return;
  }
  await saveText();
}

/**
 * 保存当前文本文件：只回传**行级差异**，不再整份送回去
 *
 * 差异是相对打开时那一份算的，所以后端会先核对磁盘上的 SHA1。发出去之前先自检一遍：
 * 把差异套回原文必须与编辑中的内容逐字相同，对不上就退回"整段替换"——
 * 多发一次全文可以接受，按错的差异改盘上的文件不行。
 */
async function saveText() {
  const file = selected.value;
  if (!file || !isText.value || busy.value || !dirty.value) return;

  const submitted = text.value;
  const uuid = instanceUuid.value;
  const from = source.value;
  saving.value = true;
  try {
    const originLines = splitLines(origin.value);
    const editedLines = splitLines(submitted);
    let hunks = diffLines(originLines, editedLines);
    if (joinLines(applyHunks(originLines, hunks)) !== submitted) {
      hunks = [{ start: 0, deleteCount: originLines.length, lines: editedLines }];
    }

    const sha1 = await writeConfigFile(
      uuid,
      from,
      file.name,
      originSha1.value,
      editedLines.length,
      hunks,
    );
    if (selected.value === file && instanceUuid.value === uuid && source.value === from) {
      origin.value = submitted;
      originSha1.value = sha1;
    }
    showToast(t("config.saved"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    saving.value = false;
  }
}

/** 「查找实体 / 方块」弹窗开着没 */
const findOpen = ref(false);

/** 查到的区块与节点路径，交给 mca 视图翻页并定位 */
const mcaTarget = ref<{ chunk: number; path: string[] } | null>(null);

/** 当前来源里的区域文件，查找弹窗里选 */
const mcaFiles = computed(() =>
  files.value.filter((item) => item.fileType === "mca").map((item) => item.name),
);

/**
 * 在存档里按坐标找实体 / 方块，命中就打开那个区域文件并定位
 *
 * - `form`: 弹窗填的那几项，坐标是字符串，这里解析
 */
async function onFindChunk(form: { kind: string; x: string; z: string; name: string }) {
  if (!instanceUuid.value) return;
  // 坐标可以都不填：那就是整份存档扫一遍
  const hasPos = form.x !== "" && form.z !== "";
  const x = hasPos ? Number(form.x) : null;
  const z = hasPos ? Number(form.z) : null;
  if ((x !== null && !Number.isFinite(x)) || (z !== null && !Number.isFinite(z))) {
    showToast(t("config.mcaFindBadPos"));
    return;
  }
  try {
    const found = await findConfigChunk(
      instanceUuid.value,
      source.value,
      form.kind,
      x === null ? null : Math.trunc(x),
      z === null ? null : Math.trunc(z),
      form.name,
    );
    if (!found.file) {
      showToast(t("config.mcaFindNone", { name: form.name }));
      return;
    }
    findOpen.value = false;
    // 文件名由后端按坐标算出来，直接切过去；mca 视图再按 chunk / reveal 翻到那一块
    const target = files.value.find((item) => item.name === found.file);
    if (target && await open(target)) {
      mcaTarget.value = { chunk: found.index, path: found.path };
    }
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 左栏宽度，单位像素；拖动中间那条分隔线改它 */
const treeWidth = ref(300);

/** 是否正在拖分隔线，用来给拖拽态加高亮 */
const resizing = ref(false);

/** 左栏宽度的上下限，单位像素 */
const TREE_MIN_WIDTH = 200;
const TREE_MAX_WIDTH = 640;

/**
 * 拖动分隔线改左栏宽度
 *
 * 拖动过程中只改本地宽度，松手才落盘 —— 每移动一像素写一次文件没有必要。
 * 监听挂在 `window` 上：指针移出那条窄带也不会中断。
 *
 * - `event`: 分隔线上的按下事件
 */
function startResize(event: PointerEvent) {
  event.preventDefault();
  resizing.value = true;
  const startX = event.clientX;
  const startWidth = treeWidth.value;

  function onMove(e: PointerEvent) {
    const next = startWidth + e.clientX - startX;
    treeWidth.value = Math.min(TREE_MAX_WIDTH, Math.max(TREE_MIN_WIDTH, next));
  }

  function onUp() {
    resizing.value = false;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    void setConfigTreeWidth(treeWidth.value);
  }

  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
}

/** 文件行的右键菜单：点击处坐标 + 右键的那个相对路径 */
const rowMenu = ref<{ x: number; y: number; name: string } | null>(null);

/** 右键菜单项：打开所在目录 / 删除（进回收站） */
const rowMenuItems = computed<RowMenuItem[]>(() => [
  { id: "open-dir", label: t("config.openFileDir") },
  { id: "delete", label: t("config.deleteFile"), danger: true, separated: true },
]);

/**
 * 右键一个文件：记下坐标与路径，弹出菜单
 *
 * - `event`: 右键事件，取点击处坐标
 * - `name`: 文件在来源内的相对路径
 */
function openRowMenu(event: MouseEvent, name: string) {
  rowMenu.value = { x: event.clientX, y: event.clientY, name };
}

/**
 * 执行右键菜单项
 *
 * - `id`: 菜单项 id，见 [`rowMenuItems`]
 */
async function onRowMenuPick(id: string) {
  const target = rowMenu.value;
  rowMenu.value = null;
  if (!target || !instanceUuid.value) return;
  try {
    if (id === "open-dir") {
      await openConfigFileDir(instanceUuid.value, source.value, target.name);
      return;
    }
    if (id === "delete") {
      if (loading.value || !(await allowTextLoss())) return;
      const generation = ++requestGeneration;
      const uuid = instanceUuid.value;
      const from = source.value;
      deleting.value = true;
      try {
        await deleteConfigFile(uuid, from, target.name);
        if (generation !== requestGeneration || uuid !== instanceUuid.value || from !== source.value) return;
        await loadFiles(true);
        showToast(t("config.deleted"));
      } finally {
        deleting.value = false;
      }
    }
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 换来源：存档下拉改了就重列 */
async function pickSource(value: string) {
  if (value === source.value) return;
  if (!(await allowTextLoss())) return;
  source.value = value;
  await loadFiles(true);
}

/** 原生 select 已改了显示值，先还原，确认后再由响应式来源更新。 */
function onSourceChange(event: Event) {
  const select = event.target as HTMLSelectElement;
  const value = select.value;
  select.value = source.value;
  void pickSource(value);
}

/**
 * Ctrl+S / Cmd+S 保存
 *
 * 有改动、且不是正在存的时候才真存；组合键一律吞掉，
 * 免得浏览器预览下弹出"保存网页"。
 *
 * - `event`: 键盘事件
 */
function onSaveHotkey(event: KeyboardEvent) {
  if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "s") {
    return;
  }
  event.preventDefault();
  if (!canSave.value || !unsaved.value || busy.value) {
    return;
  }
  void save();
}

onMounted(() => {
  void load();
  // 左栏宽度是用户偏好，走后端落 gui_config.json
  void getConfigTreeWidth().then((width) => {
    treeWidth.value = Math.min(TREE_MAX_WIDTH, Math.max(TREE_MIN_WIDTH, width));
  });
  window.addEventListener("keydown", onSaveHotkey);
});

onBeforeUnmount(() => {
  requestGeneration += 1;
  answerDiscard(false);
  window.removeEventListener("keydown", onSaveHotkey);
});
// 本窗口走了（多窗口模式下就是窗口关闭，单窗口模式下是页面切走）：打开着的 NBT 文件都放手，
// 后端那边的句柄跟着关掉，下次打开重新读盘
onBeforeUnmount(closeSessions);
// 单窗口模式切回本窗口时重读；有没保存的改动就不动，免得把编辑内容冲掉
useWindowRefresh(() => {
  if (!dirty.value && !nbtDirty.value) void load();
});
</script>

<template>
  <WindowFrame :title="t('config.title')" body-fill body-flush @close="emit('close')">
    <!-- 实例、来源与操作都放标题栏，与日志窗口同一套；内容区整块留给文件树 -->
    <template #head-right>
      <div class="head-cfg">
        <InstanceChip :instance="instanceChip" />
        <select class="field-select head-select" :value="source" :disabled="loading || busy" v-tip="t('config.sourceTip')"
          :aria-label="t('config.sourceTip')" @change="onSourceChange">
          <option value="">{{ t("config.instance") }}</option>
          <option v-for="world in worlds" :key="world.dir" :value="world.dir">
            {{ world.levelName || world.dir }}
          </option>
        </select>

        <!-- 保存状态：未保存时下面那颗保存键转强调色，工具条因此整行省掉 -->
        <span v-if="canSave && unsaved" class="head-dirty">{{ t("config.unsaved") }}</span>
        <!-- 区域文件也能改，只是保存键在区块页里，不在标题栏 -->
        <span v-else-if="selected && !canSave && selected.fileType !== 'mca'" class="head-readonly">
          {{ t("config.readonly") }}
        </span>

        <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="loading || busy" v-tip="t('config.refresh')"
          :aria-label="t('config.refresh')" @click="loadFiles()">
          <GlyphIcon name="refresh" :size="15" :weight="1.8" />
        </BaseButton>
        <BaseButton v-if="source" size="sm" variant="ghost" class="icon-btn-head" :disabled="!mcaFiles.length"
          v-tip="t('config.mcaFindTitle')" :aria-label="t('config.mcaFindTitle')" @click="findOpen = true">
          <GlyphIcon name="search" :size="15" :weight="1.8" />
        </BaseButton>
        <BaseButton v-if="canSave" size="sm" :variant="unsaved ? 'accent' : 'ghost'" class="icon-btn-head"
          :disabled="busy || !unsaved" v-tip="t('config.save')" :aria-label="t('config.save')" @click="save">
          <GlyphIcon name="save" :size="15" :weight="1.8" />
        </BaseButton>
      </div>
    </template>

    <div class="cfg-body">
      <!-- 左：与主窗口侧栏同一套，搜索固定在上，只有下面的树滚动 -->
      <aside class="cfg-list" :style="{ width: `${treeWidth}px` }">
        <div class="cfg-list-head">
          <SearchBox v-model="query" :placeholder="t('config.search')" :clear-label="t('config.searchClear')"
            width="100%" />
        </div>

        <div class="cfg-tree">
          <template v-for="row in rows" :key="row.key">
            <!-- 目录：按侧栏的分组标题写，缩进由层级给 -->
            <button v-if="row.isDir" class="cfg-dir" :style="{ paddingLeft: `${8 + row.depth * 14}px` }"
              @click="toggleExpand(row.key)" @contextmenu.prevent="openRowMenu($event, row.key)">
              <GlyphIcon class="cfg-chevron" :class="{ open: expanded.has(row.key) }" name="chevron-down" :size="13"
                :weight="2.4" />
              <span class="cfg-dir-name" :title="row.key"><HighlightText :text="row.name" :query="query" /></span>
            </button>
            <!-- 文件：按侧栏的实例行写 -->
            <button v-else class="cfg-row" :class="{ on: selected?.name === row.key }"
              :style="{ paddingLeft: `${8 + row.depth * 14}px` }" :title="row.key" @click="openFile(row.key)"
              @contextmenu.prevent="openRowMenu($event, row.key)">
              <span class="cfg-chevron-spacer"></span>
              <span class="cfg-file-name"><HighlightText :text="row.name" :query="query" /></span>
            </button>
          </template>
          <p v-if="!rows.length" class="empty-tip">{{ emptyTip }}</p>
        </div>
      </aside>

      <!-- 分隔线：拖动改左栏宽度。盖在左栏右边框上，所以看起来还是那一条细线 -->
      <div class="cfg-split" :class="{ on: resizing }" @pointerdown="startResize" />

      <!-- 右：按类型选视图。没选文件时也摆着文本编辑器，只是禁用 ——
           打开窗口就是"编辑器窗口"的样子，不再是中间一句提示 -->
      <section class="cfg-view">
        <!-- 与下面那个必须给不同的 key：同为 MonacoText，Vue 会复用实例，
             而只读是创建时的选项，复用后切不回可编辑 -->
        <MonacoText v-if="!selected || (isText && (loading || deleting))" key="empty" :model-value="deleting ? text : ''" readonly />
        <MonacoText v-else-if="isText" key="file" v-model="text" :language="langOf(selected)" />
        <NbtTree v-else-if="isNbt" ref="nbtRef" :uuid="instanceUuid" :source="source" :file="selected.name"
          @dirty="nbtDirty = $event" />
        <McaTable v-else-if="selected.fileType === 'mca'" :uuid="instanceUuid" :source="source"
          :file="selected.name" :chunk="mcaTarget?.chunk" :reveal="mcaTarget?.path" />
        <ImageView v-else-if="selected.fileType === 'image'" :uuid="instanceUuid" :source="source"
          :file="selected.name" />
        <HexView v-else :uuid="instanceUuid" :source="source" :file="selected.name" />
      </section>
    </div>

    <!-- 查找实体 / 方块：编辑存档时从标题栏打开 -->
    <BaseModal v-if="pendingDiscard" :title="t('config.unsaved')" @close="answerDiscard(false)">
      <p>{{ t("config.discardChanges") }}</p>
      <div class="modal-actions">
        <BaseButton @click="answerDiscard(false)">{{ t("config.keepEditing") }}</BaseButton>
        <BaseButton variant="danger" :disabled="busy" @click="answerDiscard(true)">{{ t("config.discard") }}</BaseButton>
      </div>
    </BaseModal>
    <McaFindModal v-if="findOpen" @find="onFindChunk" @close="findOpen = false" />

    <!-- 文件行右键菜单：打开所在目录 / 删除 -->
    <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenuItems"
      @pick="onRowMenuPick" @close="rowMenu = null" />
  </WindowFrame>
</template>

<style scoped>
/* 标题栏右侧：实例 chip + 来源下拉。这一排的通用标准在 styles/titlebar.css */
.head-cfg {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.head-dirty {
  flex-shrink: 0;
  color: var(--yellow);
  font-size: 12px;
}

.head-readonly {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 12px;
}

/* 两栏：左固定宽、右吃掉剩余；两层都要 min-height: 0，不然子元素滚不动 */
.cfg-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

/* 左栏：主窗口侧栏那一套 —— --bg-side 底 + 右侧描边 + 贴窗口的上 / 下 / 左缘，宽 300px。
   窗口走 body-flush，所以这里不再是自己一张"卡片"，而是真的侧栏 */
.cfg-list {
  display: flex;
  flex-direction: column;
  width: 300px;
  flex-shrink: 0;
  min-height: 0;
  overflow: hidden;
  background: var(--bg-side);
  border-right: 1px solid var(--border);
  /* 左栏一行的高度：目录行与文件行同取这一档。比全 app 控件那一档（32px）矮，
     树里一行就一个名字，24px 一屏能多看几条 */
  --row-h: 24px;
}

/* 搜索固定在上：内边距与侧栏的 .sidebar-head 一致 */
.cfg-list-head {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 12px 10px;
}

/* 树的滚动区：内边距与侧栏的 .group-list 一致 */
.cfg-tree {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  padding: 2px 10px 14px;
}

/* 目录行与文件行同高同圆角：一棵树里两种行的尺寸不一致，扫起来会一跳一跳。
   行高取左栏自己的 --row-h，比全 app 控件那一档矮：树里条密，一行一个名字就够 */
.cfg-dir,
.cfg-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: var(--row-h);
  /* 必须不缩：.cfg-tree 是 flex 列，条目一多就会被压扁（展开后行高比折叠时矮一半就是这么来的），
     该滚动的是容器 */
  flex-shrink: 0;
  padding: 0 12px;
  border: none;
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  font-family: inherit;
  font-size: var(--ctl-font);
  text-align: left;
  cursor: pointer;
  transition: background 0.12s;
}

/* 目录：只差颜色与字重，就是"分组标题"那个观感；缩进由模板按层级给 */
.cfg-dir {
  color: var(--text-dim);
  font-weight: 700;
  letter-spacing: 0.5px;
}

.cfg-dir:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.cfg-row:hover {
  background: var(--bg-hover);
}

/* 选中态：强调色底 + 一圈描边 */
.cfg-row.on {
  background: var(--accent-soft);
  color: var(--accent);
  outline: 1px solid var(--accent-border);
}

/* 收起时箭头朝右，展开朝下 */
.cfg-chevron {
  flex-shrink: 0;
  color: var(--text-dim);
  transform: rotate(-90deg);
  transition: transform 0.12s;
}

.cfg-chevron.open {
  transform: rotate(0);
}

/* 文件行里占住箭头那一格：同一层的文件名才与目录名对齐 */
.cfg-chevron-spacer {
  flex-shrink: 0;
  width: 13px;
}

.cfg-dir-name,
.cfg-file-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 右栏：四周留白回到全 app 内容区那一档（窗口走了 body-flush，这里自己给） */
/* 右栏：**四个编辑模式共用这一张卡片** —— 底色、描边、圆角只写这一处，
   文本 / NBT / 区块 / 二进制各自铺满它，内边距由各视图按内容疏密自己给。
   以前是"文本与 NBT 贴边、区块与二进制各带一张卡片"两套口径，切文件时边界会跳 */
.cfg-view {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  /* 直接占满右栏：与左栏同一边界口径（左栏也是贴边 + 一条右边框），不要外留白 */
  /* 不透明底：区块 / 二进制是密排文本，半透明底会和窗口背景图叠在一起 */
  background: var(--bg-solid, var(--bg-card));
  overflow: hidden;
}

/* 视图自己铺满卡片：不漏白底，也不各自撑出自带的外留白 */
.cfg-view > * {
  flex: 1;
  min-height: 0;
}
/* 分隔线：5px 的透明热区压着左栏那条右边框，看起来仍是一条细线 */
.cfg-split {
  flex: 0 0 5px;
  margin-left: -3px;
  cursor: col-resize;
  z-index: 1;
  background: transparent;
  transition: background 0.15s;
}

.cfg-split:hover,
.cfg-split.on {
  background: var(--accent-soft);
}
</style>
