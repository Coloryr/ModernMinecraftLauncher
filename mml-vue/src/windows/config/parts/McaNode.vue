<script setup lang="ts">
// 区块 NBT 的一行，只读：类型徽标 + 名字 + 值，复合与列表可展开
//
// 与 NBT 树（NbtNode.vue）观感一致，但没有编辑入口 —— 区域文件里的区块目前只读，
// 写回要走整块重写，还没做。
import { computed, ref } from "vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import type { NbtNodeDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 节点 */
  node: NbtNodeDto;
  /** 缩进层级，0 是区块根的直接成员 */
  depth: number;
}>();

/** 是否展开；浅两层默认展开，再深就折起来 */
const open = ref(props.depth < 2);

/** 能展开的子节点：复合的成员与列表 / 数组的元素 */
const children = computed(() =>
  props.node.children.length ? props.node.children : props.node.items,
);

const expandable = computed(() => children.value.length > 0);

/** 类型徽标，与主 NBT 树同一套字母 */
const badge = computed(() => {
  switch (props.node.kind) {
    case "Byte":
      return "B";
    case "Short":
      return "S";
    case "Int":
      return "I";
    case "Long":
      return "L";
    case "Float":
      return "F";
    case "Double":
      return "D";
    case "String":
      return "T";
    case "ByteArray":
      return "[B]";
    case "IntArray":
      return "[I]";
    case "LongArray":
      return "[L]";
    case "List":
      return "[]";
    case "Compound":
      return "{}";
    default:
      return "?";
  }
});

/** 值那一列：容器给个数，标量给值 */
const shown = computed(() => {
  switch (props.node.kind) {
    case "List":
    case "ByteArray":
    case "IntArray":
    case "LongArray":
      return `[${props.node.length}]`;
    case "Compound":
      return `{${props.node.children.length}}`;
    default:
      return props.node.value;
  }
});
</script>

<template>
  <div>
    <button class="nbt-line" :style="{ paddingLeft: `${depth * 14}px` }"
      @click="expandable && (open = !open)">
      <GlyphIcon v-if="expandable" class="nbt-chevron" :class="{ open }" name="chevron-down" :size="11"
        :weight="2.4" />
      <span v-else class="nbt-chevron-space" />
      <span class="nbt-kind">{{ badge }}</span>
      <span class="nbt-name">{{ node.name }}</span>
      <span class="nbt-value">{{ shown }}</span>
    </button>
    <template v-if="expandable && open">
      <McaNode v-for="child in children" :key="child.name" :node="child" :depth="depth + 1" />
    </template>
  </div>
</template>

<style scoped>
.nbt-line {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 1px 8px;
  border: none;
  background: transparent;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: default;
}

.nbt-line:hover {
  background: var(--bg-hover);
}

/* 收起时箭头指向右：与 NBT 树同一套朝向 */
.nbt-chevron {
  flex-shrink: 0;
  color: var(--text-dim);
  transform: rotate(-90deg);
  transition: transform 0.12s;
}

.nbt-chevron.open {
  transform: none;
}

.nbt-chevron-space {
  width: 11px;
  flex-shrink: 0;
}

.nbt-kind {
  width: 26px;
  flex-shrink: 0;
  color: var(--text-dim);
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  font-size: 11px;
}

.nbt-name {
  flex-shrink: 0;
  font-size: 12px;
}

.nbt-value {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-dim);
  font-size: 12px;
}
</style>
