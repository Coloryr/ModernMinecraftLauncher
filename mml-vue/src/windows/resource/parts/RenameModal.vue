<script setup lang="ts">
// 重命名资源文件 / 目录：一个输入框，回车或点确定提交
//
// 六个分类共用这一份：分类到目录的映射与校验都在后端，这里只管问名字。
// 由 useResourceOps 的 renameAsk 驱动，挂在资源窗口外壳上，各 pane 只负责发起。
import { nextTick, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";

const props = defineProps<{
  /** 现在的名字，用来预填与提示"改的是什么" */
  name: string;
}>();

const emit = defineEmits<{
  /** 关掉，不改 */
  (e: "close"): void;
  /** 提交新名字 */
  (e: "rename", newName: string): void;
}>();

/** 输入框里的草稿，打开时预填现在的名字（选中主干部分，方便直接改） */
const value = ref(props.name);
const input = ref<HTMLInputElement | null>(null);

watch(
  () => props.name,
  async (name) => {
    value.value = name;
    await nextTick();
    input.value?.focus();
    input.value?.select();
  },
  { immediate: true },
);

/** 提交：空名字或没改都不发 */
function submit() {
  const next = value.value.trim();
  if (!next || next === props.name) {
    emit("close");
    return;
  }
  emit("rename", next);
}
</script>

<template>
  <BaseModal :title="t('resource.renameTitle')" :closable="false" @close="emit('close')">
    <label class="field-label">{{ t("resource.renameName") }}</label>
    <input ref="input" v-model="value" class="field-input" spellcheck="false" autocomplete="off"
      @keydown.enter.prevent="submit" @keydown.esc.prevent="emit('close')" />
    <p class="field-desc">{{ t("resource.renameDesc", { name: props.name }) }}</p>
    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("resource.cancel") }}</BaseButton>
      <BaseButton variant="accent" @click="submit">{{ t("resource.renameOk") }}</BaseButton>
    </div>
  </BaseModal>
</template>
