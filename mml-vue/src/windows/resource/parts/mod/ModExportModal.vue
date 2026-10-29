<script setup lang="ts">
// 导出模组列表：选格式与字段，落成 txt / csv / json 文件
//
// 内容由后端按当前实例的扫描结果生成，这里只收集选项；没有选中项时不出现
// 「只导选中项」那一行。
import { ref } from "vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import { commands, type ModExportDto } from "../../../../lib/bindings";
import { t, tErr } from "../../../../lib/i18n";
import { showToast } from "../../../../lib/toast";

const props = defineProps<{
  /** 本窗口锁定的实例 uuid */
  uuid: string;
  /** 当前选中的模组 uuid；空数组表示没有选中项 */
  selectedIds: string[];
}>();

const emit = defineEmits<{
  /** 关掉弹窗；导出成功后也走它 */
  (e: "close"): void;
}>();

/** 导出格式 */
const format = ref<ModExportDto["format"]>("txt");
/** 勾中的字段，取值见下面的 options */
const fields = ref(["name", "modId", "version", "file"]);
/** 只导出选中的那些；没有选中项时恒为 false */
const onlySelected = ref(props.selectedIds.length > 0);
const busy = ref(false);
/** 可导出字段，顺序即弹窗里的排列，取值与后端 DTO 的字段名一致 */
const options = ["name", "modId", "version", "file", "author", "description", "url", "source", "projectId", "fileId", "sha1", "disable", "side", "loaders", "note"];

/** 选落点并导出：选完对话框就直接写文件，成功后关窗 */
async function saveList() {
  if (busy.value || !fields.value.length || (onlySelected.value && !props.selectedIds.length)) return;
  const uuid = props.uuid;
  const config = { format: format.value, fields: [...fields.value], ids: onlySelected.value ? [...props.selectedIds] : [] };
  busy.value = true;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const file = await save({ defaultPath: `mods.${config.format}`, filters: [{ name: config.format.toUpperCase(), extensions: [config.format] }] });
    if (!file) return;
    const count = await commands.resource.exportMods(uuid, { ...config, file });
    showToast(t("modExport.done", { count, file }));
    emit("close");
  } catch (error) {
    showToast(tErr(error));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <BaseModal :title="t('modExport.title')" :width="520" :closable="!busy" @close="emit('close')">
    <fieldset class="export-options" :disabled="busy">
      <!-- 格式：标签与下拉同一行，下拉定宽 —— 三个短选项撑满整行没有必要 -->
      <div class="export-format">
        <label class="field-label" for="mod-export-format">{{ t('modExport.format') }}</label>
        <select id="mod-export-format" v-model="format" class="field-select">
          <option value="txt">TXT</option>
          <option value="csv">CSV</option>
          <option value="json">JSON</option>
        </select>
      </div>

      <label v-if="selectedIds.length || onlySelected" class="export-check export-only-selected">
        <input v-model="onlySelected" type="checkbox" />{{ t('modExport.selected') }}
      </label>

      <p class="field-label">{{ t('modExport.fields') }}</p>
      <div class="export-fields">
        <label v-for="field in options" :key="field" class="export-check">
          <input v-model="fields" type="checkbox" :value="field" />{{ t(`modExport.${field}`) }}
        </label>
      </div>
    </fieldset>

    <div class="modal-actions">
      <BaseButton variant="primary" :disabled="!fields.length || busy || (onlySelected && !selectedIds.length)" @click="saveList">{{ t('winExport.exportBtn') }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 弹窗在 body 下，样式走 scoped（与 ModUpdateModal / ModDepsModal 同一处理） */
.export-options {
  border: 0;
  padding: 0;
  margin: 0;
  min-width: 0;
}

/* 格式：标签与下拉同一行；标签 flex: none 才不会被下拉挤成竖排 */
.export-format {
  display: flex;
  align-items: center;
  gap: 10px;
}

.export-format .field-label {
  margin: 0;
  flex: none;
}

.export-format .field-select {
  width: 180px;
}

.export-only-selected {
  margin-top: 12px;
}

/* 导出字段：三列，15 项正好五行 */
.export-fields {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px 16px;
}

.export-check {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
}

.export-check input {
  flex: none;
  width: 15px;
  height: 15px;
  margin: 0;
  accent-color: var(--accent);
  cursor: pointer;
}
</style>
