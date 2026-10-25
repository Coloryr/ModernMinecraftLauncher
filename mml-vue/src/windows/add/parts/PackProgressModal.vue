<script setup lang="ts">
// 添加实例 · 整合包安装进度弹窗，压缩包 / 网址 / 在线整合包安装共用
// 两级进度：整体是已处理条目 / 总条目，加上当前子任务，如某个文件的解压或下载
import { computed } from "vue";
import { t } from "../../../lib/i18n";
import BaseModal from "../../../components/ui/BaseModal.vue";
import type { PackProgressDto } from "../../../lib/bindings";

const props = withDefaults(
  defineProps<{
    /** 安装进度快照，由 add-pack-progress 事件推送 */
    progress: PackProgressDto;
    /** 弹窗标题：安装整合包与下载在线实例两种场景不同；不传时按安装整合包 */
    title?: string;
  }>(),
  { title: "" },
);

/** 实际显示的标题：调用方没给就退回"安装整合包" */
const shownTitle = computed(() => props.title || t("add.installing"));

const percent = computed(() =>
  props.progress.total ? (props.progress.now / props.progress.total) * 100 : 0,
);
const subPercent = computed(() =>
  props.progress.subTotal ? (props.progress.subNow / props.progress.subTotal) * 100 : 0,
);
</script>

<template>
  <BaseModal :title="shownTitle" :closable="false">
    <div class="install-progress">
      <div class="install-state">{{ t(`add.packState.${progress.state}`) }}</div>

      <div class="progress-track" role="progressbar" aria-valuemin="0" :aria-valuemax="progress.total || 1"
        :aria-valuenow="progress.now">
        <div class="progress-fill" :style="{ width: percent + '%' }" />
      </div>
      <div v-if="progress.total" class="install-num">{{ progress.now }} / {{ progress.total }}</div>

      <template v-if="progress.subText || progress.subTotal">
        <div class="install-sub">{{ progress.subText || "" }}</div>
        <div class="progress-track sub">
          <div class="progress-fill" :style="{ width: subPercent + '%' }" />
        </div>
      </template>
    </div>
  </BaseModal>
</template>

<style scoped>
.install-progress {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.install-state {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text);
}

.progress-track {
  height: 8px;
  border-radius: 4px;
  background: var(--bg-hover);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 4px;
  background: var(--accent-grad);
  transition: width 0.3s;
}

.progress-track.sub {
  height: 6px;
}

.install-num {
  font-size: 12px;
  color: var(--text-dim);
  text-align: right;
}

.install-sub {
  font-size: 12px;
  color: var(--text-dim);
  margin-top: 4px;
  word-break: break-all;
}
</style>
