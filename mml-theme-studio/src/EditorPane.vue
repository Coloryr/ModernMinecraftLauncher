<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { Theme } from './api';
import * as monaco from 'monaco-editor/editor/editor.api';
import EditorWorker from 'monaco-editor/editor/editor.worker.js?worker';
import JsonWorker from 'monaco-editor/language/json/json.worker.js?worker';
import 'monaco-editor/language/json/monaco.contribution.js';
import 'monaco-editor/features/find/register.js';
import 'monaco-editor/features/codicon/register.js';
import 'monaco-editor/editor/contrib/find/browser/findController.js';
import 'monaco-editor/editor/contrib/clipboard/browser/clipboard.js';
import 'monaco-editor/editor/contrib/format/browser/formatActions.js';
import 'monaco-editor/languages/definitions/javascript/register.js';
import 'monaco-editor/languages/definitions/typescript/register.js';
import 'monaco-editor/languages/definitions/html/register.js';
import 'monaco-editor/languages/definitions/css/register.js';
import 'monaco-editor/languages/definitions/markdown/register.js';

const props = defineProps<{ path: string; modelValue: string; readOnly: boolean; theme: Theme }>();
const emit = defineEmits<{ 'update:modelValue': [value: string]; save: [] }>();
const host = ref<HTMLDivElement>();
let editor: monaco.editor.IStandaloneCodeEditor | undefined;
let listener: monaco.IDisposable | undefined;
let syncing = false;
self.MonacoEnvironment = { getWorker: (_id, label) => label === 'json' ? new JsonWorker() : new EditorWorker() };

function language(path: string) {
  const extension = path.split('.').at(-1);
  return ({ vue: 'html', html: 'html', css: 'css', ts: 'typescript', js: 'javascript', mjs: 'javascript', cjs: 'javascript', json: 'json', md: 'markdown', svg: 'xml' } as Record<string, string>)[extension || ''] || 'plaintext';
}

function replaceModel() {
  if (!editor) return;
  syncing = true;
  const old = editor.getModel();
  editor.setModel(monaco.editor.createModel(props.modelValue, language(props.path)));
  old?.dispose();
  syncing = false;
}

onMounted(() => {
  editor = monaco.editor.create(host.value!, {
    automaticLayout: true, theme: props.theme === 'Light' ? 'vs' : 'vs-dark', fontSize: 13, tabSize: 2,
    minimap: { enabled: false }, scrollBeyondLastLine: false, wordWrap: 'on',
    readOnly: props.readOnly, accessibilitySupport: 'auto',
  });
  replaceModel();
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
onBeforeUnmount(() => { listener?.dispose(); editor?.getModel()?.dispose(); editor?.dispose(); });

defineExpose({ insert(value: string) {
  const selection = editor?.getSelection();
  if (!editor || !selection || props.readOnly) return;
  editor.executeEdits('catalog', [{ range: selection, text: value }]);
  editor.focus();
} });
</script>

<template><div ref="host" class="editor-host" /></template>
