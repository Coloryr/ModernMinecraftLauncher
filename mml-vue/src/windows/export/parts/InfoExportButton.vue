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
  <BaseButton size="sm" :disabled="disabled || busy" @click="saveInfo">
    {{ busy ? t("winExport.generatingInfo") : t("winExport.generateInfo") }}
  </BaseButton>
</template>
