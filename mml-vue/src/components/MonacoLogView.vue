<script setup lang="ts">
// Monaco 只读日志视图：线程 / 级别 / 分类筛选 + 级别着色 + 自动滚动
//
// 与 InstanceLogPanel 的差异：内容用 monaco-editor 渲染（只读，等宽控制台），
// 行颜色按级别着色，Error 红 / Warn 黄 / Debug 暗；主窗口内嵌面板
// InstanceLogPanel 不受影响，本组件供独立日志窗口使用。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor/editor/editor.api";
import { ensureLogTheme, monaco } from "../lib/monacoLog";
import { t } from "../lib/i18n";
import type { LogLine } from "../lib/bindings";

const props = defineProps<{
  /** 日志行，按时间顺序 */
  logs: LogLine[];
  /** 撑满父容器；独立日志窗口用，默认固定高度内嵌 */
  fill?: boolean;
  /** 无日志时的提示文案（默认"等待游戏日志输出…"） */
  emptyHint?: string;
}>();

const ALL = "";
const threadFilter = ref(ALL);
const levelFilter = ref(ALL);
const categoryFilter = ref(ALL);

/** 去空去重排序 */
function uniq(values: string[]): string[] {
  return [...new Set(values.filter(Boolean))].sort();
}

const threadOptions = computed(() => uniq(props.logs.map((l) => l.thread)));
const categoryOptions = computed(() => uniq(props.logs.map((l) => l.category)));

/** 级别按严重度排序 */
const levelOptions = computed(() => {
  const set = new Set(props.logs.map((l) => l.level).filter(Boolean));
  return ["Error", "Warn", "Info", "Debug"].filter((l) => set.delete(l)).concat([...set]);
});

const filteredLogs = computed(() =>
  props.logs.filter(
    (l) =>
      (threadFilter.value === ALL || l.thread === threadFilter.value) &&
      (levelFilter.value === ALL || l.level === levelFilter.value) &&
      (categoryFilter.value === ALL || l.category === categoryFilter.value),
  ),
);

// 日志被清空时重置筛选
watch(
  () => props.logs.length === 0,
  (empty) => {
    if (empty) {
      threadFilter.value = ALL;
      levelFilter.value = ALL;
      categoryFilter.value = ALL;
    }
  },
);

// ---------------- Monaco 编辑器 ----------------

const hostEl = ref<HTMLElement | null>(null);
let monacoEditor: editor.IStandaloneCodeEditor | null = null;
let decorations: editor.IEditorDecorationsCollection | null = null;
/** 模型当前已应用的行数；用于追加式更新，避免整篇 setValue */
let appliedCount = 0;

/** 级别对应的着色类名；无需着色的级别返回 `null` */
function levelClass(level: string): string | null {
  if (level === "Error") return "mml-log-error";
  if (level === "Warn") return "mml-log-warn";
  if (level === "Debug") return "mml-log-debug";
  return null;
}

/** 全量重建模型内容 + 着色 */
function rebuild() {
  if (!monacoEditor) return;
  const model = monacoEditor.getModel();
  if (!model) return;
  const value = filteredLogs.value.map((l) => l.text).join("\n");
  appliedCount = filteredLogs.value.length;
  model.setValue(value);
  applyDecorations();
  scrollBottom();
}

/** 追加新增行（日志只在尾部增长时的快路径） */
function append() {
  if (!monacoEditor) return;
  const model = monacoEditor.getModel();
  if (!model) return;
  const added = filteredLogs.value.slice(appliedCount);
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
  for (let i = 0; i < filteredLogs.value.length; i++) {
    const cls = levelClass(filteredLogs.value[i].level);
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

/** 用户是否停在底部（决定是否自动跟随滚动） */
function atBottom(): boolean {
  if (!monacoEditor) return true;
  return (
    monacoEditor.getScrollTop() + monacoEditor.getDomNode()!.clientHeight >=
    monacoEditor.getScrollHeight() - 40
  );
}

onMounted(() => {
  if (!hostEl.value) return;
  ensureLogTheme();
  monacoEditor = monaco.editor.create(hostEl.value, {
    value: "",
    language: "plaintext",
    theme: "mml-log",
    readOnly: true,
    automaticLayout: true,
    wordWrap: "on",
    minimap: { enabled: false },
    lineNumbers: "off",
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
  rebuild();
});

onBeforeUnmount(() => {
  monacoEditor?.dispose();
  monacoEditor = null;
  decorations = null;
});

watch(filteredLogs, (now) => {
  if (!monacoEditor) return;
  // 行数变少：清空或筛选变化 → 全量重建；只在尾部增长 → 追加
  if (now.length < appliedCount) {
    rebuild();
    return;
  }
  const stick = atBottom();
  append();
  // 用户停在底部时才跟随滚动，回看历史时不打扰
  if (stick) scrollBottom();
});

// 筛选条件变化 → 全量重建
watch([threadFilter, levelFilter, categoryFilter], rebuild);
</script>

<template>
  <div class="monaco-log" :class="{ fill: props.fill }">
    <div class="filter-bar">
      <select v-model="threadFilter" class="field-select filter-select" v-tip="t('launch.filterThread')">
        <option value="">{{ t("launch.filterThread") }} · {{ t("launch.filterAll") }}</option>
        <option v-for="th in threadOptions" :key="th" :value="th">{{ th }}</option>
      </select>
      <select v-model="levelFilter" class="field-select filter-select" v-tip="t('launch.filterLevel')">
        <option value="">{{ t("launch.filterLevel") }} · {{ t("launch.filterAll") }}</option>
        <option v-for="lv in levelOptions" :key="lv" :value="lv">{{ lv }}</option>
      </select>
      <select v-model="categoryFilter" class="field-select filter-select" v-tip="t('launch.filterCategory')">
        <option value="">{{ t("launch.filterCategory") }} · {{ t("launch.filterAll") }}</option>
        <option v-for="c in categoryOptions" :key="c" :value="c">{{ c }}</option>
      </select>
    </div>

    <div class="editor-host-wrap">
      <div ref="hostEl" class="editor-host"></div>
      <div v-if="filteredLogs.length === 0" class="log-empty">
        {{ props.emptyHint ?? t("launch.waitLog") }}
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

.filter-bar {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.editor-host-wrap {
  position: relative;
  height: 46vh;
  max-height: 480px;
  border: 1px solid var(--border);
  border-radius: 10px;
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
  color: #4d5560;
  pointer-events: none;
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
</style>
