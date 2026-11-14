<script setup lang="ts">
// 游戏内手柄需要 ColorASM 的确认：启动前问一次，不开启也能继续启动
import BaseModal from "./ui/BaseModal.vue";
import BaseButton from "./ui/BaseButton.vue";
import { t } from "../lib/i18n";
import { gamepadAsmOpen, answerGamepadAsm } from "../lib/gamepadAsmPrompt";

/**
 * 关闭弹窗并回答启动流程
 *
 * - `ok`: 是否继续启动
 * - `enable`: 是否为实例打开 ColorASM
 */
function answer(ok: boolean, enable: boolean) {
  answerGamepadAsm({ ok, enable });
}
</script>

<template>
  <BaseModal v-if="gamepadAsmOpen" :title="t('gamepadAsm.title')" :closable="false" @close="answer(false, false)">
    <p class="modal-text">{{ t("gamepadAsm.desc") }}</p>
    <div class="modal-actions">
      <BaseButton @click="answer(false, false)">{{ t("gamepadAsm.cancel") }}</BaseButton>
      <BaseButton @click="answer(true, false)">{{ t("gamepadAsm.skip") }}</BaseButton>
      <BaseButton variant="primary" @click="answer(true, true)">{{ t("gamepadAsm.enable") }}</BaseButton>
    </div>
  </BaseModal>
</template>
