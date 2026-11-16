<script setup lang="ts">
// 服务器添加 / 编辑表单弹窗
//
// 只管排版与输入：草稿对象与保存逻辑在 ServerPane（它是列表的拥有者），
// 保存中不关弹窗 —— 失败提示已由调用方弹出，输入内容留着让用户改。
//
// 这是个**输入类**弹窗：`closable=false` 去掉右上角 ✕ —— 误触一下就白填了，
// 要取消只能点左下角的「取消」。点遮罩不关是所有弹窗的统一行为，见 BaseModal
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import { t } from "../../../lib/i18n";
import type { ServerFormDraft } from "../types";

const props = defineProps<{
  /** 表单草稿；`edit` 为真时按原 name/ip 定位改那一条 */
  form: ServerFormDraft;
  /** 正在保存：按钮置灰，防连点 */
  busy: boolean;
}>();
const emit = defineEmits<{
  (e: "update:form", value: ServerFormDraft): void;
  /** 提交表单，保存逻辑在 ServerPane */
  (e: "submit"): void;
  /** 取消，收起弹窗 */
  (e: "close"): void;
}>();

function updateForm<K extends keyof ServerFormDraft>(key: K, value: ServerFormDraft[K]) {
  emit("update:form", { ...props.form, [key]: value });
}
</script>

<template>
  <BaseModal :title="form.edit ? t('resource.serverEdit') : t('resource.serverAdd')" :closable="false"
    @close="emit('close')">
    <div class="form-grid grid grid-cols-[auto_1fr] items-center gap-x-3 gap-y-2.5">
      <label class="form-label text-body text-dim">{{ t("resource.serverName") }}</label>
      <input :value="form.name" class="form-input min-h-ctl min-w-0 rounded-ctl border border-border bg-side px-2.5 py-[3px] text-ctl text-fg [font-family:inherit]" type="text"
        @input="updateForm('name', ($event.target as HTMLInputElement).value)" />
      <label class="form-label text-body text-dim">{{ t("resource.serverIp") }}</label>
      <input :value="form.ip" class="form-input min-h-ctl min-w-0 rounded-ctl border border-border bg-side px-2.5 py-[3px] text-ctl text-fg [font-family:inherit]" type="text"
        @input="updateForm('ip', ($event.target as HTMLInputElement).value)" />
      <!-- 接受材质是客户端选项，只在编辑已有条目时出现（新增走客户端默认） -->
      <template v-if="form.edit">
        <label class="form-label text-body text-dim">{{ t("resource.acceptTextures") }}</label>
        <input :checked="form.acceptTextures" class="form-check m-0 h-[15px] w-[15px] cursor-pointer justify-self-start accent-accent" type="checkbox"
          @change="updateForm('acceptTextures', ($event.target as HTMLInputElement).checked)" />
      </template>
    </div>
    <div class="modal-actions">
      <BaseButton :disabled="busy" @click="emit('close')">{{ t("resource.cancel") }}</BaseButton>
      <BaseButton variant="primary" :disabled="busy" @click="emit('submit')">
        {{ busy ? t("actions.saving") : t("resource.save") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<!-- 弹窗内容是 Teleport 到 body 的，不在 .resource-layout 里，所以这几条自带 scoped 样式 -->
<style scoped>
/* 聚焦态留在 CSS：伪类是工具类表达不了的那一档。 */
.form-input:focus {
  outline: none;
  border-color: var(--accent);
}
</style>
