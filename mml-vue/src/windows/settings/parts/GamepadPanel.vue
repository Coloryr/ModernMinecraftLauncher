<script setup lang="ts">
// 设备测试始终只读；发送输入须选择实例并显式启用映射。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { commands, type GamepadSettingsDto, type GamepadStatusDto, type InstanceInfoDto } from "../../../lib/bindings";
import { api } from "../../../lib/api";
import { t, tErr } from "../../../lib/i18n";
import { usePageActive } from "../../../lib/pageActive";
import { useInstanceChange } from "../../../composables/useInstanceChange";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseSwitch from "../../../components/ui/BaseSwitch.vue";
import NumberStepper from "../../../components/ui/NumberStepper.vue";
import SettingsGroup from "./SettingsGroup.vue";

defineProps<{
  /** 搜索命中后高亮此分组。 */
  flash?: boolean;
}>();
const settings = ref<GamepadSettingsDto | null>(null);
const status = ref<GamepadStatusDto | null>(null);
const instances = ref<InstanceInfoDto[]>([]);
const saved = ref("");
const error = ref("");
const saving = ref(false);
const savedNotice = ref(false);
const dirty = computed(() => settings.value && JSON.stringify(settings.value) !== saved.value);
const active = usePageActive();
let disposed = false;
let timer: ReturnType<typeof setTimeout> | undefined;

/** 实例被删除时清除草稿中的无效目标，避免顺带保存其它未提交配置。 */
async function loadInstances() {
  instances.value = await api.getInstances();
  if (settings.value?.target && !instances.value.some((item) => item.uuid === settings.value?.target)) {
    settings.value.target = null;
    settings.value.enabled = false;
  }
}
useInstanceChange(async () => {
  try { await loadInstances(); } catch (reason) { error.value = tErr(reason); }
});

/** 串行轮询设备状态，停用页面时暂停查询。 */
async function poll() {
  if (disposed) return;
  if (active.value) {
    try { status.value = await commands.settings.gamepadStatus(); }
    catch (reason) { error.value = tErr(reason); }
  }
  if (!disposed) timer = setTimeout(() => void poll(), 120);
}

/** 保存后由采样线程切换设备、释放旧目标按键。 */
async function save() {
  if (!settings.value || saving.value) return;
  saving.value = true;
  savedNotice.value = false;
  error.value = "";
  const snapshot = { ...settings.value };
  try {
    await commands.settings.gamepadSave(snapshot);
    saved.value = JSON.stringify(snapshot);
    savedNotice.value = true;
  } catch (reason) {
    error.value = tErr(reason);
  } finally {
    saving.value = false;
  }
}

onMounted(async () => {
  try {
    settings.value = await commands.settings.gamepadGet();
    saved.value = JSON.stringify(settings.value);
    await loadInstances();
  } catch (reason) {
    error.value = tErr(reason);
  }
  await poll();
});
onBeforeUnmount(() => {
  disposed = true;
  if (timer) clearTimeout(timer);
});
</script>

<template>
  <SettingsGroup id="gamepad" title-key="gamepad.title" :flash="flash">
    <div v-if="settings" class="gamepad-layout">
      <div class="gamepad-options">
        <div class="gamepad-toggle">
          <span>{{ t("gamepad.enabled") }}</span>
          <BaseSwitch v-model="settings.enabled" :disabled="saving" />
        </div>
        <label class="field-label" for="gamepad-target">{{ t("gamepad.target") }}</label>
        <select id="gamepad-target" v-model="settings.target" class="field-select" :disabled="saving">
          <option :value="null">{{ t("gamepad.selectTarget") }}</option>
          <option v-for="instance in instances" :key="instance.uuid" :value="instance.uuid">{{ instance.name }}</option>
        </select>
        <label class="field-label" for="gamepad-device">{{ t("gamepad.device") }}</label>
        <select id="gamepad-device" v-model="settings.device" class="field-select" :disabled="saving">
          <option value="">{{ t("gamepad.autoDevice") }}</option>
          <option v-for="device in status?.devices ?? []" :key="device.id" :value="device.id">{{ device.name }}</option>
          <option v-if="settings.device && !status?.devices.some((device) => device.id === settings?.device)" :value="settings.device">
            {{ t("gamepad.disconnected") }}
          </option>
        </select>
        <div class="gamepad-number">
          <span>{{ t("gamepad.deadzone") }}</span>
          <NumberStepper :model-value="Math.round(settings.deadzone * 100)" :min="5" :max="80" :step="1"
            @update:model-value="settings.deadzone = $event / 100" />
        </div>
        <div class="gamepad-number">
          <span>{{ t("gamepad.lookSpeed") }}</span>
          <NumberStepper v-model="settings.lookSpeed" :min="50" :max="5000" :step="50" />
        </div>
        <div class="gamepad-number">
          <span>{{ t("gamepad.cursorSpeed") }}</span>
          <NumberStepper v-model="settings.cursorSpeed" :min="50" :max="5000" :step="50" />
        </div>
        <p class="field-hint">{{ t("gamepad.connectionHint") }}</p>
        <div class="gamepad-save">
          <BaseButton variant="accent" :disabled="saving || !dirty || (settings.enabled && !settings.target)" @click="save">
            {{ t(saving ? "gamepad.saving" : "gamepad.save") }}
          </BaseButton>
          <span v-if="savedNotice && !dirty" class="field-hint" role="status">{{ t("gamepad.saved") }}</span>
        </div>
      </div>
      <section class="gamepad-test" :aria-label="t('gamepad.test')">
        <div class="gamepad-test-head">
          <strong>{{ t("gamepad.test") }}</strong>
          <span :class="{ 'is-active': status?.active }">{{ t(status?.active ? "gamepad.active" : "gamepad.idle") }}</span>
        </div>
        <p v-if="!status?.devices.length" class="field-hint">{{ t("gamepad.noDevice") }}</p>
        <div class="gamepad-sticks">
          <div v-for="(key, index) in ['gamepad.leftStick', 'gamepad.rightStick']" :key="key" class="gamepad-stick">
            <span>{{ t(key) }}</span>
            <div class="gamepad-axis">
              <i :style="{ transform: `translate(${(status?.axes[index * 2] ?? 0) * 38}px, ${-(status?.axes[index * 2 + 1] ?? 0) * 38}px)` }" />
            </div>
            <span class="gamepad-values">{{ (status?.axes[index * 2] ?? 0).toFixed(2) }} /
              {{ (status?.axes[index * 2 + 1] ?? 0).toFixed(2) }}</span>
          </div>
        </div>
        <div class="gamepad-buttons">
          <span v-for="button in ['South', 'East', 'West', 'North', 'LeftShoulder', 'RightShoulder', 'LeftTrigger', 'RightTrigger', 'Select', 'Start', 'LeftStick', 'RightStick', 'Up', 'Down', 'Left', 'Right']"
            :key="button" :class="{ 'is-active': status?.buttons.includes(button) }">
            {{ t(`gamepad.button.${button}`) }}
          </span>
        </div>
        <p class="field-hint">{{ t("gamepad.testHint") }}</p>
      </section>
    </div>
    <p v-if="error || status?.error" class="gamepad-error" role="alert">{{ error || t(status?.error ?? "") }}</p>
    <div class="gamepad-mapping">
      <p>{{ t("gamepad.mappingGame") }}</p>
      <p>{{ t("gamepad.mappingMenu") }}</p>
      <p class="field-hint">{{ t("gamepad.mappingButtons") }}</p>
    </div>
  </SettingsGroup>
</template>

<style scoped>
.gamepad-layout { display: grid; grid-template-columns: minmax(200px, 1fr) minmax(240px, 1fr); gap: 24px; }
.gamepad-options { display: flex; flex-direction: column; gap: 10px; }
.gamepad-toggle, .gamepad-number, .gamepad-test-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.gamepad-save { display: flex; align-items: center; gap: 12px; }
.gamepad-test { display: flex; flex-direction: column; gap: 14px; padding: 16px; border: 1px solid var(--border); border-radius: 12px; background: var(--bg-card); }
.gamepad-test-head { font-size: 11.5px; color: var(--text-dim); }
.gamepad-test-head strong { font-size: 13px; color: var(--text); }
.gamepad-sticks { display: flex; justify-content: space-around; gap: 20px; }
.gamepad-stick { display: flex; flex-direction: column; align-items: center; gap: 8px; font-size: 11.5px; }
.gamepad-axis { position: relative; display: grid; place-items: center; width: 96px; height: 96px; border: 1px solid var(--border); border-radius: 50%; background: var(--accent-soft); }
.gamepad-axis::before, .gamepad-axis::after { content: ""; position: absolute; background: var(--border); }
.gamepad-axis::before { width: 100%; height: 1px; }
.gamepad-axis::after { width: 1px; height: 100%; }
.gamepad-axis i { width: 12px; height: 12px; border-radius: 50%; background: var(--accent); z-index: 1; }
.gamepad-values { font-variant-numeric: tabular-nums; color: var(--text-dim); }
.gamepad-buttons { display: flex; flex-wrap: wrap; gap: 6px; }
.gamepad-buttons span { border: 1px solid var(--border); border-radius: 8px; padding: 4px 8px; font-size: 11.5px; color: var(--text-dim); }
.gamepad-buttons .is-active { background: var(--accent-soft); border-color: var(--accent); color: var(--accent); }
.gamepad-test-head .is-active { color: var(--green); }
.gamepad-mapping { margin-top: 16px; font-size: var(--ctl-font); line-height: 1.6; }
.gamepad-error { color: var(--red); font-size: var(--ctl-font); margin-top: 12px; }
@media (max-width: 720px) { .gamepad-layout { grid-template-columns: 1fr; gap: 16px; } }
</style>
