<script setup lang="ts">
// 整合包 / 服务器包更新的确认窗：把更新信息摆出来，用户点了确认才开更新
//
// 入口在**实例设置**面板（InstanceMetaPanel），不在资源管理窗口 —— 升级动的是整个实例。
// 更新本身在后端跑（挂在共享的整合包任务表上），进度与取消在标题栏 / 整合包任务里，
// 这里只负责"确认"这一步。
import { computed } from "vue";
import { t } from "../lib/i18n";
import BaseButton from "./ui/BaseButton.vue";
import BaseModal from "./ui/BaseModal.vue";
import type { ModPackUpdateDto } from "../lib/bindings";

const props = defineProps<{
  /** 检查结果，调用方只在 `hasUpdate` 为真时打开 */
  result: ModPackUpdateDto;
}>();

const emit = defineEmits<{
  /** 关掉，不更新 */
  (e: "close"): void;
  /** 确认更新，载荷是要升到的那份文件编号 */
  (e: "update", fileId: string): void;
}>();

/** 这次更新的是在线服务器包：版本按"本地包版本 → 发布地址上的包版本"显示 */
const serverPack = computed(() => props.result.source === "serverpack");

/** 服务器包的清单差异有没有内容：三个计数全 0 时不显示那一行 */
const changed = computed(
  () => props.result.addedFiles + props.result.removedFiles + props.result.changedFiles > 0,
);
</script>

<template>
  <BaseModal :title="t(serverPack ? 'resource.serverPackUpdateTitle' : 'resource.modpackUpdateTitle')" :closable="false"
    @close="emit('close')">
    <p class="pack-line">{{ t("update.instance", { name: props.result.instanceName }) }}</p>
    <template v-if="serverPack">
      <p class="pack-version">
        {{
          t("update.serverPackVersion", {
            from: props.result.fileId || "-",
            to: props.result.newVersion || "-",
          })
        }}
      </p>
      <p v-if="changed" class="pack-changes">
        {{
          t("update.changes", {
            added: props.result.addedFiles,
            removed: props.result.removedFiles,
            changed: props.result.changedFiles,
          })
        }}
      </p>
    </template>
    <template v-else>
      <p class="pack-version">
        {{ t("resource.modpackNewVersion", { version: props.result.newVersion || props.result.newFile }) }}
      </p>
      <p v-if="props.result.date" class="pack-date">
        {{ t("resource.modpackNewDate", { date: props.result.date }) }}
      </p>
    </template>
    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("resource.cancel") }}</BaseButton>
      <!-- 与模组更新弹窗、依赖图弹窗的确认按钮同一档（accent），别在同一个窗口里两套 -->
      <BaseButton variant="accent" @click="emit('update', props.result.newFileId)">
        {{ t(serverPack ? "resource.serverPackUpdate" : "resource.modpackUpdate") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 三行信息：实例 / 版本 / 差异（平台包第二行是发布时间），从主到次 */
.pack-line,
.pack-version,
.pack-date,
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

.pack-date,
.pack-changes {
  font-size: 11.5px;
  color: var(--text-dim);
}

.pack-changes {
  margin-top: 4px;
}
</style>
