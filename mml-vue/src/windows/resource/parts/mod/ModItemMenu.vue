<script setup lang="ts">
// 模组行右键菜单：启用 / 禁用、备注、打开文件夹、删除
//
// 与分组右键菜单 ModGroupMenu **共用一套外观与定位**，选择器都是 `.mod-ctx-menu*`，
// 动作同样全部抛给 ModPane 执行 —— 那里才有确认弹窗、备注弹窗与目录打开。
//
// ⚠️ 右键原来是"把这一行加进 / 移出多选"，见 types.ts 的 `select`。改成菜单后，
// 多选改由**行左侧的勾选框**承担：悬停该行就出现，已经在多选状态时全部常显，
// 见 resource.css 的 `.mod-select` 那段。
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { t } from "../../../../lib/i18n";
import type { ModItemDto } from "../../../../lib/bindings";

/** 菜单项；ModPane 按这个分发动作 */
export type ItemMenuAction = "enable" | "disable" | "note" | "open-folder" | "remove";

const props = defineProps<{
  /** 点击处的横坐标，视口坐标 */
  x: number;
  /** 点击处的纵坐标，视口坐标 */
  y: number;
  /** 这一行的模组，用来决定显示"启用"还是"禁用" */
  item: ModItemDto;
}>();

const emit = defineEmits<{
  /** 选了一个动作，由 ModPane 执行 */
  (e: "action", kind: ItemMenuAction): void;
  /** 收起菜单 */
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

/** 选中一项：先抛动作再关菜单 */
function pick(kind: ItemMenuAction) {
  emit("action", kind);
  emit("close");
}
</script>

<template>
  <div ref="el" class="mod-ctx-menu" :style="{ left: `${pos.x}px`, top: `${pos.y}px` }" @pointerdown.stop
    @contextmenu.prevent>
    <!-- 分组口径：备注 / 打开文件夹 一组 ─ 启用或禁用**二选一显示一个** 一组 ─ 删除 一组 -->
    <button class="mod-ctx-menu-item" @click="pick('note')">
      {{ t("resource.modNote") }}
    </button>
    <button class="mod-ctx-menu-item" @click="pick('open-folder')">
      {{ t("resource.openFolder") }}
    </button>

    <span class="mod-ctx-menu-sep" />

    <!-- 启用 / 禁用二选一：已禁用显示"启用"，已启用显示"禁用"。
         识别失败的模组改不了启用状态（勾选框也是禁用的），这一项置灰 -->
    <button v-if="item.disable" class="mod-ctx-menu-item" :disabled="item.fail" @click="pick('enable')">
      {{ t("resource.enable") }}
    </button>
    <button v-else class="mod-ctx-menu-item" :disabled="item.fail" @click="pick('disable')">
      {{ t("resource.disable") }}
    </button>

    <span class="mod-ctx-menu-sep" />

    <button class="mod-ctx-menu-item danger" @click="pick('remove')">
      {{ t("resource.delete") }}
    </button>
  </div>
</template>
