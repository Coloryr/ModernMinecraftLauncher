// 分组列表的**行虚拟化**：只渲染视口附近那几行，其余的行用上下内边距撑住高度，
// 滚动条长度与滚动位置都不变。
//
// 不引现成库：这里的分组头、拖拽排序、右键菜单都是现成结构，需要的只是"少渲染几行"。
// 窗口怎么算是纯算术（见 virtualWindow.ts），这一层只管 P 滚动状态与元素位置。
import { computed, inject, onBeforeUnmount, provide, ref, shallowRef, type Ref } from "vue";

import { visibleWindow, type VirtualWindow } from "./virtualWindow";

export { MOD_ROW_GAP, MOD_ROW_H, MOD_TR_H, visibleWindow } from "./virtualWindow";
export type { VirtualWindow } from "./virtualWindow";

/** 列表滚动状态，由资源窗口的模组页建立并 provide */
export interface ListScrollState {
  /** 滚动容器的滚动位置，单位 px */
  top: Ref<number>;
  /** 滚动容器的高度，单位 px */
  height: Ref<number>;
  /** 当前那个滚动容器；行虚拟化要拿它算"行容器在哪儿" */
  container: Ref<HTMLElement | null>;
  /**
   * 布局代次
   *
   * 分组折叠 / 展开、搜索过滤、分组排序都会让**后面那些组整体挪位置**，
   * 而挪位置本身不触发滚动事件 —— 各分组的窗口得靠它重算，否则会留一片没渲染的空白。
   * 见 [`ListScrollState.bump`]。
   */
  version: Ref<number>;
  /** 换一个滚动容器（列表 / 表格视图各一个），旧容器上的监听会摘掉 */
  attach: (el: HTMLElement | null) => void;
  /** 报一次"布局变了"：各分组据此重新量位置、重算窗口 */
  bump: () => void;
}

/** 注入键；只有资源窗口的模组页会 provide */
const LIST_SCROLL = Symbol("resource-list-scroll");

/**
 * 建立列表滚动状态并 provide 给同页的各分组
 *
 * 滚动事件很密，一帧只同步一次：各组分的窗口计算都读同一个 `top`，
 * 免得每个分组各自监听、各自算一遍。
 *
 * # 返回值
 *
 * 见 [`ListScrollState`]；调用方把它绑到当前视图的滚动容器上
 */
export function provideListScroll(): ListScrollState {
  const top = ref(0);
  const height = ref(0);
  const container = shallowRef<HTMLElement | null>(null);
  let frame = 0;

  const sync = () => {
    const el = container.value;
    if (!el) return;
    top.value = el.scrollTop;
    height.value = el.clientHeight;
  };

  const onScroll = () => {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      sync();
    });
  };

  const observer = new ResizeObserver(() => sync());

  const attach = (el: HTMLElement | null) => {
    if (el === container.value) return;
    container.value?.removeEventListener("scroll", onScroll);
    observer.disconnect();
    container.value = el;
    el?.addEventListener("scroll", onScroll, { passive: true });
    if (el) observer.observe(el);
    sync();
  };

  onBeforeUnmount(() => {
    if (frame) cancelAnimationFrame(frame);
    container.value?.removeEventListener("scroll", onScroll);
    observer.disconnect();
  });

  const version = ref(0);
  const bump = () => {
    version.value += 1;
  };
  const state: ListScrollState = { top, height, container, version, attach, bump };
  provide(LIST_SCROLL, state);
  return state;
}

/**
 * 按行虚拟化取当前窗口
 *
 * 行容器的顶边每次重算时现量：分组折叠 / 展开、搜索过滤、分组排序都会让前面那些组的高度变。
 * 量的是"根元素相对滚动容器内容区"的位置，容器自己滚出去的那一段要算回来。
 *
 * - `root`: 行容器的根元素
 * - `count`: 行数
 * - `rowH`: 行高，单位 px，与 CSS 里钉死的一致
 * - `gap`: 行间距，单位 px；默认 0
 * - `whole`: 返回 true 时**整列都渲染**（不做窗口）—— 表格里正在就地编辑备注时用：
 *   被窗口裁掉的那一行会连元素一起销毁，正在写的草稿就丢了
 *
 * # 返回值
 *
 * `start` / `end` 是要渲染的下标区间，`padTop` / `padBottom` 是上下留白
 */
export function useVirtualRows(options: {
  root: Ref<HTMLElement | null>;
  count: () => number;
  rowH: number;
  gap?: number;
  whole?: () => boolean;
}) {
  const scroll = inject<ListScrollState | null>(LIST_SCROLL, null);

  const win = computed<VirtualWindow>(() => {
    // 依赖布局代次：折叠 / 过滤 / 排序之后各组的位置变了，得重量
    void scroll?.version.value;
    const count = options.count();
    const rootEl = options.root.value;
    const containerEl = scroll?.container.value ?? null;
    // 整列都要渲染（就地编辑备注）、没有滚动状态（组件被单独用）或还没量到元素：
    // 先全渲染，别把行藏没了
    if (options.whole?.() || !scroll || !rootEl || !containerEl || count <= 0) {
      return { start: 0, end: Math.max(count, 0), padTop: 0, padBottom: 0 };
    }

    const rootTop =
      rootEl.getBoundingClientRect().top -
      containerEl.getBoundingClientRect().top +
      containerEl.scrollTop;
    return visibleWindow({
      scrollTop: scroll.top.value,
      viewHeight: scroll.height.value,
      rootTop,
      count,
      rowH: options.rowH,
      gap: options.gap ?? 0,
    });
  });

  return {
    /** 要渲染的第一行下标 */
    start: computed(() => win.value.start),
    /** 要渲染的最后一行下标，不含 */
    end: computed(() => win.value.end),
    /** 上面藏起来的行占的高度 */
    padTop: computed(() => win.value.padTop),
    /** 下面藏起来的行占的高度 */
    padBottom: computed(() => win.value.padBottom),
    /** 本组自己的行数变了（展开内置模组等）：报一次布局变化，后面那些组要重量位置 */
    remeasure: () => scroll?.bump(),
  };
}
