<script setup lang="ts">
import { ref } from "vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import { commands, type ModExportDto } from "../../../../lib/bindings";
import { t, tErr } from "../../../../lib/i18n";
import { showToast } from "../../../../lib/toast";

const props = defineProps<{ uuid: string; selectedIds: string[] }>();
const emit = defineEmits<{ close: [] }>();
const format = ref<ModExportDto["format"]>("txt");
const fields = ref(["name", "modId", "version", "file"]);
const onlySelected = ref(props.selectedIds.length > 0);
const busy = ref(false);
const options = ["name", "modId", "version", "file", "author", "description", "url", "source", "projectId", "fileId", "sha1", "disable", "side", "loaders", "note"];

async function saveList() {
  if (busy.value || !fields.value.length) return;
  busy.value = true;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const file = await save({ defaultPath: `mods.${format.value}`, filters: [{ name: format.value.toUpperCase(), extensions: [format.value] }] });
    if (!file) return;
    const count = await commands.resource.exportMods(props.uuid, {
      file, format: format.value, fields: fields.value,
      ids: onlySelected.value ? props.selectedIds : [],
    });
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
      <label>{{ t('modExport.format') }}
        <select v-model="format" class="field-select"><option value="txt">TXT</option><option value="csv">CSV</option><option value="json">JSON</option></select>
      </label>
      <label v-if="selectedIds.length"><input v-model="onlySelected" type="checkbox" />{{ t('modExport.selected') }}</label>
      <p>{{ t('modExport.fields') }}</p>
      <div class="export-fields"><label v-for="field in options" :key="field"><input v-model="fields" type="checkbox" :value="field" />{{ t(`modExport.${field}`) }}</label></div>
      <BaseButton variant="primary" :disabled="!fields.length || busy" @click="saveList">{{ t('winExport.exportBtn') }}</BaseButton>
    </fieldset>
  </BaseModal>
</template>

<style scoped>
.export-options { border: 0; padding: 0; display: grid; gap: 14px; min-width: 0; }
.export-options label { display: flex; align-items: center; gap: 8px; }
.export-fields { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
</style>
