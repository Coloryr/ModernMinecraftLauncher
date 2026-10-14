<script setup lang="ts">
// 可编辑的 Monaco 文本框：行号 + 按语言的着色 + 括号配对与折叠
//
// 配置窗口的文本视图用它；与日志视图共用同一套 Monaco 环境（worker / 主题在 lib/monaco、
// 语言包在 lib/monacoCode）与同一份自动换行偏好（lib/editorWrap，Alt+Z 切）。**不要**在这里
// import monaco-editor 的其它入口，那会把整套语言包（含各类语言服务与 worker）拉进包里。
import { onActivated, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor/editor/editor.api";
import { applyCodeTheme, monaco } from "../../lib/monacoCode";
import { toggleWordWrap, wordWrap } from "../../lib/editorWrap";

const props = withDefaults(
  defineProps<{
    /** 文本内容，配合 `v-model` 使用 */
    modelValue: string;
    /** Monaco 语言 id；不认识的一律按纯文本渲染 */
    language?: string;
    /** 只读；只读时不给编辑但没有别处差异 */
    readonly?: boolean;
  }>(),
  { language: "plaintext", readonly: false },
);

const emit = defineEmits<{
  /** `v-model` 更新：编辑器里的全文 */
  (e: "update:modelValue", v: string): void;
}>();

const hostEl = ref<HTMLElement | null>(null);
let ed: editor.IStandaloneCodeEditor | null = null;

// 主题落在 <html data-theme> 上，盯着这个属性比自己监听 theme ref 全面：
// 设置窗口改主题、其它窗口同步过来的改动、跟随系统时系统切换，最终都只是改这个属性
const themeWatcher = new MutationObserver(() => applyCodeTheme());

onMounted(() => {
  if (!hostEl.value) return;
  applyCodeTheme();
  ed = monaco.editor.create(hostEl.value, {
    value: props.modelValue,
    language: props.language,
    readOnly: props.readonly,
    automaticLayout: true,
    minimap: { enabled: false },
    // 行号常开：这个视图就是拿来看 / 改配置文件的
    lineNumbers: "on",
    glyphMargin: false,
    folding: true,
    scrollBeyondLastLine: false,
    fontFamily: '"Cascadia Code", Consolas, "Courier New", monospace',
    fontSize: 12.5,
    lineHeight: 1.65 * 12.5,
    tabSize: 2,
    // 自动换行是全局偏好：这里只是把它喂给编辑器，Alt+Z 改的也是它
    wordWrap: wordWrap.value ? "on" : "off",
    // 选中区域的四个角不切圆：整块界面都是方的，选区的圆角反而显眼
    roundedSelection: false,
    // 右键菜单关掉：壳层禁用了浏览器默认菜单，Monaco 自己那套又和主题不一致
    contextmenu: false,
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
    unicodeHighlight: { ambiguousCharacters: false, invisibleCharacters: false },
  });
  ed.onDidChangeModelContent(() => {
    emit("update:modelValue", ed?.getValue() ?? "");
  });
  // Alt+Z：与 VS Code 同键位，写的是全局偏好，日志视图与其它窗口跟着变
  ed.addCommand(monaco.KeyMod.Alt | monaco.KeyCode.KeyZ, toggleWordWrap);
  themeWatcher.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
});

// Monaco 的主题是全局的：单窗口模式下日志视图可能也还挂着，切回本窗口时把自己的那套刷回来
onActivated(applyCodeTheme);

onBeforeUnmount(() => {
  themeWatcher.disconnect();
  ed?.dispose();
  ed = null;
});

// 外部换了内容（切文件 / 读盘回来）：整篇替换。
// 内容一样就不动 —— 打字时父组件也会把同一个值传回来，setValue 会把光标顶到开头
watch(
  () => props.modelValue,
  (now) => {
    if (!ed || ed.getValue() === now) return;
    ed.setValue(now);
  },
);

// 换扩展名（切到另一种文件）：只改模型语言，不动内容，撤销栈也就还在
watch(
  () => props.language,
  (lang) => {
    const model = ed?.getModel();
    if (model) monaco.editor.setModelLanguage(model, lang);
  },
);

// 别处翻转了换行（日志视图那个开关、另一个窗口的 Alt+Z）：跟上
watch(wordWrap, (on) => {
  ed?.updateOptions({ wordWrap: on ? "on" : "off" });
});
</script>

<template>
  <div ref="hostEl" class="monaco-text"></div>
</template>

<style scoped>
/* 方正的矩形：不切圆角、聚焦也不给强调色描边（与官网示例那块编辑器、以及日志区一致）。
   聚焦反馈交给里面闪烁的光标，别再画一圈蓝框 */
.monaco-text {
  flex: 1;
  min-width: 0;
  min-height: 0;
  border: 1px solid var(--border);
  overflow: hidden;
}
</style>
