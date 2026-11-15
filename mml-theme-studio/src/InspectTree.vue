<script setup lang="ts">
import type { InspectTreeNode } from './inspector';

defineProps<{
  /** 这一层要显示的节点，按文档顺序 */
  nodes: InspectTreeNode[];
  /** 缩进层级，根层为 0 */
  depth: number;
  /** 已展开节点的 path key */
  expanded: Set<string>;
  /** 当前选中节点的 path key */
  selected: string;
}>();
const emit = defineEmits<{
  toggle: [node: InspectTreeNode];
  select: [node: InspectTreeNode];
  hover: [node: InspectTreeNode | null];
}>();

/** path 数组转成稳定 key；根节点是空串 */
function keyOf(node: InspectTreeNode) {
  return node.path.join('/');
}
</script>

<template>
  <ul class="tree" :class="{ 'is-nested': depth > 0 }">
    <li v-for="item in nodes" :key="keyOf(item)">
      <div
        class="tree-row"
        :class="{ 'is-selected': selected === keyOf(item) }"
        :style="{ paddingLeft: depth * 12 + 'px' }"
        @click="emit('select', item)"
        @mouseenter="emit('hover', item)"
        @mouseleave="emit('hover', null)"
      >
        <button
          class="tree-arrow"
          :class="{ 'is-empty': item.childCount === 0 }"
          type="button"
          :aria-label="expanded.has(keyOf(item)) ? 'collapse' : 'expand'"
          :disabled="item.childCount === 0"
          @click.stop="emit('toggle', item)"
        >{{ expanded.has(keyOf(item)) ? '▾' : '▸' }}</button>
        <span class="tree-tag">{{ item.tag }}</span>
        <span v-if="item.id" class="tree-id">#{{ item.id }}</span>
        <span v-if="item.classes.length" class="tree-class">.{{ item.classes.join('.') }}</span>
        <span v-if="item.childCount" class="tree-count">{{ item.childCount }}</span>
      </div>

      <InspectTree
        v-if="expanded.has(keyOf(item)) && item.children.length"
        :nodes="item.children"
        :depth="depth + 1"
        :expanded="expanded"
        :selected="selected"
        @toggle="emit('toggle', $event)"
        @select="emit('select', $event)"
        @hover="emit('hover', $event)"
      />
    </li>
  </ul>
</template>

<style scoped>
.tree {
  margin: 0;
  padding: 0;
  list-style: none;
}

.tree-row {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 20px;
  overflow: hidden;
  border-radius: 4px;
  color: var(--text);
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 11px;
  white-space: nowrap;
  cursor: default;
}

.tree-row:hover {
  background: var(--bg-card);
}

.tree-row.is-selected {
  background: var(--accent-soft);
  color: var(--accent);
}

.tree-arrow {
  flex: none;
  width: 14px;
  height: 16px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-dim);
  font-size: 9px;
  line-height: 1;
  cursor: pointer;
}

.tree-arrow.is-empty {
  cursor: default;
  visibility: hidden;
}

.tree-tag {
  color: var(--accent-2);
}

.tree-id {
  color: var(--yellow);
}

.tree-class {
  overflow: hidden;
  color: var(--text-dim);
  text-overflow: ellipsis;
}

.tree-count {
  margin-left: 4px;
  padding: 0 4px;
  border-radius: 999px;
  background: var(--bg-card);
  color: var(--text-dim);
  font-size: 9.5px;
}
</style>
