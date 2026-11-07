<script setup lang="ts">
// 服务器包更新提醒：启动前查到发布地址上有新版本，问用户要不要先同步
//
// 常驻在 App 里；三个出口——取消启动、直接启动、更新并启动。
// 升级本身还是走实例设置里那套整合包任务（进度在标题栏），这里只负责"问"。
import { computed } from "vue";
import BaseButton from "./ui/BaseButton.vue";
import BaseModal from "./ui/BaseModal.vue";
import { t } from "../lib/i18n";
import { answerServerPackUpdate, serverPackOpen, serverPackResult } from "../lib/serverPackPrompt";

/** 清单差异有没有内容：三个计数全 0 时不显示那一行 */
const changed = computed(() => {
  const result = serverPackResult.value;
  return !!result && result.addedFiles + result.removedFiles + result.changedFiles > 0;
});
</script>

<template>
  <BaseModal v-if="serverPackOpen && serverPackResult" :title="t('launch.serverPackTitle')" :closable="false"
    @close="answerServerPackUpdate(false, false)">
    <p class="pack-line">{{ t("update.instance", { name: serverPackResult.instanceName }) }}</p>
    <!-- 版本号没变、只换了内容时别写 "1.3 → 1.3"，那看着像没更新 -->
    <p class="pack-version">
      {{
        serverPackResult.fileId === serverPackResult.newVersion
          ? t("update.serverPackVersionSame", { version: serverPackResult.fileId || "-" })
          : t("update.serverPackVersion", {
            from: serverPackResult.fileId || "-",
            to: serverPackResult.newVersion || "-",
          })
      }}
    </p>
    <p v-if="changed" class="pack-changes">
      {{
        t("update.changes", {
          added: serverPackResult.addedFiles,
          removed: serverPackResult.removedFiles,
          changed: serverPackResult.changedFiles,
        })
      }}
    </p>
    <!-- 有可选模组要变时，下一步还要挑一次，先说一声 -->
    <p v-if="serverPackResult.mods.length" class="pack-changes">
      {{ t("update.serverPackModsPending", { n: serverPackResult.mods.length }) }}
    </p>
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
/* 三行信息：实例 / 版本变化 / 清单差异，逐行降一级 */
.pack-line,
.pack-version,
.pack-changes {
  margin: 0;
  line-height: 1.6;
}

.pack-line {
  font-size: 12.5px;
  color: var(--text);
}

.pack-version {
  font-size: 13px;
  color: var(--text);
}

.pack-changes {
  margin-top: 4px;
  font-size: 11.5px;
  color: var(--text-dim);
}
</style>
