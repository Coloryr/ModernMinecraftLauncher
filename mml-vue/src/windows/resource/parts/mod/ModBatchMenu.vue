<script setup lang="ts">
// 多选状态下的右键菜单：**动作作用在选中的那一批模组上**（用户口径）
//
// 与单行菜单（ModItemMenu）分开写：单行的动作是"这一行"的，批量的是"选中的一批"，
// 混在一个组件里会让 props 变成一堆可选字段。外观与定位共用 `.mod-ctx-menu*`。
//
// 分组排布，与单行菜单一致：启用 / 禁用按选中项状态给、可能两项都有 ─ 移到分组 / 删除。
// 单行菜单里的"备注""打开文件夹"对一批模组没有意义，所以不出现。
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { t } from "../../../../lib/i18n";

/** 批量菜单项 */
export type BatchMenuAction = "enable" | "disable" | "move" | "remove";

const props = defineProps<{
  /** 点击处（视口坐标） */
  x: number;
  y: number;
  /** 选中了多少项（标题里显示） */
  count: number;
  /** 选中项里**还有禁用的** → 给"启用"（识别失败的模组改不了状态，不计入） */
  canEnable: boolean;
  /** 选中项里**还有启用的** → 给"禁用" */
  canDisable: boolean;
}>();

const emit = defineEmits<{
  (e: "action", kind: BatchMenuAction): void;
  (e: "close"): void;
}>();

const el = ref<HTMLElement | null>(null);
const pos = ref({ x: props.x, y: props.y });

onMounted(async () => {
  await nextTick();
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

function pick(kind: BatchMenuAction) {
  emit("action", kind);
  emit("close");
}
</script>

<template>
  <div ref="el" class="mod-ctx-menu" :style="{ left: `${pos.x}px`, top: `${pos.y}px` }" @pointerdown.stop
    @contextmenu.prevent>
    <!-- 标题：说明这些动作作用在"选中的几项"上，别让人以为只动了右键那一行 -->
    <span class="mod-ctx-menu-title">{{ t("resource.groupCountItems", { n: count }) }}</span>

    <button v-if="canEnable" class="mod-ctx-menu-item" @click="pick('enable')">
      {{ t("resource.enable") }}
    </button>
    <button v-if="canDisable" class="mod-ctx-menu-item" @click="pick('disable')">
      {{ t("resource.disable") }}
    </button>

    <span class="mod-ctx-menu-sep" />

    <button class="mod-ctx-menu-item" @click="pick('move')">
      {{ t("resource.batchMoveTo") }}
    </button>
    <button class="mod-ctx-menu-item danger" @click="pick('remove')">
      {{ t("resource.delete") }}
    </button>
  </div>
</template>
