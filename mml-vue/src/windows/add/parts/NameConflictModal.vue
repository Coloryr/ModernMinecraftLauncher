<script setup lang="ts">
// 添加实例 · 实例重名确认弹窗
// 后端创建流程会暂停等这条答复：点遮罩 / 右上角关闭都按「否」处理，避免流程挂住
import { t } from "../../../lib/i18n";
import BaseModal from "../../../components/ui/BaseModal.vue";
import BaseButton from "../../../components/ui/BaseButton.vue";

defineProps<{
  /** overwrite = 覆盖同名实例；其余 = 询问是否允许自动改名 */
  kind: string;
  /** 冲突的实例名，弹窗正文里用 */
  name: string;
}>();

const emit = defineEmits<{
  /** 答复后端：true = 同意覆盖或自动改名 */
  (e: "answer", v: boolean): void;
}>();
</script>

<template>
  <BaseModal :title="t('add.nameConflictTitle')" :closable="false" @close="emit('answer', false)">
    <p class="modal-text">
      {{
        kind === "overwrite"
          ? t("add.nameConflictOverwrite", { name })
          : t("add.nameConflictRename")
      }}
    </p>
    <div class="modal-actions">
      <BaseButton @click="emit('answer', false)">{{ t("add.no") }}</BaseButton>
      <BaseButton variant="primary" @click="emit('answer', true)">{{ t("add.yes") }}</BaseButton>
    </div>
  </BaseModal>
</template>
