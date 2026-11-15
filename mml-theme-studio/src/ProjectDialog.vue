<script setup lang="ts">
import { ref, watch } from 'vue';
import { call, type CreateProject, type Template } from './api';
import { t } from './i18n';

defineProps<{ busy: boolean; error: string }>();
const emit = defineEmits<{ close: []; cancel: []; create: [options: CreateProject]; error: [error: unknown] }>();

/** 各 demo 的默认目录名 */
const DEFAULT_NAMES: Record<Template, string> = {
  home: 'custom-home',
  'home-html': 'custom-home-html',
  page: 'custom-theme',
  html: 'custom-theme-html',
  css: 'custom-css',
};

/** 旧版本的默认名；仍然等于它时按没填过处理 */
const LEGACY_DEFAULT_NAME = 'my-theme';

const template = ref<Template>('home');
const name = ref(DEFAULT_NAMES.home);
const parent = ref('');
const local = ref(false);
const source = ref('');
// 顺序与 custom/README.md 的「五个示例各自改什么」一致
const templates: Template[] = ['home', 'home-html', 'page', 'html', 'css'];

/** 用户是否手动改过目录名 */
let nameTouched = false;

/**
 * 没手动填过、留空、还是旧默认名时，用当前 demo 的默认目录名补上
 *
 * - `previous`: 切换前的模板；切模板时把上一个模板的默认名也当成"没填过"
 */
function fillDefaultName(previous?: Template) {
  const value = name.value.trim();
  const untouched = !nameTouched
    || !value
    || value === LEGACY_DEFAULT_NAME
    || (previous !== undefined && value === DEFAULT_NAMES[previous]);
  if (untouched) {
    name.value = DEFAULT_NAMES[template.value];
    nameTouched = false;
  }
}

watch(template, (_next, previous) => fillDefaultName(previous));

function onNameInput() {
  nameTouched = true;
}

function onNameBlur() {
  if (!name.value.trim()) fillDefaultName();
}

function submit() {
  if (!name.value.trim()) {
    name.value = DEFAULT_NAMES[template.value];
    nameTouched = false;
  }
  emit('create', {
    parent: parent.value,
    name: name.value.trim(),
    template: template.value,
    source: local.value ? source.value : '',
  });
}

async function pick(kind: 'parent' | 'source') {
  try {
    const path = await call('pick', kind);
    if (path) (kind === 'parent' ? parent : source).value = path;
  } catch (error) { emit('error', error); }
}
</script>

<template>
  <div class="modal-shade" @keydown.esc="!busy && emit('close')">
    <form class="modal" role="dialog" aria-modal="true" :aria-label="t('new')" @submit.prevent="submit">
      <h2>{{ t('new') }}</h2>
      <label class="field-label" for="project-template">{{ t('template') }}</label>
      <select id="project-template" v-model="template" class="field-select" :disabled="busy"><option v-for="item in templates" :key="item" :value="item">{{ t(item) }}</option></select>
      <label class="field-label" for="project-name">{{ t('name') }}</label>
      <input id="project-name" v-model="name" class="field-input" autofocus maxlength="80" :disabled="busy" @input="onNameInput" @blur="onNameBlur" />
      <label class="field-label">{{ t('chooseParent') }}</label>
      <div class="path-line"><output>{{ parent || '—' }}</output><button type="button" class="ui-btn v-plain s-sm" :disabled="busy" @click="pick('parent')">{{ t('chooseParent') }}</button></div>
      <label class="field-label" for="project-source">{{ t('source') }}</label>
      <select id="project-source" v-model="local" class="field-select" :disabled="busy"><option :value="false">{{ t('download') }}</option><option :value="true">{{ t('local') }}</option></select>
      <div v-if="local" class="path-line"><output>{{ source || '—' }}</output><button type="button" class="ui-btn v-plain s-sm" :disabled="busy" @click="pick('source')">{{ t('pickSource') }}</button></div>
      <p class="muted">{{ t('createHint') }}</p>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="modal-actions"><button type="button" class="ui-btn v-plain s-sm" @click="busy ? emit('cancel') : emit('close')">{{ t('cancel') }}</button><button class="ui-btn v-primary s-sm" :disabled="busy || !parent || (local && !source)">{{ busy ? t('working') : t('create') }}</button></div>
    </form>
  </div>
</template>
