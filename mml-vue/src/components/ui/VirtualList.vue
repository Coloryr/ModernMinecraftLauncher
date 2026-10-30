<script setup lang="ts">
// 通用行虚拟列表：只渲染视口附近那几行，其余用上下内边距撑住高度，滚动条位置与长度不变
//
// 容器由本组件给（调用方用 class 传 `.item-list` / `.group-list` 这类滚动样式），需要固定头
// （存档下拉那种）放 `#header` 插槽，它不参与窗口计算。行放默认插槽，用作用域参数
// 拿要渲染的下标区间：
//
//     <VirtualList class="item-list" :count="items.length" :row-gap="RESOURCE_ROW_GAP">
//       <template #default="{ start, end }">
//         <ResourceRow v-for="item in items.slice(start, end)" :key="item.file" ... />
//       </template>
//     </VirtualList>
//
// 两种行高口径：
// - **定高**（默认）：窗口按下标算，行高由 CSS 钉死（`.vlist-rows .item-row`）。名字会换行、
//   高度真的不齐的列表把 `measure-row` 打开，本组件量第一行当估值；
// - **变高**（`heights`）：分组列表那种"分组标题 / 实例行 / 平铺卡片各一档高"的列表用，
//   每行高度由调用方按行种给，同样是 CSS 里钉死的常量。
import { nextTick, onMounted, ref, watch } from "vue";
import {
  LIST_ROW_GAP,
  LIST_ROW_H,
  createListScroll,
  measureRowHeight,
  useVirtualRows,
} from "../../composables/useVirtualRows";

const props = withDefaults(
  defineProps<{
    /** 总行数 */
    count: number;
    /** 行高，单位 px；与 CSS 里钉死的一致。给了 `heights` 就不看它 */
    rowH?: number;
    /** 每行高度，单位 px，顺序与行一致；给了就按变高算 */
    heights?: number[];
    /** 行间距，单位 px；与调用方列表原来的 gap 一致 */
    rowGap?: number;
    /** 行高不一致时量第一行当估值，见文件头 */
    measureRow?: boolean;
  }>(),
  { rowH: LIST_ROW_H, rowGap: LIST_ROW_GAP, measureRow: false, heights: undefined },
);

/** 滚动容器 */
const container = ref<HTMLElement | null>(null);
/** 行容器：虚拟化要量它在滚动容器里的位置 */
const root = ref<HTMLElement | null>(null);

const scroll = createListScroll();
/** 当前用的行高：量出来的那份优先，量不到就用 props.rowH */
const rowH = ref(props.rowH);

const { start, end, padTop, padBottom } = useVirtualRows({
  root,
  count: () => props.count,
  rowH: () => rowH.value,
  heights: () => props.heights ?? null,
  gap: () => props.rowGap,
  scroll,
});

onMounted(() => scroll.attach(container.value));

// 行高不一致的列表：等 DOM 更新完量第一行（量不到就退回估值）
watch(
  [() => props.count, () => props.measureRow],
  async () => {
    if (!props.measureRow) return;
    await nextTick();
    rowH.value = measureRowHeight(root.value, props.rowH);
  },
  { immediate: true, flush: "post" },
);

/**
 * 第 index 行顶边在行容器里的位置
 *
 * - `index`: 行下标
 *
 * # 返回值
 *
 * 偏移量，单位 px
 */
function offsetOf(index: number): number {
  const heights = props.heights;
  if (heights && heights.length === props.count) {
    let sum = 0;
    for (let i = 0; i < index; i += 1) sum += heights[i];
    return sum + index * props.rowGap;
  }
  return index * (rowH.value + props.rowGap);
}

/**
 * 把某一行滚进视野
 *
 * 虚拟列表里被藏起来的行**没有 DOM**，靠 `scrollIntoView` 找不到，所以按行高自己算位置。
 *
 * - `index`: 行下标，越界时什么都不做
 * - `align`: `nearest` 只在看不见时滚，`center` 居中
 */
function scrollToIndex(index: number, align: "nearest" | "center" = "nearest") {
  const box = container.value;
  const rows = root.value;
  if (!box || !rows || index < 0 || index >= props.count) return;
  const rootTop = rows.getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop;
  const heights = props.heights;
  const h = heights && heights.length === props.count ? heights[index] : rowH.value;
  const top = rootTop + offsetOf(index);
  if (align === "center") {
    box.scrollTop = top + h / 2 - box.clientHeight / 2;
  } else if (top < box.scrollTop) {
    box.scrollTop = top;
  } else if (top + h > box.scrollTop + box.clientHeight) {
    box.scrollTop = top + h - box.clientHeight;
  }
}

/**
 * 行的位置变了（行高 / 行距被改、外面换了数据）时重算窗口
 *
 * 正常滚动由滚动状态自己驱动，这里只给"布局变了但没滚动"的情况兜底。
 */
function remeasure() {
  scroll.bump();
}
defineExpose({ remeasure, scrollToIndex, el: container });
</script>

<template>
  <div ref="container" class="vlist">
    <slot name="header" />
    <div ref="root" class="vlist-rows"
      :style="{ paddingTop: padTop + 'px', paddingBottom: padBottom + 'px', gap: rowGap + 'px' }">
      <slot :start="start" :end="end" />
    </div>
  </div>
</template>

<style scoped>
/* 兜底：调用方一般还会传 `.item-list` / `.group-list` 这类类，那边的滚动样式优先 */
.vlist {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow-y: auto;
}

.vlist-rows {
  display: flex;
  flex-direction: column;
}
</style>
