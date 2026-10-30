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
  <div class="acc-empty" role="status">
    <div class="empty-mark">
      <svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" stroke-width="1.6"
        stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="8" r="4" />
        <path d="M4 21c0-4 3.6-6.5 8-6.5s8 2.5 8 6.5" />
      </svg>
    </div>
    <p class="empty-title">{{ title }}</p>
    <p v-if="hint" class="empty-hint">{{ hint }}</p>
    <BaseButton v-if="showAdd" variant="primary" class="empty-add" @click="emit('add')">
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
        stroke-linecap="round">
        <path d="M12 5v14M5 12h14" />
      </svg>
      {{ t("account.addTitle") }}
    </BaseButton>
  </div>
</template>

<style scoped>
.acc-empty {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 32px 24px;
  border: 1px dashed var(--accent-border);
  border-radius: 12px;
  background: var(--bg-side);
  color: var(--text-dim);
  text-align: center;
}

.empty-mark {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 58px;
  height: 58px;
  border-radius: 50%;
  background: var(--accent-soft);
  color: var(--accent);
}

.empty-title {
  margin: 0;
  color: var(--text);
  font-size: 15px;
  font-weight: 600;
}

.empty-hint {
  margin: 0;
  max-width: 320px;
  font-size: 12.5px;
  line-height: 1.6;
}

/* 空状态里唯一的动作：与上方两行文案拉开一点，别贴在一起 */
.empty-add {
  margin-top: 8px;
}
</style>
