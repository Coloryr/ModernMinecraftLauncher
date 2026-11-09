<script setup lang="ts">
import { computed, inject, ref } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import SearchBox from "../../../components/ui/SearchBox.vue";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import {
  CONFIG_MODE_INHERIT,
  type ConfigFileAction,
  type ConfigMode,
} from "../useServerInstance";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const {
  busy,
  loading,
  changedPaths,
  configDirs,
  configRows,
  configCount,
  configModeOf,
  configAncestorLock,
  configDeleteLocked,
  configFileState,
  setConfigFile,
  setConfigFileAction,
  toggleConfigFile,
  configDirOn,
  configDirEditable,
  setConfigDir,
  setConfigDeleteOld,
  configArchiveOwner,
  configRowDeleteOld,
  configUnderRecursive,
  configModeSelect,
  setAllConfigs,
  setConfigMode,
  toggleConfigExpand,
  isConfigOpen,
  isConfigLoading,
  configView,
  configQuery,
  configRowChange,
  configSearchRows,
  configGroups,
  customEntries,
  addCustomEntry,
  removeCustomEntry,
  refreshConfigs,
} = ctx;

/** 两个分组的展开状态：现有文件 / 自定义文件 */
const filesOpen = ref(true);
const customOpen = ref(true);
/** 分组收起时不渲染这一组的行 */
const filesShown = computed(() => (filesOpen.value ? configRows.value : []));
const customShown = computed(() => (customOpen.value ? customEntries.value : []));

/** 自定义条目的路径与动作 */
const customPath = ref("");
const customAction = ref<"add" | "delete">("add");
/** 添加方式：从实例选现有文件，或直接写内容 */
const customMode = ref<"file" | "content">("file");
const customContent = ref("");
const customModeOptions = computed(() => [
  { value: "file", label: t("serverInstance.customFromFile") },
  { value: "content", label: t("serverInstance.customFromContent") },
]);

/** 加一条自定义条目；成功后清空路径与内容 */
function addCustom() {
  const content =
    customAction.value === "add" && customMode.value === "content" ? customContent.value : null;
  if (!addCustomEntry(customPath.value, customAction.value, content)) return;
  customPath.value = "";
  customContent.value = "";
}

/** 下拉里的同步方式，顺序与产品口径一致 */
const MODES: ConfigMode[] = ["none", "archiveAll", "archiveSelf", "filesAll", "filesSelf", "delete"];

/** 视图切换：普通按目录树，分组按变化 */
const viewOptions = computed(() => [
  { value: "normal", label: t("serverInstance.viewNormal"), icon: "folder" },
  { value: "group", label: t("serverInstance.viewGroup"), icon: "list" },
]);

/** 变化状态对应的行样式 */
function changeClass(path: string, dir: boolean): string {
  const change = configRowChange(path, dir);
  return change ? `is-${change}` : "";
}

/** 文件复选框与下拉的说明 */
function fileTitle(path: string): string | undefined {
  const state = configFileState(path);
  if (state === "none") return t("serverInstance.configFileNoOwner");
  if (state === "locked") return t("serverInstance.configFileLocked");
  if (state === "delete") return t("serverInstance.configFileDeleteHint");
  return undefined;
}

/** 文件下拉的取值：锁定 / 无归属时按不同步显示 */
function fileActionOf(path: string): ConfigFileAction {
  const state = configFileState(path);
  if (state === "delete") return "delete";
  return state === "sync" ? "sync" : "skip";
}

/** 文件勾选状态：同步与删除都算参与这次同步，只有主动取消勾选才是不同步 */
function fileChecked(path: string): boolean {
  const state = configFileState(path);
  return state === "sync" || state === "delete";
}

/** 文件下拉禁用：没有归属或被删除目录锁住 */
function fileLocked(path: string): boolean {
  const state = configFileState(path);
  return state === "none" || state === "locked";
}

/** 目录勾选框不可用时的说明 */
function dirTitle(path: string): string | undefined {
  if (configDeleteLocked(path)) return t("serverInstance.configFileLocked");
  const lock = configAncestorLock(path);
  if (lock === "none") return t("serverInstance.configNoneLock");
  if (lock === "recursive") return t("serverInstance.configFollowHint");
  return undefined;
}

/** 这一行能不能自己改"删旧目录"：自己有压缩包、且不是删除目录 */
function deleteOldEditable(path: string): boolean {
  return configArchiveOwner(path) === path && configModeOf(path) !== "delete";
}

/** 删旧目录勾选框的说明 */
function deleteOldTitle(path: string): string {
  const owner = configArchiveOwner(path);
  if (owner === path) {
    return configModeOf(path) === "delete"
      ? t("serverInstance.configDeleteOldLocked")
      : t("serverInstance.configDeleteOldHint");
  }
  if (owner) return t("serverInstance.configDeleteOldFollow", { dir: owner });
  return t("serverInstance.configDeleteOldNone");
}

/** 文件或文件夹大小，按档显示 */
function sizeText(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  if (size < 1024 * 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  return `${(size / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
</script>

<template>
<section class="si-section stretch">
  <div class="si-section-head">
    <span class="si-counter">{{ t("serverInstance.configCount", { n: configCount }) }}</span>
    <span class="si-head-tools">
      <SearchBox v-model="configQuery" :placeholder="t('serverInstance.configSearch')" width="200px" />
      <SegmentedTabs v-model="configView" :options="viewOptions" />
    </span>
    <!-- 勾选与刷新统一放在工具条最右端 -->
    <BaseButton size="sm" :disabled="busy || !configDirs.length" @click="setAllConfigs(true)">
      {{ t("serverInstance.configSyncAll") }}
    </BaseButton>
    <BaseButton size="sm" :disabled="busy || !configDirs.length" @click="setAllConfigs(false)">
      {{ t("serverInstance.configSyncNone") }}
    </BaseButton>
    <button class="ui-btn-icon" :disabled="busy || loading"
      v-tip="t('serverInstance.refresh')" :aria-label="t('serverInstance.refresh')"
      @click="refreshConfigs">
      <GlyphIcon name="refresh" :size="14" :weight="2" />
    </button>
  </div>

  <!-- 配置表：现有文件 / 自定义文件两个分组，组行与模组表格模式同一口径 -->
  <div class="si-table-card">
    <div class="si-config-tree">
      <div class="si-config-head">
        <span>{{ t("serverInstance.configName") }}</span>
        <span class="si-config-size">{{ t("serverInstance.configSize") }}</span>
        <span>{{ t("serverInstance.configMode") }}</span>
        <span class="si-config-delete">{{ t("serverInstance.configDeleteOld") }}</span>
      </div>
      <p v-if="!configDirs.length" class="empty-tip">{{ t("serverInstance.configEmpty") }}</p>
      <!-- 搜索：跨目录的扁平结果 -->
      <template v-else-if="configQuery.trim()">
        <p v-if="!configSearchRows.length" class="empty-tip">{{ t("serverInstance.configSearchEmpty") }}</p>
        <div v-for="row in configSearchRows" :key="row.path" class="si-config-row">
          <div class="si-config-name">
            <span class="si-change-dot" :class="row.change ? `is-${row.change}` : ''"></span>
            <span class="si-name" :title="row.path">{{ row.path }}</span>
          </div>
          <span class="si-config-size"></span>
          <div class="si-config-mode-cell"></div>
          <div class="si-config-delete-cell"></div>
        </div>
      </template>
      <!-- 分组：新增 / 修改 / 删除 -->
      <template v-else-if="configView === 'group'">
        <p v-if="!configGroups.some((group) => group.paths.length)" class="empty-tip">
          {{ t("serverInstance.configNoChange") }}
        </p>
        <template v-for="group in configGroups" :key="group.key">
          <div v-if="group.paths.length" class="si-config-group">
            <div class="si-group-head" :class="`is-${group.key}`">
              {{ t(`serverInstance.group.${group.key}`) }}
              <span class="si-group-count">{{ group.paths.length }}</span>
            </div>
            <div v-for="row in group.rows" :key="row.path" class="si-config-row">
              <div class="si-config-name" :style="{ paddingLeft: `${row.depth * 16}px` }">
                <span class="si-name" :class="{ dir: row.dir }">{{ row.name }}{{ row.dir ? "/" : "" }}</span>
              </div>
              <span class="si-config-size"></span>
              <div class="si-config-mode-cell"></div>
              <div class="si-config-delete-cell"></div>
            </div>
          </div>
        </template>
      </template>
      <!-- 普通：现有文件 + 自定义文件，组行与模组表格模式同一套观感 -->
      <template v-else>
        <div class="si-config-row si-config-group-row">
          <div class="si-config-name">
            <button type="button" class="si-config-caret" @click="filesOpen = !filesOpen">
              <GlyphIcon class="si-caret-icon" :class="{ collapsed: !filesOpen }"
                name="chevron-down" :size="13" :weight="2.4" />
            </button>
            <span class="si-config-group-label" @click="filesOpen = !filesOpen">
              {{ t("serverInstance.configExisting") }}
            </span>
            <span class="si-config-group-count">
              {{ t("serverInstance.configGroupItems", { n: configDirs.length }) }}
            </span>
          </div>
          <span class="si-config-size"></span>
          <div class="si-config-mode-cell"></div>
          <div class="si-config-delete-cell"></div>
        </div>
        <div v-for="row in filesShown" :key="row.path" class="si-config-row"
          :class="[changeClass(row.path, row.dir), { 'is-changed': changedPaths.has(row.path) || (row.depth === 0 && changedPaths.has(row.root)) }]">
          <div class="si-config-name" :style="{ paddingLeft: `${row.depth * 16}px` }">
            <!-- 勾选框：文件 / 目录都可以取消同步；目录的"删旧目录"在右边单独一列 -->
            <span class="si-check">
              <input v-if="row.dir" type="checkbox" class="si-box"
                :checked="configDirOn(row.path)" :disabled="busy || !configDirEditable(row.path)"
                :title="dirTitle(row.path)"
                @change="setConfigDir(row.path, ($event.target as HTMLInputElement).checked)" />
              <input v-else type="checkbox" class="si-box"
                :checked="fileChecked(row.path)"
                :disabled="busy || fileLocked(row.path)"
                :title="fileTitle(row.path)"
                @change="setConfigFile(row.path, ($event.target as HTMLInputElement).checked)" />
            </span>
            <span class="si-caret" :class="{ open: isConfigOpen(row.path) }"
              @click="row.dir && !configDeleteLocked(row.path) && toggleConfigExpand(row.path)">
              <GlyphIcon v-if="row.dir && !configDeleteLocked(row.path)" name="chevron-down" :size="12" :weight="2.4" />
            </span>
            <span class="si-name" :class="{ dir: row.dir }" :title="row.path"
              @click="row.dir
                ? (!configDeleteLocked(row.path) && toggleConfigExpand(row.path))
                : toggleConfigFile(row.path)">
              {{ row.name }}{{ row.dir ? "/" : "" }}
            </span>
            <span v-if="isConfigLoading(row.path)" class="si-config-loading">
              {{ t("serverInstance.configLoading") }}
            </span>
          </div>
          <span class="si-config-size">{{ sizeText(row.size) }}</span>
          <div class="si-config-mode-cell">
            <template v-if="row.dir">
              <span v-if="configAncestorLock(row.path) === 'recursive'"
                class="si-config-follow" :title="t('serverInstance.configFollowHint')">
                {{ t("serverInstance.configFollow") }}
              </span>
              <span v-else-if="configAncestorLock(row.path) === 'delete'"
                class="si-config-follow" :title="t('serverInstance.configFollowHint')">
                {{ t("serverInstance.configFollow") }}
              </span>
              <!-- 上级不同步：这块不再跟着同步，但仍可以把这一层单独删掉 -->
              <select v-else-if="configAncestorLock(row.path) === 'none'"
                class="field-select si-cell-control si-config-mode"
                :value="configModeOf(row.path) === 'delete' ? 'delete' : 'none'"
                :disabled="busy" :title="t('serverInstance.configNoneDeleteHint')"
                :aria-label="t('serverInstance.configMode')"
                @change="setConfigMode(row.path, $event)">
                <option value="none">{{ t("serverInstance.sync.none") }}</option>
                <option value="delete">{{ t("serverInstance.sync.delete") }}</option>
              </select>
              <select v-else class="field-select si-cell-control si-config-mode" :value="configModeSelect(row.path)"
                :disabled="busy" :aria-label="t('serverInstance.configMode')"
                @change="setConfigMode(row.path, $event)">
                <!-- 整包 / 整目录底下才有"跟着上级"这一项：选了就不单独配方式 -->
                <option v-if="configUnderRecursive(row.path)" :value="CONFIG_MODE_INHERIT">
                  {{ t("serverInstance.configFollow") }}
                </option>
                <option v-for="mode in MODES" :key="mode" :value="mode">
                  {{ t(`serverInstance.sync.${mode}`) }}
                </option>
              </select>
            </template>
            <!-- 文件：同步 / 不进包 / 让客户端删掉这个文件 -->
            <template v-else>
              <select class="field-select si-cell-control si-config-mode"
                :value="fileActionOf(row.path)"
                :disabled="busy || fileLocked(row.path)"
                :title="fileTitle(row.path)"
                :aria-label="t('serverInstance.configFileAction')"
                @change="setConfigFileAction(row.path, ($event.target as HTMLSelectElement).value as ConfigFileAction)">
                <option value="sync">{{ t("serverInstance.configFileSync") }}</option>
                <option value="skip">{{ t("serverInstance.sync.none") }}</option>
                <option value="delete">{{ t("serverInstance.configFileDelete") }}</option>
              </select>
            </template>
          </div>
          <div class="si-config-delete-cell">
            <input v-if="row.dir" type="checkbox" class="si-box"
              :checked="configRowDeleteOld(row.path)"
              :disabled="busy || !deleteOldEditable(row.path)"
              :title="deleteOldTitle(row.path)"
              @change="setConfigDeleteOld(row.path, ($event.target as HTMLInputElement).checked)" />
          </div>
        </div>

        <div class="si-config-row si-config-group-row">
          <div class="si-config-name">
            <button type="button" class="si-config-caret" @click="customOpen = !customOpen">
              <GlyphIcon class="si-caret-icon" :class="{ collapsed: !customOpen }"
                name="chevron-down" :size="13" :weight="2.4" />
            </button>
            <span class="si-config-group-label" @click="customOpen = !customOpen">
              {{ t("serverInstance.customTitle") }}
            </span>
            <span class="si-config-group-count">
              {{ t("serverInstance.configGroupItems", { n: customEntries.length }) }}
            </span>
          </div>
          <span class="si-config-size"></span>
          <div class="si-config-mode-cell"></div>
          <div class="si-config-delete-cell"></div>
        </div>
        <!-- 添加条放在分组第一行：跟着表体一起滚，不再钉在卡片底部 -->
        <template v-if="customOpen">
          <div class="si-custom-add si-custom-add-row">
            <input v-model="customPath" class="field-input" :disabled="busy" spellcheck="false"
              :title="t('serverInstance.customHint')" :placeholder="t('serverInstance.customPath')"
              @keydown.enter="addCustom" />
            <select v-model="customAction" class="field-select" :disabled="busy">
              <option value="add">{{ t("serverInstance.customAdd") }}</option>
              <option value="delete">{{ t("serverInstance.customDelete") }}</option>
            </select>
            <SegmentedTabs v-if="customAction === 'add'" v-model="customMode" :options="customModeOptions" />
            <BaseButton size="sm" :disabled="busy || !customPath.trim()" @click="addCustom">
              {{ t("serverInstance.customAddBtn") }}
            </BaseButton>
          </div>
          <!-- 输入内容：这段文字会作为客户端上的那个文件下发 -->
          <div v-if="customAction === 'add' && customMode === 'content'" class="si-custom-add-row is-content">
            <textarea v-model="customContent" class="field-input si-custom-content" :disabled="busy"
              spellcheck="false" :placeholder="t('serverInstance.customContentHint')"></textarea>
          </div>
        </template>
        <p v-if="customOpen && !customEntries.length" class="empty-tip">
          {{ t("serverInstance.customEmpty") }}
        </p>
        <!-- 行的底色直接表示动作：添加绿、删除红，与改动对比同一套色 -->
        <div v-for="(item, index) in customShown" :key="`${item.action}:${item.path}`"
          class="si-config-row si-custom-row" :class="item.action === 'add' ? 'is-add' : 'is-del'">
          <div class="si-config-name">
            <span class="si-name" :title="item.path">{{ item.path }}</span>
          </div>
          <span class="si-config-size"></span>
          <div class="si-config-mode-cell">
            <span v-if="item.content !== null" class="si-tag content">
              {{ t("serverInstance.customContentTag") }}
            </span>
            <span class="si-tag" :class="item.action">
              {{ t(item.action === "add" ? "serverInstance.customAdd" : "serverInstance.customDelete") }}
            </span>
          </div>
          <div class="si-config-delete-cell si-custom-remove">
            <button class="ui-btn ui-btn-icon" :disabled="busy"
              :title="t('serverInstance.customRemove')" @click="removeCustomEntry(index)">
              <GlyphIcon name="close" :size="12" :weight="2.2" />
            </button>
          </div>
        </div>
      </template>
    </div>

  </div>

</section>
</template>
