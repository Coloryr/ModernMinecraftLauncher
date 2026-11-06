<script setup lang="ts">
// 「取消启动」正好撞上服务器包同步：问是跳过这次更新继续启动，还是结束这次启动
//
// 常驻在 App 里，与其它启动前询问同一套；关闭弹窗按「继续更新」处理，不打断同步。
import { computed } from "vue";
import { t } from "../lib/i18n";
import { stateText } from "../lib/launchState";
import BaseButton from "./ui/BaseButton.vue";
import BaseModal from "./ui/BaseModal.vue";
import { answerLaunchCancel, launchCancelOpen, launchCancelStage } from "../lib/launchCancelAsk";

/** 当前阶段的文案；拿不到阶段时留空，不占一行 */
const stageText = computed(() =>
  launchCancelStage.value ? stateText(launchCancelStage.value) : "",
);
</script>

<template>
  <BaseModal v-if="launchCancelOpen" :title="t('launch.cancelAskTitle')" :closable="false"
    @close="answerLaunchCancel('resume')">
    <p class="cancel-line">{{ t("launch.cancelAskText") }}</p>
    <p v-if="stageText" class="cancel-stage">{{ stageText }}</p>
    <div class="modal-actions">
      <BaseButton @click="answerLaunchCancel('resume')">{{ t("launch.cancelAskResume") }}</BaseButton>
      <BaseButton variant="danger" @click="answerLaunchCancel('stop')">{{ t("launch.cancelAskStop") }}</BaseButton>
      <BaseButton variant="accent" @click="answerLaunchCancel('skip')">{{ t("launch.cancelAskSkip") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 两行说明：做法一句话，当前阶段另起一行给次要色 */
.cancel-line,
.cancel-stage {
  margin: 0;
  line-height: 1.6;
}

.cancel-line {
  font-size: 12.5px;
  color: var(--text);
}

.cancel-stage {
  margin-top: 4px;
  font-size: 11.5px;
  color: var(--text-dim);
}
</style>
