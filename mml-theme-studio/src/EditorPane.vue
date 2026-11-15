<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { Catalog, Theme } from './api';
import { registerCompletions } from './completions';
import * as monaco from 'monaco-editor/editor/editor.api';
import EditorWorker from 'monaco-editor/editor/editor.worker.js?worker';
import JsonWorker from 'monaco-editor/language/json/json.worker.js?worker';
import CssWorker from 'monaco-editor/language/css/css.worker.js?worker';
import HtmlWorker from 'monaco-editor/language/html/html.worker.js?worker';
import TsWorker from 'monaco-editor/language/typescript/ts.worker.js?worker';
import 'monaco-editor/language/json/monaco.contribution.js';
import 'monaco-editor/language/css/monaco.contribution.js';
import 'monaco-editor/language/html/monaco.contribution.js';
import { ModuleKind, ScriptTarget, typescriptDefaults } from 'monaco-editor/languages/features/typescript/register.js';
import 'monaco-editor/features/find/register.js';
import 'monaco-editor/features/codicon/register.js';
import 'monaco-editor/features/hover/register.js';
import 'monaco-editor/features/parameterHints/register.js';
import 'monaco-editor/features/snippet/register.js';
import 'monaco-editor/features/colorPicker/register.js';
import 'monaco-editor/features/comment/register.js';
import 'monaco-editor/editor/contrib/suggest/browser/suggestController.js';
import 'monaco-editor/editor/contrib/find/browser/findController.js';
import 'monaco-editor/editor/contrib/clipboard/browser/clipboard.js';
import 'monaco-editor/editor/contrib/format/browser/formatActions.js';
import 'monaco-editor/languages/definitions/javascript/register.js';
import 'monaco-editor/languages/definitions/typescript/register.js';
import 'monaco-editor/languages/definitions/html/register.js';
import 'monaco-editor/languages/definitions/css/register.js';
import 'monaco-editor/languages/definitions/markdown/register.js';

const props = withDefaults(defineProps<{ path: string; modelValue: string; readOnly: boolean; theme: Theme; catalog?: Catalog }>(), {
  catalog: () => ({ controls: [], variables: [] }),
});
const emit = defineEmits<{ 'update:modelValue': [value: string]; save: [] }>();
const host = ref<HTMLDivElement>();
let editor: monaco.editor.IStandaloneCodeEditor | undefined;
let listener: monaco.IDisposable | undefined;
let completions: monaco.IDisposable | undefined;
let syncing = false;
// 每个语言服务都在自己的 worker 里跑，label 是 Monaco 传下来的语言 id
self.MonacoEnvironment = {
  getWorker: (_id: string, label: string) => {
    if (label === 'json') return new JsonWorker();
    if (label === 'css' || label === 'scss' || label === 'less') return new CssWorker();
    if (label === 'html' || label === 'handlebars' || label === 'razor') return new HtmlWorker();
    if (label === 'typescript' || label === 'javascript') return new TsWorker();
    return new EditorWorker();
  },
};
// 主题项目的 import 指向仓库外的包，Monaco 解析不到；只保留语法诊断，避免整屏假报错
typescriptDefaults.setCompilerOptions({ target: ScriptTarget.ESNext, module: ModuleKind.ESNext, allowNonTsExtensions: true });
typescriptDefaults.setDiagnosticsOptions({ noSemanticValidation: true, noSyntaxValidation: false });

function language(path: string) {
  const extension = path.split('.').at(-1);
  return ({ vue: 'html', html: 'html', css: 'css', ts: 'typescript', js: 'javascript', mjs: 'javascript', cjs: 'javascript', json: 'json', md: 'markdown', svg: 'xml' } as Record<string, string>)[extension || ''] || 'plaintext';
}

function replaceModel() {
  if (!editor) return;
  syncing = true;
  const old = editor.getModel();
  editor.setModel(monaco.editor.createModel(props.modelValue, language(props.path), monaco.Uri.file(props.path)));
  old?.dispose();
  syncing = false;
}

onMounted(() => {
  editor = monaco.editor.create(host.value!, {
    automaticLayout: true, theme: props.theme === 'Light' ? 'vs' : 'vs-dark', fontSize: 13, tabSize: 2,
    minimap: { enabled: false }, scrollBeyondLastLine: false, wordWrap: 'on',
    readOnly: props.readOnly, accessibilitySupport: 'auto',
    // 编辑器嵌在会裁切的面板里，补全 / 悬停浮层挂到 body 才不会被切掉
    fixedOverflowWidgets: true,
    quickSuggestions: { other: true, comments: false, strings: true },
    suggestOnTriggerCharacters: true, acceptSuggestionOnEnter: 'on', tabCompletion: 'on',
    wordBasedSuggestions: 'currentDocument', suggest: { showWords: true, preview: true },
  });
  replaceModel();
  completions = registerCompletions(() => props.catalog);
  listener = editor.onDidChangeModelContent(() => { if (!syncing) emit('update:modelValue', editor!.getValue()); });
  editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => emit('save'));
});
watch(() => props.path, replaceModel);
watch(() => props.modelValue, (value) => {
  if (!editor || editor.getValue() === value) return;
  syncing = true;
  editor.setValue(value);
  syncing = false;
});
watch(() => props.readOnly, (value) => editor?.updateOptions({ readOnly: value }));
watch(() => props.theme, (value) => monaco.editor.setTheme(value === 'Light' ? 'vs' : 'vs-dark'));
onBeforeUnmount(() => { completions?.dispose(); listener?.dispose(); editor?.getModel()?.dispose(); editor?.dispose(); });
</script>

<template><div ref="host" class="editor-host" /></template>
