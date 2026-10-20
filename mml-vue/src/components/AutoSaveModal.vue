<script setup lang="ts">
// 启动后自动进入存档的确认：实例配了启动后自动进入某个存档，启动前问一次
//
// 常驻在 App 里，不随某个窗口卸载。两个开关只决定"要不要再问"与"要不要保留这个实例的
// 配置"，这次进不进入由下面两个按钮决定；点关闭等同于这次不进入，启动照常继续。
import { computed, ref, watch } from "vue";
import BaseModal from "./ui/BaseModal.vue";
import BaseButton from "./ui/BaseButton.vue";
import BaseSwitch from "./ui/BaseSwitch.vue";
import { t } from "../lib/i18n";
import { stripFormatting } from "../lib/formatting";
import { answerAutoSave, autoSaveOpen, autoSavePick } from "../lib/autoSavePrompt";

/** 存档显示名；名字里可能带 `§` 格式码，弹窗是纯文本，先去掉 */
const saveName = computed(() => {
  const pick = autoSavePick.value;
  return pick ? stripFormatting(pick.levelName || pick.dir) : "";
});

/** 是否不再提醒；每次打开重置为关 */
const dontNotice = ref(false);
/** 是否取消该实例的自动进入配置；每次打开重置为关 */
const cancelAuto = ref(false);

watch(autoSaveOpen, (open) => {
  if (open) {
    dontNotice.value = false;
    cancelAuto.value = false;
  }
});

/** 关闭弹窗并回答启动流程 */
function answer(enter: boolean) {
  answerAutoSave({ enter, notice: !dontNotice.value, disable: cancelAuto.value });
}
</script>

<template>
  <BaseModal v-if="autoSaveOpen && autoSavePick" :title="t('autoSave.title')" :closable="false"
    @close="answer(false)">
    <p class="modal-text">{{ t("autoSave.desc", { name: saveName }) }}</p>
    <label class="auto-save-option">
      <BaseSwitch v-model="dontNotice" />
      <span>{{ t("autoSave.notice") }}</span>
    </label>
    <label class="auto-save-option">
      <BaseSwitch v-model="cancelAuto" />
      <span>{{ t("autoSave.disable") }}</span>
    </label>
    <div class="modal-actions">
      <BaseButton @click="answer(false)">{{ t("autoSave.skip") }}</BaseButton>
      <BaseButton variant="primary" @click="answer(true)">{{ t("autoSave.enter") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
.auto-save-option {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 12px 0;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
}
</style>
