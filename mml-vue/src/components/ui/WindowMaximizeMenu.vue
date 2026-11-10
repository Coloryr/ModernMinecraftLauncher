<script setup lang="ts">
// 最大化按钮的右键菜单：全屏 / 占满屏幕。
//
// 从 WindowControls 拆出来：按钮只负责开与关，定位、Esc、点外面收起、窗口尺寸变化
// 都在这里；菜单 Teleport 到 body，不占标题栏的布局。
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { t } from "../../lib/i18n";

const props = defineProps<{
  /** 锚点：最大化按钮本身，菜单贴着它右下角弹出 */
  anchor: HTMLElement | null;
  /** 当前是否全屏：决定「全屏」那项是否打勾 */
  fullscreen: boolean;
}>();

const emit = defineEmits<{
  /** 请求收起菜单 */
  (e: "close"): void;
  /** 切换全屏 */
  (e: "toggleFullscreen"): void;
  /** 占满屏幕（窗口挪到工作区左上角并拉到工作区大小） */
  (e: "fillScreen"): void;
}>();

const menuEl = ref<HTMLElement | null>(null);
/** 菜单左上角，单位 px；首帧先按锚点右下角摆，渲染后按真实尺寸收进窗口 */
const anchorRect = props.anchor?.getBoundingClientRect();
const pos = ref(anchorRect ? { x: anchorRect.right, y: anchorRect.bottom + 4 } : { x: 0, y: 0 });
/** 菜单与窗口边缘的最小距离，单位 px */
const MARGIN = 8;

/** 收起菜单；动作发回 WindowControls 执行 */
function close() {
  emit("close");
}

/** Esc 收起：菜单开着时不再往下传，免得顺手把窗口也关了 */
function onKeydown(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  e.preventDefault();
  e.stopPropagation();
  close();
}

onMounted(async () => {
  // 先按"右缘对齐按钮右缘"摆，量到真实宽高后再收进窗口
  const rect = props.anchor?.getBoundingClientRect();
  await nextTick();
  const menuRect = menuEl.value?.getBoundingClientRect();
  if (!rect || !menuRect) return;
  pos.value = {
    x: Math.max(
      MARGIN,
      Math.min(rect.right - menuRect.width, window.innerWidth - menuRect.width - MARGIN),
    ),
    y: Math.min(rect.bottom + 4, window.innerHeight - menuRect.height - MARGIN),
  };
  window.addEventListener("keydown", onKeydown, true);
  window.addEventListener("resize", close);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown, true);
  window.removeEventListener("resize", close);
});
</script>

<template>
  <Teleport to="body">
    <div class="wc-menu-layer" @pointerdown.self="close" @contextmenu.prevent>
      <div ref="menuEl" class="wc-menu" :style="{ left: pos.x + 'px', top: pos.y + 'px' }" role="menu">
        <button class="wc-menu-item" :class="{ 'is-active': fullscreen }" role="menuitem"
          @click="emit('toggleFullscreen')">
          <svg class="wc-menu-check" viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor"
            stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
            <path d="m2.5 6.2 2.4 2.4 4.6-5.2" />
          </svg>
          {{ t("titlebar.fullscreen") }}
        </button>
        <button class="wc-menu-item" role="menuitem" @click="emit('fillScreen')">
          <svg class="wc-menu-check" viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor"
            stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
            <path d="m2.5 6.2 2.4 2.4 4.6-5.2" />
          </svg>
          {{ t("titlebar.fillScreen") }}
        </button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
/* 整屏透明层：接住菜单外的按下与右键，点哪儿都先收起菜单 */
.wc-menu-layer {
  position: fixed;
  inset: 0;
  z-index: 40;
}

.wc-menu {
  position: fixed;
  min-width: 160px;
  padding: 6px;
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-lg);
  animation: wc-menu-in 0.18s ease;
}

@keyframes wc-menu-in {
  from {
    opacity: 0;
    transform: translateY(-4px) scale(0.98);
  }

  to {
    opacity: 1;
    transform: none;
  }
}

.wc-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  font-size: 12.5px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s;
}

.wc-menu-item:hover {
  background: var(--bg-hover);
}

/* 勾选位常驻占位：切换时文字不左右跳 */
.wc-menu-check {
  flex-shrink: 0;
  opacity: 0;
  color: var(--accent);
}

.wc-menu-item.is-active .wc-menu-check {
  opacity: 1;
}

.wc-menu-item.is-active {
  color: var(--accent);
}
</style>
