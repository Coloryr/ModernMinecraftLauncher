<script setup lang="ts">
// 顶栏的手柄入口：选哪个手柄、用哪份配置、开关游戏内手柄
//
// 只改当前生效的那份配置里的 device / enabled 与生效配置，映射细节仍在设置窗口的手柄页。
import { ref, watch } from "vue";
import BaseModal from "./ui/BaseModal.vue";
import BaseButton from "./ui/BaseButton.vue";
import BaseSwitch from "./ui/BaseSwitch.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import { t, tErr } from "../lib/i18n";
import { showToast } from "../lib/toast";
import {
  commands,
  type GamepadConfigDto,
  type GamepadDto,
  type GamepadProfileDto,
} from "../lib/bindings";

const props = defineProps<{
  /** 弹窗是否打开；打开时重新扫一次手柄与配置 */
  open: boolean;
}>();

const emit = defineEmits<{
  /** 请求关闭 */
  (e: "close"): void;
}>();

/** 已连接的手柄 */
const pads = ref<GamepadDto[]>([]);
/** 全部配置 */
const profiles = ref<GamepadProfileDto[]>([]);
/** 当前生效的配置名 */
const activeName = ref("");
/** 生效配置的内容，勾手柄与开关都改它再存回去 */
const config = ref<GamepadConfigDto | null>(null);
const loading = ref(false);

/** 拉手柄、配置列表与生效配置 */
async function load() {
  if (!props.open) return;
  loading.value = true;
  try {
    pads.value = await commands.gamepad.list();
    profiles.value = await commands.gamepad.getProfiles();
    const current = profiles.value.find((item) => item.active) ?? profiles.value[0];
    activeName.value = current?.name ?? "";
    config.value = activeName.value ? await commands.gamepad.getConfig(activeName.value) : null;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) void load();
  },
  { immediate: true },
);

/** 把改过的生效配置存回去 */
async function save() {
  if (!activeName.value || !config.value) return;
  try {
    await commands.gamepad.saveConfig(activeName.value, config.value);
    showToast(t("gamepadPopup.saved"));
  } catch (e) {
    showToast(tErr(e));
  }
}

/**
 * 选手柄
 *
 * - `name`: 手柄名；空串表示自动（第一个连接的手柄）
 */
async function pickDevice(name: string) {
  if (!config.value) return;
  config.value.device = name || null;
  await save();
}

/**
 * 切当前生效的配置
 *
 * - `name`: 配置名
 */
async function pickProfile(name: string) {
  if (name === activeName.value) return;
  try {
    await commands.gamepad.setActive(name);
    activeName.value = name;
    config.value = await commands.gamepad.getConfig(name);
  } catch (e) {
    showToast(tErr(e));
  }
}

/**
 * 开关游戏内手柄
 *
 * - `value`: 新状态
 */
async function toggleEnabled(value: boolean) {
  if (!config.value) return;
  config.value.enabled = value;
  await save();
}
</script>

<template>
  <BaseModal v-if="open" :title="t('gamepadPopup.title')" :width="480" @close="emit('close')">
    <section class="pick-sec">
      <div class="pick-head">
        <span class="pick-title">{{ t("gamepadPopup.pads") }}</span>
        <BaseButton size="sm" variant="ghost" class="icon-btn-head" v-tip="t('gamepadPopup.refresh')"
          :aria-label="t('gamepadPopup.refresh')" @click="load">
          <GlyphIcon name="refresh" :size="14" :weight="1.8" />
        </BaseButton>
      </div>
      <label class="pick-row" :class="{ active: !config?.device }">
        <input type="radio" name="gamepad-device" :checked="!config?.device" :disabled="!config"
          @change="pickDevice('')" />
        <span class="pick-name">{{ t("gamepadPopup.auto") }}</span>
        <span class="pick-meta">{{ t("gamepadPopup.autoHint") }}</span>
      </label>
      <label v-for="pad in pads" :key="pad.id" class="pick-row" :class="{ active: config?.device === pad.name }">
        <input type="radio" name="gamepad-device" :checked="config?.device === pad.name" :disabled="!config"
          @change="pickDevice(pad.name)" />
        <span class="pick-name">{{ pad.name }}</span>
        <span v-if="pad.layout" class="pick-meta">{{ t("gamepadPopup.layout." + pad.layout) }}</span>
      </label>
      <p v-if="!pads.length" class="empty-tip">{{ t("gamepadPopup.empty") }}</p>
    </section>

    <section class="pick-sec">
      <div class="pick-head">
        <span class="pick-title">{{ t("gamepadPopup.profiles") }}</span>
      </div>
      <label v-for="profile in profiles" :key="profile.name" class="pick-row"
        :class="{ active: profile.name === activeName }">
        <input type="radio" name="gamepad-profile" :checked="profile.name === activeName"
          @change="pickProfile(profile.name)" />
        <span class="pick-name">{{ profile.name }}</span>
        <span v-if="profile.error" class="pick-meta is-error">{{ profile.error }}</span>
      </label>
      <p v-if="!profiles.length" class="empty-tip">{{ t("gamepadPopup.emptyProfiles") }}</p>
    </section>

    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("gamepadPopup.enabled") }}</span>
        <span class="switch-state">{{ t("gamepadPopup.enabledHint") }}</span>
      </div>
      <BaseSwitch :model-value="!!config?.enabled" :disabled="!config"
        @update:model-value="toggleEnabled" />
    </div>

    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("gamepadPopup.close") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
.pick-sec {
  margin-bottom: 14px;
}

.pick-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.pick-title {
  color: var(--text-dim);
  font-size: 11.5px;
}

.pick-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid transparent;
  border-radius: 8px;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}

.pick-row:hover {
  background: var(--bg-card);
}

.pick-row.active {
  background: var(--accent-soft);
  border-color: var(--accent-border);
}

.pick-row input {
  accent-color: var(--accent);
  margin: 0;
}

.pick-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pick-meta {
  color: var(--text-dim);
  font-size: 11.5px;
}

.pick-meta.is-error {
  color: var(--red);
}

.switch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 10px;
  border-top: 1px solid var(--border);
}

.switch-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.switch-label {
  color: var(--text);
  font-size: 12.5px;
}

.switch-state {
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
