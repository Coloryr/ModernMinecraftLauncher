<script setup lang="ts">
// 启动前 / 启动后命令的确认：实例配了命令时，启动前问一次要不要执行
//
// 常驻在 App 里，不随某个窗口卸载。「不再提醒」只在选「执行」时由调用方落盘，
// 点关闭等同于这次跳过，启动照常继续。
import { computed, ref, watch } from "vue";
import BaseModal from "./ui/BaseModal.vue";
import BaseButton from "./ui/BaseButton.vue";
import BaseSwitch from "./ui/BaseSwitch.vue";
import { t } from "../lib/i18n";
import {
  answerLaunchCommand,
  launchCommandInfo,
  launchCommandOpen,
} from "../lib/launchCommandPrompt";

/** 是否不再提醒；每次打开重置为关 */
const dontNotice = ref(false);

watch(launchCommandOpen, (open) => {
  if (open) dontNotice.value = false;
});

/** 这次真的会执行的命令，按启动前 / 启动后分组 */
const commands = computed(() => {
  const info = launchCommandInfo.value;
  const list: { label: string; text: string }[] = [];
  if (info?.pre) list.push({ label: t("launchCommand.pre"), text: info.pre });
  if (info?.post) list.push({ label: t("launchCommand.post"), text: info.post });
  return list;
});

/** 关闭弹窗并回答启动流程 */
function answer(action: "run" | "skip" | "cancel") {
  answerLaunchCommand({ action, notice: !dontNotice.value });
}
</script>

<template>
  <BaseModal v-if="launchCommandOpen" :title="t('launchCommand.title')" :closable="false"
    @close="answer('skip')">
    <p class="modal-text">{{ t("launchCommand.desc") }}</p>
    <div v-for="item in commands" :key="item.label" class="launch-command-item">
      <span class="launch-command-label">{{ item.label }}</span>
      <pre class="launch-command-text">{{ item.text }}</pre>
    </div>
    <label class="launch-command-option">
      <BaseSwitch v-model="dontNotice" />
      <span>{{ t("launchCommand.notice") }}</span>
    </label>
    <div class="modal-actions">
      <BaseButton @click="answer('cancel')">{{ t("launchCommand.cancel") }}</BaseButton>
      <BaseButton @click="answer('skip')">{{ t("launchCommand.skip") }}</BaseButton>
      <BaseButton variant="primary" @click="answer('run')">{{ t("launchCommand.run") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 命令按原样展示：等宽字、保留换行，长了在框内滚动，不把弹窗撑宽 */
.launch-command-item {
  margin-top: 10px;
}

.launch-command-label {
  display: block;
  margin-bottom: 4px;
  font-size: 11.5px;
  color: var(--text-dim);
}

.launch-command-text {
  max-height: 96px;
  margin: 0;
  padding: 8px 10px;
  overflow: auto;
  border-radius: 8px;
  background: var(--bg-raised, var(--bg-modal-raised));
  color: var(--text);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11.5px;
  white-space: pre-wrap;
  word-break: break-all;
}

.launch-command-option {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 12px 0;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
}
</style>
