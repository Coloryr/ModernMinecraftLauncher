<script setup lang="ts">
// 配置文件窗口：左侧文件树，右侧按类型显示文本、NBT、区块、图片或二进制视图。
// 实例由窗口身份或跳转参数确定；文件路径相对于实例配置目录或选定存档。
// 文本提交行级差异，NBT 会话记录编辑操作；保存前均用基线 SHA1 检查磁盘冲突。
// 实例名、来源下拉、刷新、保存状态与保存键都放进标题栏；
// 文件树上方只留搜索与区域文件的查找入口。
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
import { buildConfigTree, visibleConfigRows } from "./configTree";
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

/** 使用文本编辑器的文件类型；NBT 与区域文件由各自视图编辑 */
const TEXT_TYPES = ["json", "json5", "toml", "yaml", "properties", "xml", "javascript", "snbt", "text"];

/**
 * Monaco 语言映射；TOML 使用本地着色器，SNBT 借用 JavaScript 着色以避开 JSON 校验
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
/** 游戏版本，标题栏 chip 与资源管理窗口显示同一份 */
const instanceVersion = ref("");
/** 存档列表，"看某个存档里的文件"用 */
const worlds = ref<SaveItemDto[]>([]);
/** 当前来源：空串 = 实例本身，否则是存档目录名 */
const source = ref("");
const files = ref<FileItemDto[]>([]);
const selected = ref<FileItemDto | null>(null);
const loading = ref(false);
/** 只允许最近一次读请求回写当前编辑上下文。 */
let requestGeneration = 0;

/** 编辑内容与最近一次读取或保存成功的基线；两者不同即未保存 */
const text = ref("");
const origin = ref("");
/** 基线文本的 SHA1，每次保存成功后更新，用于检测外部修改 */
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

/**
 * 确认是否放弃文本改动；保存、删除或已有确认在等时拒绝切换
 *
 * # 返回值
 *
 * 可以继续切换为 `true`；被拒绝为 `false`
 */
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

/** 标题栏上的当前实例：与资源管理窗口一致，名字后面带游戏版本 */
const instanceChip = computed(() =>
  instanceName.value ? { name: instanceName.value, version: instanceVersion.value } : null,
);

/** 搜索关键字，空串 = 不过滤 */
const query = ref("");
/** 已展开的目录 key */
const expanded = ref<Set<string>>(new Set());

/** 路径 → 文件条目：树节点只带路径，打开文件与取类型都要回查 */
const fileByPath = computed(() => new Map(files.value.map((f) => [f.name, f])));

/** 搜索和展开只改变可见行，文件树仅在文件列表变化时重建 */
const tree = computed(() => buildConfigTree(files.value.map((file) => file.name)));
const rows = computed(() => visibleConfigRows(tree.value, expanded.value, query.value));

/** 展开或收起目录，保留其他目录的展开状态 */
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
  if (query.value.trim()) return t("config.searchEmpty");
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
    instanceVersion.value = found?.version ?? "";
    worlds.value = nextWorlds;

    // 单窗口跳转参数消费后清除，多窗口 URL 参数随窗口保留。
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
    instanceVersion.value = "";
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
  // 关闭干净的 NBT 会话以便重读磁盘，保留会话中尚未保存的编辑。
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
  // 上一个文件留下的定位目标不作数了，否则新文件会按旧区块下标翻开
  mcaTarget.value = null;
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

/** 将保存操作分发给文本编辑器或 NBT 视图 */
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

/** 保存提交瞬间的文本快照，保留等待响应期间的新编辑 */
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
    // 差异回放必须还原提交内容，否则退回整段替换。
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

/** 当前来源的区域文件，用于判断是否开放实体与方块查找 */
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
const configBody = ref<HTMLElement | null>(null);

/** 是否正在拖分隔线，用来给拖拽态加高亮 */
const resizing = ref(false);

/** 左栏宽度的上下限，单位像素 */
const TREE_MIN_WIDTH = 200;
const TREE_MAX_WIDTH = 640;
const bodyWidth = ref(TREE_MAX_WIDTH * 2);
/** 缩窄窗口时给编辑区保留至少一半空间，不改写已保存的宽度偏好 */
const treeMaxWidth = computed(() => Math.min(TREE_MAX_WIDTH, Math.floor(bodyWidth.value / 2)));
const treeMinWidth = computed(() => Math.min(TREE_MIN_WIDTH, treeMaxWidth.value));
const visibleTreeWidth = computed(() => clampTreeWidth(treeWidth.value));
let resizeObserver: ResizeObserver | null = null;
let stopResize: (() => void) | null = null;

function clampTreeWidth(width: number): number {
  return Math.min(treeMaxWidth.value, Math.max(treeMinWidth.value, width));
}

function persistTreeWidth() {
  void setConfigTreeWidth(treeWidth.value).catch((error) => showToast(tErr(error)));
}

/** 拖动时更新侧栏宽度，松手后保存偏好 */
function startResize(event: PointerEvent) {
  if (event.button !== 0 || !event.isPrimary) return;
  event.preventDefault();
  stopResize?.();
  resizing.value = true;
  const startX = event.clientX;
  const startWidth = visibleTreeWidth.value;
  const preferredWidth = treeWidth.value;

  function onMove(e: PointerEvent) {
    if (e.pointerId !== event.pointerId) return;
    treeWidth.value = clampTreeWidth(startWidth + e.clientX - startX);
  }

  function cleanup() {
    resizing.value = false;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onCancel);
    window.removeEventListener("blur", cancel);
    stopResize = null;
  }

  function onUp(e: PointerEvent) {
    if (e.pointerId !== event.pointerId) return;
    cleanup();
    if (treeWidth.value !== preferredWidth) persistTreeWidth();
  }

  function cancel() {
    treeWidth.value = preferredWidth;
    cleanup();
  }

  function onCancel(e: PointerEvent) {
    if (e.pointerId === event.pointerId) cancel();
  }

  stopResize = cleanup;
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("pointercancel", onCancel);
  window.addEventListener("blur", cancel);
}

/** 分隔线获得焦点时可用方向键调整宽度，Home 与 End 到达边界 */
function onResizeKeydown(event: KeyboardEvent) {
  if (resizing.value) return;
  let width = visibleTreeWidth.value;
  switch (event.key) {
    case "ArrowLeft": width -= 16; break;
    case "ArrowRight": width += 16; break;
    case "Home": width = treeMinWidth.value; break;
    case "End": width = treeMaxWidth.value; break;
    default: return;
  }
  event.preventDefault();
  const next = clampTreeWidth(width);
  if (next === visibleTreeWidth.value) return;
  treeWidth.value = next;
  persistTreeWidth();
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

/** 用 Ctrl+S 或 Cmd+S 保存，并阻止浏览器默认的保存网页操作 */
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
  }).catch((error) => showToast(tErr(error)));
  resizeObserver = new ResizeObserver(([entry]) => {
    // KeepAlive 隐藏时宽度可能归零，保留最后一次可见尺寸。
    if (entry && entry.contentRect.width > 0) bodyWidth.value = entry.contentRect.width;
  });
  if (configBody.value) resizeObserver.observe(configBody.value);
  window.addEventListener("keydown", onSaveHotkey);
});

onBeforeUnmount(() => {
  requestGeneration += 1;
  answerDiscard(false);
  stopResize?.();
  resizeObserver?.disconnect();
  window.removeEventListener("keydown", onSaveHotkey);
  // 卸载时关闭 NBT 会话；单窗口模式的 KeepAlive 切页不会触发卸载
  closeSessions();
});
// 单窗口模式切回本窗口时重读；有没保存的改动就不动，免得把编辑内容冲掉
useWindowRefresh(() => {
  if (!dirty.value && !nbtDirty.value) void load();
});
</script>

<template>
  <WindowFrame :title="t('config.title')" body-fill body-flush @close="emit('close')">
    <!-- 标题栏右侧：实例名 + 来源下拉 + 刷新 + 保存状态 + 保存键。
         当前文件由左侧树的选中行表示，标题栏不再重复文件名 -->
    <template #head-right>
      <div class="head-cfg">
        <InstanceChip :instance="instanceChip" />
        <select class="field-select head-select" :value="source" :disabled="loading || busy || !instanceUuid"
          v-tip="t('config.sourceTip')" :aria-label="t('config.sourceTip')" @change="onSourceChange">
          <option value="">{{ t("config.instance") }}</option>
          <option v-for="world in worlds" :key="world.dir" :value="world.dir">
            {{ world.levelName || world.dir }}
          </option>
        </select>
        <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="loading || busy || !instanceUuid"
          v-tip="t('config.refresh')" :aria-label="t('config.refresh')" @click="loadFiles()">
          <GlyphIcon name="refresh" :size="15" :weight="1.8" />
        </BaseButton>
        <span class="head-status" :class="{ 'is-dirty': canSave && unsaved }" role="status">
          <template v-if="loading">{{ t("config.loading") }}</template>
          <template v-else-if="canSave && unsaved">{{ t("config.unsaved") }}</template>
          <!-- 区域文件在区块视图内保存，不属于只读视图 -->
          <template v-else-if="selected && !canSave && selected.fileType !== 'mca'">
            {{ t("config.readonly") }}
          </template>
        </span>
        <BaseButton v-if="canSave" size="sm" :variant="unsaved ? 'accent' : 'ghost'" class="icon-btn-head"
          :disabled="loading || busy || !unsaved" @click="save"
          v-tip="t(saving || nbtSaving ? 'config.saving' : 'config.save')"
          :aria-label="t(saving || nbtSaving ? 'config.saving' : 'config.save')">
          <GlyphIcon name="save" :size="16" :weight="1.8" />
        </BaseButton>
      </div>
    </template>

    <div ref="configBody" class="cfg-body">
      <!-- 搜索与查找固定在上，文件树独立滚动 -->
      <aside id="config-file-tree" class="cfg-list" :style="{ width: `${visibleTreeWidth}px` }">
        <div class="cfg-list-head">
          <SearchBox v-model="query" :placeholder="t('config.search')" :clear-label="t('config.searchClear')"
            width="100%" />
          <BaseButton v-if="source" size="sm" variant="ghost" block :disabled="loading || busy || !mcaFiles.length"
            @click="findOpen = true">
            <GlyphIcon name="search" :size="16" :weight="1.8" />
            {{ t("config.mcaFindTitle") }}
          </BaseButton>
        </div>

        <div class="cfg-tree" :aria-busy="loading">
          <template v-for="row in rows" :key="row.key">
            <!-- 目录：按侧栏的分组标题写，缩进由层级给 -->
            <button v-if="row.isDir" class="cfg-dir" :style="{ paddingLeft: `${8 + row.depth * 14}px` }"
              :aria-expanded="row.expanded" @click="toggleExpand(row.key)"
              @contextmenu.prevent="openRowMenu($event, row.key)">
              <GlyphIcon class="cfg-chevron" :class="{ 'is-expanded': row.expanded }" name="chevron-down" :size="14"
                :weight="2.4" />
              <span class="cfg-dir-name" :title="row.key">
                <HighlightText :text="row.name" :query="query" />
              </span>
            </button>
            <!-- 文件：按侧栏的实例行写 -->
            <button v-else class="cfg-row" :class="{ 'is-active': selected?.name === row.key }"
              :aria-current="selected?.name === row.key ? 'true' : undefined"
              :style="{ paddingLeft: `${8 + row.depth * 14}px` }" :title="row.key" @click="openFile(row.key)"
              @contextmenu.prevent="openRowMenu($event, row.key)">
              <span class="cfg-chevron-spacer"></span>
              <span class="cfg-file-name">
                <HighlightText :text="row.name" :query="query" />
              </span>
            </button>
          </template>
          <p v-if="!rows.length" class="empty-tip">{{ emptyTip }}</p>
        </div>
      </aside>

      <div class="cfg-split" :class="{ 'is-resizing': resizing }" role="separator" tabindex="0"
        aria-orientation="vertical" aria-controls="config-file-tree" :aria-label="t('config.treeResize')"
        :aria-valuemin="treeMinWidth" :aria-valuemax="treeMaxWidth" :aria-valuenow="visibleTreeWidth"
        @pointerdown="startResize" @keydown="onResizeKeydown" />

      <section class="cfg-view" :aria-busy="loading || deleting">
        <!-- 占位编辑器与文件编辑器使用独立实例，隔离只读状态 -->
        <MonacoText v-if="!selected || (isText && (loading || deleting))" key="empty"
          :model-value="deleting ? text : ''" readonly />
        <MonacoText v-else-if="isText" key="file" v-model="text" :language="langOf(selected)" />
        <NbtTree v-else-if="isNbt" ref="nbtRef" :uuid="instanceUuid" :source="source" :file="selected.name"
          @dirty="nbtDirty = $event" />
        <McaTable v-else-if="selected.fileType === 'mca'" :uuid="instanceUuid" :source="source" :file="selected.name"
          :chunk="mcaTarget?.chunk" :reveal="mcaTarget?.path" />
        <ImageView v-else-if="selected.fileType === 'image'" :uuid="instanceUuid" :source="source"
          :file="selected.name" />
        <HexView v-else :uuid="instanceUuid" :source="source" :file="selected.name" />
      </section>
    </div>

    <!-- 切换上下文前确认是否放弃文本改动 -->
    <BaseModal v-if="pendingDiscard" :title="t('config.unsaved')" @close="answerDiscard(false)">
      <p>{{ t("config.discardChanges") }}</p>
      <div class="modal-actions">
        <BaseButton @click="answerDiscard(false)">{{ t("config.keepEditing") }}</BaseButton>
        <BaseButton variant="danger" :disabled="busy" @click="answerDiscard(true)">{{ t("config.discard") }}
        </BaseButton>
      </div>
    </BaseModal>
    <McaFindModal v-if="findOpen" @find="onFindChunk" @close="findOpen = false" />

    <!-- 文件行右键菜单：打开所在目录 / 删除 -->
    <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenuItems" @pick="onRowMenuPick"
      @close="rowMenu = null" />
  </WindowFrame>
</template>

<style scoped>
/* 标题栏这一排：实例名 / 来源 / 刷新 / 状态 / 保存。
   整排可收缩，窗口窄时先省略实例名，按钮不裁 */
.head-cfg {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 1;
  min-width: 0;
}

.head-status {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
  white-space: nowrap;
}

.head-status.is-dirty {
  color: var(--yellow);
}

/* 两栏均需允许收缩，滚动才会留在文件树与编辑器内部 */
.cfg-body {
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
}

/* 侧栏贴窗口边缘，宽度由用户偏好与拖动状态决定 */
.cfg-list {
  display: flex;
  flex-direction: column;
  max-width: 50%;
  flex-shrink: 0;
  min-height: 0;
  overflow: hidden;
  background: var(--bg-side);
  border-right: 1px solid var(--border);
  /* 密集文件树使用统一行高，单位像素 */
  --row-h: 24px;
}

.cfg-list-head {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
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

/* 目录与文件同高，展开目录时保持行距稳定 */
.cfg-dir,
.cfg-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: var(--row-h);
  /* 条目多时由容器滚动，不能压缩行高 */
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
.cfg-row.is-active {
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

.cfg-chevron.is-expanded {
  transform: rotate(0);
}

/* 文件行里占住箭头那一格：同一层的文件名才与目录名对齐 */
.cfg-chevron-spacer {
  flex-shrink: 0;
  width: 14px;
}

.cfg-dir-name,
.cfg-file-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 编辑区不再有独立工具条：路径与保存都搬去了标题栏，视图直接占满剩下的高度 */
.cfg-view {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  /* 不透明底：区块 / 二进制是密排文本，半透明底会和窗口背景图叠在一起 */
  background: var(--bg-solid, var(--bg-card));
  overflow: hidden;
}

/* 视图占满容器，滚动交给各视图内部 */
.cfg-view>* {
  flex: 1;
  min-height: 0;
}

/* 透明热区覆盖侧栏边框，不占用两栏的可见空间 */
.cfg-split {
  flex: 0 0 6px;
  margin: 0 -2px 0 -4px;
  cursor: col-resize;
  touch-action: none;
  z-index: 1;
  background: transparent;
  transition: background 0.15s;
}

.cfg-split:hover,
.cfg-split:focus-visible,
.cfg-split.is-resizing {
  background: var(--accent-soft);
}

.cfg-split:focus-visible {
  outline: 1px solid var(--accent);
  outline-offset: -2px;
}
</style>
