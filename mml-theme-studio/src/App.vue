<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { call, type Catalog, type CreateProject, type Project, type SourceFile, type Theme } from './api';
import { locale, t } from './i18n';
import EditorPane from './EditorPane.vue';
import ProjectDialog from './ProjectDialog.vue';
import CssCatalog from './CssCatalog.vue';

const project = ref<Project | null>(null);
const recent = ref<string[]>([]);
const files = ref<string[]>([]);
const active = ref<SourceFile | null>(null);
const draft = ref('');
const catalog = ref<Catalog>({ controls: [], variables: [] });
const output = ref('');
const error = ref('');
const busy = ref(false);
const acting = ref(false);
const saving = ref(false);
const working = computed(() => busy.value || acting.value || saving.value);
const dirty = computed(() => !!active.value && active.value.text !== draft.value);
const newProject = ref(false);
const newFile = ref(false);
const fileName = ref('');
const confirming = ref(false);
let confirmResult: ((answer: boolean) => void) | undefined;
const modal = computed(() => newProject.value || newFile.value || confirming.value);
const previewUrl = ref('');
const previewKind = ref('main');
const previewTheme = ref('Dark');
const previewHost = ref<HTMLDivElement>();
const editor = ref<InstanceType<typeof EditorPane>>();
const outputHost = ref<HTMLPreElement>();
const ratio = ref(50);
const env = ref('');
const studioTheme = ref<Theme>(new URLSearchParams(location.search).get('theme') === 'Light' ? 'Light' : 'Dark');
const kinds = ['main', 'settings', 'account', 'add', 'resource', 'download', 'log'];
let observer: ResizeObserver;
let unsubscribe: () => void;

function showError(value: unknown) { error.value = value instanceof Error ? value.message : String(value); }
async function run(action: () => Promise<unknown>) {
  if (working.value) return;
  acting.value = true;
  error.value = '';
  try { await action(); } catch (value) { showError(value); }
  finally { acting.value = false; }
}
function confirmDiscard(): Promise<boolean> {
  if (!dirty.value) return Promise.resolve(true);
  confirming.value = true;
  return new Promise((resolve) => { confirmResult = resolve; });
}
function answer(value: boolean) { confirming.value = false; confirmResult?.(value); confirmResult = undefined; }

async function loadFile(path: string) {
  if (!await confirmDiscard()) return;
  const file = await call('read', path);
  active.value = file;
  draft.value = file.text;
}
async function refresh() { files.value = await call('files'); }
async function loadProject(value: Project) {
  project.value = value;
  active.value = null;
  draft.value = '';
  catalog.value = { controls: [], variables: [] };
  recent.value = [value.root, ...recent.value.filter((path) => path !== value.root)].slice(0, 8);
  await refresh();
  const first = [value.template === 'css' ? 'custom.css' : 'src/App.vue', 'src/ReplaceWindow.vue', 'src/index.html', 'src/theme.json'].find((path) => files.value.includes(path));
  if (first) await loadFile(first);
}
async function loadCatalog() { catalog.value = await call('catalog'); }
async function openProject(root?: string) {
  if (!await confirmDiscard()) return;
  const path = root || await call('pick', 'project');
  if (path) { await loadProject(await call('open', path)); await loadCatalog(); }
}
async function create(options: CreateProject) {
  const value = await call('create', options);
  newProject.value = false;
  await loadProject(value);
  await call('install');
  await loadCatalog();
  await call('preview');
}
async function save() {
  if (!active.value || !dirty.value || working.value) return;
  saving.value = true;
  error.value = '';
  try {
    const file = await call('save', { path: active.value.path, text: draft.value, expectedHash: active.value.hash });
    active.value = file;
    draft.value = file.text;
  } catch (value) { showError(value); }
  finally { saving.value = false; }
}
async function createFile() {
  const file = await call('save', { path: fileName.value, text: '', expectedHash: null });
  newFile.value = false;
  active.value = file;
  draft.value = file.text;
  await refresh();
}
async function build() {
  if (dirty.value) { showError(t('saveBeforeBuild')); return; }
  await call('build');
  await refresh();
}
async function startPreview() {
  if (dirty.value) { showError(t('saveBeforeBuild')); return; }
  await call('preview');
}
async function showNew(kind: 'project' | 'file') {
  if (!await confirmDiscard()) return;
  if (kind === 'project') newProject.value = true;
  else { fileName.value = ''; newFile.value = true; }
}
function updateBounds() {
  const rect = previewHost.value?.getBoundingClientRect();
  if (!rect) return;
  void call('bounds', { x: rect.x, y: rect.y, width: rect.width, height: rect.height, visible: !!previewUrl.value && !modal.value }).catch(showError);
}
async function updatePreviewOptions() {
  await call('previewOptions', { kind: previewKind.value, theme: previewTheme.value });
}
async function updateStudioTheme(event: Event) {
  const select = event.target as HTMLSelectElement;
  try { studioTheme.value = await call('theme', select.value as Theme); }
  finally { select.value = studioTheme.value; }
}
function keydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') { event.preventDefault(); void save(); }
  if (event.key === 'Escape' && !working.value) { newProject.value = false; newFile.value = false; if (confirming.value) answer(false); }
}

watch(dirty, (value) => { void call('dirty', value).catch(showError); });
watch(studioTheme, (value) => { document.documentElement.dataset.theme = value; }, { immediate: true });
watch(modal, () => { void nextTick(updateBounds); });
watch(previewUrl, () => { void nextTick(updateBounds); });
watch(output, () => { void nextTick(() => { if (outputHost.value) outputHost.value.scrollTop = outputHost.value.scrollHeight; }); });
onMounted(async () => {
  unsubscribe = window.studio.onEvent((event) => {
    if (event.type === 'log') output.value = (output.value + event.text).slice(-150000);
    if (event.type === 'busy') busy.value = event.value;
    if (event.type === 'preview') previewUrl.value = event.url;
  });
  observer = new ResizeObserver(updateBounds);
  if (previewHost.value) observer.observe(previewHost.value);
  window.addEventListener('keydown', keydown);
  try {
    const state = await call('state');
    locale.value = state.locale;
    studioTheme.value = state.theme;
    recent.value = state.recent;
    env.value = state.environment.error || `Node.js ${state.environment.nodeVersion} · ${state.environment.git}`;
    if (state.project) await loadProject(state.project);
  } catch (value) { showError(value); }
});
onBeforeUnmount(() => { observer?.disconnect(); unsubscribe?.(); window.removeEventListener('keydown', keydown); confirmResult?.(false); });
</script>

<template>
  <div class="studio-shell">
    <header class="studio-header">
      <div class="brand"><span class="brand-mark">M²L</span><div><h1>{{ t('title') }}</h1><small>{{ project?.name || 'THEME STUDIO' }}</small></div></div>
      <div class="toolbar">
        <button class="ui-btn v-plain s-sm" :disabled="working" @click="run(() => showNew('project'))">{{ t('new') }}</button>
        <button class="ui-btn v-plain s-sm" :disabled="working" @click="run(() => openProject())">{{ t('open') }}</button>
        <label class="theme-control" for="studio-theme"><span>{{ t('studioTheme') }}</span><select id="studio-theme" :value="studioTheme" class="field-select theme-select" :disabled="working" @change="run(() => updateStudioTheme($event))"><option value="Light">{{ t('light') }}</option><option value="Dark">{{ t('dark') }}</option></select></label>
        <select v-model="locale" class="field-select language-select" :aria-label="t('language')" :disabled="working" @change="call('locale', locale).catch(showError)"><option value="zh-CN">中文</option><option value="en-US">English</option></select>
      </div>
    </header>
    <nav class="action-bar" :aria-label="t('title')">
      <button class="ui-btn v-plain s-sm" :disabled="!dirty || working" title="Ctrl+S" @click="save">{{ t('save') }}<span v-if="dirty">●</span></button>
      <button class="ui-btn v-plain s-sm" :disabled="!project || working" @click="run(async () => { await call('install'); await loadCatalog(); })">{{ t('install') }}</button>
      <button class="ui-btn v-primary s-sm" :disabled="!project || working" @click="run(startPreview)">▷ {{ t('preview') }}</button>
      <button class="ui-btn v-plain s-sm" :disabled="!previewUrl || working" @click="run(() => call('stop'))">{{ t('stop') }}</button>
      <button class="ui-btn v-accent s-sm" :disabled="!project || working" @click="run(build)">{{ t('build') }}</button>
      <button class="ui-btn v-ghost s-sm" :disabled="!project || working" @click="run(() => call('reveal'))">{{ t('reveal') }}</button>
      <span class="grow" /><span class="status" :class="{ pending: working }">{{ working ? t('working') : t('ready') }}</span>
      <button v-if="busy" class="ui-btn v-danger s-sm" @click="call('cancel').catch(showError)">{{ t('cancel') }}</button>
    </nav>
    <div v-if="error" class="error-banner" role="alert"><span>{{ error }}</span><button class="ui-btn v-ghost s-sm" :aria-label="t('close')" @click="error = ''">×</button></div>
    <main class="workspace" :style="{ '--editor-ratio': ratio + 'fr', '--preview-ratio': (100 - ratio) + 'fr' }">
      <aside class="sidebar">
        <div class="panel-head"><strong>{{ t('files') }}</strong><button class="ui-btn v-ghost s-sm" :disabled="!project || working" :title="t('newFile')" @click="run(() => showNew('file'))">＋</button><button class="ui-btn v-ghost s-sm" :disabled="!project || working" :title="t('refreshFiles')" @click="run(refresh)">↻</button></div>
        <div class="file-list" :aria-label="t('files')">
          <button v-for="path in files" :key="path" class="file-entry ui-btn v-ghost s-sm" :class="{ selected: active?.path === path }" :disabled="working" :title="path" @click="run(() => loadFile(path))"><span>{{ path.includes('/') ? '◇' : '▤' }}</span>{{ path }}</button>
          <template v-if="!project"><p class="muted">{{ t('empty') }}</p><p v-if="recent.length" class="muted">{{ t('recent') }}</p><button v-for="path in recent" :key="path" class="recent-entry ui-btn v-ghost s-sm" :title="path" :disabled="working" @click="run(() => openProject(path))">{{ path }}</button></template>
        </div>
        <CssCatalog v-if="catalog.controls.length || catalog.variables.length" :catalog="catalog" :enabled="!!active && !working" @insert="editor?.insert($event)" />
      </aside>
      <section class="code-pane">
        <div class="panel-head"><span class="file-title">{{ active?.path || t('chooseFile') }}</span><span v-if="dirty" class="dirty-mark" :title="t('unsaved')">●</span><span class="grow" /><button class="ui-btn v-ghost s-sm" :disabled="!active || working" :title="t('reopenFile')" @click="run(() => loadFile(active!.path))">↻</button></div>
        <EditorPane v-if="active" ref="editor" v-model="draft" :path="project!.root + '/' + active.path" :read-only="working" :theme="studioTheme" @save="save" />
        <div v-else class="empty-pane"><span class="empty-symbol">{ }</span><p>{{ t('chooseFile') }}</p><p class="muted">CSS · Vue · HTML · TypeScript</p></div>
      </section>
      <section class="preview-pane">
        <div class="panel-head"><strong>{{ t('contentPreview') }}</strong><span class="grow" /><button class="ui-btn v-ghost s-sm" :disabled="!previewUrl" @click="call('reload').catch(showError)">↻</button><button class="ui-btn v-accent s-sm" :disabled="!previewUrl" :title="t('inspectorHint')" @click="call('devtools').catch(showError)">F12</button></div>
        <div class="preview-options">
          <select v-model="previewKind" class="field-select" :aria-label="t('window')" :disabled="!previewUrl || project?.template === 'html'" @change="updatePreviewOptions().catch(showError)"><option v-for="kind in kinds" :key="kind">{{ kind }}</option></select>
          <select v-model="previewTheme" class="field-select" :aria-label="t('theme')" :disabled="!previewUrl || project?.template === 'html'" @change="updatePreviewOptions().catch(showError)"><option value="Dark">{{ t('dark') }}</option><option value="Light">{{ t('light') }}</option></select>
          <input v-model.number="ratio" type="range" min="25" max="70" :aria-label="t('splitRatio')" :title="t('splitRatio')" />
        </div>
        <div ref="previewHost" class="preview-host"><div class="empty-pane"><span class="empty-symbol">▷</span><p>{{ t('previewEmpty') }}</p></div></div>
        <p class="preview-hint">{{ t('previewHint') }}</p>
      </section>
      <section class="output-pane"><div class="panel-head"><strong>{{ t('output') }}</strong><span class="grow" /><span class="muted">{{ env }}</span></div><pre ref="outputHost" class="output-log" aria-live="off">{{ output || t('ready') }}</pre></section>
    </main>
    <footer class="status-bar"><span>{{ project?.root || t('empty') }}</span><span v-if="project?.revision">{{ t('revision') }} {{ project.revision.slice(0, 12) }}</span><span>UTF-8</span></footer>
    <ProjectDialog v-if="newProject" :busy="working" :error="error" @close="newProject = false" @cancel="call('cancel').catch(showError)" @create="run(() => create($event))" @error="showError" />
    <div v-if="newFile" class="modal-shade"><form class="modal" role="dialog" aria-modal="true" :aria-label="t('newFile')" @submit.prevent="run(createFile)"><h2>{{ t('newFile') }}</h2><label class="field-label" for="file-path">{{ t('path') }}</label><input id="file-path" v-model="fileName" class="field-input" required autofocus :disabled="working" /><p v-if="error" class="form-error" role="alert">{{ error }}</p><div class="modal-actions"><button type="button" class="ui-btn v-plain s-sm" :disabled="working" @click="newFile = false">{{ t('cancel') }}</button><button class="ui-btn v-primary s-sm" :disabled="working || !fileName">{{ t('newFile') }}</button></div></form></div>
    <div v-if="confirming" class="modal-shade"><div class="modal" role="alertdialog" aria-modal="true" :aria-label="t('unsaved')"><h2>{{ t('unsaved') }}</h2><p>{{ t('confirmDiscard') }}</p><div class="modal-actions"><button class="ui-btn v-plain s-sm" autofocus @click="answer(false)">{{ t('cancel') }}</button><button class="ui-btn v-danger s-sm" @click="answer(true)">{{ t('discard') }}</button></div></div></div>
  </div>
</template>
