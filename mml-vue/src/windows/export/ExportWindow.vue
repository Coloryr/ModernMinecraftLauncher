<script setup lang="ts">
// 实例导出窗口：把游戏实例打包为通用标准整合包，格式为 CurseForge zip 或 Modrinth .mrpack
//
// 目标实例按优先级取：openWindow 带入的目标参数 → `export-focus` 事件 → 实例下拉。
// 目标参数单窗口模式走 windowParams，多窗口走 URL uuid；
// `export-focus` 是窗口已存在时再次打开，由壳层推送。
// 打包在 Rust 侧后台任务执行，即 export_run，进度经 export-progress 事件上报。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import BaseSwitch from "../../components/ui/BaseSwitch.vue";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { api, onExportFocus, onExportProgress } from "../../lib/api";
import { isTauri, targetUuid, windowParams } from "../windowManager";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import type { ExportInfoDto, ExportProgressDto, InstanceInfoDto } from "../../lib/bindings";

/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

const inTauri = isTauri();

const instances = ref<InstanceInfoDto[]>([]);
const currentUuid = ref("");
const info = ref<ExportInfoDto | null>(null);
const loadingInfo = ref(false);

// ---------------- 导出配置 ----------------

const pack = ref<"curseforge" | "modrinth">("curseforge");
const packName = ref("");
const author = ref("");
const version = ref("1.0.0");
const summary = ref("");
const includeMods = ref(true);
const includeConfig = ref(true);
const includeResourcePacks = ref(true);
const includeShaderPacks = ref(true);

const packOptions = [
  { value: "curseforge", label: "CurseForge" },
  { value: "modrinth", label: "Modrinth (.mrpack)" },
];

/** 导出文件名后缀（按格式） */
const fileExt = computed(() => (pack.value === "modrinth" ? "mrpack" : "zip"));

// ---------------- 实例切换 ----------------

/** 拉选中实例的导出信息，实例名作为包名默认值 */
async function loadInfo() {
  if (!currentUuid.value) return;
  loadingInfo.value = true;
  try {
    const data = await api.getExportInfo(currentUuid.value);
    info.value = data;
    // 元数据默认值跟实例走（用户可改）
    packName.value = data.name;
    summary.value = "";
  } catch (e) {
    console.error("[ExportWindow] 加载导出信息失败", e);
    info.value = null;
  } finally {
    loadingInfo.value = false;
  }
}

watch(currentUuid, loadInfo);

// ---------------- 导出执行 ----------------

const progress = ref<ExportProgressDto | null>(null);
const running = computed(() => progress.value?.state === "running");

/** 进度百分比：仅在 running 且 total 有效时有意义 */
const percent = computed(() => {
  const p = progress.value;
  if (!p || p.total <= 0) return 0;
  return Math.min(100, Math.round((p.now / p.total) * 100));
});

/** 选保存位置后交给 Rust 侧后台导出，进度走 export-progress 事件 */
async function startExport() {
  if (!currentUuid.value || running.value) return;
  if (!inTauri) return;
  if (includeMods.value && !info.value) return;

  // 选保存位置（文件名带格式后缀）
  const { save } = await import("@tauri-apps/plugin-dialog");
  const picked = await save({
    title: t("winExport.pickSave"),
    defaultPath: `${packName.value || info.value?.name || "modpack"}.${fileExt.value}`,
    filters: [
      {
        name: pack.value === "modrinth" ? "Modrinth" : "CurseForge",
        extensions: [fileExt.value],
      },
    ],
  });
  if (!picked) return;

  progress.value = { state: "running", now: 0, total: 0, text: "", error: null, file: picked };
  try {
    await api.runExport(currentUuid.value, {
      pack: pack.value,
      file: picked,
      includeMods: includeMods.value,
      includeConfig: includeConfig.value,
      includeResourcePacks: includeResourcePacks.value,
      includeShaderPacks: includeShaderPacks.value,
      name: packName.value,
      author: author.value,
      version: version.value,
      summary: summary.value,
    });
  } catch (e) {
    showToast(tErr(e));
    progress.value = null;
  }
}

// ---------------- 事件 ----------------

let unlistenProgress: UnlistenFn | null = null;
let unlistenFocus: UnlistenFn | null = null;

/** 定位到 openWindow 带入的目标实例；没有或不存在就保持下拉默认值 */
function applyTarget() {
  const uuid = targetUuid();
  if (uuid && instances.value.some((i) => i.uuid === uuid)) {
    currentUuid.value = uuid;
  }
}

/** 重新拉实例列表并定位目标实例（首次挂载、切回本窗口都用） */
async function syncInstances() {
  try {
    instances.value = await api.getInstances();
    applyTarget();
  } catch (e) {
    console.error("[ExportWindow] 初始化失败", e);
  }
}

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载，自己补一次列表与定位
useWindowRefresh(syncInstances);

// 已经停在本窗口时又被打开：openWindow 只更新参数、不换组件，跟着参数换实例
watch(() => windowParams.value.uuid, applyTarget);

onMounted(async () => {
  await syncInstances();

  unlistenProgress = await onExportProgress((p) => {
    progress.value = p;
    if (p.state === "done") {
      showToast(t("winExport.done", { file: p.file }));
    } else if (p.state === "failed") {
      showToast(t("winExport.failed") + (p.error ? `: ${p.error}` : ""));
    }
  });

  // 窗口已存在时再次打开：壳层推送新的目标实例
  unlistenFocus = await onExportFocus((uuid) => {
    if (instances.value.some((i) => i.uuid === uuid)) {
      currentUuid.value = uuid;
    }
  });
});

onBeforeUnmount(() => {
  unlistenProgress?.();
  unlistenFocus?.();
});
</script>

<template>
  <WindowFrame body-fill :title="t('winExport.title')" @close="$emit('close')">
    <div class="export-window">
      <!-- 实例选择 -->
      <div class="row">
        <label class="field-label">{{ t("winExport.instance") }}</label>
        <select v-model="currentUuid" class="field-select">
          <option value="" disabled>{{ t("winExport.pickInstance") }}</option>
          <option v-for="i in instances" :key="i.uuid" :value="i.uuid">{{ i.name }}</option>
        </select>
      </div>

      <template v-if="info">
        <!-- 实例概况 -->
        <p class="inst-meta">
          {{ info.version }} · {{ info.loader }}<template v-if="info.loaderVersion"> {{ info.loaderVersion }}</template>
        </p>

        <!-- 导出格式 -->
        <div class="row">
          <label class="field-label">{{ t("winExport.format") }}</label>
          <SegmentedTabs v-model="pack" :options="packOptions" />
        </div>

        <!-- 元数据 -->
        <div class="grid-2">
          <div>
            <label class="field-label">{{ t("winExport.metaName") }}</label>
            <input v-model="packName" class="field-input" spellcheck="false" />
          </div>
          <div>
            <label class="field-label">{{ t("winExport.metaAuthor") }}</label>
            <input v-model="author" class="field-input" spellcheck="false" />
          </div>
          <div>
            <label class="field-label">{{ t("winExport.metaVersion") }}</label>
            <input v-model="version" class="field-input" spellcheck="false" />
          </div>
          <div>
            <label class="field-label">{{ t("winExport.metaSummary") }}</label>
            <input v-model="summary" class="field-input" spellcheck="false" />
          </div>
        </div>

        <!-- 导出内容 -->
        <h3 class="group-title">{{ t("winExport.secInclude") }}</h3>
        <div class="switch-list">
          <div class="switch-row">
            <div class="switch-text">
              <span class="switch-label">{{ t("winExport.includeMods") }}</span>
              <span class="switch-state">
                {{ t("winExport.onlineMods", { n: info.onlineMods.length }) }}
                ·
                {{ t("winExport.localMods", { n: info.localMods.length }) }}
              </span>
            </div>
            <BaseSwitch v-model="includeMods" />
          </div>
          <div class="switch-row" :class="{ dim: !info.hasConfig }">
            <span class="switch-label">{{ t("winExport.includeConfig") }}</span>
            <BaseSwitch v-model="includeConfig" :disabled="!info.hasConfig" />
          </div>
          <div class="switch-row" :class="{ dim: !info.hasResourcePacks }">
            <span class="switch-label">{{ t("winExport.includeResourcePacks") }}</span>
            <BaseSwitch v-model="includeResourcePacks" :disabled="!info.hasResourcePacks" />
          </div>
          <div class="switch-row" :class="{ dim: !info.hasShaderPacks }">
            <span class="switch-label">{{ t("winExport.includeShaderPacks") }}</span>
            <BaseSwitch v-model="includeShaderPacks" :disabled="!info.hasShaderPacks" />
          </div>
        </div>

        <!-- 导出按钮 + 进度 -->
        <div class="export-actions">
          <button class="export-btn" :disabled="running || loadingInfo" @click="startExport">
            {{ running ? t("winExport.exporting") : t("winExport.exportBtn") }}
          </button>
          <span v-if="running" class="progress-text" v-tip="progress?.text">
            {{ progress?.text }}
          </span>
        </div>
        <div v-if="running" class="progress-track">
          <div class="progress-fill" :style="{ width: percent + '%' }" />
        </div>
      </template>
      <div v-else-if="loadingInfo" class="hint-full">{{ t("winExport.loading") }}</div>
      <div v-else class="hint-full">{{ t("winExport.pickInstance") }}</div>
    </div>
  </WindowFrame>
</template>

<style scoped>
.export-window {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  min-height: 0;
  padding: 14px 16px;
  box-sizing: border-box;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
}

.row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.inst-meta {
  font-size: 12px;
  color: var(--text-dim);
  margin: 0;
}

.grid-2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 14px;
}

.group-title {
  font-size: 13px;
  font-weight: 700;
  margin: 4px 0 0;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border);
}

.switch-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.switch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.switch-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.switch-label {
  font-size: 12.5px;
  font-weight: 600;
}

.switch-state {
  font-size: 11.5px;
  color: var(--text-dim);
}

.switch-row.dim {
  opacity: 0.55;
}

.export-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 6px;
}

.export-btn {
  padding: 9px 26px;
  border: none;
  border-radius: 10px;
  background: var(--accent);
  color: #fff;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  flex-shrink: 0;
}

.export-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.progress-text {
  font-size: 12px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.progress-track {
  height: 6px;
  border-radius: 3px;
  background: var(--bg-raised);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 3px;
  background: var(--accent);
  transition: width 0.2s ease;
}

.hint-full {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  font-size: 13px;
}
</style>
