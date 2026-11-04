<script setup lang="ts">
// Java 安装进度弹窗：入口是标题栏上的 JavaTitleIndicator，关掉只收起、安装照跑
import BaseModal from "./ui/BaseModal.vue";
import JavaInstallBar from "./JavaInstallBar.vue";
import { t } from "../lib/i18n";
import type { JavaInstallTaskDto } from "../lib/bindings";

defineProps<{
  /** 安装任务，来自 App.vue 那份全局订阅 */
  tasks: JavaInstallTaskDto[];
}>();

const emit = defineEmits<{
  /** 关闭弹窗；安装不受影响 */
  (e: "close"): void;
}>();
</script>

<template>
  <BaseModal :title="t('winJavaDownload.tasks')" :width="560" fixed-height="420px" below-titlebar
    @close="emit('close')">
    <!-- 高度锁死，任务多了让列表自己滚，别让弹窗一直长高 -->
    <JavaInstallBar :tasks="tasks" />
  </BaseModal>
</template>
