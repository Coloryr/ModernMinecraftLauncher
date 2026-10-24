<script setup lang="ts">
import { computed, ref } from 'vue';

interface Row {
  name: string;
  path: string;
  dir: boolean;
  depth: number;
}

interface Node {
  name: string;
  path: string;
  dir: boolean;
  children: Map<string, Node>;
}

const props = defineProps<{
  /** 项目内的文件路径，使用 / 分隔 */
  files: string[];
  /** 当前打开的文件路径；空串表示没有选中 */
  activePath: string;
  /** 有长任务时禁止交互 */
  disabled: boolean;
}>();
const emit = defineEmits<{ select: [path: string] }>();

// 默认全部展开；只记录被用户收起的目录，刷新文件时不会重置展开状态
const collapsed = ref<Set<string>>(new Set());

const rows = computed<Row[]>(() => {
  const root: Node = { name: '', path: '', dir: true, children: new Map() };
  for (const file of props.files) {
    const parts = file.split('/');
    let node = root;
    for (let index = 0; index < parts.length; index += 1) {
      const name = parts[index];
      const dir = index < parts.length - 1;
      let child = node.children.get(name);
      if (!child) {
        child = {
          name,
          path: parts.slice(0, index + 1).join('/'),
          dir,
          children: new Map(),
        };
        node.children.set(name, child);
      }
      node = child;
    }
  }

  const result: Row[] = [];
  const walk = (node: Node, depth: number) => {
    const children = [...node.children.values()].sort((a, b) =>
      a.dir === b.dir ? a.name.localeCompare(b.name) : a.dir ? -1 : 1);
    for (const child of children) {
      result.push({ name: child.name, path: child.path, dir: child.dir, depth });
      if (child.dir && !collapsed.value.has(child.path)) walk(child, depth + 1);
    }
  };
  walk(root, 0);
  return result;
});

function toggle(row: Row) {
  const next = new Set(collapsed.value);
  if (next.has(row.path)) next.delete(row.path);
  else next.add(row.path);
  collapsed.value = next;
}
</script>

<template>
  <template v-for="row in rows" :key="row.path">
    <button
      v-if="row.dir"
      class="file-entry tree-row dir-row ui-btn v-ghost s-sm"
      :style="{ paddingLeft: `${8 + row.depth * 14}px` }"
      :disabled="disabled"
      :title="row.path"
      :aria-expanded="!collapsed.has(row.path)"
      @click="toggle(row)"
    >
      <span class="tree-caret">{{ collapsed.has(row.path) ? '▸' : '▾' }}</span>
      <span class="tree-glyph">▤</span>
      <span class="tree-name">{{ row.name }}</span>
    </button>
    <button
      v-else
      class="file-entry tree-row ui-btn v-ghost s-sm"
      :class="{ selected: row.path === activePath }"
      :style="{ paddingLeft: `${8 + row.depth * 14}px` }"
      :disabled="disabled"
      :title="row.path"
      @click="emit('select', row.path)"
    >
      <span class="tree-caret" />
      <span class="tree-glyph">◇</span>
      <span class="tree-name">{{ row.name }}</span>
    </button>
  </template>
</template>

<style scoped>
.file-entry.tree-row { gap: 4px; }
.tree-row.dir-row { font-weight: 500; }
.tree-caret { flex: 0 0 12px; width: 12px; color: var(--text-dim); font-size: 10.5px; text-align: center; }
.tree-glyph { flex: 0 0 12px; width: 12px; color: var(--text-dim); text-align: center; }
.tree-name { min-width: 0; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.file-entry.tree-row.selected .tree-name { color: var(--accent); }
</style>
