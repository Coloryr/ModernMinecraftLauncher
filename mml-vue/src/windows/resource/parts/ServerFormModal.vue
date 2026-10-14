<script setup lang="ts">
// 服务器添加 / 编辑表单弹窗
//
// 只管排版与输入：草稿对象与保存逻辑在 ServerPane（它是列表的拥有者），
// 保存中不关弹窗 —— 失败提示已由调用方弹出，输入内容留着让用户改。
//
// 两个"别关"一起给上：`closable=false` 去掉右上角 ✕，`overlay-close=false` 让点遮罩
// 也不关 —— 这是个**输入类**弹窗，误触一下就白填了；BaseModal 的 overlayClose 就是
// 为这种情况留的。要取消就点左下角的「取消」
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import { t } from "../../../lib/i18n";
import type { ServerFormDraft } from "../types";

defineProps<{
  /** 表单草稿；`edit` 为真时按原 name/ip 定位改那一条 */
  form: ServerFormDraft;
  /** 正在保存：按钮置灰，防连点 */
  busy: boolean;
}>();
const emit = defineEmits<{
  /** 提交表单，保存逻辑在 ServerPane */
  (e: "submit"): void;
  /** 取消，收起弹窗 */
  (e: "close"): void;
}>();
</script>

<template>
  <BaseModal :title="form.edit ? t('resource.serverEdit') : t('resource.serverAdd')" :closable="false"
    :overlay-close="false" @close="emit('close')">
    <div class="form-grid">
      <label class="form-label">{{ t("resource.serverName") }}</label>
      <input v-model="form.name" class="form-input" type="text" />
      <label class="form-label">{{ t("resource.serverIp") }}</label>
      <input v-model="form.ip" class="form-input" type="text" />
      <!-- 接受材质是客户端选项，只在编辑已有条目时出现（新增走客户端默认） -->
      <template v-if="form.edit">
        <label class="form-label">{{ t("resource.acceptTextures") }}</label>
        <input v-model="form.acceptTextures" class="form-check" type="checkbox" />
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
.form-grid {
  display: grid;
  grid-template-columns: auto 1fr;
  align-items: center;
  gap: 10px 12px;
}

.form-label {
  font-size: 12px;
  color: var(--text-dim);
}

.form-input {
  padding: 3px 10px;
  /* 尺寸与全 app 控件同档；只靠 padding 撑的话这里会比同排控件矮一头 */
  min-height: var(--ctl-h);
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-side);
  color: var(--text);
  font-size: var(--ctl-font);
  font-family: inherit;
  min-width: 0;
}

.form-input:focus {
  outline: none;
  border-color: var(--accent);
}

/* 复选框：grid 里默认 `justify-self: normal` 会把它**拉伸**到整列宽，
   浏览器又把那个小方框画在拉伸框的正中间 —— 看着就是"开关居中"。
   钉到列首，与上面两个输入框左对齐 */
.form-check {
  justify-self: start;
  width: 15px;
  height: 15px;
  margin: 0;
  cursor: pointer;
  accent-color: var(--accent);
}
</style>
