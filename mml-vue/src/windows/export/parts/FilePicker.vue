<script setup lang="ts">
// 导出用的文件树：一层一层向 Rust 要目录内容，勾选状态留在父组件
//
// 勾选沿目录继承：勾了目录，里面的东西都算勾上；取消勾选记成排除项，父目录勾着也能去掉某一条。
// 展平成一维行来渲染：递归组件在只有"目录 + 文件"两层结构时反而更绕。
import { computed, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { api } from "../../../lib/api";
import type { ExportEntryDto } from "../../../lib/bindings";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { isPickedPath } from "../picked";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 已勾选的条目表，键是相对根目录的路径；由父组件持有 */
  picked: Map<string, unknown>;
  /** 取消勾选的路径；父目录勾着时单独去掉某一条，默认空 */
  unpicked?: Set<string>;
  /** 根目录：game = 游戏目录（默认），instance = 实例目录；「直接压缩」用后者 */
  base?: "game" | "instance";
  /** 初始展开的目录（相对根目录）；默认勾选落在子目录里时靠它露出来 */
  expand?: string[];
  /** 必选项判定：命中时勾选框固定勾上并禁用，默认没有必选项 */
  locked?: (path: string) => boolean;
}>();

const emit = defineEmits<{
  /** 勾选 / 取消勾选一个条目 */
  (e: "toggle", entry: ExportEntryDto): void;
}>();

/** 相对路径 → 该目录下的子项；根目录用空串当键 */
const children = ref(new Map<string, ExportEntryDto[]>());
/** 展开着的目录 */
const opened = ref(new Set<string>());
/** 正在拉的目录，防重复请求 */
const loading = ref(new Set<string>());

/** 展平后的可见行：只含已展开目录的后代 */
interface TreeRow {
  /** 这一行的条目 */
  entry: ExportEntryDto;
  /** 缩进层级，根目录的子项为 0 */
  depth: number;
}

const rows = computed<TreeRow[]>(() => {
  const list: TreeRow[] = [];
  const walk = (dir: string, depth: number) => {
    for (const entry of children.value.get(dir) ?? []) {
      list.push({ entry, depth });
      if (entry.dir && opened.value.has(entry.path)) {
        walk(entry.path, depth + 1);
      }
    }
  };
  walk("", 0);
  return list;
});

/** 按根目录选接口：实例目录给「直接压缩」用 */
function listDir(dir: string) {
  return props.base === "instance"
    ? api.listExportInstanceDir(props.uuid, dir)
    : api.listExportDir(props.uuid, dir);
}

/** 拉一个目录的内容，拉过就不再拉 */
async function load(dir: string) {
  if (children.value.has(dir) || loading.value.has(dir)) return;

  // 这一发回来时可能已经换了实例 / 根目录，那张表整份换掉了：别写进新的那张
  const table = children.value;
  loading.value.add(dir);
  try {
    const list = await listDir(dir);
    if (table === children.value) table.set(dir, list);
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value.delete(dir);
  }
}

/** 展开 / 收起一个目录 */
async function toggleDir(dir: string) {
  if (opened.value.has(dir)) {
    opened.value.delete(dir);
    return;
  }
  opened.value.add(dir);
  await load(dir);
}

/** 没有排除项时共用的空表，免得每渲染一行就新建一个 */
const NO_UNPICKED: ReadonlySet<string> = new Set();

/** 勾选状态：勾了目录，里面每一条都跟着算勾上 */
function checked(path: string): boolean {
  return isPickedPath(path, props.picked, props.unpicked ?? NO_UNPICKED);
}

/** 必选条目：勾选框固定勾上并禁用，点了也不改勾选 */
function isLocked(path: string): boolean {
  return props.locked?.(path) === true;
}

/** 勾选 / 取消一行；必选条目不动 */
function toggleRow(entry: ExportEntryDto) {
  if (!isLocked(entry.path)) emit("toggle", entry);
}

/** 文件大小，按档显示 */
function sizeText(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

// 换实例或换根目录就把整棵树清掉重来，再把指定的目录展开
watch(
  () => [props.uuid, props.base],
  async () => {
    children.value = new Map();
    opened.value = new Set();
    if (!props.uuid) return;
    await load("");
    for (const dir of props.expand ?? []) {
      if (!children.value.get("")?.some((e) => e.dir && e.path === dir)) continue;
      opened.value.add(dir);
      await load(dir);
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="tree">
    <p v-if="!rows.length" class="empty-tip">{{ t("winExport.noFiles") }}</p>
    <div v-for="row in rows" :key="row.entry.path" class="tree-row"
      :style="{ paddingLeft: `${8 + row.depth * 16}px` }">
      <span class="tree-caret" :class="{ open: opened.has(row.entry.path) }" @click="toggleDir(row.entry.path)">
        <GlyphIcon v-if="row.entry.dir" name="chevron-down" :size="12" :weight="2.4" />
      </span>
      <input type="checkbox" class="tree-box" :checked="isLocked(row.entry.path) || checked(row.entry.path)"
        :disabled="isLocked(row.entry.path)"
        :title="isLocked(row.entry.path) ? t('winExport.mandatory') : undefined"
        @change="toggleRow(row.entry)" />
      <span class="tree-name" :class="{ dir: row.entry.dir, locked: isLocked(row.entry.path) }"
        :title="row.entry.path"
        @click="row.entry.dir ? toggleDir(row.entry.path) : toggleRow(row.entry)">{{
          row.entry.path.split("/").pop()
        }}</span>
      <span class="tree-size">{{ row.entry.dir ? "—" : sizeText(row.entry.size) }}</span>
    </div>
  </div>
</template>

<style scoped>
.tree {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-solid, var(--bg-card));
  /* 只留滚动条那一侧的槽：条目左右内边距本来就一样，两侧都留反而把内容推向右 */
  scrollbar-gutter: stable;
}

.tree-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 27px;
  padding: 0 8px;
  border-radius: 8px;
  font-size: 12.5px;
  /* 点一下就是一整行的操作，别让它顺手选中文字 */
  user-select: none;
}

.tree-row:hover {
  background: var(--bg-hover);
}

/* 箭头占位：文件行也要占同样的宽度，名字才对得齐。收起朝右、展开朝下 */
.tree-caret {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  flex-shrink: 0;
  color: var(--text-dim);
  cursor: pointer;
  transform: rotate(-90deg);
  transition: transform 0.12s;
}

.tree-caret.open {
  transform: rotate(0);
}

/* 勾选框放大一档，原生控件只调尺寸，配色仍跟主题走 */
.tree-box {
  flex-shrink: 0;
  width: 15px;
  height: 15px;
  margin: 0;
  accent-color: var(--accent);
  cursor: pointer;
}

/* 必选项：勾着但点不动 */
.tree-box:disabled {
  cursor: default;
}

.tree-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.tree-name.dir {
  font-weight: 600;
}

.tree-name.locked {
  cursor: default;
}

.tree-size {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
