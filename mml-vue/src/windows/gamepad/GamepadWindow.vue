<script setup lang="ts">
// 手柄配置窗口：左侧配置列表，右侧编辑按钮 / 轴映射与摇杆参数
//
// 配置存在运行目录的 gamepad 目录下，一份映射一个 json；保存后后端立即重新加载，
// 运行中的游戏下一次读映射就走新值。列表里的「生效中」是后端按 Selected 选出来的那份。
import { computed, onMounted, reactive, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import ActionSelect from "./parts/ActionSelect.vue";
import GamepadTestPanel from "./parts/GamepadTestPanel.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import {
  commands,
  type GamepadAxisDto,
  type GamepadButtonDto,
  type GamepadConfigDto,
  type GamepadProfileDto,
} from "../../lib/bindings";
import { isTauri } from "../windowManager";

/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

const inTauri = isTauri();

/** 按钮在手柄上的记号，设备标识不翻译 */
const BUTTON_LABEL: Record<GamepadButtonDto, string> = {
  south: "A",
  east: "B",
  north: "Y",
  west: "X",
  c: "C",
  z: "Z",
  leftTrigger: "LT",
  leftTrigger2: "LB",
  rightTrigger: "RT",
  rightTrigger2: "RB",
  select: "Select",
  start: "Start",
  mode: "Mode",
  leftThumb: "L3",
  rightThumb: "R3",
  dPadUp: "↑",
  dPadDown: "↓",
  dPadLeft: "←",
  dPadRight: "→",
};

/** 轴的记号 */
const AXIS_LABEL: Record<GamepadAxisDto, string> = {
  leftStickX: "LS X",
  leftStickY: "LS Y",
  leftZ: "LT",
  rightStickX: "RS X",
  rightStickY: "RS Y",
  rightZ: "RT",
  dPadX: "DPad X",
  dPadY: "DPad Y",
};

const ALL_BUTTONS = Object.keys(BUTTON_LABEL) as GamepadButtonDto[];
const ALL_AXES = Object.keys(AXIS_LABEL) as GamepadAxisDto[];

/** 配置列表 */
const profiles = ref<GamepadProfileDto[]>([]);
/** 正在编辑的配置名；空串表示没有配置 */
const editing = ref("");
/** 编辑中的内容；读取失败时是 null */
const config = ref<GamepadConfigDto | null>(null);
/** 最近一次读取 / 保存成功时的内容，用来判未保存 */
const origin = ref("");
const loading = ref(false);
const saving = ref(false);

/** 可选动作名 */
const actions = ref<string[]>([]);

/** 有无未保存的改动 */
const dirty = computed(() => !!config.value && JSON.stringify(config.value) !== origin.value);

/** 当前配置是否生效中 */
const active = computed(() =>
  profiles.value.some((item) => item.name === editing.value && item.active),
);

/** 新建按钮绑定时的草稿 */
const newButton = reactive({
  button: "south" as GamepadButtonDto,
  modifier: null as GamepadButtonDto | null,
  action: "Space",
});
/** 新建轴绑定时的草稿 */
const newAxis = reactive({ axis: "leftStickY" as GamepadAxisDto, from: -1, to: -0.35, action: "W" });

const createOpen = ref(false);
const createName = ref("");
const createError = ref("");
const deleteOpen = ref(false);
/** 待切换的配置名：有未保存改动时先问一句 */
const pendingName = ref<string | null>(null);

/** 按钮是否已经有普通绑定，加新行时用来挑下一个按钮 */
function isBound(button: GamepadButtonDto): boolean {
  return !!config.value?.buttons.some((rule) => rule.button === button && !rule.modifier);
}

/** 同一个按钮加同一个修饰键只能绑一条 */
function isTaken(button: GamepadButtonDto, modifier: GamepadButtonDto | null): boolean {
  return !!config.value?.buttons.some(
    (rule) => rule.button === button && (rule.modifier ?? null) === modifier,
  );
}

/** 配置名是否合法，与后端校验同一口径 */
function validName(name: string): boolean {
  return (
    name.length > 0 &&
    name.length <= 32 &&
    !name.startsWith(".") &&
    !name.endsWith(".") &&
    !/[\\/:*?"<>|]/.test(name)
  );
}

/** 拉配置列表 */
async function loadProfiles() {
  profiles.value = await commands.gamepad.getProfiles();
}

/** 打开一份配置；失败时保留编辑状态但清空内容，界面上给删除入口 */
async function openProfile(name: string) {
  editing.value = name;
  config.value = null;
  origin.value = "";
  if (!name) return;

  loading.value = true;
  try {
    const data = await commands.gamepad.getConfig(name);
    config.value = data;
    origin.value = JSON.stringify(data);
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 首次装载：动作表 + 配置列表，默认打开生效中的那份 */
async function init() {
  if (!inTauri) return;
  try {
    actions.value = await commands.gamepad.getActions();
    await loadProfiles();
    const current = profiles.value.find((item) => item.active) ?? profiles.value[0];
    await openProfile(current?.name ?? "");
  } catch (e) {
    showToast(tErr(e));
  }
}

onMounted(init);

/** 单窗口模式切回本页时只刷新列表，不动正在编辑的内容 */
function refreshProfiles() {
  loadProfiles().catch((e) => showToast(tErr(e)));
}

useWindowRefresh(refreshProfiles);

/** 点列表项：有未保存改动时先确认 */
function requestSelect(name: string) {
  if (name === editing.value) return;
  if (dirty.value) {
    pendingName.value = name;
    return;
  }
  void openProfile(name);
}

/** 放弃改动并切换 */
function discardAndSelect() {
  const name = pendingName.value;
  pendingName.value = null;
  if (name) void openProfile(name);
}

/** 新建：先保存未落盘的改动，再打开输入名字的弹窗 */
async function openCreate() {
  if (dirty.value && !(await save())) return;
  createName.value = "";
  createError.value = "";
  createOpen.value = true;
}

/** 建一份新配置，内容复制当前生效的那份 */
async function doCreate() {
  const name = createName.value.trim();
  if (!validName(name)) {
    createError.value = t("winGamepad.nameInvalid");
    return;
  }
  try {
    await commands.gamepad.createProfile(name);
    createOpen.value = false;
    await loadProfiles();
    await openProfile(name);
  } catch (e) {
    createError.value = tErr(e);
  }
}

/** 删除当前编辑的配置 */
async function doDelete() {
  if (!editing.value) return;
  try {
    await commands.gamepad.deleteProfile(editing.value);
  } catch (e) {
    showToast(tErr(e));
    deleteOpen.value = false;
    return;
  }
  deleteOpen.value = false;
  await loadProfiles();
  const next = profiles.value.find((item) => item.active) ?? profiles.value[0];
  await openProfile(next?.name ?? "");
}

/**
 * 把当前配置设为生效；后端会写 Selected
 *
 * 只把 selected 同步到编辑内容与保存基线，未保存的改动仍然算未保存。
 */
async function activate() {
  if (!editing.value) return;
  try {
    await commands.gamepad.setActive(editing.value);
    await loadProfiles();
    if (config.value) config.value.selected = true;
    if (origin.value) {
      const saved = JSON.parse(origin.value) as GamepadConfigDto;
      saved.selected = true;
      origin.value = JSON.stringify(saved);
    }
  } catch (e) {
    showToast(tErr(e));
  }
}

/**
 * 把输入框可能留下的空串归一成数字
 *
 * `<input type="number">` 清空时 `v-model.number` 给的是空串，直接发给后端会反序列化失败。
 */
function normalize(data: GamepadConfigDto) {
  data.mouse.cursorSpeed = Number(data.mouse.cursorSpeed);
  data.mouse.turnSpeed = Number(data.mouse.turnSpeed);
  data.mouse.deadzone = Number(data.mouse.deadzone);
  for (const rule of data.axes) {
    rule.from = Number(rule.from);
    rule.to = Number(rule.to);
  }
}

/** 保存前在本地校验，避免只拿到一句泛化的后端错误 */
function validate(data: GamepadConfigDto): string | null {
  if (!Number.isFinite(data.mouse.cursorSpeed) || !Number.isFinite(data.mouse.turnSpeed)) {
    return t("winGamepad.invalidSpeed");
  }
  if (data.mouse.cursorSpeed < 0 || data.mouse.turnSpeed < 0) {
    return t("winGamepad.invalidSpeed");
  }
  if (!Number.isFinite(data.mouse.deadzone) || data.mouse.deadzone < 0 || data.mouse.deadzone >= 1) {
    return t("winGamepad.invalidDeadzone");
  }
  for (const rule of data.axes) {
    if (!rule.action) return t("winGamepad.invalidAction");
    const ok =
      Number.isFinite(rule.from) &&
      Number.isFinite(rule.to) &&
      rule.from >= -1 &&
      rule.from <= 1 &&
      rule.to >= -1 &&
      rule.to <= 1 &&
      rule.from !== rule.to;
    if (!ok) return t("winGamepad.invalidRange");
  }
  for (const rule of data.buttons) {
    if (!rule.action) return t("winGamepad.invalidAction");
    if (rule.modifier === rule.button) return t("winGamepad.invalidModifier");
  }
  return null;
}

/**
 * 保存当前配置，后端编译通过才落盘
 *
 * # 返回值
 *
 * 保存成功返回 true；没打开配置、校验失败或后端拒绝时返回 false
 */
async function save(): Promise<boolean> {
  if (!config.value || !editing.value) return false;
  normalize(config.value);
  const problem = validate(config.value);
  if (problem) {
    showToast(problem);
    return false;
  }

  saving.value = true;
  try {
    await commands.gamepad.saveConfig(editing.value, config.value);
    origin.value = JSON.stringify(config.value);
    showToast(t("winGamepad.saved"));
    return true;
  } catch (e) {
    showToast(tErr(e));
    return false;
  } finally {
    saving.value = false;
  }
}

/** 加一条按钮绑定，加完自动跳到下一个没有普通绑定的按钮 */
function addButton() {
  if (!config.value || isTaken(newButton.button, newButton.modifier)) return;
  config.value.buttons.push({
    button: newButton.button,
    modifier: newButton.modifier,
    action: newButton.action,
  });
  const next = ALL_BUTTONS.find((button) => !isBound(button));
  if (next) newButton.button = next;
}

/** 加一条轴绑定 */
function addAxis() {
  if (!config.value) return;
  config.value.axes.push({
    axis: newAxis.axis,
    from: newAxis.from,
    to: newAxis.to,
    action: newAxis.action,
  });
}
</script>

<template>
  <WindowFrame :title="t('winTitle.gamepad')" body-fill body-flush @close="$emit('close')">
    <template #head-right>
      <span v-if="dirty" class="gp-dirty">{{ t("winGamepad.unsaved") }}</span>
      <BaseButton size="sm" variant="accent" :disabled="!config || saving" @click="save">
        {{ t("winGamepad.save") }}
      </BaseButton>
    </template>

    <div class="gp-body">
      <!-- 左侧：配置文件列表 + 新建 / 生效 / 删除 -->
      <aside class="gp-side">
        <div class="gp-side-head">
          <span class="gp-side-title">{{ t("winGamepad.profiles") }}</span>
          <BaseButton size="sm" @click="openCreate">{{ t("winGamepad.new") }}</BaseButton>
        </div>

        <div class="gp-list">
          <button
            v-for="item in profiles"
            :key="item.name"
            class="gp-item"
            :class="{ 'is-active': item.name === editing, 'is-broken': !!item.error }"
            :title="item.error ?? ''"
            @click="requestSelect(item.name)"
          >
            <span class="gp-item-name">{{ item.name }}</span>
            <span v-if="item.active" class="gp-badge">{{ t("winGamepad.active") }}</span>
          </button>
          <p v-if="!profiles.length" class="empty-tip">{{ t("winGamepad.empty") }}</p>
        </div>

        <div class="gp-side-foot">
          <BaseButton size="sm" :disabled="!config || active" @click="activate">
            {{ t("winGamepad.activate") }}
          </BaseButton>
          <BaseButton size="sm" variant="danger" :disabled="!editing" @click="deleteOpen = true">
            {{ t("winGamepad.delete") }}
          </BaseButton>
        </div>
      </aside>

      <!-- 右侧：映射编辑 -->
      <section class="gp-main" :aria-busy="loading">
        <p v-if="!editing" class="empty-tip">{{ t("winGamepad.empty") }}</p>
        <p v-else-if="!config" class="empty-tip">{{ t("winGamepad.broken") }}</p>

        <template v-else>
          <h2 class="gp-title">{{ editing }}</h2>

          <GamepadTestPanel />
          <section class="gp-sec">
            <h3 class="gp-sec-title">{{ t("winGamepad.buttons") }}</h3>
            <p class="field-desc">{{ t("winGamepad.buttonHint") }}</p>
            <div class="gp-row gp-row-head gp-row-button">
              <span>{{ t("winGamepad.button") }}</span>
              <span>{{ t("winGamepad.modifier") }}</span>
              <span>{{ t("winGamepad.action") }}</span>
              <span></span>
            </div>
            <div v-for="(rule, index) in config.buttons" :key="index" class="gp-row gp-row-button">
              <span class="gp-key">{{ BUTTON_LABEL[rule.button] }}</span>
              <select v-model="rule.modifier" class="field-select">
                <option :value="null">{{ t("winGamepad.modifierNone") }}</option>
                <option
                  v-for="button in ALL_BUTTONS"
                  :key="button"
                  :value="button"
                  :disabled="button === rule.button"
                >
                  {{ BUTTON_LABEL[button] }}
                </option>
              </select>
              <ActionSelect v-model="rule.action" :actions="actions" />
              <BaseButton size="sm" variant="ghost" class="gp-remove" @click="config.buttons.splice(index, 1)">
                ✕
              </BaseButton>
            </div>
            <div class="gp-row gp-row-add-button">
              <select v-model="newButton.button" class="field-select">
                <option v-for="button in ALL_BUTTONS" :key="button" :value="button">
                  {{ BUTTON_LABEL[button] }}
                </option>
              </select>
              <select v-model="newButton.modifier" class="field-select">
                <option :value="null">{{ t("winGamepad.modifierNone") }}</option>
                <option
                  v-for="button in ALL_BUTTONS"
                  :key="button"
                  :value="button"
                  :disabled="button === newButton.button"
                >
                  {{ BUTTON_LABEL[button] }}
                </option>
              </select>
              <ActionSelect v-model="newButton.action" :actions="actions" />
              <BaseButton
                size="sm"
                :disabled="isTaken(newButton.button, newButton.modifier) || newButton.modifier === newButton.button"
                @click="addButton"
              >
                {{ t("winGamepad.addButton") }}
              </BaseButton>
            </div>
          </section>

          <section class="gp-sec">
            <h3 class="gp-sec-title">{{ t("winGamepad.axes") }}</h3>
            <p class="field-desc">{{ t("winGamepad.axisHint") }}</p>
            <div class="gp-row gp-row-head gp-row-axis">
              <span>{{ t("winGamepad.axis") }}</span>
              <span>{{ t("winGamepad.from") }}</span>
              <span>{{ t("winGamepad.to") }}</span>
              <span>{{ t("winGamepad.action") }}</span>
              <span></span>
            </div>
            <div v-for="(rule, index) in config.axes" :key="index" class="gp-row gp-row-axis">
              <select v-model="rule.axis" class="field-select">
                <option v-for="axis in ALL_AXES" :key="axis" :value="axis">{{ AXIS_LABEL[axis] }}</option>
              </select>
              <input v-model.number="rule.from" class="field-input" type="number" step="0.05" min="-1" max="1" />
              <input v-model.number="rule.to" class="field-input" type="number" step="0.05" min="-1" max="1" />
              <ActionSelect v-model="rule.action" :actions="actions" />
              <BaseButton size="sm" variant="ghost" class="gp-remove" @click="config.axes.splice(index, 1)">
                ✕
              </BaseButton>
            </div>
            <div class="gp-row gp-row-add-axis">
              <select v-model="newAxis.axis" class="field-select">
                <option v-for="axis in ALL_AXES" :key="axis" :value="axis">{{ AXIS_LABEL[axis] }}</option>
              </select>
              <input v-model.number="newAxis.from" class="field-input" type="number" step="0.05" min="-1" max="1" />
              <input v-model.number="newAxis.to" class="field-input" type="number" step="0.05" min="-1" max="1" />
              <ActionSelect v-model="newAxis.action" :actions="actions" />
              <BaseButton size="sm" @click="addAxis">{{ t("winGamepad.addAxis") }}</BaseButton>
            </div>
          </section>

          <section class="gp-sec">
            <h3 class="gp-sec-title">{{ t("winGamepad.mouse") }}</h3>
            <div class="gp-field-row">
              <div class="gp-field">
                <label class="field-label">{{ t("winGamepad.cursorStick") }}</label>
                <select v-model="config.mouse.cursorStick" class="field-select">
                  <option value="left">{{ t("winGamepad.stickLeft") }}</option>
                  <option value="right">{{ t("winGamepad.stickRight") }}</option>
                </select>
              </div>
              <div class="gp-field">
                <label class="field-label">{{ t("winGamepad.turnStick") }}</label>
                <select v-model="config.mouse.turnStick" class="field-select">
                  <option value="left">{{ t("winGamepad.stickLeft") }}</option>
                  <option value="right">{{ t("winGamepad.stickRight") }}</option>
                </select>
              </div>
              <div class="gp-field">
                <label class="field-label">{{ t("winGamepad.cursorSpeed") }}</label>
                <input v-model.number="config.mouse.cursorSpeed" class="field-input" type="number" step="50" min="0" />
              </div>
              <div class="gp-field">
                <label class="field-label">{{ t("winGamepad.turnSpeed") }}</label>
                <input v-model.number="config.mouse.turnSpeed" class="field-input" type="number" step="50" min="0" />
              </div>
              <div class="gp-field">
                <label class="field-label">{{ t("winGamepad.deadzone") }}</label>
                <input v-model.number="config.mouse.deadzone" class="field-input" type="number" step="0.05" min="0" max="1" />
              </div>
            </div>
          </section>
        </template>
      </section>
    </div>

    <!-- 新建配置 -->
    <BaseModal v-if="createOpen" :title="t('winGamepad.createTitle')" @close="createOpen = false">
      <label class="field-label">{{ t("winGamepad.nameLabel") }}</label>
      <input v-model="createName" class="field-input" spellcheck="false" @keyup.enter="doCreate" />
      <p class="field-desc">{{ t("winGamepad.nameHint") }}</p>
      <p v-if="createError" class="error-text">{{ createError }}</p>
      <div class="modal-actions">
        <BaseButton @click="createOpen = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="doCreate">{{ t("winGamepad.create") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 删除确认 -->
    <BaseModal v-if="deleteOpen" :title="t('winGamepad.delete')" @close="deleteOpen = false">
      <p class="modal-text">{{ t("winGamepad.deleteConfirm", { name: editing }) }}</p>
      <div class="modal-actions">
        <BaseButton @click="deleteOpen = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="doDelete">{{ t("winGamepad.delete") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 切换前提示未保存 -->
    <BaseModal v-if="pendingName" :title="t('winGamepad.discardTitle')" @close="pendingName = null">
      <p class="modal-text">{{ t("winGamepad.discardText") }}</p>
      <div class="modal-actions">
        <BaseButton @click="pendingName = null">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="discardAndSelect">{{ t("winGamepad.discard") }}</BaseButton>
      </div>
    </BaseModal>
  </WindowFrame>
</template>

<style scoped>
/* 两栏：左侧配置列表贴边，右侧表单自己滚 */
.gp-body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.gp-side {
  width: 240px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--border);
  background: var(--bg-side);
}

.gp-side-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 14px 14px 10px;
}

.gp-side-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-dim);
}

.gp-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 8px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.gp-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.gp-item:hover {
  background: var(--bg-hover);
}

.gp-item.is-active {
  background: var(--accent-soft);
  color: var(--accent);
}

/* 读不出来的配置：名字标红，原因在悬停提示里 */
.gp-item.is-broken .gp-item-name {
  color: var(--red);
}

.gp-item-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.gp-badge {
  flex-shrink: 0;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 10.5px;
}

.gp-side-foot {
  display: flex;
  gap: 8px;
  padding: 10px 14px 14px;
  border-top: 1px solid var(--border);
}

.gp-main {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 18px 22px 26px;
  scrollbar-gutter: stable;
}

.gp-title {
  margin: 0 0 14px;
  font-size: 15px;
}

.gp-sec {
  margin-bottom: 22px;
}

.gp-sec-title {
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 600;
}

/* 表格行：按钮与轴各一套列宽，表头与数据行共用 */
.gp-row {
  display: grid;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
}

.gp-row-button {
  grid-template-columns: 64px 110px minmax(0, 1fr) 30px;
}

.gp-row-axis {
  grid-template-columns: 110px 76px 76px minmax(0, 1fr) 30px;
}

.gp-row-head {
  color: var(--text-dim);
  font-size: 11.5px;
  padding-bottom: 2px;
}

.gp-row-add-button {
  grid-template-columns: 64px 110px minmax(0, 1fr) auto;
}

.gp-row-add-axis {
  grid-template-columns: 110px 76px 76px minmax(0, 1fr) auto;
}

/* 新增行与上面的已有绑定用虚线隔开 */
.gp-row-add-button,
.gp-row-add-axis {
  margin-top: 8px;
  padding-top: 10px;
  border-top: 1px dashed var(--border);
}

.gp-key {
  font-weight: 600;
}

.gp-remove {
  justify-self: end;
}

.gp-field-row {
  display: flex;
  gap: 10px;
}

.gp-field {
  display: flex;
  flex-direction: column;
  flex: 1 1 0;
  min-width: 0;
}

.gp-dirty {
  color: var(--yellow);
  font-size: 11.5px;
  white-space: nowrap;
}
</style>
