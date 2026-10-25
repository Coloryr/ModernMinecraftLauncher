<script setup lang="ts">
// 整合包更新的确认窗：把平台上查到的新版本摆出来，用户点了「立即更新」才开更新
//
// 入口在**实例设置**面板（InstanceMetaPanel），不在资源管理窗口 —— 升级动的是整个实例。
//
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

/** 这次更新的是在线服务器包：文案换成"同步"，也没有发布时间 */
const serverPack = computed(() => props.result.source === "serverpack");
</script>

<template>
  <BaseModal
    :title="t(serverPack ? 'resource.serverPackUpdateTitle' : 'resource.modpackUpdateTitle')"
    :closable="false"
    @close="emit('close')"
  >
    <p class="pack-version">
      {{ t("resource.modpackNewVersion", { version: props.result.newVersion || props.result.newFile }) }}
    </p>
    <p v-if="props.result.date" class="pack-date">{{ t("resource.modpackNewDate", { date: props.result.date }) }}</p>
    <p class="pack-tip">
      {{ t(serverPack ? "resource.serverPackUpdateTip" : "resource.modpackUpdateTip") }}
    </p>
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
/* 版本号是主角，比下面的说明大一号 */
.pack-version {
  margin: 0 0 4px;
  font-size: 13.5px;
  color: var(--text);
}

.pack-date {
  margin: 0;
  font-size: 12px;
  color: var(--text-dim);
}

/* 更新范围的说明：与确认按钮之间留一段 */
.pack-tip {
  margin: 12px 0 0;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text-dim);
}
</style>
