<script setup lang="ts">
// 资源行的右键菜单：**单行动作与批量动作共用同一个组件**
//
// 所有分类都走它（模组、存档、数据包、资源包、光影、结构、截图、服务器）：
// 没有选中项时是单行菜单，动作只作用在右键的那一行；有选中项时是批量菜单，
// 动作作用在选中的那一批 —— 两者的区别只在调用方传进来的 `items` 与 `title`。
//
// 只负责画与定位，动作抛回调用方执行：确认弹窗、备注弹窗、目录打开都在各自的 Pane 里。
// 外观与分组菜单 ModGroupMenu 共用一套（`.row-ctx-menu*`），定位方式也一样。
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";

/** 一个菜单项 */
export interface RowMenuItem {
  /** 动作 id，含义由调用方定义 */
  id: string;
  /** 文案，调用方已 `t()` 好的成品 */
  label: string;
  /** 危险动作：悬停变红 */
  danger?: boolean;
  /** 置灰不可点，比如识别失败的模组改不了启用状态 */
  disabled?: boolean;
  /** 在这一项**之前**画一条分隔线，用来分组 */
  separated?: boolean;
}

const props = defineProps<{
  /** 点击处的横坐标，视口坐标 */
  x: number;
  /** 点击处的纵坐标，视口坐标 */
  y: number;
  /** 菜单项，按数组顺序渲染 */
  items: RowMenuItem[];
  /** 菜单标题；批量菜单用来写明动作作用在选中的几项上，单行菜单不传 */
  title?: string;
}>();

const emit = defineEmits<{
  /** 选了某一项，由调用方执行 */
  (e: "pick", id: string): void;
  /** 收起菜单 */
  (e: "close"): void;
}>();

const el = ref<HTMLElement | null>(null);
const pos = ref({ x: props.x, y: props.y });

onMounted(async () => {
  await nextTick();
  // 贴着点击处弹，但别越过窗口边缘
  const box = el.value?.getBoundingClientRect();
  if (box) {
    const pad = 8;
    pos.value = {
      x: Math.max(pad, Math.min(props.x, window.innerWidth - box.width - pad)),
      y: Math.max(pad, Math.min(props.y, window.innerHeight - box.height - pad)),
    };
  }
  // 捕获阶段监听：点菜单内部时 `el.contains` 会命中，不会误关
  window.addEventListener("pointerdown", onOutside, true);
  window.addEventListener("keydown", onKey, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", onOutside, true);
  window.removeEventListener("keydown", onKey, true);
});

function onOutside(e: PointerEvent) {
  if (!el.value?.contains(e.target as Node)) emit("close");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

/** 选中一项：先抛动作再关菜单 */
function pick(id: string) {
  emit("pick", id);
  emit("close");
}
</script>

<template>
  <div ref="el" class="row-ctx-menu" :style="{ left: `${pos.x}px`, top: `${pos.y}px` }" @pointerdown.stop
    @contextmenu.prevent>
    <span v-if="title" class="row-ctx-menu-title">{{ title }}</span>
    <template v-for="item in items" :key="item.id">
      <span v-if="item.separated" class="row-ctx-menu-sep" />
      <button class="row-ctx-menu-item" :class="{ danger: item.danger }" :disabled="item.disabled"
        @click="pick(item.id)">
        {{ item.label }}
      </button>
    </template>
  </div>
</template>
