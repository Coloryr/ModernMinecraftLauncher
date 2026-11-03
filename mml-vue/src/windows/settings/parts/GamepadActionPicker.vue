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

/** 键盘上的候选键，按 QWERTY 摆；不支持的会被过滤掉 */
const KEY_ROWS: string[][] = [
  ["Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"],
  ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
  ["Tab", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"],
  ["Backspace", "A", "S", "D", "F", "G", "H", "J", "K", "L", "Enter"],
  ["Shift", "Z", "X", "C", "V", "B", "N", "M"],
];

/** 单独一行的特殊键 */
const SPECIAL_KEYS = [
  "Space", "Enter", "Tab", "Backspace", "Escape", "Insert", "Delete",
  "Home", "End", "PageUp", "PageDown", "Up", "Down", "Left", "Right",
];

/** 修饰键，组合里排在主键前面 */
const MODIFIER_KEYS = ["LeftControl", "LeftShift", "LeftAlt"];

/** 鼠标键 */
const MOUSE_KEYS = ["MouseLeft", "MouseRight", "MouseMiddle", "Mouse4", "Mouse5"];

/** 滚轮 */
const SCROLL_KEYS = ["ScrollUp", "ScrollDown"];

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

/** 收尾：修饰键 + 主键组成一段动作 */
function commit(name: string) {
  if (!props.actions.includes(name)) return;
  const steps = [...order.value.filter((item) => item !== name), name];
  emit("update:modelValue", steps.join("+"));
  close();
}

/** 点一个键：修饰键只进顺序，其它键直接收尾 */
function choose(name: string) {
  if (MODIFIER_KEYS.includes(name)) {
    addModifier(name);
    return;
  }
  commit(name);
}

/** 物理键盘：修饰键进顺序，主键收尾 */
function onKeydown(event: KeyboardEvent) {
  const name = fromKeyboard(event);
  if (!name || !props.actions.includes(name)) return;
  event.preventDefault();
  choose(name);
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

watch(open, (value) => {
  if (value) window.addEventListener("keydown", onKeydown);
  else window.removeEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>
<template>
  <div class="ap">
    <button type="button" class="ap-chip" @click="show">
      <span>{{ currentText }}</span>
      <span class="ap-chip-hint">▾</span>
    </button>

    <BaseModal v-if="open" :title="t('winGamepad.actionTitle')" :width="640" :closable="false"
      @close="close">
      <p class="field-desc">{{ t("winGamepad.actionHint") }}</p>

      <div class="ap-layout">
        <div class="ap-keyboard">
          <div v-for="(row, index) in KEY_ROWS" :key="index" class="ap-row">
            <button v-for="key in supported(row)" :key="key" type="button" class="ap-key"
              :class="{ on: key === order[order.length - 1], mod: order.includes(key) }"
              @click="choose(key)">
              {{ key }}
            </button>
          </div>
          <div class="ap-row">
            <button v-for="key in supported(SPECIAL_KEYS)" :key="key" type="button" class="ap-key wide"
              :class="{ on: key === order[order.length - 1], mod: order.includes(key) }"
              @click="choose(key)">
              {{ key }}
            </button>
          </div>
          <div class="ap-row">
            <button v-for="key in supported(MODIFIER_KEYS)" :key="key" type="button" class="ap-key wide"
              :class="{ on: key === order[order.length - 1], mod: order.includes(key) }"
              @click="choose(key)">
              {{ actionLabel(key) }}
            </button>
          </div>
        </div>

        <div class="ap-side">
          <div class="ap-side-title">{{ t("winGamepad.mouseActions") }}</div>
          <button v-for="key in supported(MOUSE_KEYS)" :key="key" type="button" class="ap-key wide"
            :class="{ on: key === order[order.length - 1], mod: order.includes(key) }"
            @click="choose(key)">
            {{ actionLabel(key) }}
          </button>
          <div class="ap-side-title">{{ t("winGamepad.scrollActions") }}</div>
          <button v-for="key in supported(SCROLL_KEYS)" :key="key" type="button" class="ap-key wide"
            :class="{ on: key === order[order.length - 1], mod: order.includes(key) }"
            @click="choose(key)">
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
/* 芯片：和手柄按键选择器同一套外观 */
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

.ap-layout {
  display: flex;
  gap: 14px;
  align-items: flex-start;
}

.ap-keyboard {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-width: 0;
  padding: 10px;
  border-radius: 12px;
  background: var(--bg-hover);
}

.ap-row {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.ap-key {
  min-width: 30px;
  min-height: 30px;
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-card);
  color: var(--text);
  font-size: 11.5px;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s, color 0.12s;
}

.ap-key:hover {
  border-color: var(--accent);
}

/* 已在顺序里的键（一般是修饰键） */
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

.ap-key.wide {
  min-width: 48px;
}

.ap-side {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 110px;
  flex-shrink: 0;
}

.ap-side-title {
  font-size: 11.5px;
  color: var(--text-dim);
  margin-top: 4px;
}

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
