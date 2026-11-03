<script setup lang="ts">
// 输出动作选择器：点开是键盘 + 鼠标的简易图，支持按顺序组合（LeftAlt+E）
//
// 选中规则与后端一致：修饰键按 Control → Shift → Alt 的顺序排在前面，主键放最后，
// 存成加号连接的字符串；后端按下按这个顺序发、松开按反序发。
// 键盘键可以直接按物理键盘选中，按住修饰键再按主键就是组合；鼠标与滚轮只能点选。
import { computed, onBeforeUnmount, ref, watch } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import { t } from "../../../lib/i18n";

const props = defineProps<{
  /** 当前动作，可能是 LeftAlt+E 这样的组合 */
  modelValue: string;
  /** 后端给的可选动作名 */
  actions: string[];
}>();

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
}>();

/** 是否展开选择面板 */
const open = ref(false);
/** 面板里正在拼的动作顺序 */
const order = ref<string[]>([]);

/** 一颗键：label 是显示文字，action 是后端动作名，span 是宽度（单位键） */
interface KeyDef {
  /** 键帽上的字 */
  label: string;
  /** 后端动作名；没有表示这颗键只是占位，点了没反应 */
  action?: string;
  /** 宽度，单位键 */
  span: number;
}

/// 一行键
type KeyRow = KeyDef[];

/** 主键盘：按真实键盘的行排，宽度用单位键算 */
const KEY_ROWS: KeyRow[] = [
  [
    { label: "Esc", action: "Escape", span: 1.15 },
    { label: "F1", action: "F1", span: 1.15 },
    { label: "F2", action: "F2", span: 1.15 },
    { label: "F3", action: "F3", span: 1.15 },
    { label: "F4", action: "F4", span: 1.15 },
    { label: "F5", action: "F5", span: 1.15 },
    { label: "F6", action: "F6", span: 1.15 },
    { label: "F7", action: "F7", span: 1.15 },
    { label: "F8", action: "F8", span: 1.15 },
    { label: "F9", action: "F9", span: 1.15 },
    { label: "F10", action: "F10", span: 1.15 },
    { label: "F11", action: "F11", span: 1.15 },
    { label: "F12", action: "F12", span: 1.15 },
  ],
  [
    { label: "`", action: "Grave", span: 1 },
    { label: "1", action: "1", span: 1 },
    { label: "2", action: "2", span: 1 },
    { label: "3", action: "3", span: 1 },
    { label: "4", action: "4", span: 1 },
    { label: "5", action: "5", span: 1 },
    { label: "6", action: "6", span: 1 },
    { label: "7", action: "7", span: 1 },
    { label: "8", action: "8", span: 1 },
    { label: "9", action: "9", span: 1 },
    { label: "0", action: "0", span: 1 },
    { label: "-", action: "Minus", span: 1 },
    { label: "=", action: "Equal", span: 1 },
    { label: "Backspace", action: "Backspace", span: 2 },
  ],
  [
    { label: "Tab", action: "Tab", span: 1.5 },
    { label: "Q", action: "Q", span: 1 },
    { label: "W", action: "W", span: 1 },
    { label: "E", action: "E", span: 1 },
    { label: "R", action: "R", span: 1 },
    { label: "T", action: "T", span: 1 },
    { label: "Y", action: "Y", span: 1 },
    { label: "U", action: "U", span: 1 },
    { label: "I", action: "I", span: 1 },
    { label: "O", action: "O", span: 1 },
    { label: "P", action: "P", span: 1 },
    { label: "[", action: "LeftBracket", span: 1 },
    { label: "]", action: "RightBracket", span: 1 },
    { label: "\\", action: "Backslash", span: 1.5 },
  ],
  [
    { label: "Caps", action: "CapsLock", span: 1.75 },
    { label: "A", action: "A", span: 1 },
    { label: "S", action: "S", span: 1 },
    { label: "D", action: "D", span: 1 },
    { label: "F", action: "F", span: 1 },
    { label: "G", action: "G", span: 1 },
    { label: "H", action: "H", span: 1 },
    { label: "J", action: "J", span: 1 },
    { label: "K", action: "K", span: 1 },
    { label: "L", action: "L", span: 1 },
    { label: ";", action: "Semicolon", span: 1 },
    { label: "'", action: "Apostrophe", span: 1 },
    { label: "Enter", action: "Enter", span: 2.25 },
  ],
  [
    { label: "Shift", action: "LeftShift", span: 2.25 },
    { label: "Z", action: "Z", span: 1 },
    { label: "X", action: "X", span: 1 },
    { label: "C", action: "C", span: 1 },
    { label: "V", action: "V", span: 1 },
    { label: "B", action: "B", span: 1 },
    { label: "N", action: "N", span: 1 },
    { label: "M", action: "M", span: 1 },
    { label: ",", action: "Comma", span: 1 },
    { label: ".", action: "Period", span: 1 },
    { label: "/", action: "Slash", span: 1 },
    { label: "Shift", action: "LeftShift", span: 2.75 },
  ],
  [
    { label: "Ctrl", action: "LeftControl", span: 1.25 },
    { label: "Win", action: "LeftSuper", span: 1.25 },
    { label: "Alt", action: "LeftAlt", span: 1.25 },
    { label: "Space", action: "Space", span: 6.25 },
    { label: "Alt", action: "LeftAlt", span: 1.25 },
    { label: "Win", action: "LeftSuper", span: 1.25 },
    { label: "Menu", action: "Menu", span: 1.25 },
    { label: "Ctrl", action: "LeftControl", span: 1.25 },
  ],
];

/** 方向键：单独一小块，摆在键盘右边 */
const ARROW_ROWS: KeyRow[] = [
  [{ label: "↑", action: "Up", span: 1 }],
  [
    { label: "←", action: "Left", span: 1 },
    { label: "↓", action: "Down", span: 1 },
    { label: "→", action: "Right", span: 1 },
  ],
];

/** 编辑键：键盘右边一竖排 */
const EDIT_KEYS: string[] = [
  "Insert", "Delete", "Home", "End", "PageUp", "PageDown",
];

/** 修饰键，组合里排在主键前面 */
const MODIFIER_KEYS = ["LeftControl", "LeftShift", "LeftAlt"];

/** 鼠标键 */
const MOUSE_KEYS = ["MouseLeft", "MouseRight", "MouseMiddle", "Mouse4", "Mouse5"];

/** 滚轮 */
const SCROLL_KEYS = ["ScrollUp", "ScrollDown"];



/** 单位键宽，单位像素 */
const UNIT = 26;
/** 键与键的间距，单位像素 */
const GAP = 4;

/** 按单位键算一颗键的宽度 */
function keyWidth(span: number): string {
  return Math.round(UNIT * span + GAP * (span - 1)) + "px";
}

/** 这颗键在当前后端支持的动作里 */
function isActive(key: KeyDef): boolean {
  return !!key.action && props.actions.includes(key.action);
}

/** 这个动作是不是顺序里的最后一颗 */
function isLastAction(name: string): boolean {
  return name === order.value[order.value.length - 1];
}

/** 这个动作是否已经在顺序里 */
function isPickedAction(name: string): boolean {
  return order.value.includes(name);
}

/** 这颗键是不是顺序里的最后一颗 */
function isLast(key: KeyDef): boolean {
  return !!key.action && isLastAction(key.action);
}

/** 这颗键是否已经在顺序里 */
function isPicked(key: KeyDef): boolean {
  return !!key.action && isPickedAction(key.action);
}
/** 过滤出后端真正支持的那些键 */
function supported(keys: string[]): string[] {
  return keys.filter((key) => props.actions.includes(key));
}

/** 芯片上显示的文字：组合按 " + " 连起来 */
const currentText = computed(() =>
  props.modelValue
    .split("+")
    .filter(Boolean)
    .map((name) => actionLabel(name))
    .join(" + "),
);

/** 面板里已经拼好的顺序 */
const orderText = computed(() => order.value.map((name) => actionLabel(name)).join(" + "));

/**
 * 动作名转界面文字
 *
 * 键盘键保留原名，鼠标与滚轮换成能看懂的说法。
 *
 * - 参数 name: 动作名
 */
function actionLabel(name: string): string {
  const labels: Record<string, string> = {
    MouseLeft: t("winGamepad.mouseLeft"),
    MouseRight: t("winGamepad.mouseRight"),
    MouseMiddle: t("winGamepad.mouseMiddle"),
    Mouse4: t("winGamepad.mouse4"),
    Mouse5: t("winGamepad.mouse5"),
    ScrollUp: t("winGamepad.scrollUp"),
    ScrollDown: t("winGamepad.scrollDown"),
  };
  return labels[name] ?? name;
}

/** 物理键盘按到的键转成动作名 */
function fromKeyboard(event: KeyboardEvent): string | null {
  const code = event.code;
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(code)) return code;
  const map: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Backspace: "Backspace",
    Escape: "Escape",
    Insert: "Insert",
    Delete: "Delete",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    ShiftLeft: "LeftShift",
    ControlLeft: "LeftControl",
    AltLeft: "LeftAlt",
    MetaLeft: "LeftSuper",
    Backquote: "Grave",
    Minus: "Minus",
    Equal: "Equal",
    BracketLeft: "LeftBracket",
    BracketRight: "RightBracket",
    Backslash: "Backslash",
    Semicolon: "Semicolon",
    Quote: "Apostrophe",
    Comma: "Comma",
    Period: "Period",
    Slash: "Slash",
    CapsLock: "CapsLock",
    ContextMenu: "Menu",
  };
  return map[code] ?? null;
}

/** 把修饰键放进顺序里，重复的不加 */
function addModifier(name: string) {
  if (order.value.includes(name)) return;
  order.value = [...order.value, name].sort(
    (left, right) => MODIFIER_KEYS.indexOf(left) - MODIFIER_KEYS.indexOf(right),
  );
}

/**
 * 记录一个键：修饰键进修饰位，其它键当主键放在最后
 *
 * 面板开着的时候只记录、不关闭，也不回写配置；确认要等用户点「确定」。
 *
 * - `name`: 动作名
 */
function record(name: string) {
  if (!props.actions.includes(name)) return;
  if (MODIFIER_KEYS.includes(name)) {
    addModifier(name);
    return;
  }
  // 主键只有一个：保留修饰键，替换掉之前的主键
  const modifiers = order.value.filter((item) => MODIFIER_KEYS.includes(item));
  order.value = [...modifiers, name];
}

/** 点一个键：只记录 */
function choose(name: string) {
  record(name);
}

/**
 * 物理键盘：只记录，且拦截浏览器默认行为
 *
 * 面板开着时键盘完全归这里：不触发焦点按钮、不切换焦点、不响应回车 / 空格，
 * 无论这颗键认不认得都吞掉，避免弹窗被按钮误触关掉。
 */
function onKeydown(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  const name = fromKeyboard(event);
  if (!name) return;
  record(name);
}

/** 打开面板，先按当前值把顺序摆好 */
function show() {
  order.value = props.modelValue.split("+").filter(Boolean);
  open.value = true;
}

/** 收起面板 */
function close() {
  open.value = false;
}

/** 清空顺序，重录 */
function reset() {
  order.value = [];
}

/** 用当前顺序收尾；顺序为空时直接收起 */
function confirm() {
  if (!order.value.length) {
    close();
    return;
  }
  emit("update:modelValue", order.value.join("+"));
  close();
}

// 捕获阶段监听：抢在焦点元素之前把按键吃掉
watch(open, (value) => {
  if (value) window.addEventListener("keydown", onKeydown, true);
  else window.removeEventListener("keydown", onKeydown, true);
});

onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown, true));

</script>
<template>
  <div class="ap">
    <button type="button" class="ap-chip" @click="show">
      <span>{{ currentText }}</span>
      <span class="ap-chip-hint">▾</span>
    </button>

    <BaseModal v-if="open" :title="t('winGamepad.actionTitle')" :width="720" :closable="false"
      @close="close">
      <p class="field-desc">{{ t("winGamepad.actionHint") }}</p>

      <div class="ap-layout">
        <!-- 主键盘：按真实键盘的行与宽度排，后端不支持的键只占位 -->
        <div class="ap-keyboard">
          <div v-for="(row, index) in KEY_ROWS" :key="index" class="ap-row">
            <button v-for="(key, i) in row" :key="i" type="button" class="ap-key"
              :class="{ on: isLast(key), mod: isPicked(key), ghost: !isActive(key) }"
              :style="{ width: keyWidth(key.span) }" :disabled="!isActive(key)"
              @click="key.action && choose(key.action)">
              {{ key.label }}
            </button>
          </div>
        </div>

        <!-- 编辑键：键盘右边一竖排 -->
        <div class="ap-edits">
          <button v-for="name in supported(EDIT_KEYS)" :key="name" type="button" class="ap-key"
            :class="{ on: isLastAction(name), mod: isPickedAction(name) }" @click="choose(name)">
            {{ name }}
          </button>
        </div>

        <!-- 方向键 + 鼠标 + 滚轮 -->
        <div class="ap-side">
          <div class="ap-side-title">{{ t("winGamepad.arrowKeys") }}</div>
          <div v-for="(row, index) in ARROW_ROWS" :key="index" class="ap-arrows">
            <button v-for="key in row" :key="key.label" type="button" class="ap-key"
              :class="{ on: isLast(key), mod: isPicked(key) }"
              :style="{ width: keyWidth(key.span) }"
              @click="key.action && choose(key.action)">
              {{ key.label }}
            </button>
          </div>
          <div class="ap-side-title">{{ t("winGamepad.mouseActions") }}</div>
          <button v-for="key in supported(MOUSE_KEYS)" :key="key" type="button" class="ap-key wide"
            :class="{ on: isLastAction(key), mod: isPickedAction(key) }" @click="choose(key)">
            {{ actionLabel(key) }}
          </button>
          <div class="ap-side-title">{{ t("winGamepad.scrollActions") }}</div>
          <button v-for="key in supported(SCROLL_KEYS)" :key="key" type="button" class="ap-key wide"
            :class="{ on: isLastAction(key), mod: isPickedAction(key) }" @click="choose(key)">
            {{ actionLabel(key) }}
          </button>
        </div>
      </div>

      <!-- 顺序预览：修饰键在前、主键在后 -->
      <div class="ap-order">
        <span class="ap-order-label">{{ t("winGamepad.pickOrder") }}</span>
        <span v-if="order.length" class="ap-order-key">{{ orderText }}</span>
        <span v-else class="ap-order-empty">{{ t("winGamepad.pickEmpty") }}</span>
        <span class="ap-gap" />
        <BaseButton v-if="order.length" size="sm" variant="ghost" @click="reset">
          {{ t("winGamepad.pickReset") }}
        </BaseButton>
      </div>

      <div class="modal-actions">
        <BaseButton @click="close">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" :disabled="!order.length" @click="confirm">
          {{ t("actions.confirm") }}
        </BaseButton>
      </div>
    </BaseModal>
  </div>
</template>

<style scoped>
.ap-chip {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: var(--ctl-h);
  width: 100%;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: var(--bg-card);
  color: var(--text);
  font-size: var(--ctl-font);
  text-align: left;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s;
}

.ap-chip:hover {
  border-color: var(--accent);
}

.ap-chip-hint {
  margin-left: auto;
  color: var(--text-dim);
}

/* 三块并排：主键盘 / 编辑键 / 方向与鼠标 */
.ap-layout {
  display: flex;
  gap: 12px;
  align-items: flex-start;
}

.ap-keyboard {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px;
  border-radius: 12px;
  background: var(--bg-hover);
}

.ap-row {
  display: flex;
  gap: 4px;
}

/* 方向键：两行都居中，↑ 正好落在 ↓ 上方 */
.ap-arrows {
  display: flex;
  gap: 4px;
  justify-content: center;
}

/* 一颗键：宽度由单位键算，矮一点才像键盘 */
.ap-key {
  height: 30px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-card);
  color: var(--text);
  font-size: 11px;
  overflow: hidden;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s, color 0.12s;
}

.ap-key:hover {
  border-color: var(--accent);
}

/* 占位键：后端不支持，只用来撑出键盘的形状 */
.ap-key.ghost {
  border-color: var(--border);
  background: transparent;
  color: var(--text-dim);
  opacity: 0.45;
  cursor: default;
}

/* 已经在顺序里的键（通常是修饰键） */
.ap-key.mod {
  border-color: var(--accent);
  color: var(--accent);
  font-weight: 600;
}

/* 顺序里的最后一颗 */
.ap-key.on {
  border-color: var(--accent);
  background: var(--accent);
  color: var(--bg-card);
}

/* 编辑键一竖排 */
.ap-edits {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ap-edits .ap-key {
  width: 64px;
}

.ap-side {
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 112px;
  flex-shrink: 0;
}

.ap-side .ap-key {
  width: 100%;
}

.ap-side-title {
  font-size: 11.5px;
  color: var(--text-dim);
  margin-top: 6px;
}

/* 顺序行 */
.ap-order {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  min-height: 32px;
}

.ap-order-label {
  font-size: 11.5px;
  color: var(--text-dim);
}

.ap-order-key {
  padding: 2px 10px;
  border: 1px solid var(--accent);
  border-radius: 999px;
  color: var(--accent);
  font-size: 11.5px;
  font-weight: 600;
}

.ap-order-empty {
  font-size: 11.5px;
  color: var(--text-dim);
}

.ap-gap {
  flex: 1;
}
</style>
