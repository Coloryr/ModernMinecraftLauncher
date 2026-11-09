<script setup lang="ts">
// 服务器包更新的可选模组选择：清单里这些可选模组这次会变，让玩家去掉不想更新的
//
// 常驻在 App 里、跟 ServerPackUpdateModal 一起；勾着的下载新版本，没勾的这次跳过，
// 本地旧文件原样留着。
import { computed, ref, watch } from "vue";
import { t } from "../lib/i18n";
import BaseButton from "./ui/BaseButton.vue";
import BaseModal from "./ui/BaseModal.vue";
import type { ServerPackModDto } from "../lib/bindings";
import {
  answerServerPackMods,
  serverPackModsInstall,
  serverPackModsList,
  serverPackModsOpen,
} from "../lib/serverPackMods";

/** 勾选中的文件路径 */
const checked = ref<Set<string>>(new Set());

// 每次打开重铺勾选：上一次的选择不能带到下一次
watch(serverPackModsOpen, (open) => {
  if (!open) return;
  checked.value = new Set(serverPackModsList.value.map((item) => item.path));
});

/**
 * 这一条勾着没
 *
 * - `item`: 列表里的一行
 */
function isChecked(item: ServerPackModDto): boolean {
  return checked.value.has(item.path);
}

/**
 * 勾选 / 取消这一条
 *
 * - `item`: 列表里的一行
 */
function toggle(item: ServerPackModDto) {
  const next = new Set(checked.value);
  if (next.has(item.path)) next.delete(item.path);
  else next.add(item.path);
  checked.value = next;
}

/** 全勾了 */
const allChecked = computed(
  () =>
    serverPackModsList.value.length > 0 &&
    checked.value.size === serverPackModsList.value.length,
);

/** 勾了一部分：全选框显示成半选 */
const someChecked = computed(() => checked.value.size > 0 && !allChecked.value);

/** 全选 / 全不选 */
function toggleAll() {
  checked.value = allChecked.value
    ? new Set()
    : new Set(serverPackModsList.value.map((item) => item.path));
}

/** 确认：没勾的就是这次不更新；一个都不勾等于只同步必装文件 */
function apply() {
  answerServerPackMods(
    serverPackModsList.value
      .filter((item) => !isChecked(item))
      .map((item) => item.path),
  );
}
</script>

<template>
  <BaseModal v-if="serverPackModsOpen"
    :title="t(serverPackModsInstall ? 'update.serverPackModsTitleInstall' : 'update.serverPackModsTitle')"
    :width="620" :closable="false" @close="answerServerPackMods(null)">
    <p class="mods-hint">
      {{ t(serverPackModsInstall ? "update.serverPackModsHintInstall" : "update.serverPackModsHint") }}
    </p>
    <div class="mods-toolbar">
      <label class="mods-all">
        <input type="checkbox" class="mods-check" :checked="allChecked" :indeterminate="someChecked"
          @change="toggleAll" />
        <span>{{ t("resource.selectAll") }}</span>
      </label>
      <span class="mods-count">
        {{ t("update.serverPackModsCount", { n: checked.size, total: serverPackModsList.length }) }}
      </span>
    </div>
    <div class="mods-list">
      <label v-for="item in serverPackModsList" :key="item.path" class="mods-row"
        :class="{ 'mods-row-on': isChecked(item) }">
        <input type="checkbox" class="mods-check" :checked="isChecked(item)" @change="toggle(item)" />
        <span class="mods-body">
          <span class="mods-head">
            <span class="mods-name">{{ item.name }}</span>
            <span v-if="item.added" class="mods-tag">{{ t("update.serverPackModsAdded") }}</span>
            <span v-else-if="!item.enabled && !serverPackModsInstall" class="mods-tag">
              {{ t("update.serverPackModsDisabled") }}
            </span>
          </span>
          <span class="mods-path">{{ item.path }}</span>
          <span v-if="item.text" class="mods-text">{{ item.text }}</span>
        </span>
      </label>
    </div>
    <div class="modal-actions">
      <BaseButton @click="answerServerPackMods(null)">{{ t("resource.cancel") }}</BaseButton>
      <BaseButton variant="accent" @click="apply">
        {{ t(serverPackModsInstall ? "update.serverPackModsConfirmInstall" : "update.serverPackModsConfirm") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 说明行：为什么会有这个弹窗 */
.mods-hint {
  margin: 0 0 8px;
  color: var(--text-dim);
  font-size: 11.5px;
  line-height: 1.6;
}

.mods-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 6px;
}

.mods-all {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
}

/* 弹窗挂在 body 下，拿不到窗口级样式，勾选框走原生 + accent-color */
.mods-check {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  margin: 0;
  accent-color: var(--accent);
  cursor: pointer;
}

.mods-count {
  color: var(--text-dim);
  font-size: 11.5px;
}

/* 一行一条，密集列表不用卡片：一屏能看清更多条目 */
.mods-list {
  max-height: 52vh;
  overflow-y: auto;
  border-top: 1px solid var(--border);
}

.mods-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  cursor: pointer;
}

/* 勾中的那条给主色浅底：一眼看得出这次会更新哪些 */
.mods-row-on {
  background: var(--accent-soft);
}

.mods-body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.mods-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 3px 8px;
}

.mods-name {
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  word-break: break-all;
}

.mods-tag {
  padding: 0 6px;
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-dim);
  font-size: 10.5px;
}

.mods-path {
  margin-top: 2px;
  color: var(--text-dim);
  font-size: 11.5px;
  word-break: break-all;
}

/* 发布端写的说明：跟在路径下面，最多两行，别把弹窗撑成整页 */
.mods-text {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  margin-top: 2px;
  color: var(--text-dim);
  font-size: 11.5px;
  line-height: 1.5;
  word-break: break-word;
}
</style>
