<script setup lang="ts">
// Monaco 只读日志视图：把给它的日志行画成控制台，管自动换行 / 自动下拉 / 页内查找
//
// **筛选不在这里**：等级 / 线程 / 分组都归日志窗口的工具栏管，筛完的行才传进来，
// 免得同一套筛选条件在两边各存一份、还要互相同步。
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor/editor/editor.api";
import { applyLogTheme, monaco } from "../lib/monacoLog";
import type { LogLine } from "../lib/bindings";

const props = defineProps<{
  /** 日志行，按时间顺序；调用方已按等级 / 线程 / 分组筛过 */
  logs: LogLine[];
  /** 撑满父容器；独立窗口与浮动面板用，默认固定高度内嵌 */
  fill?: boolean;
  /** 无日志时的遮罩提示；不给就不盖东西，没有日志时直接是空的编辑器 */
  emptyHint?: string;
  /** 自动换行 */
  wordWrap?: boolean;
  /** 自动下拉：有新行就跟到末尾 */
  autoScroll?: boolean;
}>();

const emit = defineEmits<{
  /** 用户往上滚了：自动下拉该关掉，由调用方关并落盘 */
  (e: "scroll-away"): void;
}>();

// ---------------- Monaco 编辑器 ----------------

const hostEl = ref<HTMLElement | null>(null);
let monacoEditor: editor.IStandaloneCodeEditor | null = null;
let decorations: editor.IEditorDecorationsCollection | null = null;
/** 模型当前已应用的行数；用于追加式更新，避免整篇 setValue */
let appliedCount = 0;

/** 跟随滚动的判定余量，单位 px：离底部这么近就算"停在底部" */
const STICK_MARGIN = 40;

/** 级别对应的着色类名；无需着色的级别返回 `null` */
function levelClass(level: string): string | null {
  if (level === "Error" || level === "Fatal") return "mml-log-error";
  if (level === "Warn") return "mml-log-warn";
  if (level === "Debug") return "mml-log-debug";
  return null;
}

/** 全量重建模型内容 + 着色 */
function rebuild() {
  if (!monacoEditor) return;
  const model = monacoEditor.getModel();
  if (!model) return;
  const value = props.logs.map((l) => l.text).join("\n");
  appliedCount = props.logs.length;
  model.setValue(value);
  applyDecorations();
  if (props.autoScroll) scrollBottom();
}

/** 追加新增行（日志只在尾部增长时的快路径） */
function append() {
  if (!monacoEditor) return;
  const model = monacoEditor.getModel();
  if (!model) return;
  const added = props.logs.slice(appliedCount);
  if (added.length === 0) {
    applyDecorations();
    return;
  }
  const lineCount = model.getLineCount();
  const lastLine = model.getLineContent(lineCount);
  const text = added.map((l) => l.text).join("\n");
  appliedCount += added.length;
  monacoEditor.executeEdits("mml-log", [
    {
      range: new monaco.Range(
        lineCount,
        lastLine.length + 1,
        lineCount,
        lastLine.length + 1,
      ),
      text: (lineCount === 1 && lastLine.length === 0 ? "" : "\n") + text,
    },
  ]);
  applyDecorations();
}

/** 按级别给行着色，Error 红 / Warn 黄 / Debug 暗 */
function applyDecorations() {
  if (!monacoEditor || !decorations) return;
  const items: editor.IModelDeltaDecoration[] = [];
  for (let i = 0; i < props.logs.length; i++) {
    const cls = levelClass(props.logs[i].level);
    if (!cls) continue;
    items.push({
      range: new monaco.Range(i + 1, 1, i + 1, 1),
      options: { inlineClassName: cls },
    });
  }
  decorations.set(items);
}

function scrollBottom() {
  if (!monacoEditor) return;
  monacoEditor.revealLine(monacoEditor.getModel()?.getLineCount() ?? 1);
  monacoEditor.setScrollTop(monacoEditor.getScrollHeight());
}

/** 用户是否停在底部 */
function atBottom(): boolean {
  if (!monacoEditor) return true;
  return (
    monacoEditor.getScrollTop() + monacoEditor.getDomNode()!.clientHeight >=
    monacoEditor.getScrollHeight() - STICK_MARGIN
  );
}

/** 打开页内查找框；日志窗口的「搜索」按钮用它（浏览器快捷键被壳层关掉了，只能按钮触发） */
function openFind() {
  monacoEditor?.getAction("actions.find")?.run();
}

defineExpose({ openFind });

// 主题落在 <html data-theme> 上，盯着这个属性比自己监听 theme ref 全面：
// 设置窗口改主题、其它窗口同步过来的改动、跟随系统时系统切换，最终都只是改这个属性
const themeWatcher = new MutationObserver(() => applyLogTheme());

onMounted(() => {
  if (!hostEl.value) return;
  applyLogTheme();
  monacoEditor = monaco.editor.create(hostEl.value, {
    value: "",
    language: "plaintext",
    readOnly: true,
    automaticLayout: true,
    wordWrap: props.wordWrap === false ? "off" : "on",
    minimap: { enabled: false },
    // 行号常开（包括没有日志的空态）：这里就是日志正文的文本编辑器
    lineNumbers: "on",
    glyphMargin: false,
    folding: false,
    scrollBeyondLastLine: false,
    renderLineHighlight: "none",
    contextmenu: false,
    overviewRulerLanes: 0,
    hideCursorInOverviewRuler: true,
    overviewRulerBorder: false,
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
    fontFamily: '"Cascadia Code", Consolas, "Courier New", monospace',
    fontSize: 12.5,
    lineHeight: 1.65 * 12.5,
    unicodeHighlight: { ambiguousCharacters: false, invisibleCharacters: false },
    occurrencesHighlight: "off",
    selectionHighlight: false,
    matchBrackets: "never",
    guides: { indentation: false },
  });
  decorations = monacoEditor.createDecorationsCollection([]);
  themeWatcher.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  monacoEditor.onDidScrollChange(() => {
    // 自动下拉开着时用户往上滚：说明他在回头看历史，让调用方把开关关掉
    if (props.autoScroll && !atBottom()) emit("scroll-away");
  });
  rebuild();
});

onBeforeUnmount(() => {
  themeWatcher.disconnect();
  monacoEditor?.dispose();
  monacoEditor = null;
  decorations = null;
});

watch(
  () => props.logs,
  (now) => {
    if (!monacoEditor) return;
    // 行数变少：清空或筛选变化 → 全量重建；只在尾部增长 → 追加
    if (now.length < appliedCount) {
      rebuild();
      return;
    }
    append();
    if (props.autoScroll) scrollBottom();
  },
);

// 自动换行开关：只是编辑器的一个渲染选项，不用重建文档
watch(() => props.wordWrap, (on) => {
  monacoEditor?.updateOptions({ wordWrap: on ? "on" : "off" });
});
</script>

<template>
  <div class="monaco-log" :class="{ fill: props.fill }">
    <div class="editor-host-wrap">
      <div ref="hostEl" class="editor-host"></div>
      <!-- 只有调用方给了提示才盖（如切换日志来源正在加载）；没有日志时不再默认盖一层文案 -->
      <div v-if="props.logs.length === 0 && props.emptyHint" class="log-empty">
        {{ props.emptyHint }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.monaco-log {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
}

.monaco-log.fill {
  flex: 1;
}

.editor-host-wrap {
  position: relative;
  height: 46vh;
  max-height: 480px;
  border: 1px solid var(--border);
  /* 日志区是整块面板，四角不做圆角；overflow 仍要留着裁掉编辑器溢出的内容 */
  overflow: hidden;
}

.monaco-log.fill .editor-host-wrap {
  height: auto;
  flex: 1;
  max-height: none;
}

.editor-host {
  position: absolute;
  inset: 0;
}

.log-empty {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  /* 空态整块盖住编辑器，并且自己接住点击：原来放行给下面那层只读编辑器，
     点一下就会在空内容里冒出一个文本光标 */
  user-select: none;
}
</style>

<!-- 着色类供 monaco 内部渲染层使用，无 scope、全局生效 -->
<style>
.monaco-editor .mml-log-error {
  color: #ff7b72;
}

.monaco-editor .mml-log-warn {
  color: #e3b341;
}

.monaco-editor .mml-log-debug {
  color: #7d8590;
}

/* 亮色底上上面那套会发白，换一档更深的红 / 黄 / 灰 */
[data-theme="Light"] .monaco-editor .mml-log-error {
  color: #cf222e;
}

[data-theme="Light"] .monaco-editor .mml-log-warn {
  color: #9a6700;
}

[data-theme="Light"] .monaco-editor .mml-log-debug {
  color: #6e7781;
}

/* 光标闪烁：Monaco 对**只读**编辑器一律走实心光标（源码 ViewCursors 里 readOnly 直接
   返回 TextEditorCursorBlinkingStyle.Solid，cursorBlinking 选项不起作用），日志区就少了
   一半"这是个编辑器"的观感。这里按 Monaco 自己的节奏补一个 1s 周期的闪烁。
   只覆盖 `cursor` 元素本身的 opacity，不影响它负责定位的 inline top / left；
   日志视图是全仓唯一的 Monaco 实例，选择器不会波及别处 */
.monaco-editor .cursors-layer > .cursor {
  animation: mml-log-caret-blink 1s step-end infinite;
}

@keyframes mml-log-caret-blink {

  0%,
  100% {
    opacity: 1;
  }

  50% {
    opacity: 0;
  }
}
</style>
