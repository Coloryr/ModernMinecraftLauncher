<script setup lang="ts">
// 添加实例 · 模式四：在线实例（下载地址）
import { nextTick, ref, watch } from "vue";
import { t } from "../../../lib/i18n";

const props = defineProps<{
  /** 在线实例的下载地址 */
  url: string;
  /** 校验失败：下载地址必填（高亮 + 自动聚焦） */
  invalid?: boolean;
}>();

const emit = defineEmits<{
  /** 下载地址变化 */
  (e: "update:url", v: string): void;
}>();

const urlInput = ref<HTMLInputElement | null>(null);

// 必填校验失败：聚焦地址框（高亮见模板的 is-invalid）
watch(
  () => props.invalid,
  async (invalid) => {
    if (!invalid) return;
    await nextTick();
    urlInput.value?.focus();
  },
);
</script>

<template>
  <label class="field-label">{{ t("add.url") }} <span class="req">*</span></label>
  <input ref="urlInput" :value="url" class="field-input" :class="{ 'is-invalid': invalid }"
    :placeholder="t('add.urlPlaceholder')" spellcheck="false" autocomplete="off"
    @input="emit('update:url', ($event.target as HTMLInputElement).value)" />
  <p class="field-hint">{{ t("add.urlHint") }}</p>
</template>

<style scoped>
/* 卡片里第一条标签顶格，不留多余的 12px */
.field-label:first-child {
  margin-top: 0;
}
</style>
