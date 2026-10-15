<script setup lang="ts">
// 导出用的文件树：一层一层向 Rust 要目录内容，勾选状态留在父组件
//
// 勾目录等于"整目录进 overrides"（后端会把目录展开成文件），所以目录也能勾。
// 展平成一维行来渲染：递归组件在只有"目录 + 文件"两层结构时反而更绕。
import { computed, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { api } from "../../../lib/api";
import type { ExportEntryDto } from "../../../lib/bindings";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 已勾选的条目表，键是相对游戏目录的路径；由父组件持有 */
  picked: Map<string, unknown>;
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

/** 拉一个目录的内容，拉过就不再拉 */
async function load(dir: string) {
  if (children.value.has(dir) || loading.value.has(dir)) return;

  loading.value.add(dir);
  try {
    children.value.set(dir, await api.listExportDir(props.uuid, dir));
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

/** 文件大小，按档显示 */
function sizeText(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

// 换实例就把整棵树清掉重来
watch(
  () => props.uuid,
  () => {
    children.value = new Map();
    opened.value = new Set();
    if (props.uuid) void load("");
  },
  { immediate: true },
);
</script>

<template>
  <div class="tree">
    <p v-if="!rows.length" class="empty-tip">{{ t("winExport.noFiles") }}</p>
    <div v-for="row in rows" :key="row.entry.path" class="tree-row" :style="{ paddingLeft: `${6 + row.depth * 14}px` }">
      <span class="tree-caret" :class="{ open: opened.has(row.entry.path) }" @click="toggleDir(row.entry.path)">
        <GlyphIcon v-if="row.entry.dir" name="chevron-down" :size="12" :weight="2.4" />
      </span>
      <input type="checkbox" class="tree-box" :checked="picked.has(row.entry.path)"
        @change="emit('toggle', row.entry)" />
      <span class="tree-name" :class="{ dir: row.entry.dir }" :title="row.entry.path"
        @click="row.entry.dir && toggleDir(row.entry.path)">{{ row.entry.path.split("/").pop() }}</span>
      <span v-if="!row.entry.dir" class="tree-size">{{ sizeText(row.entry.size) }}</span>
    </div>
  </div>
</template>

<style scoped>
.tree {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
}

.tree-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 6px;
  border-radius: 5px;
  font-size: 12.5px;
}

.tree-row:hover {
  background: var(--bg-hover);
}

/* 箭头占位：文件行也要占同样的宽度，名字才对得齐。收起朝右、展开朝下 */
.tree-caret {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 12px;
  flex-shrink: 0;
  color: var(--text-dim);
  cursor: pointer;
  transform: rotate(-90deg);
  transition: transform 0.12s;
}

.tree-caret.open {
  transform: rotate(0);
}

.tree-box {
  flex-shrink: 0;
  margin: 0;
  cursor: pointer;
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

.tree-size {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11px;
}
</style>
