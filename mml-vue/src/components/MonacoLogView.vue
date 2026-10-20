<script setup lang="ts">
// Monaco 只读日志视图：把给它的日志行画成控制台，管自动下拉 / 页内查找。
// 自动换行是**全局偏好**（lib/editorWrap，Alt+Z 切），日志与配置编辑器共用，这里只把它喂给编辑器。
//
// **筛选不在这里**：等级 / 线程 / 分组都归日志窗口的工具栏管，筛完的行才传进来，
// 免得同一套筛选条件在两边各存一份、还要互相同步。
import { nextTick, onActivated, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor/editor/editor.api";
import { applyLogTheme, monaco } from "../lib/monaco";
import { t } from "../lib/i18n";
import { LOG_LANGUAGE, registerLog } from "../lib/monacoLog";
import { toggleWordWrap, wordWrap } from "../lib/editorWrap";
import type { LogLine } from "../lib/bindings";

const props = defineProps<{
  /** 日志行，按时间顺序；调用方已按等级 / 线程 / 分组筛过 */
  logs: LogLine[];
  /** 撑满父容器；独立窗口与浮动面板用，默认固定高度内嵌 */
  fill?: boolean;
  /** 无日志时的遮罩提示；不给就不盖东西，没有日志时直接是空的编辑器 */
  emptyHint?: string;
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
/** 语言模型自己持有：隐式模型的归属不明确，卸载时也收不干净 */
let logModel: editor.ITextModel | null = null;
let decorations: editor.IEditorDecorationsCollection | null = null;
/** 模型当前已应用的行数；用于追加式更新，避免整篇 setValue */
let appliedCount = 0;
/** 已经上过整行底色的行数；底色只补新增那一段 */
let decoratedCount = 0;

/** 上一次看到的滚动位置，单位 px：只有它变小才说明用户在往上滚 */
let lastScrollTop = 0;
/** 这段时间内的滚动事件是程序化滚动/重建造成的，不算用户操作 */
let ignoreScrollUntil = 0;

/** 标记接下来的滚动事件不是用户操作 */
function markProgrammatic() {
  ignoreScrollUntil = Date.now() + 60;
}

/**
 * 级别对应的整行底色类名；无需底色的级别返回 `null`
 *
 * 只铺背景，正文颜色交给 log 语法（token 色）—— 整行改文字颜色会把 token 色盖住。
 *
 * - `level`: 内核解析出的级别，可能是空串
 */
function levelClass(level: string): string | null {
  if (level === "Error" || level === "Fatal") return "mml-log-line-error";
  if (level === "Warn") return "mml-log-line-warn";
  if (level === "Debug") return "mml-log-line-debug";
  return null;
}

/** 全量重建模型内容 + 底色 */
function rebuild() {
  if (!monacoEditor || !logModel) return;
  const value = props.logs.map((l) => l.text).join("\n");
  appliedCount = props.logs.length;
  // 整篇替换会把滚动位置清掉，别让那次事件被当成用户往上滚
  markProgrammatic();
  logModel.setValue(value);
  decorations?.clear();
  decoratedCount = 0;
  applyDecorations();
  if (props.autoScroll) scrollBottom();
}

/** 追加新增行（日志只在尾部增长时的快路径） */
function append() {
  if (!monacoEditor || !logModel) return;
  const added = props.logs.slice(appliedCount);
  if (added.length === 0) {
    applyDecorations();
    return;
  }
  const lineCount = logModel.getLineCount();
  const lastLine = logModel.getLineContent(lineCount);
  const text = added.map((l) => l.text).join("\n");
  appliedCount += added.length;
  // 内容走模型，不走编辑器：编辑器是只读的，而 executeEdits 在只读编辑器上直接返回 false
  // （见 Monaco codeEditorWidget.executeEdits 的 readOnly 分支），会被静默吞掉
  logModel.applyEdits([
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

/** 给新增行补整行底色 */
function applyDecorations() {
  if (!decorations) return;
  const items: editor.IModelDeltaDecoration[] = [];
  // 只补新增那一段：日志是逐行追加的，每次都重建全量是 O(N²)
  for (let i = decoratedCount; i < props.logs.length; i++) {
    const cls = levelClass(props.logs[i].level);
    if (!cls) continue;
    items.push({
      range: new monaco.Range(i + 1, 1, i + 1, 1),
      options: { isWholeLine: true, className: cls },
    });
  }
  decoratedCount = props.logs.length;
  if (items.length > 0) decorations.append(items);
}

function scrollBottom() {
  if (!monacoEditor) return;
  markProgrammatic();
  monacoEditor.revealLine(monacoEditor.getModel()?.getLineCount() ?? 1);
  monacoEditor.setScrollTop(monacoEditor.getScrollHeight());
}


/** 打开页内查找框；日志窗口的「搜索」按钮用它（浏览器快捷键被壳层关掉了，只能按钮触发） */
function openFind() {
  monacoEditor?.getAction("actions.find")?.run();
}

defineExpose({ openFind });

// ---------------- 右键菜单：只读日志的全选 / 复制 ----------------

/** 日志是只读的，右键只留这两项 */
const LOG_MENU = [
  { label: "edit.copy", command: "editor.action.clipboardCopyAction" },
  { label: "edit.selectAll", command: "editor.action.selectAll" },
] as const;

/** 右键菜单的视口坐标；null = 没开 */
const menu = ref<{ x: number; y: number } | null>(null);
const menuEl = ref<HTMLElement | null>(null);
/** 有选区才让点「复制」 */
const canCopy = ref(false);

/** 在编辑器里右键：弹出自己的菜单，只认正文 */
function onContextMenu(e: editor.IEditorMouseEvent) {
  const kind = e.target.type;
  if (
    kind !== monaco.editor.MouseTargetType.CONTENT_TEXT &&
    kind !== monaco.editor.MouseTargetType.CONTENT_EMPTY &&
    kind !== monaco.editor.MouseTargetType.TEXTAREA
  ) {
    return;
  }
  e.event.preventDefault();
  e.event.stopPropagation();
  // 点在选区外先把光标挪过去，随后的复制 / 全选就作用在那个位置
  const pos = e.target.position;
  const sels = monacoEditor?.getSelections() ?? [];
  if (pos && !sels.some((sel) => sel.containsPosition(pos))) {
    monacoEditor?.setPosition(pos);
  }
  // 挪光标会清掉原选区，这里按挪完后的状态判断能否复制
  canCopy.value = (monacoEditor?.getSelections() ?? []).some((sel) => !sel.isEmpty());
  void openMenu(e.event.browserEvent.clientX, e.event.browserEvent.clientY);
}

/** 贴着点击处弹菜单；先画出来量宽高，再夹回窗口里 */
async function openMenu(x: number, y: number) {
  menu.value = { x, y };
  await nextTick();
  const box = menuEl.value?.getBoundingClientRect();
  if (!box) return;
  const pad = 8;
  menu.value = {
    x: Math.max(pad, Math.min(x, window.innerWidth - box.width - pad)),
    y: Math.max(pad, Math.min(y, window.innerHeight - box.height - pad)),
  };
}

/** 跑一项：先收起菜单、把焦点还给编辑器，再发命令 */
function pick(command: string) {
  menu.value = null;
  monacoEditor?.focus();
  monacoEditor?.trigger("mml-menu", command, null);
}

/** 点到菜单外面：收起 */
function onOutside(e: Event) {
  if (!menu.value || menuEl.value?.contains(e.target as Node)) return;
  menu.value = null;
}

/** Esc 收起菜单；这一下不再往下传，免得顺手把窗口也关了 */
function onMenuKey(e: KeyboardEvent) {
  if (e.key !== "Escape" || !menu.value) return;
  e.preventDefault();
  e.stopPropagation();
  menu.value = null;
  monacoEditor?.focus();
}

// 主题落在 <html data-theme> 上，盯着这个属性比自己监听 theme ref 全面：
// 设置窗口改主题、其它窗口同步过来的改动、跟随系统时系统切换，最终都只是改这个属性
const themeWatcher = new MutationObserver(() => applyLogTheme());

onMounted(() => {
  if (!hostEl.value) return;
  applyLogTheme();
  // 语法要在建模型之前注册；模型显式持有，卸载时自己 dispose
  registerLog();
  logModel = monaco.editor.createModel("", LOG_LANGUAGE);
  monacoEditor = monaco.editor.create(hostEl.value, {
    model: logModel,
    readOnly: true,
    automaticLayout: true,
    wordWrap: wordWrap.value ? "on" : "off",
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
  monacoEditor.onContextMenu(onContextMenu);
  window.addEventListener("pointerdown", onOutside, true);
  window.addEventListener("keydown", onMenuKey, true);
  // Alt+Z：与 VS Code 同键位，写的是全局偏好，配置编辑器与其它窗口跟着变
  monacoEditor.addCommand(monaco.KeyMod.Alt | monaco.KeyCode.KeyZ, toggleWordWrap);
  themeWatcher.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  monacoEditor.onDidScrollChange(() => {
    // 菜单贴的是视口坐标，编辑器一滚就跟内容对不上了
    menu.value = null;
    const top = monacoEditor?.getScrollTop() ?? 0;
    // 判据只能是"滚动位置变小"：日志爆发时内容一次涨很多、重建整篇、我们自己下拉，
    // 都会触发滚动事件但位置不变或变大。之前按"不在底部"判，这些情况都会被误判成
    // 用户往上滚，于是自动下拉自己关掉
    if (
      props.autoScroll &&
      Date.now() >= ignoreScrollUntil &&
      top < lastScrollTop - 4
    ) {
      emit("scroll-away");
    }
    lastScrollTop = top;
  });
  rebuild();

});

// Monaco 的主题是全局的：单窗口模式下配置窗口可能也还挂着，切回日志时把自己的那套刷回来
onActivated(applyLogTheme);

onBeforeUnmount(() => {
  themeWatcher.disconnect();
  window.removeEventListener("pointerdown", onOutside, true);
  window.removeEventListener("keydown", onMenuKey, true);
  monacoEditor?.dispose();
  monacoEditor = null;
  logModel?.dispose();
  logModel = null;
  decorations = null;
});

watch(
  () => props.logs,
  (now, before) => {
    if (!monacoEditor || !logModel) return;
    // 首行是不是同一行，决定"只是尾部长了"还是"整窗换了内容"：
    // 日志撞到行数上限后是抽掉最旧、补上最新（长度不变、首行变了），
    // 只按长度判断的话这时既不追加也不重建，正文会定格在第一次满窗那一刻。
    const sameHead = now.length > 0 && now[0] === before?.[0];
    if (sameHead && now.length > appliedCount) {
      append();
    } else if (!sameHead || now.length < appliedCount) {
      rebuild();
      return;
    }
    if (props.autoScroll) scrollBottom();
  },
);

// 重新打开自动下拉要立刻跟到末尾：不然得等下一行日志，日志一停就永远停在原处
watch(
  () => props.autoScroll,
  (on) => {
    if (on) scrollBottom();
  },
);

// 别处翻转了换行（配置编辑器里的 Alt+Z、另一个窗口、工具栏开关）：跟上，不用重建文档
watch(wordWrap, (on) => {
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

    <!-- 右键菜单：全选 / 复制，越界由 openMenu 夹回窗口里 -->
    <div v-if="menu" ref="menuEl" class="text-menu" :style="{ left: menu.x + 'px', top: menu.y + 'px' }"
      @pointerdown.stop @contextmenu.prevent>
      <button class="text-menu-item" :disabled="!canCopy" @click="pick(LOG_MENU[0].command)">
        {{ t(LOG_MENU[0].label) }}
      </button>
      <button class="text-menu-item" @click="pick(LOG_MENU[1].command)">
        {{ t(LOG_MENU[1].label) }}
      </button>
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
  /* 不能裁：Monaco 的 .overflow-guard 自己会裁正文，这里再裁会把查找框的悬停提示
     切成两半；提示被 IntersectionObserver 判成不可见，于是反复闪现 */
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

<!-- 右键菜单：与配置编辑器的 MonacoText 共用一份样式 -->
<style scoped src="../styles/parts/text-menu.css"></style>

<!-- 着色类供 monaco 内部渲染层使用，无 scope、全局生效 -->
<style>
/* 整行底色：只铺背景，正文颜色交给 log 语法的 token 色 —— 整行改文字颜色会盖住 token */
.monaco-editor .mml-log-line-error {
  background: rgb(248 81 73 / 0.12);
}

.monaco-editor .mml-log-line-warn {
  background: rgb(210 153 34 / 0.12);
}

.monaco-editor .mml-log-line-debug {
  background: rgb(110 118 129 / 0.1);
}

/* 亮色底上同浓度偏重，各降一档 */
[data-theme="Light"] .monaco-editor .mml-log-line-error {
  background: rgb(207 34 46 / 0.1);
}

[data-theme="Light"] .monaco-editor .mml-log-line-warn {
  background: rgb(154 103 0 / 0.12);
}

[data-theme="Light"] .monaco-editor .mml-log-line-debug {
  background: rgb(110 119 129 / 0.12);
}

/* 光标闪烁：Monaco 对**只读**编辑器一律走实心光标（源码 ViewCursors 里 readOnly 直接
   返回 TextEditorCursorBlinkingStyle.Solid，cursorBlinking 选项不起作用），日志区就少了
   一半"这是个编辑器"的观感。这里按 Monaco 自己的节奏补一个 1s 周期的闪烁。
   只覆盖 `cursor` 元素本身的 opacity，不影响它负责定位的 inline top / left；
   选择器限定在日志这块 —— 配置窗口那个编辑器是可写的，光标本来就闪，不用这条 */
.monaco-log .monaco-editor .cursors-layer > .cursor {
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
