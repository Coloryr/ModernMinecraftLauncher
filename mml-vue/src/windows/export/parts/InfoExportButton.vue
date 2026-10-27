<script setup lang="ts">
import { ref } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import { commands } from "../../../lib/bindings";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";

const props = defineProps<{ uuid: string; name: string; disabled: boolean }>();
const busy = ref(false);

async function saveInfo() {
  if (busy.value || props.disabled || !props.uuid) return;
  const uuid = props.uuid;
  const name = props.name;
  busy.value = true;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const file = await save({ defaultPath: `${name}-info.json`, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!file) return;
    await commands.export.generateInfo(uuid, file);
    showToast(t("winExport.done", { file }));
  } catch (error) { showToast(tErr(error)); }
  finally { busy.value = false; }
}
</script>

<template>
  <BaseButton size="sm" class="icon-btn-head" :disabled="disabled || busy"
    v-tip="busy ? t('winExport.generatingInfo') : t('winExport.generateInfo')"
    :aria-label="busy ? t('winExport.generatingInfo') : t('winExport.generateInfo')" @click="saveInfo">
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
      stroke-linecap="round" stroke-linejoin="round">
      <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
      <path d="M14 3v5h5M9 13h6M9 17h6" />
    </svg>
  </BaseButton>
</template>
