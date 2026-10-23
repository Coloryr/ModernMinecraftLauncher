<script setup lang="ts">
import { ref } from 'vue';
import { call, type CreateProject, type Template } from './api';
import { t } from './i18n';

defineProps<{ busy: boolean; error: string }>();
const emit = defineEmits<{ close: []; cancel: []; create: [options: CreateProject]; error: [error: unknown] }>();
const name = ref('my-theme');
const template = ref<Template>('css');
const parent = ref('');
const local = ref(false);
const source = ref('');
const templates: Template[] = ['css', 'page', 'home', 'html'];

async function pick(kind: 'parent' | 'source') {
  try {
    const path = await call('pick', kind);
    if (path) (kind === 'parent' ? parent : source).value = path;
  } catch (error) { emit('error', error); }
}
</script>

<template>
  <div class="modal-shade" @keydown.esc="!busy && emit('close')">
    <form class="modal" role="dialog" aria-modal="true" :aria-label="t('new')" @submit.prevent="emit('create', { parent, name, template, source: local ? source : '' })">
      <h2>{{ t('new') }}</h2>
      <label class="field-label" for="project-template">{{ t('template') }}</label>
      <select id="project-template" v-model="template" class="field-select" :disabled="busy"><option v-for="item in templates" :key="item" :value="item">{{ t(item) }}</option></select>
      <label class="field-label" for="project-name">{{ t('name') }}</label>
      <input id="project-name" v-model="name" class="field-input" autofocus required maxlength="80" :disabled="busy" />
      <label class="field-label">{{ t('chooseParent') }}</label>
      <div class="path-line"><output>{{ parent || '—' }}</output><button type="button" class="ui-btn v-plain s-sm" :disabled="busy" @click="pick('parent')">{{ t('chooseParent') }}</button></div>
      <label class="field-label" for="project-source">{{ t('source') }}</label>
      <select id="project-source" v-model="local" class="field-select" :disabled="busy"><option :value="false">{{ t('download') }}</option><option :value="true">{{ t('local') }}</option></select>
      <div v-if="local" class="path-line"><output>{{ source || '—' }}</output><button type="button" class="ui-btn v-plain s-sm" :disabled="busy" @click="pick('source')">{{ t('pickSource') }}</button></div>
      <p class="muted">{{ t('createHint') }}</p>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="modal-actions"><button type="button" class="ui-btn v-plain s-sm" @click="busy ? emit('cancel') : emit('close')">{{ t('cancel') }}</button><button class="ui-btn v-primary s-sm" :disabled="busy || !name.trim() || !parent || (local && !source)">{{ busy ? t('working') : t('create') }}</button></div>
    </form>
  </div>
</template>
