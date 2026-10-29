<script setup lang="ts">
// 资源下载进度弹窗：标题栏指示器点开后浮在当前页面之上
//
// 内容直接复用 ResourceDownloadBar，这里只套一层弹窗壳。关掉弹窗只是收起，下载照跑。
import BaseModal from "./ui/BaseModal.vue";
import ResourceDownloadBar from "./ResourceDownloadBar.vue";
import { t } from "../lib/i18n";
import type { ResourceStatusDto } from "../lib/bindings";

defineProps<{
  /** 资源下载任务状态，来自 App.vue 那份全局订阅 */
  status: ResourceStatusDto;
}>();

const emit = defineEmits<{
  /** 关闭弹窗；下载任务不受影响 */
  (e: "close"): void;
}>();
</script>

<template>
  <BaseModal :title="t('addResource.bar.title')" :width="560" fixed-height="420px" below-titlebar
    @close="emit('close')">
    <!-- 高度锁死而不是随任务数长高：任务增删时高度一直跳很难看，任务多了让内容区自己滚 -->
    <ResourceDownloadBar :status="status" fill-height />
  </BaseModal>
</template>
