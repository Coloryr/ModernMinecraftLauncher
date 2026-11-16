<script setup lang="ts">
// 账户空状态：占满视图内容区，避免只剩顶部一行提示
import BaseButton from "../../components/ui/BaseButton.vue";
import { t } from "../../lib/i18n";

withDefaults(
  defineProps<{
    /** 主文案 */
    title: string;
    /** 补充说明；筛选无结果时不传 */
    hint?: string;
    /** 是否给「添加账户」按钮：筛选无结果时不给，加一个也找不到原本要的那条 */
    showAdd?: boolean;
  }>(),
  { showAdd: false },
);

const emit = defineEmits<{
  /** 点「添加账户」：由所属窗口打开添加弹窗 */
  (e: "add"): void;
}>();
</script>

<template>
  <div class="acc-empty flex min-h-0 flex-1 flex-col items-center justify-center gap-2 rounded-panel border border-dashed border-accent-border bg-side px-6 py-8 text-center text-dim" role="status">
    <div class="empty-mark flex h-[58px] w-[58px] items-center justify-center rounded-full bg-accent-soft text-accent">
      <svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" stroke-width="1.6"
        stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="8" r="4" />
        <path d="M4 21c0-4 3.6-6.5 8-6.5s8 2.5 8 6.5" />
      </svg>
    </div>
    <p class="empty-title m-0 text-heading font-semibold text-fg">{{ title }}</p>
    <p v-if="hint" class="empty-hint m-0 max-w-[320px] text-body leading-[1.6]">{{ hint }}</p>
    <BaseButton v-if="showAdd" variant="primary" class="empty-add mt-2" @click="emit('add')">
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
        stroke-linecap="round">
        <path d="M12 5v14M5 12h14" />
      </svg>
      {{ t("account.addTitle") }}
    </BaseButton>
  </div>
</template>
