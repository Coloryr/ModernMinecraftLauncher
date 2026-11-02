<script setup lang="ts">
// 首启向导第一步：只问界面语言
//
// 排在旧数据询问之前 —— 先让用户看懂界面，再决定要不要搬旧数据。
// 选中即生效（弹窗自己当场换语言），但**先不落盘**：走完最后一步再一起写进配置，
// 这样中途关掉启动器，向导下次还会从语言这一步开始。
import { computed } from "vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import { locale, setLocale, t } from "../../lib/i18n";

const emit = defineEmits<{ (e: "close"): void }>();

/** 语言选项：名字用各自的语言显示，不随当前语言翻译 */
const options = computed(() => [
  { value: "zh_cn", label: t("winSettings.langZhCn") },
  { value: "en_us", label: t("winSettings.langEnUs") },
]);

/**
 * 换界面语言：只改界面，配置留到最后一步落盘
 *
 * - `v`: `zh_cn` / `en_us`
 */
function pick(v: string) {
  setLocale(v === "en_us" ? "en_us" : "zh_cn", false);
}
</script>

<template>
  <BaseModal :title="t('firstRunLang.title')" :width="420" :closable="false" @close="emit('close')">
    <p class="lang-hint">{{ t("firstRunLang.hint") }}</p>
    <SegmentedTabs :model-value="locale" :options="options" @update:model-value="pick" />
    <div class="modal-actions">
      <BaseButton variant="accent" @click="emit('close')">{{ t("firstRunLang.next") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
.lang-hint {
  margin: 0 0 10px;
  font-size: 11.5px;
  color: var(--text-dim);
}
</style>
