<script setup lang="ts">
// 「查找实体 / 方块」弹窗，口径照 ColorMC 的查找对话框
//
// 三栏输入：找哪一类、区块坐标（绝对坐标，与区块列表显示的一致）、可空的 id 片段。
// 区域文件由后端按坐标自己算。只负责收集与校验，真正的查找由调用方发命令。
import { ref } from "vue";
import { t } from "../../../lib/i18n";
import BaseModal from "../../../components/ui/BaseModal.vue";
import BaseButton from "../../../components/ui/BaseButton.vue";

const emit = defineEmits<{
  /** 填完了，参数是原样字符串，由调用方解析 */
  (e: "find", value: { kind: string; x: string; z: string; name: string }): void;
  /** 关掉弹窗 */
  (e: "close"): void;
}>();

/** 找哪一类：方块实体或实体 */
const kind = ref("block_entities");
/** 方块坐标；两格都空表示整份存档扫一遍 */
const x = ref("");
const z = ref("");
const name = ref("");

function submit() {
  emit("find", {
    kind: kind.value,
    x: x.value.trim(),
    z: z.value.trim(),
    name: name.value.trim(),
  });
}
</script>

<template>
  <BaseModal :title="t('config.mcaFindTitle')" :width="380" :closable="false" @close="emit('close')">
    <label class="field-label">{{ t("config.mcaFindType") }}</label>
    <select v-model="kind" class="field-select">
      <option value="block_entities">{{ t("config.mcaFindBlock") }}</option>
      <option value="Entities">{{ t("config.mcaFindEntity") }}</option>
    </select>

    <div class="field-row-2">
      <div>
        <label class="field-label">{{ t("config.mcaFindBlockX") }}</label>
        <input v-model="x" class="field-input" inputmode="numeric" spellcheck="false"
          :placeholder="t('config.mcaFindPosTip')" />
      </div>
      <div>
        <label class="field-label">{{ t("config.mcaFindBlockZ") }}</label>
        <input v-model="z" class="field-input" inputmode="numeric" spellcheck="false"
          :placeholder="t('config.mcaFindPosTip')" />
      </div>
    </div>
    <p class="find-hint">{{ t("config.mcaFindPosHint") }}</p>

    <label class="field-label">{{ t("config.mcaFindName") }}</label>
    <input v-model="name" class="field-input" spellcheck="false" @keydown.enter="submit" />

    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("resource.cancel") }}</BaseButton>
      <BaseButton variant="primary" @click="submit">
        {{ t("config.mcaFindGo") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 坐标说明：讲清"留空 = 全扫"，不给控件加解释性文案 */
.find-hint {
  margin: 6px 0 0;
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
