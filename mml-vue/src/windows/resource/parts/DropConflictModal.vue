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
  /** 关闭弹窗，这一批文件都不导入 */
  (e: "close"): void;
  /** 选了处理方式；`mode` 取 overwrite / skip / rename */
  (e: "pick", mode: string): void;
}>();

/** 清单里最多列几个同名项，其余折成一行汇总 */
const previewLimit = 5;
</script>

<template>
  <BaseModal :title="t('resource.dropConflictTitle')" :closable="false" @close="emit('close')">
    <p class="delete-tip">{{ t("resource.dropConflictText", { count: props.conflicts.length }) }}</p>
    <!-- 只列前几条：拖一批进来时清单可能很长，弹窗不能长到看不见按钮 -->
    <ul class="drop-conflict-list mt-1 mb-0 ml-0 mr-[-8px] max-h-[148px] overflow-y-auto pl-[18px] pr-2 text-title text-dim">
      <li v-for="name in props.conflicts.slice(0, previewLimit)" :key="name">{{ name }}</li>
      <li v-if="props.conflicts.length > previewLimit" class="drop-conflict-more -ml-[18px] list-none text-dim">
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

