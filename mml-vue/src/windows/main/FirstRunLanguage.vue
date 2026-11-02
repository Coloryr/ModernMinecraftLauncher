<script setup lang="ts">
// 首启向导第一步：界面语言与窗口模式
//
// 排在旧数据询问之前 —— 先让用户看懂界面、定好窗口形态，再决定要不要搬旧数据。
// 语言选中即生效（弹窗自己当场换文案）；窗口模式只改内存，两项都**先不落盘**：
// 走完最后一步再一起写进配置，这样中途关掉启动器，向导下次还会从头开始。
import { computed } from "vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import { locale, setLocale, t } from "../../lib/i18n";
import { multiWindow, restoreMultiWindow } from "../windowManager";

const emit = defineEmits<{ (e: "close"): void }>();

/** 语言选项：名字用各自的语言显示，不随当前语言翻译 */
const options = computed(() => [
  { value: "zh_cn", label: t("winSettings.langZhCn") },
  { value: "en_us", label: t("winSettings.langEnUs") },
]);

/** 窗口模式选项 */
const modeOptions = computed(() => [
  { value: "multi", label: t("winSettings.multi") },
  { value: "single", label: t("winSettings.single") },
]);

/** 当前窗口模式；true 为多窗口 */
const mode = computed(() => (multiWindow.value ? "multi" : "single"));

/**
 * 换界面语言：只改界面，配置留到最后一步落盘
 *
 * - `v`: `zh_cn` / `en_us`
 */
function pick(v: string) {
  setLocale(v === "en_us" ? "en_us" : "zh_cn", false);
}

/**
 * 换窗口模式：只改内存，同样留到最后一步落盘
 *
 * 此刻只有主窗口，与设置页的"切换模式"不同，这里不必重载界面。
 *
 * - `v`: `multi` / `single`
 */
function pickMode(v: string) {
  restoreMultiWindow(v !== "single");
}
</script>

<template>
  <BaseModal :title="t('firstRunLang.title')" :width="440" :closable="false" @close="emit('close')">
    <div class="first-run-lang">
      <div class="run-field">
        <span class="field-label">{{ t("winSettings.language") }}</span>
        <SegmentedTabs class="run-tabs" :model-value="locale" :options="options" @update:model-value="pick" />
      </div>

      <div class="run-field">
        <span class="field-label">{{ t("winSettings.windowMode") }}</span>
        <SegmentedTabs class="run-tabs" :model-value="mode" :options="modeOptions"
          @update:model-value="pickMode" />
      </div>

      <p class="run-hint">{{ t("firstRunLang.hint") }}</p>
    </div>
    <div class="modal-actions">
      <BaseButton variant="accent" @click="emit('close')">{{ t("firstRunLang.next") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
.first-run-lang {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.run-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 分段控件默认只包住内容，这里拉满整行、选项均分 */
.run-field :deep(.seg-tabs) {
  display: flex;
  width: 100%;
}

.run-field :deep(.seg-btn) {
  flex: 1;
  justify-content: center;
}

.run-hint {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-dim);
}
</style>