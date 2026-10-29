<script setup lang="ts">
// 可编辑的 Monaco 文本框：行号 + 按语言的着色 + 括号配对与折叠
//
// 配置窗口的文本视图用它；与日志视图共用同一套 Monaco 环境（worker / 主题在 lib/monaco、
// 语言包在 lib/monacoCode）与同一份自动换行偏好（lib/editorWrap，Alt+Z 切）。**不要**在这里
// import monaco-editor 的其它入口，那会把整套语言包（含各类语言服务与 worker）拉进包里。
import { computed, nextTick, onActivated, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor/editor/editor.api";
import { applyCodeTheme, monaco } from "../../lib/monacoCode";
import { toggleWordWrap, wordWrap } from "../../lib/editorWrap";
import { t } from "../../lib/i18n";

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

/** 右键菜单的一项 */
interface EditMenuItem {
  /** 文案 i18n key */
  label: string;
  /** Monaco 的命令 id；动作与命令都靠它发出去 */
  command: string;
  /** 会改内容：只读时置灰 */
  edit?: boolean;
  /** 要有对应的历史才可点：撤销 / 重做 */
  history?: "undo" | "redo";
  /** 要用当前语言的格式化器：没有就不列出来 */
  format?: boolean;
  /** 在这一项**之前**画一条分隔线 */
  separated?: boolean;
}

/** 右键菜单条目：与 Monaco 自带那一套同源，顺序也照它 */
const MENU_ITEMS: EditMenuItem[] = [
  { label: "edit.undo", command: "undo", edit: true, history: "undo" },
  { label: "edit.redo", command: "redo", edit: true, history: "redo" },
  { label: "edit.cut", command: "editor.action.clipboardCutAction", edit: true, separated: true },
  { label: "edit.copy", command: "editor.action.clipboardCopyAction" },
  { label: "edit.paste", command: "editor.action.clipboardPasteAction", edit: true },
  { label: "edit.selectAll", command: "editor.action.selectAll", separated: true },
  { label: "edit.format", command: "editor.action.formatDocument", format: true },
  { label: "edit.find", command: "actions.find", separated: true },
];

const hostEl = ref<HTMLElement | null>(null);
const menuEl = ref<HTMLElement | null>(null);
let ed: editor.IStandaloneCodeEditor | null = null;

/** 右键菜单位置，视口坐标；null = 没开 */
const menu = ref<{ x: number; y: number } | null>(null);

/** 有没有格式化器、有没有可撤销 / 可重做的历史；都在弹菜单那一刻取，见 openMenu */
const canFormat = ref(false);
const canUndo = ref(false);
const canRedo = ref(false);

/**
 * 真正画出来的条目
 *
 * 只读时会改内容的那几项置灰；历史到底了撤销 / 重做置灰；当前语言没有格式化器的
 * （只有 json 有）干脆不列"格式化代码"。
 */
const menuItems = computed(() =>
  MENU_ITEMS.filter((item) => !item.format || canFormat.value).map((item) => ({
    ...item,
    disabled:
      (!!item.edit && props.readonly) ||
      (item.history === "undo" && !canUndo.value) ||
      (item.history === "redo" && !canRedo.value),
  })),
);

/**
 * 在编辑器里右键：弹出自己的菜单
 *
 * 点在选区外先把光标挪过去，与 Monaco 自带菜单同一口径 —— 随后的剪切 / 复制就作用在那个位置。
 *
 * - `e`: Monaco 的编辑器鼠标事件
 */
function onContextMenu(e: editor.IEditorMouseEvent) {
  // 只认正文：落在浮层控件（查找框等）、边距、滚动条上时不抢，交给它自己的处理
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
  const pos = e.target.position;
  const sels = ed?.getSelections() ?? [];
  if (pos && !sels.some((sel) => sel.containsPosition(pos))) {
    ed?.setPosition(pos);
  }
  void openMenu(e.event.browserEvent.clientX, e.event.browserEvent.clientY);
}

/**
 * 贴着点击处弹菜单，越过窗口边缘就夹回来
 *
 * - `x`: 视口横坐标
 * - `y`: 视口纵坐标
 */
async function openMenu(x: number, y: number) {
  // 格式化器是按语言注册的，且可能后到（json 的 provider 是语言首次用到时异步注册的）。
  // 这个动作的前置条件里就有"当前有没有格式化器"，且它读的是上下文键、不是响应式的，
  // 所以每次弹菜单都重新问一次
  canFormat.value = ed?.getAction("editor.action.formatDocument")?.isSupported() ?? false;
  // 撤销 / 重做问模型自己的历史栈：到底了就不能点
  const model = ed?.getModel();
  canUndo.value = model?.canUndo() ?? false;
  canRedo.value = model?.canRedo() ?? false;
  menu.value = { x, y };
  // 先画出来才有宽高可量
  await nextTick();
  const box = menuEl.value?.getBoundingClientRect();
  if (!box) return;
  // 菜单与窗口边缘的最小距离，单位 px
  const pad = 8;
  menu.value = {
    x: Math.max(pad, Math.min(x, window.innerWidth - box.width - pad)),
    y: Math.max(pad, Math.min(y, window.innerHeight - box.height - pad)),
  };
}

/**
 * 跑一项：先收起菜单、把焦点还给编辑器，再发命令
 *
 * - `command`: Monaco 的命令 id，见 [`MENU_ITEMS`]
 */
function pick(command: string) {
  menu.value = null;
  ed?.focus();
  ed?.trigger("mml-menu", command, null);
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
  ed?.focus();
}

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
    // 右键菜单自己画（见 onContextMenu），不用 Monaco 自带那套：
    // 壳层禁了浏览器默认菜单，这一项是明说"别用编辑器自带的"
    contextmenu: false,
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
    unicodeHighlight: { ambiguousCharacters: false, invisibleCharacters: false },
  });
  ed.onDidChangeModelContent(() => {
    emit("update:modelValue", ed?.getValue() ?? "");
  });
  // Alt+Z：与 VS Code 同键位，写的是全局偏好，日志视图与其它窗口跟着变
  ed.addCommand(monaco.KeyMod.Alt | monaco.KeyCode.KeyZ, toggleWordWrap);
  ed.onContextMenu(onContextMenu);
  // 滚一下就收起：菜单贴的是视口坐标，编辑器一滚它就跟内容对不上了
  ed.onDidScrollChange(() => {
    menu.value = null;
  });
  themeWatcher.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  window.addEventListener("pointerdown", onOutside, true);
  window.addEventListener("keydown", onMenuKey, true);
});

// Monaco 的主题是全局的：单窗口模式下日志视图可能也还挂着，切回本窗口时把自己的那套刷回来
onActivated(applyCodeTheme);

onBeforeUnmount(() => {
  themeWatcher.disconnect();
  window.removeEventListener("pointerdown", onOutside, true);
  window.removeEventListener("keydown", onMenuKey, true);
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

  <!-- 右键菜单：固定定位、贴点击处弹出，越界由 openMenu 夹回窗口里 -->
  <div v-if="menu" ref="menuEl" class="text-menu" :style="{ left: menu.x + 'px', top: menu.y + 'px' }"
    @pointerdown.stop @contextmenu.prevent>
    <template v-for="item in menuItems" :key="item.command">
      <span v-if="item.separated" class="text-menu-sep" />
      <button class="text-menu-item" :disabled="item.disabled" @click="pick(item.command)">
        {{ t(item.label) }}
      </button>
    </template>
  </div>
</template>

<style scoped>
/* 方正的矩形：不切圆角、聚焦也不给强调色描边（与官网示例那块编辑器、以及日志区一致）。
   聚焦反馈交给里面闪烁的光标，别再画一圈蓝框 */
.monaco-text {
  flex: 1;
  min-width: 0;
  min-height: 0;
  border: 1px solid var(--border);
  /* 不设 overflow: hidden：查找框的悬停提示要能伸出编辑器之外 */
}
</style>

<!-- 右键菜单的样式与日志视图共用，见 styles/parts/text-menu.css -->
<style scoped src="../../styles/parts/text-menu.css"></style>
