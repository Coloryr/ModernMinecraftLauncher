<script setup lang="ts">
// 手柄按键选择器：点开是一张实时手柄图，靠"按下顺序"记录组合键
//
// 打开时先把手柄当前按住的键记进来（比如已经按着 LT），之后每按下一次追加到顺序末尾；
// 顺序的第一颗当修饰键、最后一颗当主键，和后端 modifier + button 的口径一致。
// 图上也能直接点，点中的键同样进顺序。
import { computed, onBeforeUnmount, ref } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import GamepadDiagram from "./GamepadDiagram.vue";
import { t, tErr } from "../../../lib/i18n";
import { isTauri } from "../../windowManager";
import { ALL_BUTTONS, buttonLabel } from "../gamepadLabels";
import { commands, type GamepadButtonDto, type GamepadLayoutDto, type GamepadStateDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 主键；null 表示还没选 */
  button: GamepadButtonDto | null;
  /** 组合键的修饰键；null 表示单键 */
  modifier: GamepadButtonDto | null;
  /** 当前外观，只影响图上记号 */
  layout: GamepadLayoutDto;
  /** 是否交换 A / B 位置 */
  swapAb: boolean;
}>();

const emit = defineEmits<{
  (event: "update:button", value: GamepadButtonDto | null): void;
  (event: "update:modifier", value: GamepadButtonDto | null): void;
}>();

/** 采样间隔，单位毫秒 */
const TICK = 100;

/** 是否展开选择面板 */
const open = ref(false);
/** 面板里显示的实时状态 */
const state = ref<GamepadStateDto | null>(null);
/** 记录下来的按下顺序，最多两颗：修饰键 + 主键 */
const order = ref<GamepadButtonDto[]>([]);

let timer: ReturnType<typeof setTimeout> | undefined;
let disposed = false;
let previous = new Set<GamepadButtonDto>();

/** 主键记号；没选时显示提示 */
const buttonText = computed(() =>
  props.button ? label(props.button) : t("winGamepad.pickButton"),
);

/** 组合预览，例如 "LT + ←" */
const comboText = computed(() => {
  if (!props.modifier) return "";
  return props.button
    ? t("winGamepad.combo", { modifier: label(props.modifier), button: label(props.button) })
    : label(props.modifier);
});

/** 取按钮记号：随外观与 A / B 交换变 */
function label(button: GamepadButtonDto): string {
  return buttonLabel(button, props.layout, props.swapAb);
}

/** 当前按住的键 */
function pressedNow(snapshot: GamepadStateDto | null): GamepadButtonDto[] {
  if (!snapshot) return [];
  return ALL_BUTTONS.filter((button) =>
    snapshot.buttons.some((item) => item.button === button && item.value >= 0.5),
  );
}

/**
 * 追加一颗键到顺序里
 *
 * 顺序最多两颗：已经有两颗时，后按的那颗顶掉主键位置，修饰键保持第一次按下的那颗。
 *
 * - `button`: 这次按下的键
 */
function record(button: GamepadButtonDto) {
  if (order.value[order.value.length - 1] === button) return;
  if (order.value.length >= 2) {
    order.value = [order.value[0], button];
    return;
  }
  order.value = [...order.value, button];
}

/** 打开面板：记录打开瞬间已经按住的键，然后开始采样 */
function show() {
  if (!isTauri()) return;
  order.value = [];
  previous = new Set();
  open.value = true;
  disposed = false;
  void poll(true);
}

/** 关掉面板：停采样 */
function close() {
  open.value = false;
  disposed = true;
  if (timer) {
    clearTimeout(timer);
    timer = undefined;
  }
  state.value = null;
}

/** 采样一帧：更新状态图，并把新按下的键记进顺序 */
async function poll(first = false) {
  if (disposed) return;
  try {
    const devices = await commands.gamepad.list();
    const id = devices[0]?.id ?? null;
    const snapshot = id === null ? null : await commands.gamepad.state(id);
    if (disposed) return;
    state.value = snapshot;

    const now = pressedNow(snapshot);
    for (const button of now) {
      if (!previous.has(button)) record(button);
    }
    previous = new Set(now);
  } catch (error) {
    if (first) tErr(error);
  }
  if (!disposed) timer = setTimeout(() => void poll(), TICK);
}

/** 确认：顺序最后一颗作主键，多于一颗时第一颗作修饰键 */
function confirm() {
  const list = order.value;
  if (!list.length) {
    close();
    return;
  }
  emit("update:modifier", list.length > 1 ? list[0] : null);
  emit("update:button", list[list.length - 1]);
  close();
}

/** 重录：清空顺序，等用户重新按 */
function reset() {
  order.value = [];
  previous = new Set(pressedNow(state.value));
}

onBeforeUnmount(close);
</script>

<template>
  <div class="pk">
    <button type="button" class="pk-chip" @click="show">
      <span v-if="modifier" class="pk-chip-combo">{{ comboText }}</span>
      <span v-else class="pk-chip-single">{{ buttonText }}</span>
      <span class="pk-chip-hint">▾</span>
    </button>

    <BaseModal v-if="open" :title="t('winGamepad.pickTitle')" :width="560" :closable="false" @close="close">
      <p class="field-desc">{{ t("winGamepad.pickHint") }}</p>

      <GamepadDiagram :state="state" :layout="layout" :swap-ab="swapAb" interactive
        @pick="record" />

      <!-- 记录下来的顺序：第一颗是修饰键，最后一颗是主键 -->
      <div class="pk-order">
        <span class="pk-order-label">{{ t("winGamepad.pickOrder") }}</span>
        <template v-if="order.length">
          <span v-for="(button, index) in order" :key="button" class="pk-order-item">
            <span v-if="index > 0" class="pk-order-arrow">→</span>
            <span class="pk-order-key">{{ label(button) }}</span>
          </span>
        </template>
        <span v-else class="pk-order-empty">{{ t("winGamepad.pickEmpty") }}</span>
        <span class="pk-gap" />
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
.pk-chip {
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

.pk-chip:hover {
  border-color: var(--accent);
}

.pk-chip-combo {
  color: var(--accent);
  font-weight: 600;
}

.pk-chip-hint {
  margin-left: auto;
  color: var(--text-dim);
}

/* 顺序行：记录下来的按键按按下顺序横排 */
.pk-order {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  min-height: 32px;
}

.pk-order-label {
  font-size: 11.5px;
  color: var(--text-dim);
}

.pk-order-item {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.pk-order-arrow {
  color: var(--text-dim);
}

.pk-order-key {
  padding: 2px 10px;
  border: 1px solid var(--accent);
  border-radius: 999px;
  color: var(--accent);
  font-size: 11.5px;
  font-weight: 600;
}

.pk-order-empty {
  font-size: 11.5px;
  color: var(--text-dim);
}

.pk-gap {
  flex: 1;
}
</style>
