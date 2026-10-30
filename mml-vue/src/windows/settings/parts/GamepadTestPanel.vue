<script setup lang="ts">
// 复用内核采样快照，测试面板本身不发送输入，也不启动第二套设备线程。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { commands, type GamepadAxisDto, type GamepadDto, type GamepadStateDto } from "../../../lib/bindings";
import { t, tErr } from "../../../lib/i18n";
import { usePageActive } from "../../../lib/pageActive";
import { isTauri } from "../../windowManager";

const devices = ref<GamepadDto[]>([]);
const selected = ref<number | null>(null);
const state = ref<GamepadStateDto | null>(null);
const error = ref("");
const active = usePageActive();
let disposed = false;
let timer: ReturnType<typeof setTimeout> | undefined;

const sticks: { label: string; x: GamepadAxisDto; y: GamepadAxisDto }[] = [
  { label: "winGamepad.stickLeft", x: "leftStickX", y: "leftStickY" },
  { label: "winGamepad.stickRight", x: "rightStickX", y: "rightStickY" },
];
const pressed = computed(() => state.value?.buttons.filter((button) => button.value >= 0.5) ?? []);

/** 查询当前轴值；设备断开时回到中点。 */
function axis(name: GamepadAxisDto): number {
  return state.value?.axes.find((axis) => axis.axis === name)?.value ?? 0;
}

/** 串行查询，窗口停用或销毁后不再轮询。 */
async function poll() {
  if (disposed || !isTauri()) return;
  if (active.value) {
    try {
      const available = await commands.gamepad.list();
      if (disposed) return;
      devices.value = available;
      if (!available.some((device) => device.id === selected.value)) {
        selected.value = available[0]?.id ?? null;
        state.value = null;
      }
      const id = selected.value;
      const snapshot = id === null ? null : await commands.gamepad.state(id);
      if (disposed) return;
      if (id === selected.value) state.value = snapshot;
      error.value = "";
    } catch (reason) {
      if (!disposed) {
        state.value = null;
        error.value = tErr(reason);
      }
    }
  }
  if (!disposed) timer = setTimeout(() => void poll(), 120);
}

onMounted(() => void poll());
onBeforeUnmount(() => {
  disposed = true;
  if (timer) clearTimeout(timer);
});
</script>

<template>
  <section class="gamepad-test" :aria-label="t('winGamepad.testTitle')">
    <div class="gamepad-test-head">
      <h3>{{ t("winGamepad.testTitle") }}</h3>
      <select v-if="devices.length" v-model="selected" class="field-select" :aria-label="t('winGamepad.testDevice')"
        @change="state = null">
        <option v-for="device in devices" :key="device.id" :value="device.id">{{ device.name }}</option>
      </select>
    </div>
    <p class="field-hint">{{ t("winGamepad.testHint") }}</p>
    <p v-if="error" class="gamepad-error" role="alert">{{ error }}</p>
    <p v-else-if="!devices.length" class="field-hint">{{ t("winGamepad.testEmpty") }}</p>
    <template v-else>
      <div class="gamepad-sticks">
        <div v-for="stick in sticks" :key="stick.x" class="gamepad-stick">
          <span>{{ t(stick.label) }}</span>
          <div class="gamepad-axis" aria-hidden="true">
            <i :style="{ transform: `translate(${axis(stick.x) * 38}px, ${-axis(stick.y) * 38}px)` }" />
          </div>
          <span class="gamepad-values">{{ axis(stick.x).toFixed(2) }} / {{ axis(stick.y).toFixed(2) }}</span>
        </div>
      </div>
      <div class="gamepad-buttons">
        <span v-for="button in pressed" :key="button.button">{{ button.button }} {{ button.value.toFixed(2) }}</span>
        <span v-if="!pressed.length" class="field-hint">{{ t("winGamepad.testNoButtons") }}</span>
      </div>
    </template>
  </section>
</template>

<style scoped>
.gamepad-test {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 16px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
}

.gamepad-test-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
}

.gamepad-test-head h3 {
  margin: 0;
  font-size: 13px;
}

.gamepad-test-head select {
  width: auto;
  max-width: 100%;
}

.gamepad-sticks {
  display: flex;
  justify-content: space-around;
  flex-wrap: wrap;
  gap: 20px;
}

.gamepad-stick {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  font-size: 11.5px;
}

.gamepad-axis {
  position: relative;
  display: grid;
  place-items: center;
  width: 96px;
  height: 96px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--accent-soft);
}

.gamepad-axis::before,
.gamepad-axis::after {
  content: "";
  position: absolute;
  background: var(--border);
}

.gamepad-axis::before {
  width: 100%;
  height: 1px;
}

.gamepad-axis::after {
  width: 1px;
  height: 100%;
}

.gamepad-axis i {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--accent);
  z-index: 1;
}

.gamepad-values {
  font-variant-numeric: tabular-nums;
  color: var(--text-dim);
}

.gamepad-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.gamepad-buttons span:not(.field-hint) {
  border: 1px solid var(--accent);
  border-radius: 8px;
  padding: 4px 8px;
  font-size: 11.5px;
  color: var(--accent);
  background: var(--accent-soft);
}

.gamepad-error {
  color: var(--red);
  font-size: var(--ctl-font);
}
</style>
