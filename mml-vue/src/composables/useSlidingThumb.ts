// 竖向导航的滑动选中块：设置页、资源 / 方块 / 收藏分类栏、服务器实例分页共用
//
// 位置与尺寸都按真实按钮量，文案长短、拖动排序、窗口缩放后仍能对齐；
// 首次挂载先量好再显示，避免从左上角滑进来那一下。
import { nextTick, onBeforeUnmount, onMounted, ref, watch, type Ref } from "vue";

/**
 * 给竖向导航挂一块跟随选中项的色块
 *
 * - `root`: 导航容器
 * - `selector`: 导航项选择器，如 `.tab-item`
 * - `activeIndex`: 当前选中项下标，返回 -1 时隐藏色块
 *
 * # 返回值
 *
 * - `thumbStyle`: 色块的内联位置与尺寸
 * - `thumbReady`: 首次测量是否完成
 * - `refresh`: 列表顺序 / 文案变化后手动重量
 */
export function useSlidingThumb(
  root: Ref<HTMLElement | null>,
  selector: string,
  activeIndex: () => number,
) {
  const thumbStyle = ref<Record<string, string>>({
    width: "0px",
    height: "0px",
    transform: "translate(0, 0)",
  });
  const thumbReady = ref(false);
  let resizeObserver: ResizeObserver | null = null;

  /** 按当前选中项重算色块位置 */
  function update() {
    const rootEl = root.value;
    if (!rootEl) return;
    const button = rootEl.querySelectorAll<HTMLElement>(selector)[activeIndex()];
    if (!button) {
      thumbReady.value = false;
      return;
    }
    thumbStyle.value = {
      width: `${button.offsetWidth}px`,
      height: `${button.offsetHeight}px`,
      transform: `translate(${button.offsetLeft}px, ${button.offsetTop}px)`,
    };
    if (!thumbReady.value) {
      // 先让 transition:none 的初始帧落进 DOM，下一帧再开启过渡
      requestAnimationFrame(() => {
        thumbReady.value = true;
      });
    }
  }

  /** 等 DOM 更新后再量 */
  function refresh() {
    void nextTick(update);
  }

  watch(activeIndex, refresh);

  onMounted(() => {
    refresh();
    if (typeof ResizeObserver !== "undefined" && root.value) {
      resizeObserver = new ResizeObserver(() => update());
      resizeObserver.observe(root.value);
    }
  });

  onBeforeUnmount(() => {
    resizeObserver?.disconnect();
    resizeObserver = null;
  });

  return { thumbStyle, thumbReady, refresh };
}
