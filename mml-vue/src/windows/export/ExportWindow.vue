<script setup lang="ts">
// 实例导出窗口：把游戏实例打包为通用标准整合包，格式为 CurseForge zip 或 Modrinth .mrpack
//
// 目标实例按优先级取：openWindow 带入的目标参数 → `export-focus` 事件 → 实例下拉。
// 目标参数单窗口模式走 windowParams，多窗口走 URL uuid；
// `export-focus` 是窗口已存在时再次打开，由壳层推送。
// 打包在 Rust 侧后台任务执行，即 export_run，进度经 export-progress 事件上报。
//
// 勾选分两块：**模组**（`mods` 目录，来自 export_get_info）与**文件**（游戏目录下的懒加载树）。
// 每一条都能填下载地址：Modrinth 格式下填了地址的写进 index 的下载清单，没填的与目录一起打进
// overrides；CurseForge 的清单格式里只有项目号 / 文件号，地址那一块在那个格式下不作数。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import IconTabs from "../../components/ui/IconTabs.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import FilePicker from "./parts/FilePicker.vue";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { api, onExportFocus, onExportProgress } from "../../lib/api";
import { isTauri, targetUuid, windowParams } from "../windowManager";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import type { ExportEntryDto, ExportInfoDto, ExportProgressDto, InstanceInfoDto } from "../../lib/bindings";

/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

/** 页签 id：模组 / 文件 / 整合包信息 */
type ExportTab = "mods" | "files" | "meta";

const inTauri = isTauri();

const instances = ref<InstanceInfoDto[]>([]);
const currentUuid = ref("");
const info = ref<ExportInfoDto | null>(null);
const loadingInfo = ref(false);

// ---------------- 导出配置 ----------------

/** 导出格式 id */
type PackId = "curseforge" | "modrinth" | "mmc" | "hmcl";

const pack = ref<PackId>("curseforge");
const packName = ref("");
const author = ref("");
const version = ref("1.0.0");
const summary = ref("");

/** 勾选的模组：相对游戏目录的路径 → 条目 */
const pickedMods = ref(new Map<string, ExportEntryDto>());
/** 勾选的文件与目录 */
const pickedFiles = ref(new Map<string, ExportEntryDto>());
/** 路径 → 下载地址；空串表示没填，按 overrides 打包 */
const urls = ref(new Map<string, string>());
/** 模组搜索词 */
const modKeyword = ref("");

/** 当前选项卡：整合包信息 / 模组 / 文件 */
const tab = ref<ExportTab>("meta");

/** 格式选项：两种通用清单格式，加两种整包实例格式（供别的启动器导入） */
const packOptions = [
  { value: "curseforge", label: "CurseForge" },
  { value: "modrinth", label: "Modrinth (.mrpack)" },
  { value: "mmc", label: "MMC" },
  { value: "hmcl", label: "HMCL" },
];

/** 存盘对话框里的格式名 */
const packLabel = computed(() => packOptions.find((x) => x.value === pack.value)?.label ?? "Modpack");

/** 导出文件名后缀（按格式） */
const fileExt = computed(() => (pack.value === "modrinth" ? "mrpack" : "zip"));

/** 只有 Modrinth 的清单带下载地址，其余格式整包打包 */
const withUrls = computed(() => pack.value === "modrinth");

/** 自定义加载器只在两种整包格式下会丢：它们的包内没有这个组件位，其余格式原样带着 */
const customLoaderLost = computed(
  () => !!info.value?.customLoader && (pack.value === "mmc" || pack.value === "hmcl"),
);

/** 默认勾选的目录：就是导出一直以来的那三块，重建同名前先保持行为不变 */
const DEFAULT_DIRS = ["config", "resourcepacks", "shaderpacks"];

/** 过滤后的模组列表 */
const mods = computed(() => {
  const list = info.value?.mods ?? [];
  const keyword = modKeyword.value.trim().toLowerCase();
  if (!keyword) return list;
  return list.filter((x) => x.path.toLowerCase().includes(keyword));
});

/** 勾选项里能填地址的：模组与单个文件，目录不算 */
const urlRows = computed(() =>
  [...pickedMods.value.values(), ...pickedFiles.value.values()].filter((x) => !x.dir),
);

/** 文件树的勾选表按路径判在不在，交给子组件只看键 */
const pickedFilePaths = computed(() => new Map([...pickedFiles.value].map(([path]) => [path, true])));

/** 选项卡标签：整合包信息在最前，两个勾选区把已选数量挂在标签尾巴上 */
const tabOptions = computed<
  Array<{ id: ExportTab; labelKey: string; icon: string; suffix?: string }>
>(() => [
  { id: "meta", labelKey: "winExport.tabMeta", icon: "box" },
  {
    id: "mods",
    labelKey: "winExport.tabMods",
    icon: "cube",
    suffix: `${pickedMods.value.size}/${info.value?.mods.length ?? 0}`,
  },
  { id: "files", labelKey: "winExport.tabFiles", icon: "folder", suffix: `${pickedFiles.value.size}` },
]);

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
    applyDefaults(data);
  } catch (e) {
    console.error("[ExportWindow] 加载导出信息失败", e);
    info.value = null;
  } finally {
    loadingInfo.value = false;
  }
}

/** 勾选状态的初始值：模组全勾、三个常规目录勾上，模组的下载地址按来源预填 */
async function applyDefaults(data: ExportInfoDto) {
  pickedMods.value = new Map(data.mods.map((x) => [x.path, x]));
  pickedFiles.value = new Map();
  urls.value = new Map();

  for (const mod of data.mods) {
    if (mod.url) urls.value.set(mod.path, mod.url);
  }

  try {
    const root = await api.listExportDir(currentUuid.value, "");
    for (const entry of root) {
      if (entry.dir && DEFAULT_DIRS.includes(entry.path)) {
        pickedFiles.value.set(entry.path, entry);
      }
    }
  } catch (e) {
    console.error("[ExportWindow] 读取实例目录失败", e);
  }
}

/** 勾选 / 取消一个模组 */
function toggleMod(entry: ExportEntryDto) {
  if (pickedMods.value.has(entry.path)) {
    pickedMods.value.delete(entry.path);
  } else {
    pickedMods.value.set(entry.path, entry);
    // 有在线来源的顺手把地址填上，用户可改可清
    if (entry.url && !urls.value.has(entry.path)) urls.value.set(entry.path, entry.url);
  }
}

/** 勾选 / 取消一个文件或目录 */
function toggleFile(entry: ExportEntryDto) {
  if (pickedFiles.value.has(entry.path)) {
    pickedFiles.value.delete(entry.path);
  } else {
    pickedFiles.value.set(entry.path, entry);
  }
}

/** 全选 / 全不选当前过滤出来的模组 */
function pickAllMods(all: boolean) {
  for (const mod of mods.value) {
    if (all) {
      pickedMods.value.set(mod.path, mod);
    } else {
      pickedMods.value.delete(mod.path);
    }
  }
}

/** 填 / 清某条的下载地址 */
function setUrl(path: string, value: string) {
  if (value) {
    urls.value.set(path, value);
  } else {
    urls.value.delete(path);
  }
}

/** 从已选里去掉一条，文件页右侧的移除按钮用 */
function removePick(path: string) {
  pickedFiles.value.delete(path);
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

/** 勾选表 → 后端要的入参：路径 + 下载地址 */
function picksOf(picked: Map<string, ExportEntryDto>) {
  return [...picked.keys()].map((path) => ({ path, url: urls.value.get(path) ?? "" }));
}

/** 选保存位置后交给 Rust 侧后台导出，进度走 export-progress 事件 */
async function startExport() {
  if (!currentUuid.value || running.value) return;
  if (!inTauri) return;

  // 选保存位置（文件名带格式后缀）
  const { save } = await import("@tauri-apps/plugin-dialog");
  const picked = await save({
    title: t("winExport.pickSave"),
    defaultPath: `${packName.value || info.value?.name || "modpack"}.${fileExt.value}`,
    filters: [
      {
        name: packLabel.value,
        extensions: [fileExt.value],
      },
    ],
  });
  if (!picked) return;

  // 选完保存位置再提醒一次：这一条决定了收包的人能不能直接跑起来
  if (customLoaderLost.value) showToast(t("winExport.customLoaderWarn"), 4000);

  progress.value = { state: "running", now: 0, total: 0, text: "", error: null, file: picked };
  try {
    await api.runExport(currentUuid.value, {
      pack: pack.value,
      file: picked,
      mods: picksOf(pickedMods.value),
      files: picksOf(pickedFiles.value),
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
    <!-- 实例选择与导出按钮放进标题栏：不占内容区高度，也不随选项卡滚动 -->
    <template #head-right>
      <select v-model="currentUuid" class="field-select head-select">
        <!-- 标题栏里位置窄，占位用短文案；内容区的空态另有一句更长的提示 -->
        <option value="" disabled>{{ t("winExport.instance") }}</option>
        <option v-for="i in instances" :key="i.uuid" :value="i.uuid">{{ i.name }}</option>
      </select>
      <BaseButton size="sm" variant="primary" :disabled="running || loadingInfo || !info" @click="startExport">
        {{ running ? t("winExport.exporting") : t("winExport.exportBtn") }}
      </BaseButton>
    </template>

    <div class="export-window">
      <template v-if="info">
        <!-- 概况整段不显示了：版本与加载器在主页面上都有，导出窗口里不必重复 -->
        <div class="row">
          <label class="field-label">{{ t("winExport.format") }}</label>
          <SegmentedTabs v-model="pack" :options="packOptions" />
        </div>

        <!-- 这层限制在选择格式时就说清楚，别让用户以为包里带了自定义加载器 -->
        <p v-if="customLoaderLost" class="warn-hint">{{ t("winExport.customLoaderWarn") }}</p>

        <!-- 选项卡：与「添加实例」窗口同一套图标页签 -->
        <IconTabs v-model="tab" :tabs="tabOptions" />

        <div class="tab-body">
          <!-- 整合包信息：写进清单的元数据，配着上面的格式一起看 -->
          <template v-if="tab === 'meta'">
            <p class="sec-hint">
              {{
                t("winExport.summaryLine", {
                  mods: pickedMods.size,
                  files: pickedFiles.size,
                  urls: urlRows.filter((x) => (urls.get(x.path) ?? "").length > 0).length,
                })
              }}
            </p>
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
          </template>

          <!-- 模组 -->
          <template v-else-if="tab === 'mods'">
            <div class="sec-bar">
              <input v-model="modKeyword" class="field-input search" spellcheck="false"
                :placeholder="t('winExport.searchMods')" />
              <BaseButton size="sm" @click="pickAllMods(true)">{{ t("winExport.selectAll") }}</BaseButton>
              <BaseButton size="sm" @click="pickAllMods(false)">{{ t("winExport.selectNone") }}</BaseButton>
            </div>
            <div class="mod-list">
              <p v-if="!mods.length" class="empty-tip">{{ t("winExport.noMods") }}</p>
              <div v-for="mod in mods" :key="mod.path" class="mod-row">
                <input type="checkbox" :checked="pickedMods.has(mod.path)" @change="toggleMod(mod)" />
                <span class="mod-name" :title="mod.path">{{ mod.path.split("/").pop() }}</span>
                <span class="mod-tag" :class="{ online: !!mod.modId }">
                  {{ mod.modId ? t("winExport.online") : t("winExport.local") }}
                </span>
                <input v-if="withUrls && pickedMods.has(mod.path)" class="field-input mod-url"
                  spellcheck="false" :value="urls.get(mod.path) ?? ''"
                  :placeholder="t('winExport.urlPlaceholder')"
                  @input="setUrl(mod.path, ($event.target as HTMLInputElement).value)" />
              </div>
            </div>
          </template>

          <!-- 文件：左边树、右边已选，地址填在右边的条目上 -->
          <template v-else>
            <div class="files-pane">
              <div class="files-left">
                <p class="sec-hint">{{ t("winExport.dirHint") }}</p>
                <FilePicker :uuid="currentUuid" :picked="pickedFilePaths" @toggle="toggleFile" />
              </div>
              <div class="files-right">
                <p class="sec-hint">
                  {{ withUrls ? t("winExport.urlsHint") : t("winExport.pickedHint") }}
                </p>
                <div class="picked-list">
                  <p v-if="!pickedFiles.size" class="empty-tip">{{ t("winExport.selectedNone") }}</p>
                  <div v-for="row in [...pickedFiles.values()]" :key="row.path" class="picked-row">
                    <span class="url-path" :title="row.path">{{ row.path }}</span>
                    <div class="picked-line">
                      <input v-if="withUrls && !row.dir" class="field-input url-input" spellcheck="false"
                        :value="urls.get(row.path) ?? ''" :placeholder="t('winExport.urlPlaceholder')"
                        @input="setUrl(row.path, ($event.target as HTMLInputElement).value)" />
                      <BaseButton size="sm" @click="removePick(row.path)">{{ t("winExport.remove") }}</BaseButton>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>

        <!-- 导出进度：固定在底部，不随选项卡滚动 -->
        <div v-if="running" class="progress-row">
          <span class="progress-text" v-tip="progress?.text">{{ progress?.text }}</span>
          <div class="progress-track">
            <div class="progress-fill" :style="{ width: percent + '%' }" />
          </div>
        </div>
      </template>
      <div v-else-if="loadingInfo" class="hint-full">{{ t("winExport.loading") }}</div>
      <div v-else class="hint-full">{{ t("winExport.pickInstance") }}</div>
    </div>
  </WindowFrame>
</template>

<style scoped>
/* 四周留白交给 .frame-body（上下 22 / 左右 26）：这里**不能再加一份 padding**，
   否则顶部会变成 36px、左右 42px，比别的窗口鼓一圈。
   内容自己滚，所以滚动条槽位仍留给这个容器 */
.export-window {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  min-height: 0;
  box-sizing: border-box;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
}

/* 页签自带的下外边距是给「添加实例」窗口的表单留的，这一页用的是 flex gap，
   不取消就会变成 12 + 14 = 26px */
.export-window :deep(.tabs) {
  margin-bottom: 0;
}

/* 标题栏里的实例下拉：尺寸口径与账户 / 方块 / 设置三个窗口的标题栏控件保持一致，
   改这里的 36px 要同步那几处。实例名比账户窗口那个筛选长，最小宽度按名字留够，
   再长就省略号，别把导出按钮挤出去 */
.head-select {
  width: auto;
  min-width: 260px;
  max-width: 340px;
  /* 必须显式覆盖 .field-select 的 min-height: var(--field-h)（42px），
     不覆盖会被撑到 42，比旁边的控件高一截 */
  min-height: 0;
  height: 36px;
  /* 定高时 36 = 行高 18 + 上下内边距 8×2 + 上下边框 1×2，对不上文字会偏离中心 */
  line-height: 18px;
  padding: 8px 28px 8px 12px;
  border-radius: 9px;
  font-size: 12.5px;
  /* 箭头比标准下拉略小、离右边略近（标题栏空间紧） */
  background-position: right 7px center;
  background-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 导出格式：标签与选择器同一行。选择器是 inline-flex，只包住自己的选项内容，
   不会被拉着撑满整行 */
.row {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* .field-label 默认是块级、上下都带外边距（12 / 6），并排时会把这一行顶歪 */
.row .field-label {
  margin: 0;
}

.grid-2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 14px;
}

/* 选项卡内容：吃掉剩余高度，内部各自滚 */
.tab-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-height: 0;
}

.sec-hint {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

/* 会丢东西的提示：与设置页里那些"改了要重启"的提示同一档观感 */
.warn-hint {
  margin: 0;
  font-size: 12px;
  color: var(--yellow);
}

.sec-bar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.sec-bar .search {
  flex: 1;
  min-width: 0;
}

/* 模组列表：内容多时自己滚，别把窗口撑长 */
.mod-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
}

.mod-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 6px;
  border-radius: 5px;
  font-size: 12.5px;
}

.mod-row:hover {
  background: var(--bg-hover);
}

.mod-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 在线 / 本地标签：在线走强调色，本地是中性灰 */
.mod-tag {
  flex-shrink: 0;
  padding: 0 6px;
  border-radius: 4px;
  background: var(--bg-hover);
  color: var(--text-dim);
  font-size: 10.5px;
}

.mod-tag.online {
  background: var(--accent-soft);
  color: var(--accent);
}

/* 模组行里的地址：固定宽度，不给它抢文件名的位置 */
.mod-url {
  width: 260px;
  flex-shrink: 0;
}

/* 文件页：左边树吃掉剩余宽度，右边已选固定宽度 */
.files-pane {
  display: flex;
  gap: 12px;
  flex: 1;
  min-height: 0;
}

.files-left {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.files-right {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 330px;
  flex-shrink: 0;
  min-height: 0;
}

.picked-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
}

/* 一条已选：路径一行，地址与移除按钮一行 */
.picked-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.url-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11.5px;
  color: var(--text-dim);
}

.picked-line {
  display: flex;
  align-items: center;
  gap: 6px;
}

.url-input {
  flex: 1;
  min-width: 0;
}

/* 进度：文件名一行、进度条一行，都不随选项卡滚动 */
.progress-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
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
