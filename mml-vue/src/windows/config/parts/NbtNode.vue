<script setup lang="ts">
// NBT 树上的一个节点：复合递归成员、列表递归元素，自己也是个可点开的行
//
// 数组（字节 / 整数 / 长整数）**不展开内容**，行尾只报元素个数 —— 一个高度图动辄上万项，
// 铺开既慢也没人看；真要看字节去二进制视图。
import { computed, ref } from "vue";
import { t } from "../../../lib/i18n";
import type { NbtNodeDto } from "../../../lib/bindings";

defineOptions({ name: "NbtNode" });

const props = defineProps<{
  /** 本节点 */
  node: NbtNodeDto;
  /** 缩进层级，从 0 起；前两层默认展开 */
  depth: number;
}>();

/** 复合与列表才有下钻 */
const isBranch = computed(() => props.node.kind === "Compound" || props.node.kind === "List");
/** 展开状态 */
const open = ref(props.depth < 2);

/** 下钻的子节点：列表用 `items`、复合用 `children` */
const below = computed(() => (props.node.kind === "List" ? props.node.items : props.node.children));

/** 行尾的值：标量给值，列表与数组给个数 */
const valueLabel = computed(() => {
  if (props.node.kind === "List" || props.node.kind.endsWith("Array")) {
    return t("config.nbtCount", { n: props.node.length });
  }
  return props.node.value;
});
</script>

<template>
  <div class="nbt-node">
    <div class="nbt-row" :class="{ branch: isBranch }" @click="isBranch && (open = !open)">
      <span class="nbt-caret" :class="{ open }">{{ isBranch ? "▸" : "" }}</span>
      <span class="nbt-name">{{ node.name }}</span>
      <span class="nbt-kind">{{ node.kind }}</span>
      <span class="nbt-value" :title="valueLabel">{{ valueLabel }}</span>
    </div>
    <div v-if="isBranch && open" class="nbt-children">
      <NbtNode v-for="(child, index) in below" :key="`${child.name}-${index}`" :node="child" :depth="depth + 1" />
    </div>
  </div>
</template>

<style scoped>
.nbt-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 6px;
  border-radius: 5px;
  font-size: 12.5px;
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
}

.nbt-row.branch {
  cursor: pointer;
}

.nbt-row.branch:hover {
  background: var(--bg-hover);
}

/* 箭头旋转 90° 表示展开 */
.nbt-caret {
  width: 10px;
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 10px;
  transition: transform 0.12s;
}

.nbt-caret.open {
  transform: rotate(90deg);
}

.nbt-name {
  flex-shrink: 0;
  color: var(--text);
}

.nbt-kind {
  flex-shrink: 0;
  padding: 0 5px;
  border-radius: 4px;
  background: var(--bg-hover);
  color: var(--text-dim);
  font-size: 10.5px;
}

/* 值可被挤压 */
.nbt-value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-dim);
}

.nbt-children {
  padding-left: 14px;
}
</style>
