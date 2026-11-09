<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const { busy, name, version, text, url, folder, pickFolder } = ctx;
</script>

<template>
<section class="si-section">
  <div class="field-row-2">
    <div>
      <label class="field-label" for="si-name">{{ t("serverInstance.name") }}</label>
      <input id="si-name" v-model="name" class="field-input" :disabled="busy" spellcheck="false" />
    </div>
    <div>
      <label class="field-label" for="si-version">{{ t("serverInstance.version") }}</label>
      <input id="si-version" v-model="version" class="field-input" :disabled="busy" spellcheck="false" />
    </div>
  </div>

  <label class="field-label" for="si-text">{{ t("serverInstance.desc") }}</label>
  <input id="si-text" v-model="text" class="field-input" :disabled="busy" spellcheck="false"
    :placeholder="t('serverInstance.descPlaceholder')" />

  <label class="field-label" for="si-url">{{ t("serverInstance.url") }}</label>
  <input id="si-url" v-model="url" class="field-input" :disabled="busy" spellcheck="false"
    :placeholder="t('serverInstance.urlPlaceholder')" />
  <p class="field-hint">{{ t("serverInstance.urlHint") }}</p>

  <label class="field-label" for="si-folder">{{ t("serverInstance.folder") }}</label>
  <div class="path-row">
    <input id="si-folder" v-model="folder" class="field-input" :disabled="busy" spellcheck="false"
      :placeholder="t('serverInstance.folderPlaceholder')" />
    <BaseButton :disabled="busy" @click="pickFolder">{{ t("serverInstance.pick") }}</BaseButton>
  </div>
  <p class="field-hint">{{ t("serverInstance.folderHint") }}</p>
</section>
</template>
