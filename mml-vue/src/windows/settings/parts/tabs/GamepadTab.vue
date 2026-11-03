<script setup lang="ts">
// 手柄设置页：顶部配置下拉与配置操作，接着是手柄状态图，往下才是按钮 / 轴 / 摇杆设置
//
// 状态在 useSettingsGamepad：切到别的标签时本组件会被卸载，未保存的编辑内容
// 必须留在设置窗口那一层。
import { computed, onMounted, ref } from "vue";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import SettingsGroup from "../SettingsGroup.vue";
import GamepadTestPanel from "../GamepadTestPanel.vue";
import GamepadKeyPicker from "../GamepadKeyPicker.vue";
import GamepadActionPicker from "../GamepadActionPicker.vue";
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
  newButton,
  newAxis,
  createOpen,
  createName,
  createError,
  deleteOpen,
  pendingName,
  isTaken,
  init,
  requestSelect,
  discardAndSelect,
  openCreate,
  doCreate,
  doDelete,
  activate,
  save,
  addButton,
  addAxis,
} = props.settings;

onMounted(init);

/** 按钮触发方式的三档，顺序即下拉顺序 */
const MODES: GamepadButtonModeDto[] = ["normal", "toggle", "repeat"];

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
    <!-- 配置：下拉选一份，右边是保存 / 删除 / 新建 -->
    <div class="gp-bar">
      <span class="gp-bar-label">{{ t("winGamepad.profiles") }}</span>
      <select class="field-select gp-select" :value="editing" :aria-label="t('winGamepad.profiles')"
        @change="onPick">
        <option v-for="item in profiles" :key="item.name" :value="item.name">{{ item.name }}</option>
      </select>
      <BaseButton size="sm" :disabled="!config || active" @click="activate">
        {{ active ? t("winGamepad.active") : t("winGamepad.activate") }}
      </BaseButton>
      <span class="gp-bar-gap" />
      <span v-if="dirty" class="gp-dirty">{{ t("winGamepad.unsaved") }}</span>
      <BaseButton size="sm" variant="accent" :disabled="!config || saving" @click="save">
        {{ t("winGamepad.save") }}
      </BaseButton>
      <BaseButton size="sm" variant="danger" :disabled="profiles.length <= 1" @click="deleteOpen = true">
        {{ t("winGamepad.delete") }}
      </BaseButton>
      <BaseButton size="sm" @click="openCreate">{{ t("winGamepad.new") }}</BaseButton>
    </div>

    <!-- 手柄列表与状态图：没有可编辑的配置时也照样显示实时状态 -->
    <GamepadTestPanel :layout="config?.layout ?? null" :swap-ab="config?.swapAb ?? null"
      @detected="detected = $event" @update:layout="setLayout" @update:swap-ab="setSwapAb" />

    <p v-if="!editing" class="empty-tip">{{ t("winGamepad.empty") }}</p>
    <p v-else-if="!config" class="empty-tip">{{ t("winGamepad.broken") }}</p>

    <template v-else>
      <section class="gp-sec">
        <h3 class="gp-sec-title">{{ t("winGamepad.buttons") }}</h3>
        <p class="field-desc">{{ t("winGamepad.buttonHint") }}</p>
        <div class="gp-row gp-row-head gp-row-button">
          <span>{{ t("winGamepad.button") }}</span>
          <span>{{ t("winGamepad.action") }}</span>
          <span>{{ t("winGamepad.mode") }}</span>
          <span></span>
        </div>
        <div v-for="(rule, index) in config.buttons" :key="index" class="gp-row gp-row-button">
          <GamepadKeyPicker :button="rule.button" :modifier="rule.modifier" :layout="effectiveLayout"
            :swap-ab="config.swapAb" @update:button="(value) => setRuleButton(rule, value)"
            @update:modifier="(value) => setRuleModifier(rule, value)" />
          <GamepadActionPicker v-model="rule.action" :actions="actions" />
          <select v-model="rule.mode" class="field-select gp-mode">
            <option v-for="item in MODES" :key="item" :value="item">{{ modeLabel(item) }}</option>
          </select>
          <BaseButton size="sm" variant="ghost" class="gp-remove" @click="config.buttons.splice(index, 1)">
            ✕
          </BaseButton>
        </div>
        <div class="gp-row gp-row-add-button">
          <GamepadKeyPicker :button="newButton.button" :modifier="newButton.modifier" :layout="effectiveLayout"
            :swap-ab="config.swapAb" @update:button="(value) => newButton.button = value ?? 'south'"
            @update:modifier="(value) => newButton.modifier = value" />
          <GamepadActionPicker v-model="newButton.action" :actions="actions" />
          <select v-model="newButton.mode" class="field-select gp-mode">
            <option v-for="item in MODES" :key="item" :value="item">{{ modeLabel(item) }}</option>
          </select>
          <BaseButton size="sm"
            :disabled="isTaken(newButton.button, newButton.modifier) || newButton.modifier === newButton.button"
            @click="addButton">
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
        <template v-for="(rule, index) in config.axes" :key="index">
          <div class="gp-row gp-row-axis">
            <select v-model="rule.axis" class="field-select">
              <option v-for="axis in ALL_AXES" :key="axis" :value="axis">{{ axisName(axis) }}</option>
            </select>
            <input v-model.number="rule.from" class="field-input" type="number" step="0.05" min="-1" max="1" />
            <input v-model.number="rule.to" class="field-input" type="number" step="0.05" min="-1" max="1" />
            <GamepadActionPicker v-model="rule.action" :actions="actions" />
              <BaseButton size="sm" variant="ghost" class="gp-remove" @click="config.axes.splice(index, 1)">
                ✕
              </BaseButton>
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
          <BaseButton size="sm" @click="addAxis">{{ t("winGamepad.addAxis") }}</BaseButton>
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
  </SettingsGroup>
</template>

<style scoped>
/* 顶部配置行：下拉吃掉剩余宽度，动作按钮靠右 */
.gp-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 16px;
}

.gp-bar-label {
  flex-shrink: 0;
  font-size: 12.5px;
  color: var(--text-dim);
}

.gp-select {
  width: 220px;
  max-width: 100%;
}

.gp-bar-gap {
  flex: 1;
}

.gp-dirty {
  color: var(--yellow);
  font-size: 11.5px;
  white-space: nowrap;
}

.gp-sec {
  margin-bottom: 24px;
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
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 96px 30px;
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
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 96px auto;
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
</style>