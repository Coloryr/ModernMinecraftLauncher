<script setup lang="ts">
// 手柄设置页：顶部配置下拉与配置操作，接着是手柄状态图，往下才是按钮 / 轴 / 摇杆设置
//
// 状态在 useSettingsGamepad：切到别的标签时本组件会被卸载，未保存的编辑内容
// 必须留在设置窗口那一层。
import { computed, nextTick, onMounted, ref, watch } from "vue";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import SettingsGroup from "../SettingsGroup.vue";
import GamepadTestPanel from "../GamepadTestPanel.vue";
import GamepadKeyPicker from "../GamepadKeyPicker.vue";
import GamepadActionPicker from "../GamepadActionPicker.vue";
import SegmentedTabs from "../../../../components/ui/SegmentedTabs.vue";
import AxisRangeBar from "../AxisRangeBar.vue";
import { ALL_AXES, axisLabel } from "../../gamepadLabels";
import { t } from "../../../../lib/i18n";
import { useGamepadState } from "../../composables/useGamepadState";
import type {
  GamepadAxisDto,
  GamepadButtonDto,
  GamepadButtonModeDto,
  GamepadLayoutDto,
} from "../../../../lib/bindings";
import type { useSettingsGamepad } from "../../composables/useSettingsGamepad";

const props = defineProps<{
  /** 手柄标签的状态与操作，来自 useSettingsGamepad */
  settings: ReturnType<typeof useSettingsGamepad>;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();

const {
  profiles,
  editing,
  config,
  dirty,
  active,
  saving,
  actions,
  buttonMode,
  buttonRules,
  newButton,
  newAxis,
  createOpen,
  createName,
  createError,
  createBusy,
  resetOpen,
  resetBusy,
  deleteOpen,
  pendingName,
  isTaken,
  init,
  requestSelect,
  discardAndSelect,
  openCreate,
  doCreate,
  openReset,
  doReset,
  doDelete,
  activate,
  save,
  addButton,
  addAxis,
} = props.settings;

onMounted(init);

/** 新建弹窗的输入框：打开即聚焦，免得还要再点一下才能输入 */
const createInput = ref<HTMLInputElement | null>(null);
watch(createOpen, async (open) => {
  if (!open) return;
  await nextTick();
  createInput.value?.focus();
});

/**
 * 输入框回车提交
 *
 * - `event`: 键盘事件；输入法组词中的回车只用来确认候选词，不能拿去建配置
 */
function submitCreate(event: KeyboardEvent) {
  if (event.isComposing) return;
  void doCreate();
}

/** 新配置复制的是生效中的那份，弹窗里写清楚复制来源 */
const copySource = computed(() => profiles.value.find((item) => item.active)?.name ?? "");

/** 按钮触发方式的三档，顺序即下拉顺序 */
const MODES: GamepadButtonModeDto[] = ["normal", "toggle", "repeat"];

/** 按钮映射的两套：游戏内模式用游戏里那套，鼠标模式用鼠标那套 */
const modeOptions = computed(() => [
  { value: "game", label: t("winGamepad.btnModeIngame") },
  { value: "cursor", label: t("winGamepad.btnModeCursor") },
]);

/**
 * 切换按钮映射看哪一套
 *
 * - `value`: game / cursor
 */
function setButtonMode(value: string) {
  buttonMode.value = value;
}

/** 手柄实时状态：轴区间条显示当前值用 */
const { axisValue } = useGamepadState();

/**
 * 触发方式的界面文字
 *
 * - `value`: normal / toggle / repeat
 */
function modeLabel(value: GamepadButtonModeDto): string {
  const keys: Record<GamepadButtonModeDto, string> = {
    normal: "winGamepad.modeNormal",
    toggle: "winGamepad.modeToggle",
    repeat: "winGamepad.modeRepeat",
  };
  return t(keys[value]);
}

/** 后端按手柄认出来的外观；没接手柄或认不出时为 null，回落到配置里存的那份 */
const detected = ref<GamepadLayoutDto | null>(null);

/** 表格里实际用的外观 */
const effectiveLayout = computed(() => detected.value ?? config.value?.layout ?? "xbox");

/** 取轴记号 */
function axisName(axis: GamepadAxisDto): string {
  return axisLabel(axis);
}

/**
 * 下拉换配置：有未保存改动时先问一句
 *
 * 下拉自己已经先跳到新项了，答完再切；这里立刻退回当前项，免得显示的和编辑的不是同一份。
 *
 * - `event`: 下拉的 change 事件
 */
function onPick(event: Event) {
  const el = event.target as HTMLSelectElement;
  const name = el.value;
  if (name === editing.value) return;
  requestSelect(name);
  el.value = editing.value;
}

/**
 * 换外观：只改记号，映射仍是按位置走的
 *
 * - `value`: `xbox` / `playstation`
 */
function setLayout(value: GamepadLayoutDto) {
  if (config.value) config.value.layout = value;
}

/**
 * 换 A / B 位置
 *
 * - `value`: 是否交换
 */
function setSwapAb(value: boolean) {
  if (config.value) config.value.swapAb = value;
}

/**
 * 选择器改主键：清空时保留原值，避免把一行弄成没有触发键
 *
 * - `rule`: 正在编辑的按钮规则
 * - `value`: 选中的主键，null 表示用户没选
 */
function setRuleButton(rule: { button: GamepadButtonDto }, value: GamepadButtonDto | null) {
  if (!value) return;
  rule.button = value;
}

/**
 * 选择器改组合键的修饰键
 *
 * - `rule`: 正在编辑的按钮规则
 * - `value`: 修饰键，null 表示取消组合
 */
function setRuleModifier(rule: { modifier: GamepadButtonDto | null }, value: GamepadButtonDto | null) {
  rule.modifier = value;
}
</script>

<template>
  <SettingsGroup id="gamepad" :flash="flashGroup === 'gamepad'">
    <!-- 配置：下拉选一份；除保存外都是图标键，文字进提示 -->
    <div class="gp-bar">
      <span class="gp-bar-label">{{ t("winGamepad.profiles") }}</span>
      <select class="field-select gp-select" :value="editing" :aria-label="t('winGamepad.profiles')"
        @change="onPick">
        <option v-for="item in profiles" :key="item.name" :value="item.name">{{ item.name }}</option>
      </select>
      <button class="ui-btn-icon gp-activate" :class="{ 'is-active': active }" :disabled="!config || active"
        v-tip="active ? t('winGamepad.active') : t('winGamepad.activate')"
        :aria-label="active ? t('winGamepad.active') : t('winGamepad.activate')" @click="activate">
        <GlyphIcon name="check" :size="14" />
      </button>
      <span class="gp-bar-gap" />
      <!-- 未保存提示常驻占位：一出现就把整排按钮挤到下一行 -->
      <span class="gp-dirty" :class="{ 'is-on': dirty }">{{ t("winGamepad.unsaved") }}</span>
      <BaseButton size="sm" variant="accent" :disabled="!config || saving" @click="save">
        {{ t("winGamepad.save") }}
      </BaseButton>
      <button class="ui-btn-icon" :disabled="!config" v-tip="t('winGamepad.resetHint')"
        :aria-label="t('winGamepad.reset')" @click="openReset">
        <GlyphIcon name="refresh" :size="14" />
      </button>
      <button class="ui-btn-icon" :disabled="profiles.length <= 1" v-tip="t('winGamepad.delete')"
        :aria-label="t('winGamepad.delete')" @click="deleteOpen = true">
        <GlyphIcon name="trash" :size="14" />
      </button>
      <button class="ui-btn-icon" v-tip="t('winGamepad.new')" :aria-label="t('winGamepad.new')"
        @click="openCreate">
        <GlyphIcon name="plus" :size="14" />
      </button>
    </div>

    <!-- 手柄列表与状态图：没有可编辑的配置时也照样显示实时状态 -->
    <GamepadTestPanel :layout="config?.layout ?? null" :swap-ab="config?.swapAb ?? null"
      @detected="detected = $event" @update:layout="setLayout" @update:swap-ab="setSwapAb" />

    <!-- 游戏内手柄总开关：关着时只用于界面上的测试与预览 -->
    <div v-if="config" class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winGamepad.ingame") }}</span>
        <span class="switch-state">{{ t("winGamepad.ingameHint") }}</span>
      </div>
      <BaseSwitch :model-value="config.enabled" @update:model-value="(value) => config && (config.enabled = value)" />
    </div>
    <p v-if="!editing" class="empty-tip">{{ t("winGamepad.empty") }}</p>
    <p v-else-if="!config" class="empty-tip">{{ t("winGamepad.broken") }}</p>

    <template v-else>
      <section class="gp-sec">
        <h3 class="gp-sec-title">{{ t("winGamepad.buttons") }}</h3>
        <p class="field-desc">{{ t("winGamepad.buttonHint") }}</p>
        <!-- 鼠标模式单独一套时，下面这张表跟着上面的分段切 -->
        <div class="gp-button-mode">
          <div class="switch-text">
            <span class="switch-label">{{ t("winGamepad.separateCursor") }}</span>
            <span class="switch-state">{{ t("winGamepad.separateCursorHint") }}</span>
          </div>
          <BaseSwitch :model-value="config.separateCursor"
            @update:model-value="(value) => config && (config.separateCursor = value)" />
        </div>
        <SegmentedTabs v-if="config.separateCursor" class="gp-btn-mode" :model-value="buttonMode"
          :options="modeOptions" @update:model-value="setButtonMode" />
        <div class="gp-row gp-row-head gp-row-button">
          <span>{{ t("winGamepad.button") }}</span>
          <span>{{ t("winGamepad.action") }}</span>
          <span>{{ t("winGamepad.mode") }}</span>
          <span></span>
        </div>
        <div v-for="(rule, index) in buttonRules" :key="rule.button + '|' + rule.modifier" class="gp-row gp-row-button">
          <GamepadKeyPicker :button="rule.button" :modifier="rule.modifier" :layout="effectiveLayout"
            :swap-ab="config.swapAb" @update:button="(value) => setRuleButton(rule, value)"
            @update:modifier="(value) => setRuleModifier(rule, value)" />
          <GamepadActionPicker v-model="rule.action" :actions="actions" />
          <select v-model="rule.mode" class="field-select gp-mode">
            <option v-for="item in MODES" :key="item" :value="item">{{ modeLabel(item) }}</option>
          </select>
          <button class="ui-btn-icon gp-remove" v-tip="t('winGamepad.removeBinding')"
            :aria-label="t('winGamepad.removeBinding')" @click="buttonRules.splice(index, 1)">
            <GlyphIcon name="close" :size="14" />
          </button>
        </div>
        <div class="gp-row gp-row-add-button">
          <GamepadKeyPicker :button="newButton.button" :modifier="newButton.modifier" :layout="effectiveLayout"
            :swap-ab="config.swapAb" @update:button="(value) => newButton.button = value ?? 'south'"
            @update:modifier="(value) => newButton.modifier = value" />
          <GamepadActionPicker v-model="newButton.action" :actions="actions" />
          <select v-model="newButton.mode" class="field-select gp-mode">
            <option v-for="item in MODES" :key="item" :value="item">{{ modeLabel(item) }}</option>
          </select>
          <button class="ui-btn-icon"
            :disabled="isTaken(newButton.button, newButton.modifier) || newButton.modifier === newButton.button"
            v-tip="t('winGamepad.addButton')" :aria-label="t('winGamepad.addButton')" @click="addButton">
            <GlyphIcon name="plus" :size="14" />
          </button>
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
        <template v-for="(rule, index) in config.axes" :key="rule.axis">
          <div class="gp-row gp-row-axis">
            <select v-model="rule.axis" class="field-select">
              <option v-for="axis in ALL_AXES" :key="axis" :value="axis">{{ axisName(axis) }}</option>
            </select>
            <input v-model.number="rule.from" class="field-input" type="number" step="0.05" min="-1" max="1" />
            <input v-model.number="rule.to" class="field-input" type="number" step="0.05" min="-1" max="1" />
            <GamepadActionPicker v-model="rule.action" :actions="actions" />
            <button class="ui-btn-icon gp-remove" v-tip="t('winGamepad.removeBinding')"
              :aria-label="t('winGamepad.removeBinding')" @click="config.axes.splice(index, 1)">
              <GlyphIcon name="close" :size="14" />
            </button>
          </div>
          <!-- 这条轴现在的读数，落在区间里时高亮 -->
          <AxisRangeBar :from="rule.from" :to="rule.to" :value="axisValue(rule.axis)" />
        </template>
        <div class="gp-row gp-row-add-axis">
          <select v-model="newAxis.axis" class="field-select">
            <option v-for="axis in ALL_AXES" :key="axis" :value="axis">{{ axisName(axis) }}</option>
          </select>
          <input v-model.number="newAxis.from" class="field-input" type="number" step="0.05" min="-1" max="1" />
          <input v-model.number="newAxis.to" class="field-input" type="number" step="0.05" min="-1" max="1" />
          <GamepadActionPicker v-model="newAxis.action" :actions="actions" />
          <button class="ui-btn-icon" v-tip="t('winGamepad.addAxis')" :aria-label="t('winGamepad.addAxis')"
            @click="addAxis">
            <GlyphIcon name="plus" :size="14" />
          </button>
        </div>
        <AxisRangeBar :from="newAxis.from" :to="newAxis.to" :value="axisValue(newAxis.axis)" />
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
            <input v-model.number="config.mouse.deadzone" class="field-input" type="number" step="0.05" min="0"
              max="1" />
          </div>
        </div>
      </section>
    </template>

    <!-- 新建配置 -->
    <BaseModal v-if="createOpen" :title="t('winGamepad.createTitle')" @close="createOpen = false">
      <label class="field-label" for="gamepad-profile-name">{{ t("winGamepad.nameLabel") }}</label>
      <input id="gamepad-profile-name" ref="createInput" v-model="createName" class="field-input"
        spellcheck="false" autocomplete="off" maxlength="32" @keydown.enter="submitCreate" />
      <p v-if="copySource" class="field-desc">{{ t("winGamepad.createCopy", { name: copySource }) }}</p>
      <p class="field-desc">{{ t("winGamepad.nameHint") }}</p>
      <p v-if="createError" class="error-text">{{ createError }}</p>
      <div class="modal-actions">
        <BaseButton @click="createOpen = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" :disabled="!createName.trim() || createBusy" @click="doCreate">
          {{ t("winGamepad.create") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- 恢复默认键位：先确认，套用后仍是未保存状态 -->
    <BaseModal v-if="resetOpen" :title="t('winGamepad.resetTitle')" @close="resetOpen = false">
      <p class="modal-text">{{ t("winGamepad.resetConfirm") }}</p>
      <div class="modal-actions">
        <BaseButton @click="resetOpen = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="danger" :disabled="resetBusy" @click="doReset">
          {{ t("winGamepad.reset") }}
        </BaseButton>
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
  </SettingsGroup>
</template>

<style scoped>
/* 顶部配置行：下拉吃掉剩余宽度，动作按钮靠右；往下滚时吸在内容区顶部 */
.gp-bar {
  position: sticky;
  top: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 10px;
  /* 吸顶后 margin 会留在上方、露出一条滚过去的行，间距一律用 padding */
  margin: 0;
  /* 上面是分组标题与副标题，紧贴着会看着像标题的附属控件 */
  padding: 14px 0 16px;
  /* 有背景图 / 系统材质时 --bg 是半透明的，吸顶会透出下面滚过去的行，改用不透明底 */
  background: var(--bg-solid, var(--bg));
  border-bottom: 1px solid var(--border);
}

.gp-bar-label {
  flex-shrink: 0;
  font-size: 12.5px;
  color: var(--text-dim);
}

/* 顶栏图标键：方形不参与压缩，窄窗口先压下拉 */
.gp-bar .ui-btn-icon {
  flex-shrink: 0;
}

/* 生效中的配置：按钮此时禁用，默认的 0.5 透明度会把状态一起压掉，
   图标常亮强调色才看得出"当前用的是这份" */
.gp-activate.is-active:disabled {
  color: var(--accent);
  opacity: 1;
}

/* 窄窗口先压下拉，别让按钮换行 */
.gp-select {
  flex: 0 1 220px;
  min-width: 120px;
  max-width: 100%;
}

.gp-bar-gap {
  flex: 1 1 0;
  min-width: 0;
}

.gp-dirty {
  flex: 0 0 auto;
  /* 一直占着这一格，只切换可见性，出现 / 消失时整排不跟着挪 */
  visibility: hidden;
  color: var(--yellow);
  font-size: 11.5px;
  white-space: nowrap;
}

.gp-dirty.is-on {
  visibility: visible;
}

.gp-sec {
  margin-bottom: 24px;
}

/* 按钮映射上方的分离开关与模式切换 */
.gp-button-mode {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 10px;
}

.gp-btn-mode {
  margin: 10px 0 12px;
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
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 96px var(--ctl-h);
}

.gp-row-axis {
  grid-template-columns: 110px 76px 76px minmax(0, 1fr) var(--ctl-h);
}

.gp-row-head {
  color: var(--text-dim);
  font-size: 11.5px;
  padding-bottom: 2px;
}

.gp-row-add-button {
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 96px var(--ctl-h);
}

.gp-row-add-axis {
  grid-template-columns: 110px 76px 76px minmax(0, 1fr) var(--ctl-h);
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
</style>