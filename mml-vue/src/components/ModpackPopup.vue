<script setup lang="ts">
// 整合包安装进度弹窗：右下角 chip 点开后浮在当前页面之上
//
// 内容直接复用 ModpackInstallBar，与"下载整合包"窗口里那条、以及多窗口模式下主窗口那条
// 完全是同一份渲染，这里只负责套一层弹窗壳。
import BaseModal from "./ui/BaseModal.vue";
import ModpackInstallBar from "./ModpackInstallBar.vue";
import { t } from "../lib/i18n";
import type { ModPackStatusDto } from "../lib/bindings";

defineProps<{
  /** 整合包安装任务状态，来自 App.vue 那份全局订阅 */
  status: ModPackStatusDto;
}>();

const emit = defineEmits<{
  /** 关闭弹窗；安装任务不受影响 */
  (e: "close"): void;
}>();
</script>

<template>
  <BaseModal :title="t('modpack.bar.title')" :width="560" fixed-height="420px" below-titlebar :overlay-close="false"
    @close="emit('close')">
    <!-- collapsible=false：弹窗是点开来看详情的，任务直接铺开，不再折一层。
         高度锁死（fixed-height）而不是随任务数长高：装多个整合包时高度一直跳很难看，
         任务多了让内容区自己滚，由 fill-height 把那截高度交给任务列表 -->
    <ModpackInstallBar :status="status" :collapsible="false" fill-height />
  </BaseModal>
</template>
