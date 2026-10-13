<script setup lang="ts">
// 统一弹窗：标题 + 内容插槽 + 关闭
//
// 只在**所属页面处于前台**时渲染，见 lib/pageActive.ts：单窗口模式下 App.vue 把窗口组件
// KeepAlive 缓存，切走只是停用，页面 DOM 被挪走，可本弹窗是 `Teleport to="body"` 的 ——
// 不挡住的话它会留在别的页面上照常显示、还能点，而点下去的执行上下文是原页面。
// "是否继续添加"的"否"就这么把启动器关掉过。状态在父组件里，回到那一页它自然还在。
import GlyphIcon from "./GlyphIcon.vue";
import { usePageActive } from "../../lib/pageActive";

const props = withDefaults(
  defineProps<{
    /** 标题；空串则不画标题栏，右上角关闭按钮也跟着没有 */
    title?: string;
    /** 面板宽度，单位 px */
    width?: number;
    /** 锁死面板高度，CSS 长度如 "520px"。给定时面板竖直方向不再随内容变化：
     *  内容区吃掉剩余高度、内部自己滚，下载弹窗的线程表一直在增删行，不锁会一直抽动；
     *  不传则按内容自适应 */
    fixedHeight?: string;
    /** 是否画右上角关闭按钮 */
    closable?: boolean;
    /** 遮罩从自绘标题栏下方开始，让标题栏（可拖动 / 窗口按钮）保持可操作。
     *  默认开启：所有弹窗都不应挡住标题栏 */
    belowTitlebar?: boolean;
    /** 点击遮罩空白处是否关闭；输入类弹窗建议关掉，避免误触丢内容 */
    overlayClose?: boolean;
  }>(),
  {
    title: "",
    width: 420,
    fixedHeight: "",
    closable: true,
    belowTitlebar: true,
    overlayClose: true,
  },
);

const emit = defineEmits<{
  /** 请求关闭；`overlayClose` 为假时点遮罩不发这条 */
  (e: "close"): void;
}>();

/** 所属页面是否在前台；隐藏时连遮罩一起收掉，别盖到别的页面上 */
const active = usePageActive();
</script>

<template>
  <Teleport to="body">
    <div v-if="active" class="modal-mask" :class="{ 'below-titlebar': props.belowTitlebar }"
      @click.self="props.overlayClose && emit('close')">
      <div class="modal" :class="{ 'fixed-h': !!props.fixedHeight }"
        :style="{ width: width + 'px', height: props.fixedHeight || undefined }">
        <div v-if="title" class="modal-head">
          <h3>{{ title }}</h3>
          <button v-if="closable" class="modal-x" @click="emit('close')">
            <GlyphIcon name="close" :size="14" :weight="2.2" />
          </button>
        </div>
        <div class="modal-body">
          <slot />
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-mask {
  position: fixed;
  inset: 0;
  background: var(--overlay);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

/* 避开自绘标题栏：遮罩从标题栏下沿开始，标题栏仍可拖动 / 点窗口按钮 */
.modal-mask.below-titlebar {
  top: var(--titlebar-h);
}

.modal {
  max-width: 92vw;
  max-height: 85vh;
  overflow-y: auto;
  /* 滚动条槽位**两侧都留**：只留右侧时，内容整体左移一截，`padding` 虽然左右对称，
     看着却是右边宽（见 styles/scrollbar.css 的 9px）。槽位常驻，所以滚动条出现 /
     消失时内容不会横向跳。
     左右内边距因此各减 9px：15 + 9 = 设计上的 24px，内容区仍在原来的位置，
     只是把原先白占的右侧槽位补回来 */
  scrollbar-gutter: stable both-edges;
  /* 专用底色而不是 --bg-card：弹窗面板比普通表面实一些，见 themes.css 的 --bg-modal */
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 22px 15px;
  box-shadow: var(--shadow-lg);
}

/* 锁死高度：fixedHeight 传了值。面板大小不再随内容变，内容区吃掉剩余高度、内部自己滚。
   只在这一种形态下开 flex —— 平时保持块级布局，免得相邻外边距不再合并、
   把各弹窗的间距改掉。max-height 仍然生效：窗口太矮时按 85vh 封顶，同样与内容无关 */
.modal.fixed-h {
  display: flex;
  flex-direction: column;
}

.modal.fixed-h .modal-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.modal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.modal-head h3 {
  font-size: 17px;
}

.modal-x {
  border: none;
  background: transparent;
  color: var(--text-dim);
  font-size: 14px;
  cursor: pointer;
  padding: 4px 6px;
  border-radius: 6px;
  line-height: 1;
}

.modal-x:hover {
  background: var(--bg-hover);
  color: var(--text);
}
</style>
