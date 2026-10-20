<script setup lang="ts">
// 账户占用确认：启动时发现当前账户正被其它实例使用，问是否继续
//
// 常驻在 App 里，不随某个窗口卸载；勾选「继续检测」关闭时才写配置，
// 取消启动不改变检查开关。
import { ref, watch } from "vue";
import BaseModal from "./ui/BaseModal.vue";
import BaseButton from "./ui/BaseButton.vue";
import BaseSwitch from "./ui/BaseSwitch.vue";
import { t } from "../lib/i18n";
import { accountLockOpen, answerAccountLock } from "../lib/accountLock";

/** 是否继续检测账户占用；每次打开重置为开 */
const keep = ref(true);

watch(accountLockOpen, (open) => {
  if (open) keep.value = true;
});

/** 关闭弹窗并回答启动流程 */
function answer(ok: boolean) {
  answerAccountLock(ok, keep.value);
}
</script>

<template>
  <BaseModal v-if="accountLockOpen" :title="t('accountLock.title')" :closable="false" @close="answer(false)">
    <p class="modal-text">{{ t("accountLock.desc") }}</p>
    <label class="lock-keep">
      <BaseSwitch v-model="keep" />
      <span>{{ t("accountLock.keep") }}</span>
    </label>
    <div class="modal-actions">
      <BaseButton @click="answer(false)">{{ t("accountLock.cancel") }}</BaseButton>
      <BaseButton variant="primary" @click="answer(true)">{{ t("accountLock.continue") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
.lock-keep {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 12px 0;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
}
</style>
