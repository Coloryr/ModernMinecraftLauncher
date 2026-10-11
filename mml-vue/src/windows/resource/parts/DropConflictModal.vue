<script setup lang="ts">
// 拖拽导入的同名冲突弹窗：覆盖 / 跳过 / 重命名 三选一
//
// 资源窗口拖入文件时，若目标位置已有同名项，就先弹这里问一次（"每次问我"那条要求），
// 选完才真正落盘。按钮沿用全仓通用的 BaseButton + 全局 .modal-actions。
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import { t } from "../../../lib/i18n";

const props = defineProps<{
  /** 与盘上已有的同名项 */
  conflicts: string[];
  /** 有导入在跑：按钮统一置灰 */
  busy: boolean;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  /** mode: overwrite / skip / rename */
  (e: "pick", mode: string): void;
}>();

const previewLimit = 5;
</script>

<template>
  <BaseModal :title="t('resource.dropConflictTitle')" :closable="false" @close="emit('close')">
    <p class="delete-tip">{{ t("resource.dropConflictText", { count: props.conflicts.length }) }}</p>
    <!-- 只列前几条：拖一批进来时清单可能很长，弹窗不能长到看不见按钮 -->
    <ul class="drop-conflict-list">
      <li v-for="name in props.conflicts.slice(0, previewLimit)" :key="name">{{ name }}</li>
      <li v-if="props.conflicts.length > previewLimit" class="drop-conflict-more">
        {{ t("resource.dropConflictMore", { count: props.conflicts.length - previewLimit }) }}
      </li>
    </ul>
    <div class="modal-actions">
      <BaseButton :disabled="props.busy" @click="emit('pick', 'skip')">
        {{ t("resource.dropSkip") }}
      </BaseButton>
      <BaseButton variant="accent" :disabled="props.busy" @click="emit('pick', 'rename')">
        {{ t("resource.dropRename") }}
      </BaseButton>
      <BaseButton variant="danger" :disabled="props.busy" @click="emit('pick', 'overwrite')">
        {{ t("resource.dropOverwrite") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 列出来的那几个同名项：拖一批进来时清单可能很长，限高自己滚，别把按钮顶出屏幕 */
.drop-conflict-list {
  margin: 4px 0 0;
  padding-left: 18px;
  max-height: 148px;
  overflow-y: auto;
  font-size: 13px;
  color: var(--text-dim);
}

.drop-conflict-more {
  list-style: none;
  margin-left: -18px;
  color: var(--text-dim);
}
</style>
