<script setup lang="ts">
// 配置文件窗口：左侧是可搜索的文件树，右侧按类型给四种视图
//
// - 文本（json / json5 / toml / yaml / properties / xml / js / snbt / 纯文本）→ Monaco 编辑（行号 + 着色）+ 保存
// - NBT（.dat / .nbt / .rio）→ NBT 树
// - 区域文件（.mca）→ 区块表
// - 图片（png / jpg / gif / webp）→ 直接显示
// - 其余（压缩包与别的二进制）→ 二进制视图（一页 4 KiB 的十六进制）
//
// 来源两条：实例自己的 options.txt 与 `config` / `kubejs` / `local`，或某个存档目录下的文件。
// 实例取自 gui_config 的 `mainWindow.selectedInstance`（与资源管理窗口同一口径，窗口自己读）；
// 后端只认"相对游戏目录的路径"，列表里给什么就原样回传。
// 标题栏跟日志窗口同一套：左边窗口名，右边实例 chip + 来源下拉，操作留在工具条。
import { computed, onMounted, ref } from "vue";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import {
  api,
  listConfigFiles,
  listConfigSaveFiles,
  listSaves,
  openInstanceDir,
  readConfigFile,
  writeConfigFile,
} from "../../lib/api";
import { loadGuiConfig } from "../../lib/guiConfig";
import { buildTree, type FileNode } from "../../lib/fileTree";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import InstanceChip from "../../components/ui/InstanceChip.vue";
import MonacoText from "../../components/ui/MonacoText.vue";
import HexView from "./parts/HexView.vue";
import ImageView from "./parts/ImageView.vue";
import McaTable from "./parts/McaTable.vue";
import NbtTree from "./parts/NbtTree.vue";
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
const saving = ref(false);

const dirty = computed(() => text.value !== origin.value);
const isText = computed(() => !!selected.value && TEXT_TYPES.includes(selected.value.fileType));

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
  // 树与搜索都跟着这批文件走：换了来源 / 实例，旧的目录 key 与关键字都没意义了
  expanded.value = new Set();
  query.value = "";
  if (!instanceUuid.value) {
    files.value = [];
    return;
  }

  loading.value = true;
  try {
    const list = source.value
      ? await listConfigSaveFiles(instanceUuid.value, source.value)
      : await listConfigFiles(instanceUuid.value);
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
  if (!TEXT_TYPES.includes(file.fileType)) return;

  loading.value = true;
  try {
    const data = await readConfigFile(instanceUuid.value, file.name);
    text.value = data;
    origin.value = data;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 保存当前文本文件，整份覆盖 */
async function save() {
  const file = selected.value;
  if (!file || !isText.value || saving.value || !dirty.value) return;

  saving.value = true;
  try {
    await writeConfigFile(instanceUuid.value, file.name, text.value);
    origin.value = text.value;
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
// 单窗口模式切回本窗口时重读；有没保存的改动就不动，免得把编辑内容冲掉
useWindowRefresh(() => {
  if (!dirty.value) void load();
});
</script>

<template>
  <WindowFrame :title="t('config.title')" body-fill @close="emit('close')">
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
        <span v-if="isText && dirty" class="head-dirty">{{ t("config.unsaved") }}</span>
        <span v-else-if="!isText && selected" class="head-readonly">{{ t("config.readonly") }}</span>

        <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="loading" v-tip="t('config.refresh')"
          :aria-label="t('config.refresh')" @click="loadFiles">
          <GlyphIcon name="refresh" :size="15" :weight="1.8" />
        </BaseButton>
        <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="!instanceUuid" v-tip="t('config.openDir')"
          :aria-label="t('config.openDir')" @click="openDir">
          <GlyphIcon name="folder" :size="15" :weight="1.8" />
        </BaseButton>
        <BaseButton v-if="isText" size="sm" :variant="dirty ? 'accent' : 'ghost'" class="icon-btn-head"
          :disabled="saving || !dirty" v-tip="t('config.save')" :aria-label="t('config.save')" @click="save">
          <GlyphIcon name="save" :size="15" :weight="1.8" />
        </BaseButton>
      </div>
    </template>

    <div class="cfg-body">
      <!-- 左：搜索固定，只有树滚动 -->
      <aside class="cfg-list">
        <div class="cfg-search">
          <input v-model="query" class="field-input" :placeholder="t('config.search')" spellcheck="false" />
          <button v-if="query" class="cfg-search-clear" v-tip="t('config.searchClear')"
            :aria-label="t('config.searchClear')" @click="query = ''">
            <GlyphIcon name="close" :size="12" />
          </button>
        </div>

        <div class="cfg-tree">
          <button v-for="row in rows" :key="row.key" class="cfg-row"
            :class="{ on: selected?.name === row.key }"
            :style="{ paddingLeft: `${6 + row.depth * 14}px` }" :title="row.key"
            @click="row.isDir ? toggleExpand(row.key) : openFile(row.key)">
            <GlyphIcon v-if="row.isDir" class="cfg-chevron" :class="{ open: expanded.has(row.key) }" name="chevron-down"
              :size="11" :weight="2.4" />
            <span v-else class="cfg-chevron"></span>
            <span class="cfg-file-name">{{ row.name }}</span>
          </button>
          <p v-if="!rows.length" class="empty-tip">{{ emptyTip }}</p>
        </div>
      </aside>

      <!-- 右：按类型选视图 -->
      <section class="cfg-view">
        <p v-if="!selected" class="empty-tip">{{ t("config.pickFile") }}</p>
        <MonacoText v-else-if="isText" v-model="text" :language="langOf(selected)" />
        <NbtTree v-else-if="selected.fileType === 'nbt'" :uuid="instanceUuid" :file="selected.name" />
        <McaTable v-else-if="selected.fileType === 'mca'" :uuid="instanceUuid" :file="selected.name" />
        <ImageView v-else-if="selected.fileType === 'image'" :uuid="instanceUuid" :file="selected.name" />
        <HexView v-else :uuid="instanceUuid" :file="selected.name" />
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
  gap: 12px;
  flex: 1;
  min-height: 0;
}

/* 左栏：搜索固定在上，只有下面的树滚动 */
.cfg-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  /* 比原来宽 30px：树按层级缩进，路径又长，250px 下名字动不动就走省略号 */
  width: 280px;
  flex-shrink: 0;
  overflow: hidden;
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
}

.cfg-tree {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* 搜索框：清空按钮压在输入框右端 */
.cfg-search {
  position: relative;
  flex-shrink: 0;
}

.cfg-search .field-input {
  width: 100%;
  padding-right: 28px;
}

.cfg-search-clear {
  position: absolute;
  top: 50%;
  right: 6px;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.cfg-search-clear:hover {
  background: var(--bg-hover);
  color: var(--text);
}

/* 树的一行：目录点开 / 收起，文件点开进右栏。缩进由模板按层级给 padding-left */
.cfg-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text);
  font-family: inherit;
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
}

.cfg-row:hover {
  background: var(--bg-hover);
}

.cfg-row.on {
  background: var(--accent-soft);
  color: var(--accent);
}

/* 收起时箭头朝右，展开朝下 */
.cfg-chevron {
  flex-shrink: 0;
  /* 文件行这里是占位：给同宽，同一层的文件名才和目录名对齐 */
  width: 11px;
  color: var(--text-dim);
  transform: rotate(-90deg);
  transition: transform 0.12s;
}

.cfg-chevron.open {
  transform: rotate(0);
}

.cfg-file-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cfg-view {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
}
</style>
