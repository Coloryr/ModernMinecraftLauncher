<script setup lang="ts">
// 添加实例 · 模式一：从头新建，含版本类型 / 版本 / 加载器 / 加载器版本 / 自定义加载器路径
// 选项数据全部来自 mml-core 的独立 ID，显示名走 i18n；含版本类型 / 加载器 / 加载器版本
// 版本类型为多选：下拉展开后是复选项；选中多个类型时，右侧版本列表按类型分组
import { computed, nextTick, onUnmounted, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import BaseButton from "../../../components/ui/BaseButton.vue";
import LoaderQueryProgress from "../../../components/LoaderQueryProgress.vue";
import type { VersionInfoDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 可选版本列表，父组件已按版本类型过滤 */
  versions: VersionInfoDto[];
  /** 版本列表是否已加载，用来区分"加载中"与"该类型下无版本" */
  versionsLoaded: boolean;
  /** 版本列表刷新中 */
  verLoading: boolean;
  /** 已选版本类型（多选，空 = 不过滤） */
  verTypes: string[];
  /** 版本类型 ID 列表（mml-core） */
  versionTypes: string[];
  /** 已选游戏版本 ID，空 = 还没选 */
  newVersion: string;
  /** 已选加载器 ID，取值来自 `loaders` */
  loader: string;
  /** 当前版本支持的加载器 ID 列表（mml-core） */
  loaders: string[];
  /** 支持列表查询中（查询期间下拉禁用） */
  loaderLoading: boolean;
  /** 支持列表查询已完成的步数 */
  queryStep: number;
  /** 支持列表查询的总步数 */
  queryTotal: number;
  /** 已选加载器版本，原版 / 自定义为空 */
  loaderVersion: string;
  /** 可用加载器版本列表，由父组件按加载器 + 游戏版本拉取 */
  loaderVersions: string[];
  /** 加载器版本列表拉取中 */
  loaderVerLoading: boolean;
  /** 当前加载器没有版本列表（原版 / 自定义），刷新按钮禁用 */
  noLoaderVersion: boolean;
  /** 自定义加载器的路径，仅 `loader === "custom"` 时用得上 */
  loaderPath: string;
  /** 校验失败：游戏版本必填（高亮 + 自动聚焦） */
  invalidVersion?: boolean;
}>();

const emit = defineEmits<{
  /** 已选版本类型变化，空数组 = 不过滤 */
  (e: "update:verTypes", v: string[]): void;
  /** 已选游戏版本变化 */
  (e: "update:newVersion", v: string): void;
  /** 已选加载器变化 */
  (e: "update:loader", v: string): void;
  /** 已选加载器版本变化 */
  (e: "update:loaderVersion", v: string): void;
  /** 自定义加载器路径变化 */
  (e: "update:loaderPath", v: string): void;
  /** 点了版本列表的刷新按钮 */
  (e: "refreshVersions"): void;
  /** 点了支持加载器列表的刷新按钮 */
  (e: "refreshLoaders"): void;
  /** 点了加载器版本列表的刷新按钮 */
  (e: "refreshLoaderVersions"): void;
}>();

const loaderPathInput = ref<HTMLInputElement | null>(null);
const versionSelect = ref<HTMLSelectElement | null>(null);

// 必填校验失败：把焦点落到版本下拉上，高亮见模板的 is-invalid
watch(
  () => props.invalidVersion,
  async (invalid) => {
    if (!invalid) return;
    await nextTick();
    versionSelect.value?.focus();
  },
);

// ================= 版本类型多选下拉 =================

const typeOpen = ref(false);
const comboEl = ref<HTMLDivElement | null>(null);

/** 点击下拉外部时收起（复选项在面板内点击不收起） */
function onDocMousedown(e: MouseEvent) {
  if (typeOpen.value && comboEl.value && !comboEl.value.contains(e.target as Node)) {
    typeOpen.value = false;
  }
}
document.addEventListener("mousedown", onDocMousedown);
onUnmounted(() => document.removeEventListener("mousedown", onDocMousedown));

/** 触发按钮文字：已选类型名，未选时显示“全部类型” */
const typeLabel = computed(() => {
  const names = props.verTypes.map((vt) => t(`add.type.${vt}`));
  return names.length ? names.join("、") : t("add.typeAll");
});

/** 版本列表按类型分组（多个类型选中时分组合并展示） */
const groupedVersions = computed(() => {
  const types = props.versionTypes.filter((vt) =>
    props.verTypes.length === 0 || props.verTypes.includes(vt),
  );
  return types
    .map((vt) => ({
      type: vt,
      items: props.versions.filter((v) => v.versionType === vt),
    }))
    .filter((g) => g.items.length > 0);
});

function toggleType(vt: string) {
  const sel = props.verTypes.includes(vt)
    ? props.verTypes.filter((x) => x !== vt)
    : [...props.verTypes, vt];
  emit("update:verTypes", sel);
}

function onLoaderChange(e: Event) {
  // 加载器版本列表由父组件 watch loader/newVersion 变化后拉取
  emit("update:loader", (e.target as HTMLSelectElement).value);
}

function onLoaderVersionChange(e: Event) {
  emit("update:loaderVersion", (e.target as HTMLSelectElement).value);
}

function onLoaderPathPick(e: Event) {
  const input = e.target as HTMLInputElement;
  if (input.files?.[0]) emit("update:loaderPath", input.files[0].name);
}
</script>

<template>
  <div class="field-row-2">
    <div class="add-field">
      <label class="field-label">{{ t("add.verType") }}</label>
      <!-- 多选下拉：展开后是复选项，勾选不关闭，点外部关闭 -->
      <div ref="comboEl" class="type-combo">
        <button type="button" class="field-select type-trigger" :aria-expanded="typeOpen" @click="typeOpen = !typeOpen">
          <span class="type-label">{{ typeLabel }}</span>
        </button>
        <div v-if="typeOpen" class="type-drop" role="listbox" aria-multiselectable="true">
          <button v-for="vt in versionTypes" :key="vt" type="button" class="type-opt" role="option"
            :aria-selected="verTypes.includes(vt)" :class="{ selected: verTypes.includes(vt) }" @click="toggleType(vt)">
            {{ t(`add.type.${vt}`) }}
          </button>
        </div>
      </div>
    </div>
    <div class="add-field">
      <label class="field-label">{{ t("add.version") }} <span class="req">*</span></label>
      <div class="path-row">
        <!-- 选中多个版本类型时按类型分组展示 -->
        <select ref="versionSelect" :value="newVersion" class="field-select" :class="{ 'is-invalid': invalidVersion }"
          @change="emit('update:newVersion', ($event.target as HTMLSelectElement).value)">
          <optgroup v-for="grp in groupedVersions" :key="grp.type" :label="t(`add.type.${grp.type}`)">
            <option v-for="v in grp.items" :key="v.id" :value="v.id">{{ v.id }}</option>
          </optgroup>
        </select>
        <BaseButton size="sm" variant="ghost" :disabled="verLoading" v-tip="t('add.versionRefresh')"
          @click="emit('refreshVersions')">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
            <polyline points="21 3 21 9 15 9" />
          </svg>
        </BaseButton>
      </div>
      <div v-if="versions.length === 0" class="empty-tip">
        {{ versionsLoaded ? t("add.noVersion") : t("add.loading") }}
      </div>
    </div>
  </div>

  <div class="field-row-2">
    <div class="add-field">
      <label class="field-label">{{ t("add.loader") }}</label>
      <div class="path-row">
        <!-- 未选中版本或查询支持列表期间禁用 -->
        <select :value="loader" class="field-select" :disabled="!newVersion || loaderLoading" @change="onLoaderChange">
          <option v-for="l in loaders" :key="l" :value="l">{{ t(`add.loader.${l}`) }}</option>
        </select>
        <BaseButton size="sm" variant="ghost" :disabled="!newVersion || loaderLoading" v-tip="t('add.loaderRefresh')"
          @click="emit('refreshLoaders')">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
            <polyline points="21 3 21 9 15 9" />
          </svg>
        </BaseButton>
      </div>
      <!-- 查询进度就地显示在这里：它属于"加载器类型"这个下拉，浮在窗口正上方离得太远 -->
      <LoaderQueryProgress v-if="loaderLoading" kind="query" :visible="true" :step="queryStep" :total="queryTotal" />
      <!-- 没选版本时的提示放下拉框下方，不作为列表项 -->
      <div v-else-if="!newVersion" class="field-hint">{{ t("add.pickVersionFirst") }}</div>
    </div>
    <div class="add-field">
      <label class="field-label">{{ t("add.loaderVersion") }}</label>
      <div class="path-row">
        <!-- 始终显示：拉取中 / 原版、自定义等无版本列表的加载器置灰 -->
        <select :value="loaderVersion" class="field-select" :disabled="loaderVerLoading || !loaderVersions.length"
          @change="onLoaderVersionChange">
          <option v-for="lv in loaderVersions" :key="lv" :value="lv">{{ lv }}</option>
        </select>
        <BaseButton size="sm" variant="ghost" :disabled="!newVersion || noLoaderVersion || loaderVerLoading"
          v-tip="t('add.loaderVerRefresh')" @click="emit('refreshLoaderVersions')">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
            <polyline points="21 3 21 9 15 9" />
          </svg>
        </BaseButton>
      </div>
      <!-- 拉取加载器版本：进度同样就地显示，别浮到窗口顶上 -->
      <LoaderQueryProgress v-if="loaderVerLoading" kind="versions" :visible="true" :step="0" :total="0" />
    </div>
  </div>

  <!-- 自定义加载器：提供路径选择 -->
  <template v-if="loader === 'custom'">
    <label class="field-label">{{ t("add.loaderPath") }}</label>
    <div class="path-row">
      <input :value="loaderPath" class="field-input" :placeholder="t('add.loaderPathPlaceholder')" spellcheck="false"
        autocomplete="off" @input="emit('update:loaderPath', ($event.target as HTMLInputElement).value)" />
      <BaseButton size="sm" variant="accent" @click="loaderPathInput?.click()">…</BaseButton>
      <input ref="loaderPathInput" type="file" accept=".jar" class="hidden-input" @change="onLoaderPathPick" />
    </div>
  </template>
</template>

<style scoped>
/* 版本类型多选下拉 */
.type-combo {
  position: relative;
}

.type-trigger {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  cursor: pointer;
  text-align: left;
}

.type-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.type-drop {
  position: absolute;
  z-index: 20;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  background: var(--bg-elev, var(--bg, #fff));
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.15));
  padding: 4px;
  max-height: 220px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
}

.type-opt {
  display: block;
  width: 100%;
  margin: 2px 0;
  padding: 7px 8px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text);
  cursor: pointer;
  font-size: 13px;
  font-family: inherit;
  text-align: left;
}

.type-opt:hover {
  background: var(--hover, rgba(0, 0, 0, 0.06));
}

/* 已选中：背景高亮 */
.type-opt.selected {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}
</style>
