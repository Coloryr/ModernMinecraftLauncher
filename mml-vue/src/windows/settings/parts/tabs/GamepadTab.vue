<script setup lang="ts">
// 手柄设置页：左侧配置列表，右侧编辑按钮 / 轴映射与摇杆参数
//
// 状态在 useSettingsGamepad：切到别的标签时本组件会被卸载，未保存的编辑内容
// 必须留在设置窗口那一层。
import { onMounted } from "vue";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import SettingsGroup from "../SettingsGroup.vue";
import ActionSelect from "../ActionSelect.vue";
import GamepadTestPanel from "../GamepadTestPanel.vue";
import { t } from "../../../../lib/i18n";
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
  loading,
  saving,
  actions,
  newButton,
  newAxis,
  createOpen,
  createName,
  createError,
  deleteOpen,
  pendingName,
  BUTTON_LABEL,
  AXIS_LABEL,
  ALL_BUTTONS,
  ALL_AXES,
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
</script>

<template>
  <SettingsGroup id="gamepad" :flash="flashGroup === 'gamepad'">
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
          <div class="gp-head">
            <h2 class="gp-title">{{ editing }}</h2>
            <span v-if="dirty" class="gp-dirty">{{ t("winGamepad.unsaved") }}</span>
            <BaseButton size="sm" variant="accent" :disabled="!config || saving" @click="save">
              {{ t("winGamepad.save") }}
            </BaseButton>
          </div>

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
  </SettingsGroup>
</template>

<style scoped>
.gp-body {
  display: flex;
  align-items: flex-start;
  gap: 20px;
}

.gp-side {
  width: 200px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
}

.gp-side-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 0 8px;
}

.gp-side-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-dim);
}

.gp-list {
  max-height: 260px;
  overflow-y: auto;
  padding: 0 2px 4px 0;
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
  padding: 2px 6px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 10.5px;
}

.gp-side-foot {
  display: flex;
  gap: 8px;
  padding-top: 10px;
}

.gp-main {
  flex: 1;
  min-width: 0;
}

.gp-head {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
}

.gp-title {
  flex: 1;
  margin: 0;
  font-size: 15px;
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
