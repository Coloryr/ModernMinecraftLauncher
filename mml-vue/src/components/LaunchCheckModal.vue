<script setup lang="ts">
// 启动前检查：加载器没启用 / 模组多而内存低，问一次是否继续
import { computed, ref, watch } from "vue";
import BaseModal from "./ui/BaseModal.vue";
import BaseButton from "./ui/BaseButton.vue";
import BaseSwitch from "./ui/BaseSwitch.vue";
import { t } from "../lib/i18n";
import {
  launchCheckOpen,
  launchCheckKind,
  launchCheckInfo,
  answerLaunchCheck,
} from "../lib/launchCheck";

/** 是否顺手关掉这一项检查；每次打开重置为否 */
const disable = ref(false);

watch(launchCheckOpen, (open) => {
  if (open) disable.value = false;
});

/** 本次是加载器检查还是内存检查 */
const isLoader = computed(() => launchCheckKind.value === "loader");

/** 关闭弹窗并回答启动流程 */
function answer(ok: boolean) {
  answerLaunchCheck(ok, disable.value);
}
</script>

<template>
  <BaseModal v-if="launchCheckOpen" :title="t(isLoader ? 'launchCheck.titleLoader' : 'launchCheck.titleMemory')"
    :closable="false" @close="answer(false)">
    <p class="modal-text">
      {{
        t(isLoader ? "launchCheck.descLoader" : "launchCheck.descMemory", {
          mods: launchCheckInfo?.mods ?? 0,
          maxMemory: launchCheckInfo?.maxMemory ?? 0,
        })
      }}
    </p>
    <p class="launch-check-hint">
      {{ t(isLoader ? "launchCheck.hintLoader" : "launchCheck.hintMemory") }}
    </p>
    <label class="launch-check-keep">
      <BaseSwitch v-model="disable" />
      <span>{{ t(isLoader ? "launchCheck.disableLoader" : "launchCheck.disableMemory") }}</span>
    </label>
    <div class="modal-actions">
      <BaseButton @click="answer(false)">{{ t("launchCheck.cancel") }}</BaseButton>
      <BaseButton variant="primary" @click="answer(true)">{{ t("launchCheck.continue") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 补充说明比正文低一档：正文讲数量，这行只讲去哪儿改 */
.launch-check-hint {
  margin-top: 8px;
  font-size: 11.5px;
  color: var(--text-dim);
}

.launch-check-keep {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 12px 0;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
}
</style>
