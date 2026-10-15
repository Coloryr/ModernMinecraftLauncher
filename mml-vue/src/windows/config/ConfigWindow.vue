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
// 实例取自 gui_config 的 `mainWindow.selectedInstance`（与资源管理窗口同一口径，窗口自己读）；
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
  listConfigFiles,
  listSaves,
  openInstanceDir,
  readConfigFile,
  writeConfigFile,
} from "../../lib/api";
import { loadGuiConfig } from "../../lib/guiConfig";
import { buildTree, type FileNode } from "../../lib/fileTree";
import { applyHunks, diffLines, joinLines, splitLines } from "../../lib/textDiff";
import { multiWindow, targetSaveDir, windowParams } from "../windowManager";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import SearchBox from "../../components/ui/SearchBox.vue";
import InstanceChip from "../../components/ui/InstanceChip.vue";
import MonacoText from "../../components/ui/MonacoText.vue";
import HexView from "./parts/HexView.vue";
import ImageView from "./parts/ImageView.vue";
import McaTable from "./parts/McaTable.vue";
import NbtTree from "./parts/NbtTree.vue";
import { clearCleanSessions, closeSessions } from "./parts/nbtSession";
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
const busy = computed(() => saving.value || nbtSaving.value);

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
  loading.value = true;
  try {
    const [list, cfg] = await Promise.all([api.getInstances(), loadGuiConfig()]);
    const uuid = cfg?.mainWindow.selectedInstance ?? "";
    const found = list.find((i) => i.uuid === uuid) ?? null;
    // 换了实例就把来源退回"实例本身"：原来那个存档名在新实例里没有
    if (found?.uuid !== instanceUuid.value) source.value = "";
    instanceUuid.value = found?.uuid ?? "";
    instanceName.value = found?.name ?? "";
    worlds.value = found ? await listSaves(found.uuid) : [];

    // "聚焦某个存档"的跳转（资源窗口右键存档 → 修改配置文件），跟世界名对上就套到来源上。
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
    showToast(tErr(e));
    instanceUuid.value = "";
    instanceName.value = "";
    worlds.value = [];
  } finally {
    loading.value = false;
  }
  await loadFiles();
}

/** 按当前来源列文件 */
async function loadFiles() {
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
  if (!instanceUuid.value) {
    files.value = [];
    return;
  }

  loading.value = true;
  try {
    // 来源直接交给后端：空串是实例本身，否则是存档目录，路径一律相对该来源
    const list = await listConfigFiles(instanceUuid.value, source.value);
    // 顺序交给文件树统一给（每层目录在前）；这里只拷一份，不和 IPC 返回的数组共享引用
    files.value = [...list];
  } catch (e) {
    files.value = [];
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 选中一个文件：文本类顺手把内容拉下来，其余交给各自的视图组件 */
async function open(file: FileItemDto) {
  selected.value = file;
  text.value = "";
  origin.value = "";
  originSha1.value = "";
  nbtDirty.value = false;
  if (!TEXT_TYPES.includes(file.fileType)) return;

  loading.value = true;
  try {
    const data = await readConfigFile(instanceUuid.value, source.value, file.name);
    text.value = data.text;
    origin.value = data.text;
    originSha1.value = data.sha1;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/**
 * 保存当前视图：文本走行级差异，NBT 交给树上的登记表
 *
 * 窗口自己不认识 NBT 的改动，只负责把标题栏那一颗保存键分发给对应视图。
 */
async function save() {
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
  if (!file || !isText.value || saving.value || !dirty.value) return;

  saving.value = true;
  try {
    const originLines = splitLines(origin.value);
    const editedLines = splitLines(text.value);
    let hunks = diffLines(originLines, editedLines);
    if (joinLines(applyHunks(originLines, hunks)) !== text.value) {
      hunks = [{ start: 0, deleteCount: originLines.length, lines: editedLines }];
    }

    const sha1 = await writeConfigFile(
      instanceUuid.value,
      source.value,
      file.name,
      originSha1.value,
      editedLines.length,
      hunks,
    );
    origin.value = text.value;
    originSha1.value = sha1;
    showToast(t("config.saved"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    saving.value = false;
  }
}

/** 在文件管理器里打开这个实例的目录 */
function openDir() {
  if (!instanceUuid.value) return;
  void openInstanceDir(instanceUuid.value);
}

/** 换来源：存档下拉改了就重列 */
async function pickSource(value: string) {
  if (value === source.value) return;
  source.value = value;
  await loadFiles();
}

onMounted(() => void load());
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
        <select class="field-select head-select" :value="source" :disabled="loading" v-tip="t('config.sourceTip')"
          :aria-label="t('config.sourceTip')" @change="pickSource(($event.target as HTMLSelectElement).value)">
          <option value="">{{ t("config.instance") }}</option>
          <option v-for="world in worlds" :key="world.dir" :value="world.dir">
            {{ world.levelName || world.dir }}
          </option>
        </select>

        <!-- 保存状态：未保存时下面那颗保存键转强调色，工具条因此整行省掉 -->
        <span v-if="canSave && unsaved" class="head-dirty">{{ t("config.unsaved") }}</span>
        <span v-else-if="selected && !canSave" class="head-readonly">{{ t("config.readonly") }}</span>

        <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="loading" v-tip="t('config.refresh')"
          :aria-label="t('config.refresh')" @click="loadFiles">
          <GlyphIcon name="refresh" :size="15" :weight="1.8" />
        </BaseButton>
        <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="!instanceUuid" v-tip="t('config.openDir')"
          :aria-label="t('config.openDir')" @click="openDir">
          <GlyphIcon name="folder" :size="15" :weight="1.8" />
        </BaseButton>
        <BaseButton v-if="canSave" size="sm" :variant="unsaved ? 'accent' : 'ghost'" class="icon-btn-head"
          :disabled="busy || !unsaved" v-tip="t('config.save')" :aria-label="t('config.save')" @click="save">
          <GlyphIcon name="save" :size="15" :weight="1.8" />
        </BaseButton>
      </div>
    </template>

    <div class="cfg-body">
      <!-- 左：与主窗口侧栏同一套，搜索固定在上，只有下面的树滚动 -->
      <aside class="cfg-list">
        <div class="cfg-list-head">
          <SearchBox v-model="query" :placeholder="t('config.search')" :clear-label="t('config.searchClear')"
            width="100%" />
        </div>

        <div class="cfg-tree">
          <template v-for="row in rows" :key="row.key">
            <!-- 目录：按侧栏的分组标题写，缩进由层级给 -->
            <button v-if="row.isDir" class="cfg-dir" :style="{ paddingLeft: `${8 + row.depth * 14}px` }"
              @click="toggleExpand(row.key)">
              <GlyphIcon class="cfg-chevron" :class="{ open: expanded.has(row.key) }" name="chevron-down" :size="13"
                :weight="2.4" />
              <span class="cfg-dir-name" :title="row.key">{{ row.name }}</span>
            </button>
            <!-- 文件：按侧栏的实例行写 -->
            <button v-else class="cfg-row" :class="{ on: selected?.name === row.key }"
              :style="{ paddingLeft: `${8 + row.depth * 14}px` }" :title="row.key" @click="openFile(row.key)">
              <span class="cfg-chevron-spacer"></span>
              <span class="cfg-file-name">{{ row.name }}</span>
            </button>
          </template>
          <p v-if="!rows.length" class="empty-tip">{{ emptyTip }}</p>
        </div>
      </aside>

      <!-- 右：按类型选视图。没选文件时也摆着文本编辑器，只是禁用 ——
           打开窗口就是"编辑器窗口"的样子，不再是中间一句提示 -->
      <section class="cfg-view" :class="{ flush: !selected || isText || selected.fileType === 'image' || selected.fileType === 'nbt' }">
        <!-- 与下面那个必须给不同的 key：同为 MonacoText，Vue 会复用实例，
             而只读是创建时的选项，复用后切不回可编辑 -->
        <MonacoText v-if="!selected" key="empty" :model-value="''" readonly />
        <MonacoText v-else-if="isText" key="file" v-model="text" :language="langOf(selected)" />
        <NbtTree v-else-if="isNbt" ref="nbtRef" :uuid="instanceUuid" :source="source" :file="selected.name"
          @dirty="nbtDirty = $event" />
        <McaTable v-else-if="selected.fileType === 'mca'" :uuid="instanceUuid" :source="source"
          :file="selected.name" />
        <ImageView v-else-if="selected.fileType === 'image'" :uuid="instanceUuid" :source="source"
          :file="selected.name" />
        <HexView v-else :uuid="instanceUuid" :source="source" :file="selected.name" />
      </section>
    </div>
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
.cfg-view {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  padding: 22px 26px;
}

/* 文本、图片、NBT 与默认态贴边满铺：编辑器自己那圈描边就是分界，图片与 NBT 树不套卡片、
   也不该再留一圈空。卡片类视图（区块 / 二进制）自带圆角与描边，仍要上面那圈外留白 */
.cfg-view.flush {
  padding: 0;
}
</style>
