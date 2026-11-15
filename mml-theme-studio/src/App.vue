<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { bootstrap, call, onEvent, type Catalog, type CreateProject, type Project, type SourceFile, type Theme } from './api';
import { locale, t } from './i18n';
import EditorPane from './EditorPane.vue';
import FileTree from './FileTree.vue';
import ProjectDialog from './ProjectDialog.vue';
import CssCatalog from './CssCatalog.vue';
import InspectorPanel from './InspectorPanel.vue';
import GlyphIcon from '../../mml-vue/src/components/ui/GlyphIcon.vue';
import { INSPECT_PROTOCOL } from './inspector';

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
// 状态栏步骤与已用时间：npm install 这类长命令在没有终端时一行都不打印，
// 只写“正在处理”会被当成卡死；至少让秒数走起来，并显示当前在哪一步
const step = ref('');
const elapsedMs = ref(0);
let elapsedTimer: number | undefined;
watch(working, (value) => {
  window.clearInterval(elapsedTimer);
  elapsedTimer = undefined;
  elapsedMs.value = 0;
  if (!value) return;
  const started = Date.now();
  elapsedTimer = window.setInterval(() => { elapsedMs.value = Date.now() - started; }, 1000);
});
const elapsedText = computed(() => {
  const seconds = Math.floor(elapsedMs.value / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
});
const statusText = computed(() => working.value ? `${step.value || t('working')} ${elapsedText.value}` : t('ready'));
const dirty = computed(() => !!active.value && active.value.text !== draft.value);
const newProject = ref(false);
const newFile = ref(false);
// IDE 布局：活动栏控制项目文件 / 预览 / 检查 / 底部面板
const sidebarOpen = ref(true);
const previewOpen = ref(true);
const panelOpen = ref(true);
const panelCollapsed = ref(false);
const panelTab = ref<'output' | 'inspector'>('output');
const fileName = ref('');
const confirming = ref(false);
let confirmResult: ((answer: boolean) => void) | undefined;
const previewUrl = ref('');
const previewKind = ref('main');
const previewTheme = ref('Dark');
const previewSurface = ref<HTMLDivElement>();
const previewFrame = ref<HTMLIFrameElement>();
const previewSize = ref({ width: 900, height: 600 });
const previewPos = ref({ x: 25, y: 25 });
const previewKey = ref(0);
// 元素检查：F12 在预览里切换，元素树与样式显示在预览下方的面板
const inspecting = ref(false);
// 面板重载后要重新拉一次树，预览重载时靠这个 key 让它重建
const previewEpoch = ref(0);
const inspectorPanel = ref<{ handle: (data: unknown) => void } | null>(null);
const surfaceExtent = ref({ width: 0, height: 0 });
const minSizes = ref<Record<string, { width: number; height: number }>>({});
const PREVIEW_GAP = 4;
const PREVIEW_MARGIN = 16;
const PREVIEW_INSET = PREVIEW_GAP + 1 + 4 + PREVIEW_MARGIN;
// 启动器通用窗口下限；具体 kind 的下限从 studio_state 的 windowMinSizes 取
const PREVIEW_MIN_WIDTH = 640;
const PREVIEW_MIN_HEIGHT = 480;
const PREVIEW_MAX_WIDTH = 4096;
const PREVIEW_MAX_HEIGHT = 4096;
const previewMin = computed(() => minSizes.value[previewKind.value] ?? { width: PREVIEW_MIN_WIDTH, height: PREVIEW_MIN_HEIGHT });
const PREVIEW_HANDLES = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'] as const;
type PreviewHandle = (typeof PREVIEW_HANDLES)[number];
let resizeStart: { handle: PreviewHandle; x: number; y: number; left: number; top: number; width: number; height: number } | undefined;
const frameStyle = computed(() => ({
  left: `${previewPos.value.x - PREVIEW_GAP - 1}px`,
  top: `${previewPos.value.y - PREVIEW_GAP - 1}px`,
  width: `${previewSize.value.width + PREVIEW_GAP * 2 + 2}px`,
  height: `${previewSize.value.height + PREVIEW_GAP * 2 + 2}px`,
}));
const canvasStyle = computed(() => ({
  width: `${Math.max(
    surfaceExtent.value.width,
    previewPos.value.x + previewSize.value.width + PREVIEW_INSET,
  )}px`,
  height: `${Math.max(
    surfaceExtent.value.height,
    previewPos.value.y + previewSize.value.height + PREVIEW_INSET,
  )}px`,
}));
const previewSrc = computed(() => {
  if (!previewUrl.value) return '';
  const url = new URL(previewUrl.value);
  url.searchParams.set('window', previewKind.value);
  url.searchParams.set('theme', previewTheme.value);
  return url.href;
});
// 两个 HTML 模板在预览里都是独立页面，没有窗口 / 配色参数可切
const isStaticPreview = computed(() => project.value?.template === 'html' || project.value?.template === 'home-html');
const outputHost = ref<HTMLPreElement>();
const env = ref('');
const studioTheme = ref<Theme>(document.documentElement.dataset.theme === 'Light' ? 'Light' : 'Dark');
// 窗口清单与最小尺寸都来自 studio_state；清单顺序由后端按启动器注册表给出
const kinds = ref<string[]>([]);
let observer: ResizeObserver;
let unsubscribe: (() => void) | undefined;

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
// 页面 / 主页主题的样式来自 Vite 打包，首次往 custom.css 追加时要确保入口真的引用了它
async function ensureCustomCssImport() {
  const template = project.value?.template;
  const entry = template === 'page' ? 'src/replace.ts' : template === 'home' ? 'src/entry.ts' : '';
  if (!entry || !files.value.includes(entry)) return;
  // 页面主题没有 vite-env.d.ts，直接 import CSS 会让 vue-tsc 找不到模块
  try {
    await call('read', 'src/vite-env.d.ts');
  } catch {
    await call('save', {
      path: 'src/vite-env.d.ts',
      text: '/// <reference types="vite/client" />\n',
      expectedHash: null,
    });
  }
  const source = await call('read', entry);
  if (source.text.includes('custom.css')) return;
  const newline = source.text.includes('\r\n') ? '\r\n' : '\n';
  await call('save', {
    path: entry,
    text: `import "../custom.css";${newline}${source.text}`,
    expectedHash: source.hash,
  });
}
async function appendToCustomCss(snippet: string) {
  if (!project.value) return;
  const switching = active.value?.path !== 'custom.css';
  if (switching && dirty.value) {
    if (!await confirmDiscard()) return;
    active.value = null;
    draft.value = '';
  }
  let base: SourceFile | null = null;
  if (!switching && active.value) {
    base = { ...active.value, text: draft.value };
  } else if (files.value.includes('custom.css')) {
    base = await call('read', 'custom.css');
  }
  const newline = base?.text.includes('\r\n') ? '\r\n' : '\n';
  const body = snippet.trim().replace(/\r?\n/g, newline);
  const existing = (base?.text ?? '').replace(/[ \t\r\n]+$/, '');
  const text = existing ? `${existing}${newline}${newline}${body}${newline}` : `${body}${newline}`;
  const file = await call('save', { path: 'custom.css', text, expectedHash: base?.hash ?? null });
  if (!files.value.includes('custom.css')) await refresh();
  await ensureCustomCssImport();
  await refresh();
  if (switching) await loadFile('custom.css');
  else { active.value = file; draft.value = file.text; }
}
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
function clampRange(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}
function surfaceSize() {
  const surface = previewSurface.value;
  return {
    width: Math.max(1, surface?.clientWidth || surfaceExtent.value.width || 0),
    height: Math.max(1, surface?.clientHeight || surfaceExtent.value.height || 0),
  };
}
function clampPreviewSize() {
  const min = previewMin.value;
  const width = clampRange(Math.round(Number(previewSize.value.width) || min.width), min.width, PREVIEW_MAX_WIDTH);
  const height = clampRange(Math.round(Number(previewSize.value.height) || min.height), min.height, PREVIEW_MAX_HEIGHT);
  const x = Math.max(PREVIEW_INSET, Math.round(Number(previewPos.value.x) || PREVIEW_INSET));
  const y = Math.max(PREVIEW_INSET, Math.round(Number(previewPos.value.y) || PREVIEW_INSET));
  if (width !== previewSize.value.width || height !== previewSize.value.height) previewSize.value = { width, height };
  if (x !== previewPos.value.x || y !== previewPos.value.y) previewPos.value = { x, y };
}
/** 把预览窗口摆到预览区中间；放不下时从左上角排，超出部分由画布滚动 */
function placePreview() {
  const surface = surfaceSize();
  previewPos.value = {
    x: Math.max(PREVIEW_INSET, Math.round((surface.width - previewSize.value.width) / 2)),
    y: Math.max(PREVIEW_INSET, Math.round((surface.height - previewSize.value.height) / 2)),
  };
}
/** 默认按当前窗口的最小客户区尺寸显示，不铺满预览区 */
function resetPreviewSize() {
  const min = previewMin.value;
  previewSize.value = { width: min.width, height: min.height };
  clampPreviewSize();
  placePreview();
}
function fitPreview() {
  const surface = surfaceSize();
  const min = previewMin.value;
  const width = Math.min(PREVIEW_MAX_WIDTH, Math.max(min.width, surface.width - PREVIEW_INSET * 2));
  const height = Math.min(PREVIEW_MAX_HEIGHT, Math.max(min.height, surface.height - PREVIEW_INSET * 2));
  previewSize.value = { width, height };
  previewPos.value = {
    x: Math.max(PREVIEW_INSET, Math.round((surface.width - width) / 2)),
    y: Math.max(PREVIEW_INSET, Math.round((surface.height - height) / 2)),
  };
}
function startResize(event: PointerEvent, handle: PreviewHandle) {
  if (!previewUrl.value) return;
  event.preventDefault();
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  resizeStart = {
    handle,
    x: event.clientX,
    y: event.clientY,
    left: previewPos.value.x,
    top: previewPos.value.y,
    width: previewSize.value.width,
    height: previewSize.value.height,
  };
}
function moveResize(event: PointerEvent) {
  if (!resizeStart) return;
  const dx = event.clientX - resizeStart.x;
  const dy = event.clientY - resizeStart.y;
  const minWidth = previewMin.value.width;
  const minHeight = previewMin.value.height;
  let left = resizeStart.left;
  let top = resizeStart.top;
  let right = resizeStart.left + resizeStart.width;
  let bottom = resizeStart.top + resizeStart.height;
  if (resizeStart.handle.includes('w')) {
    left = clampRange(left + dx, Math.max(PREVIEW_INSET, right - PREVIEW_MAX_WIDTH), right - minWidth);
  }
  if (resizeStart.handle.includes('e')) {
    right = clampRange(right + dx, left + minWidth, left + PREVIEW_MAX_WIDTH);
  }
  if (resizeStart.handle.includes('n')) {
    top = clampRange(top + dy, Math.max(PREVIEW_INSET, bottom - PREVIEW_MAX_HEIGHT), bottom - minHeight);
  }
  if (resizeStart.handle.includes('s')) {
    bottom = clampRange(bottom + dy, top + minHeight, top + PREVIEW_MAX_HEIGHT);
  }
  previewPos.value = { x: Math.round(left), y: Math.round(top) };
  previewSize.value = { width: Math.round(right - left), height: Math.round(bottom - top) };
}
function endResize(event: PointerEvent) {
  if (!resizeStart) return;
  resizeStart = undefined;
  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
}
function reloadPreview() {
  previewKey.value += 1;
}

/** 把检查协议消息发给预览页的注入脚本 */
function postInspect(message: Record<string, unknown>) {
  // path 等字段可能来自响应式数据，代理对象过不了 postMessage 的结构化克隆
  const payload = JSON.parse(JSON.stringify(message));
  previewFrame.value?.contentWindow?.postMessage({ __mmlInspector: INSPECT_PROTOCOL, ...payload }, '*');
}

/** 打开或收起检查模式；打开时把底部面板切到元素页 */
function setInspecting(value: boolean) {
  inspecting.value = value;
  if (value) {
    panelOpen.value = true;
    panelCollapsed.value = false;
    panelTab.value = 'inspector';
  } else if (panelTab.value === 'inspector') {
    panelTab.value = 'output';
  }
  postInspect({ type: value ? 'enable' : 'disable' });
}

function toggleInspector() {
  if (!previewUrl.value) return;
  setInspecting(!inspecting.value);
}

/** 活动栏 / 面板的「元素检查」入口：打开检查并把底部面板切过去 */
function openInspector() {
  if (!previewUrl.value) return;
  if (!inspecting.value) {
    setInspecting(true);
    return;
  }
  panelOpen.value = true;
  panelCollapsed.value = false;
  panelTab.value = 'inspector';
}

function showOutput() {
  panelTab.value = 'output';
  panelCollapsed.value = false;
}

/** 活动栏的面板按钮：收起 / 展开底部面板 */
function togglePanel() {
  panelOpen.value = !panelOpen.value;
  if (panelOpen.value) panelCollapsed.value = false;
}

function stopInspector() {
  if (!inspecting.value) return;
  setInspecting(false);
}

/** 预览重载后页面里的脚本状态没了：面板重建、检查模式重新打开 */
function onPreviewLoad() {
  previewEpoch.value += 1;
  if (inspecting.value) postInspect({ type: 'enable' });
}
async function updateStudioTheme(event: Event) {
  const select = event.target as HTMLSelectElement;
  try { studioTheme.value = await call('theme', select.value as Theme); }
  finally { select.value = studioTheme.value; }
}
function keydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') { event.preventDefault(); void save(); }
  // F12 / Ctrl+Shift+I 归预览的元素检查：编辑器里的 F12 留给 Monaco
  const inspect = event.key === 'F12' || ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 'i');
  if (inspect && previewUrl.value) {
    const focus = document.activeElement as HTMLElement | null;
    if (!focus?.closest('.editor')) {
      event.preventDefault();
      toggleInspector();
      return;
    }
  }
  if (event.key === 'Escape' && !working.value) {
    newProject.value = false;
    newFile.value = false;
    if (confirming.value) answer(false);
    if (inspecting.value) stopInspector();
  }
}
// 预览页里的检查脚本把选中元素回传到这里；F12 也由它转发成 toggle
function previewMessage(event: MessageEvent) {
  if (!previewUrl.value) return;
  if (event.source !== previewFrame.value?.contentWindow) return;
  const data = event.data as { __mmlInspector?: unknown; type?: unknown } | null;
  if (!data || typeof data !== 'object' || data.__mmlInspector !== INSPECT_PROTOCOL) return;
  if (data.type === 'toggle') { toggleInspector(); return; }
  if (data.type === 'exit') { setInspecting(false); return; }
  if (data.type === 'pick' || data.type === 'node' || data.type === 'tree' || data.type === 'inline') {
    inspectorPanel.value?.handle(data);
  }
}

watch(dirty, (value) => { void call('dirty', value).catch(showError); });
watch(studioTheme, (value) => { document.documentElement.dataset.theme = value; }, { immediate: true });
watch(previewUrl, async (value) => {
  // 预览停了就收起检查面板；新地址由 iframe 的 load 事件重新打开检查
  if (!value) stopInspector();
  // 等尺寸行和画框渲染完再量预览区，否则高度会把尺寸行算漏
  await nextTick();
  if (previewUrl.value) placePreview();
});
// 换窗口种类或首次拿到最小尺寸时，按该窗口的下限显示，不自动铺满
watch([previewKind, minSizes], () => { resetPreviewSize(); }, { deep: true });
// 布局变化只重新居中，不改变用户调过的窗口大小
watch([sidebarOpen, previewOpen, panelOpen, panelCollapsed], async () => {
  await nextTick();
  if (previewUrl.value) placePreview();
});
watch(output, () => { void nextTick(() => { if (outputHost.value) outputHost.value.scrollTop = outputHost.value.scrollHeight; }); });
onMounted(async () => {
  try {
    unsubscribe = await onEvent((event) => {
      if (event.type === 'log') output.value = (output.value + event.text).slice(-150000);
      if (event.type === 'busy') busy.value = event.value;
      if (event.type === 'step') step.value = event.text;
      if (event.type === 'preview') previewUrl.value = event.url;
    });
  } catch (value) { showError(value); }
  observer = new ResizeObserver(() => {
    const surface = previewSurface.value;
    if (surface) surfaceExtent.value = { width: surface.clientWidth, height: surface.clientHeight };
  });
  if (previewSurface.value) {
    observer.observe(previewSurface.value);
    surfaceExtent.value = { width: previewSurface.value.clientWidth, height: previewSurface.value.clientHeight };
  }
  window.addEventListener('keydown', keydown);
  window.addEventListener('message', previewMessage);
  try {
    const state = await bootstrap();
    locale.value = state.locale;
    studioTheme.value = state.theme;
    recent.value = state.recent;
    minSizes.value = state.windowMinSizes ?? {};
    kinds.value = state.windowKinds?.length ? state.windowKinds : Object.keys(minSizes.value).sort();
    env.value = state.environment.error || `Node.js ${state.environment.nodeVersion} · ${state.environment.git}`;
    if (state.project) await loadProject(state.project);
  } catch (value) { showError(value); }
});
onBeforeUnmount(() => { window.clearInterval(elapsedTimer); observer?.disconnect(); unsubscribe?.(); window.removeEventListener('keydown', keydown); window.removeEventListener('message', previewMessage); confirmResult?.(false); });
</script>

<template>
  <div class="studio-shell">
    <header class="titlebar">
      <div class="title-brand">
        <span class="brand-mark">M²L</span>
        <div class="title-text"><h1>{{ t('title') }}</h1><small>{{ project?.name || t('empty') }}</small></div>
      </div>
      <nav class="title-actions" :aria-label="t('title')">
        <button class="title-icon" :disabled="working" :title="t('new')" :aria-label="t('new')" @click="run(() => showNew('project'))"><GlyphIcon name="plus" :size="16" /></button>
        <button class="title-icon" :disabled="working" :title="t('open')" :aria-label="t('open')" @click="run(() => openProject())"><GlyphIcon name="folder" :size="16" /></button>
        <button class="title-icon" :disabled="!dirty || working" :title="t('save')" :aria-label="t('save')" @click="save"><GlyphIcon name="save" :size="16" /></button>
        <button class="title-icon" :disabled="!project || working" :title="t('install')" :aria-label="t('install')" @click="run(async () => { await call('install'); await loadCatalog(); })"><GlyphIcon name="download" :size="16" /></button>
        <button class="title-icon" :disabled="!project || working" :title="t('build')" :aria-label="t('build')" @click="run(build)"><GlyphIcon name="export" :size="16" /></button>
        <button class="title-icon" :disabled="!project || working" :title="t('openEditorHint')" :aria-label="t('openEditor')" @click="run(() => call('openEditor'))"><GlyphIcon name="wrench" :size="16" /></button>
      </nav>
      <div class="title-right">
        <label class="theme-control" for="studio-theme">
          <span>{{ t('studioTheme') }}</span>
          <select id="studio-theme" :value="studioTheme" class="field-select" :disabled="working" @change="run(() => updateStudioTheme($event))"><option value="Light">{{ t('light') }}</option><option value="Dark">{{ t('dark') }}</option></select>
        </label>
        <select v-model="locale" class="field-select language-select" :aria-label="t('language')" :disabled="working" @change="call('locale', locale).catch(showError)"><option value="zh-CN">中文</option><option value="en-US">English</option></select>
      </div>
    </header>
    <div v-if="error" class="error-banner" role="alert"><span>{{ error }}</span><button class="ui-btn v-ghost s-sm" :aria-label="t('close')" @click="error = ''">×</button></div>

    <div class="workbench">
      <nav class="activity-bar" :aria-label="t('title')">
        <button class="activity-btn" :class="{ 'is-active': sidebarOpen }" :title="t('toggleFiles')" :aria-label="t('toggleFiles')" @click="sidebarOpen = !sidebarOpen"><GlyphIcon name="document" :size="18" /></button>
        <button class="activity-btn" :class="{ 'is-active': previewOpen }" :title="t('preview')" :aria-label="t('preview')" @click="previewOpen = !previewOpen"><GlyphIcon name="grid" :size="18" /></button>
        <button class="activity-btn" :class="{ 'is-active': inspecting }" :disabled="!previewUrl" :title="t('inspect')" :aria-label="t('inspect')" @click="inspecting ? stopInspector() : openInspector()"><GlyphIcon name="bug" :size="18" /></button>
        <button class="activity-btn" :class="{ 'is-active': panelOpen && !panelCollapsed }" :title="t('output')" :aria-label="t('output')" @click="togglePanel"><GlyphIcon name="terminal" :size="18" /></button>
      </nav>

      <aside v-if="sidebarOpen" class="sidebar">
        <div class="panel-head"><strong>{{ t('files') }}</strong><span class="grow" /><button class="title-icon" :disabled="!project || working" :title="t('newFile')" :aria-label="t('newFile')" @click="run(() => showNew('file'))"><GlyphIcon name="plus" :size="14" /></button><button class="title-icon" :disabled="!project || working" :title="t('refreshFiles')" :aria-label="t('refreshFiles')" @click="run(refresh)"><GlyphIcon name="refresh" :size="14" /></button></div>
        <div class="file-list" :aria-label="t('files')">
          <FileTree v-if="project" :key="project.root" :files="files" :active-path="active?.path ?? ''" :disabled="working" @select="run(() => loadFile($event))" />
          <template v-if="!project"><p class="muted">{{ t('empty') }}</p><p v-if="recent.length" class="muted">{{ t('recent') }}</p><button v-for="path in recent" :key="path" class="recent-entry ui-btn v-ghost s-sm" :title="path" :disabled="working" @click="run(() => openProject(path))">{{ path }}</button></template>
        </div>
        <CssCatalog v-if="catalog.controls.length || catalog.variables.length" :catalog="catalog" :enabled="!!project && !working" @insert="run(() => appendToCustomCss($event))" />
      </aside>

      <main class="editor">
        <div class="editor-tabs">
          <div class="editor-tab" :class="{ 'is-empty': !active }">
            <GlyphIcon name="document" :size="12" />
            <span class="editor-tab-name">{{ active?.path || t('chooseFile') }}</span>
            <span v-if="dirty" class="dirty-mark" :title="t('unsaved')">●</span>
          </div>
          <span class="grow" />
          <button class="title-icon" :disabled="!active || working" :title="t('reopenFile')" :aria-label="t('reopenFile')" @click="run(() => loadFile(active!.path))"><GlyphIcon name="refresh" :size="14" /></button>
        </div>
        <EditorPane v-if="active" v-model="draft" :path="project!.root + '/' + active.path" :read-only="working" :theme="studioTheme" @save="save" />
        <div v-else class="empty-pane"><span class="empty-symbol">{ }</span><p>{{ t('chooseFile') }}</p><p class="muted">CSS · Vue · HTML · TypeScript</p></div>
      </main>

      <aside v-if="previewOpen" class="preview-panel">
        <div class="panel-head">
          <strong>{{ t('contentPreview') }}</strong>
          <span class="grow" />
          <button class="title-icon" :disabled="!project || working" :title="t('preview')" :aria-label="t('preview')" @click="run(startPreview)"><GlyphIcon name="play" :size="14" /></button>
          <button class="title-icon" :disabled="!previewUrl || working" :title="t('stop')" :aria-label="t('stop')" @click="run(() => call('stop'))"><GlyphIcon name="stop" :size="14" /></button>
          <button class="title-icon" :disabled="!previewUrl" :title="t('reload')" :aria-label="t('reload')" @click="reloadPreview"><GlyphIcon name="refresh" :size="14" /></button>
          <button class="title-icon" :class="{ 'is-active': inspecting }" :disabled="!previewUrl" :title="t('inspect')" :aria-label="t('inspect')" @click="inspecting ? stopInspector() : openInspector()"><GlyphIcon name="bug" :size="14" /></button>
        </div>
        <div class="preview-tools">
          <select v-model="previewKind" class="field-select" :aria-label="t('window')" :disabled="!previewUrl || isStaticPreview"><option v-for="kind in kinds" :key="kind">{{ kind }}</option></select>
          <select v-model="previewTheme" class="field-select" :aria-label="t('theme')" :disabled="!previewUrl || isStaticPreview"><option value="Dark">{{ t('dark') }}</option><option value="Light">{{ t('light') }}</option></select>
        </div>
        <div v-if="previewUrl" class="preview-size">
          <span>{{ t('windowSize') }}</span>
          <input v-model.lazy.number="previewSize.width" class="field-input" type="number" :min="previewMin.width" :max="PREVIEW_MAX_WIDTH" :aria-label="t('windowSize')" @change="clampPreviewSize" />
          <span>×</span>
          <input v-model.lazy.number="previewSize.height" class="field-input" type="number" :min="previewMin.height" :max="PREVIEW_MAX_HEIGHT" :aria-label="t('windowSize')" @change="clampPreviewSize" />
          <span class="grow" />
          <button class="ui-btn v-ghost s-sm" @click="fitPreview">{{ t('fitPreview') }}</button>
        </div>
        <div class="preview-host">
          <div ref="previewSurface" class="preview-surface">
            <div v-if="previewUrl" class="preview-canvas" :style="canvasStyle">
              <div class="preview-frame" :style="frameStyle">
                <div class="preview-frame-body">
                  <iframe ref="previewFrame" :key="previewKey" class="preview-iframe" :src="previewSrc" :title="t('contentPreview')" @load="onPreviewLoad" />
                </div>
                <span v-for="handle in PREVIEW_HANDLES" :key="handle" class="preview-handle" :class="`is-${handle}`" :title="t('resizeWindow')" @pointerdown="startResize($event, handle)" @pointermove="moveResize" @pointerup="endResize" @pointercancel="endResize" />
              </div>
            </div>
            <div v-else class="empty-pane"><span class="empty-symbol">▷</span><p>{{ t('previewEmpty') }}</p></div>
          </div>
        </div>
        <p class="preview-hint">{{ t('previewHint') }}</p>
      </aside>
    </div>

    <section v-if="panelOpen" class="bottom-panel" :class="{ 'is-collapsed': panelCollapsed }">
      <div class="panel-bar">
        <button class="panel-tab" :class="{ 'is-active': panelTab === 'output' }" @click="showOutput">{{ t('output') }}</button>
        <button class="panel-tab" :class="{ 'is-active': panelTab === 'inspector' }" :disabled="!previewUrl" @click="openInspector">{{ t('inspector') }}</button>
        <span class="grow" />
        <span v-if="panelTab === 'output' && !panelCollapsed" class="panel-meta">{{ env }}</span>
        <button class="title-icon" :title="panelCollapsed ? t('expandOutput') : t('collapseOutput')" :aria-label="panelCollapsed ? t('expandOutput') : t('collapseOutput')" @click="panelCollapsed = !panelCollapsed"><GlyphIcon name="chevron-down" :size="14" :class="{ 'is-flipped': panelCollapsed }" /></button>
      </div>
      <div v-if="!panelCollapsed" class="panel-body">
        <pre v-if="panelTab === 'output'" ref="outputHost" class="output-log" aria-live="off">{{ output || t('ready') }}</pre>
        <InspectorPanel v-else ref="inspectorPanel" :key="previewEpoch" :send="postInspect" embedded @close="stopInspector" />
      </div>
    </section>

    <footer class="statusbar">
      <button class="status-link" :disabled="!project" :title="t('reveal')" @click="run(() => call('reveal'))">{{ project?.root || t('empty') }}</button>
      <span v-if="project?.revision">{{ t('revision') }} {{ project.revision.slice(0, 12) }}</span>
      <span class="grow" />
      <span class="env-text">{{ env }}</span>
      <span class="status-state" :class="{ 'is-busy': working }">{{ statusText }}</span>
      <button v-if="busy" class="status-link" @click="call('cancel').catch(showError)">{{ t('cancel') }}</button>
    </footer>
    <ProjectDialog v-if="newProject" :busy="working" :error="error" @close="newProject = false" @cancel="call('cancel').catch(showError)" @create="run(() => create($event))" @error="showError" />
    <div v-if="newFile" class="modal-shade"><form class="modal" role="dialog" aria-modal="true" :aria-label="t('newFile')" @submit.prevent="run(createFile)"><h2>{{ t('newFile') }}</h2><label class="field-label" for="file-path">{{ t('path') }}</label><input id="file-path" v-model="fileName" class="field-input" required autofocus :disabled="working" /><p v-if="error" class="form-error" role="alert">{{ error }}</p><div class="modal-actions"><button type="button" class="ui-btn v-plain s-sm" :disabled="working" @click="newFile = false">{{ t('cancel') }}</button><button class="ui-btn v-primary s-sm" :disabled="working || !fileName">{{ t('newFile') }}</button></div></form></div>
    <div v-if="confirming" class="modal-shade"><div class="modal" role="alertdialog" aria-modal="true" :aria-label="t('unsaved')"><h2>{{ t('unsaved') }}</h2><p>{{ t('confirmDiscard') }}</p><div class="modal-actions"><button class="ui-btn v-plain s-sm" autofocus @click="answer(false)">{{ t('cancel') }}</button><button class="ui-btn v-danger s-sm" @click="answer(true)">{{ t('discard') }}</button></div></div></div>
  </div>
</template>
