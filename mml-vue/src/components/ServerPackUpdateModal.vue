<script setup lang="ts">
// 服务器包更新提醒：启动前查到发布地址上有新版本，问用户要不要先同步
//
// 常驻在 App 里；三个出口——取消启动、直接启动、更新并启动。
// 升级本身还是走实例设置里那套整合包任务（进度在标题栏），这里只负责"问"。
import BaseButton from "./ui/BaseButton.vue";
import BaseModal from "./ui/BaseModal.vue";
import { t } from "../lib/i18n";
import { answerServerPackUpdate, serverPackOpen, serverPackResult } from "../lib/serverPackPrompt";
</script>

<template>
  <BaseModal
    v-if="serverPackOpen && serverPackResult"
    :title="t('launch.serverPackTitle')"
    :closable="false"
    @close="answerServerPackUpdate(false, false)"
  >
    <p class="pack-version">{{ t("launch.serverPackVersion", { version: serverPackResult.newVersion }) }}</p>
    <p class="pack-local">{{ t("launch.serverPackLocal", { version: serverPackResult.fileId || "-" }) }}</p>
    <p class="pack-tip">{{ t("launch.serverPackTip") }}</p>
    <div class="modal-actions">
      <BaseButton @click="answerServerPackUpdate(false, false)">{{ t("resource.cancel") }}</BaseButton>
      <BaseButton @click="answerServerPackUpdate(true, false)">{{ t("launch.serverPackSkip") }}</BaseButton>
      <BaseButton variant="accent" @click="answerServerPackUpdate(true, true)">
        {{ t("launch.serverPackUpdate") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 新版本号是主角，本地版本作对照，说明放在下面 */
.pack-version {
  margin: 0 0 4px;
  font-size: 13px;
  color: var(--text);
}

.pack-local {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

.pack-tip {
  margin: 12px 0 0;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text-dim);
}
</style>
