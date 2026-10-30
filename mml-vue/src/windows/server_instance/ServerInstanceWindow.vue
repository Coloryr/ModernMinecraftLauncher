<script setup lang="ts">
// 服务器实例生成窗口：左侧分页 + 右侧内容
//
// 分页：总体设定 / 模组 / 资源包 / 配置文件 / 开始生成。
// 配置文件不是文件树，而是"添加要同步的目录"，每个目录各自选同步方式：
// 压缩包同步是整目录打一个 zip，逐文件下载是目录里的文件一条一条列进清单。
//
// 目标实例按优先级取：openWindow 带入的参数 → server-instance-focus 事件 → 标题栏下拉。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
// 通用行虚拟列表（资源窗口那几个分类也在用）：几百个模组时只渲染视口附近的行
import VirtualList from "../resource/parts/VirtualList.vue";
import { api, onServerInstanceFocus, onServerInstanceProgress } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { useInstanceChange } from "../../composables/useInstanceChange";
import { targetUuid, windowParams } from "../windowManager";
import type {
  ExportEntryDto,
  InstanceInfoDto,
  ServerInstanceProgressDto,
  ServerInstanceResultDto,
} from "../../lib/bindings";

/** 关闭窗口 */
const emit = defineEmits<{ (e: "close"): void }>();

/** 生成格式：mml 写 server.json，hmcl 写 server-manifest.json */
type Format = "mml" | "hmcl";
/** 配置目录的同步方式 */
type ConfigMode = "archive" | "files";
/** 左侧分页 */
type Section = "general" | "mods" | "packs" | "configs" | "start";

/** 模组表格的行高，单位 px；必须与样式里的 .si-mod-row 一致 */
const MOD_ROW_H = 36;
/** 资源包列表的行高，单位 px；必须与样式里的 .si-row-item 一致 */
const PACK_ROW_H = 28;

/** 列表里不当作"配置文件目录"的顶层目录：模组与资源包各有自己的页，其余是运行期产物 */
const SKIP_DIRS = [
  "mods", "resourcepacks", "saves", "logs", "crash-reports", "backups", "backup", "cache",
  ".cache", "temp", "versions", "libraries", "assets", "resources", "natives", "downloads",
];

/** 默认逐文件下发的目录：kubejs 是几千个小脚本，服主要逐个比对，打成压缩包反而没法看 */
const FILES_DIRS = ["kubejs"];

const instances = ref<InstanceInfoDto[]>([]);
const currentUuid = ref("");
const section = ref<Section>("general");

// ---------------- 总体设定 ----------------

const format = ref<Format>("mml");
const name = ref("");
const version = ref("1.0.0");
const text = ref("");
const url = ref("");
const folder = ref("");

// ---------------- 勾选 ----------------

/** 模组目录里的文件 */
const mods = ref<ExportEntryDto[]>([]);
/** 勾选的模组路径 */
const pickedMods = ref(new Set<string>());
/** 每个模组的下载地址，路径 → 地址；载入时用实例记录的那份铺底 */
const modUrls = ref(new Map<string, string>());
/** 资源包目录里的文件 */
const packs = ref<ExportEntryDto[]>([]);
/** 勾选的资源包路径 */
const pickedPacks = ref(new Set<string>());
/** 已添加的配置目录与各自的同步方式 */
const configs = ref<Array<{ dir: string; mode: ConfigMode }>>([]);
/** 游戏目录下的顶层目录，供"添加"下拉用 */
const topDirs = ref<ExportEntryDto[]>([]);
/** 下拉里当前选中的目录 */
const newDir = ref("");
/** 新添加目录默认的同步方式 */
const newMode = ref<ConfigMode>("archive");

// ---------------- 运行状态 ----------------

const busy = ref(false);
const progress = ref<ServerInstanceProgressDto | null>(null);
const result = ref<ServerInstanceResultDto | null>(null);
const error = ref("");

/** 进度事件取消订阅 */
let unlistenProgress: UnlistenFn | null = null;
/** 切换目标实例事件取消订阅 */
let unlistenFocus: UnlistenFn | null = null;

/** 当前选中的实例 */
const current = computed(() => instances.value.find((i) => i.uuid === currentUuid.value));

/** 左侧分页 */
const sections = computed<Array<{ id: Section; labelKey: string; icon: "gear" | "cube" | "package" | "sliders" | "play" }>>(() => [
  { id: "general", labelKey: "serverInstance.tab.general", icon: "gear" },
  { id: "mods", labelKey: "serverInstance.tab.mods", icon: "cube" },
  { id: "packs", labelKey: "serverInstance.tab.packs", icon: "package" },
  { id: "configs", labelKey: "serverInstance.tab.configs", icon: "sliders" },
  { id: "start", labelKey: "serverInstance.tab.start", icon: "play" },
]);

/** 格式选项 */
const formatOptions = computed(() => [
  { value: "mml", label: t("serverInstance.mml") },
  { value: "hmcl", label: t("serverInstance.hmcl") },
]);

/** 当前格式的说明 */
const hint = computed(() => t(`serverInstance.hint.${format.value}`));

/** 还没加进列表的顶层目录 */
const dirChoices = computed(() =>
  topDirs.value.filter(
    (entry) => entry.dir && !SKIP_DIRS.includes(entry.path) && !configs.value.some((c) => c.dir === entry.path),
  ),
);

/** 进度百分比；总数未知时返回 -1 */
const percent = computed(() => {
  const p = progress.value;
  if (!p || p.total <= 0) return -1;
  return Math.min(100, Math.max(0, Math.round((p.now / p.total) * 100)));
});

/** 必填项齐不齐 */
const ready = computed(() => !!currentUuid.value && !!name.value.trim() && !!url.value.trim() && !!folder.value.trim());

/** 是否已经选了要打包的东西 */
const pickedCount = computed(() => pickedMods.value.size + pickedPacks.value.size + configs.value.length);

/** 这个模组当前填的下载地址 */
function urlOf(path: string): string {
  return modUrls.value.get(path) ?? "";
}

/** 资源包条目有没有在线来源 */
function packOnline(entry: ExportEntryDto): boolean {
  return !!entry.url || (!!entry.modId && !!entry.fileId);
}

/** 改某一行的下载地址 */
function setModUrl(path: string, event: Event) {
  const value = (event.target as HTMLInputElement).value;
  const next = new Map(modUrls.value);
  next.set(path, value);
  modUrls.value = next;
}

/** 文件大小，按档显示 */
function sizeText(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

/** 文件名 */
function baseName(path: string): string {
  return path.split("/").pop() ?? path;
}

/** 勾选 / 取消一个路径，返回新集合以触发响应式 */
function toggled(set: Set<string>, path: string): Set<string> {
  const next = new Set(set);
  if (next.has(path)) next.delete(path);
  else next.add(path);
  return next;
}

/** 全选 / 全不选模组 */
function pickAllMods(all: boolean) {
  pickedMods.value = all ? new Set(mods.value.map((item) => item.path)) : new Set();
}

/** 全选 / 全不选资源包 */
function pickAllPacks(all: boolean) {
  pickedPacks.value = all ? new Set(packs.value.map((item) => item.path)) : new Set();
}

/** 把下拉里选中的目录加进配置列表 */
function addConfig() {
  const dir = newDir.value;
  if (!dir || configs.value.some((item) => item.dir === dir)) return;
  const mode = FILES_DIRS.includes(dir) ? "files" : newMode.value;
  configs.value = [...configs.value, { dir, mode }];
  newDir.value = "";
}

/** 移除一个配置目录 */
function removeConfig(dir: string) {
  configs.value = configs.value.filter((item) => item.dir !== dir);
}

/** 改一行的同步方式 */
function setConfigMode(dir: string, mode: ConfigMode) {
  configs.value = configs.value.map((item) => (item.dir === dir ? { ...item, mode } : item));
}

/** 加载序号：快速切实例时只认最后一次请求的结果 */
let loadSeq = 0;

/** 重新读一个实例的模组、资源包与顶层目录，勾选与配置列表重置成默认值 */
async function loadInstance(uuid: string) {
  const seq = ++loadSeq;
  try {
    const info = await api.getExportInfo(uuid);
    if (seq !== loadSeq) return;
    mods.value = info.mods.filter((item) => !item.dir);
    pickedMods.value = new Set(mods.value.map((item) => item.path));
    modUrls.value = new Map(mods.value.map((item) => [item.path, item.url ?? ""]));
  } catch (e) {
    if (seq !== loadSeq) return;
    mods.value = [];
    pickedMods.value = new Set();
    modUrls.value = new Map();
    showToast(tErr(e));
  }

  let packsNext: ExportEntryDto[] = [];
  try {
    packsNext = (await api.listExportDir(uuid, "resourcepacks")).filter((item) => !item.dir);
  } catch {
    // 没有 resourcepacks 目录就是空表
  }
  if (seq !== loadSeq) return;
  packs.value = packsNext;
  pickedPacks.value = new Set(packs.value.map((item) => item.path));

  let dirsNext: ExportEntryDto[] = [];
  try {
    dirsNext = await api.listExportDir(uuid, "");
  } catch {
    // 读不到就当没有可同步的目录
  }
  if (seq !== loadSeq) return;
  topDirs.value = dirsNext;

  // 默认把所有能同步的文件夹都加上，用户再按需改方式或删掉
  configs.value = topDirs.value
    .filter((entry) => entry.dir && !SKIP_DIRS.includes(entry.path))
    .map((entry) => ({
      dir: entry.path,
      mode: (FILES_DIRS.includes(entry.path) ? "files" : "archive") as ConfigMode,
    }));
  newDir.value = "";
  newMode.value = "archive";
  result.value = null;
  progress.value = null;
  error.value = "";
}

/** 切到某个实例 */
function focusInstance(uuid: string) {
  if (!instances.value.some((i) => i.uuid === uuid)) return;
  const changed = currentUuid.value !== uuid;
  currentUuid.value = uuid;
  const inst = instances.value.find((i) => i.uuid === uuid);
  if (inst) name.value = inst.name;
  // 切回来还是同一个实例时不重铺，别把用户刚挑好的东西清掉
  if (changed) void loadInstance(uuid);
}

/** 下拉换实例 */
function onInstanceChange() {
  const inst = current.value;
  if (inst) name.value = inst.name;
  void loadInstance(currentUuid.value);
}

/** 定位到 openWindow 带入的目标实例 */
function applyTarget() {
  const uuid = targetUuid();
  if (uuid && instances.value.some((i) => i.uuid === uuid)) focusInstance(uuid);
}

/** 清掉目标实例与它带出来的勾选：实例没了，界面上不能留着上一份快照 */
function clearInstance() {
  // 在途的加载结果作废，否则它回来又会把已删实例的数据铺上
  loadSeq += 1;
  currentUuid.value = "";
  name.value = "";
  mods.value = [];
  pickedMods.value = new Set();
  modUrls.value = new Map();
  packs.value = [];
  pickedPacks.value = new Set();
  configs.value = [];
  topDirs.value = [];
  newDir.value = "";
  result.value = null;
  progress.value = null;
  error.value = "";
}

/** 重新拉实例列表并定位目标实例；当前实例被删掉时落到第一个 */
async function syncInstances() {
  try {
    instances.value = await api.getInstances();
  } catch (e) {
    console.error("[ServerInstanceWindow] 读取实例列表失败", e);
    return;
  }
  // 当前实例可能在别的窗口里被删掉了：先清空再重新定位，别留一个不存在的选中项
  if (currentUuid.value && !instances.value.some((i) => i.uuid === currentUuid.value)) {
    clearInstance();
  }
  const uuid = targetUuid();
  if (uuid && instances.value.some((i) => i.uuid === uuid)) {
    focusInstance(uuid);
    return;
  }
  if (!currentUuid.value && instances.value.length) focusInstance(instances.value[0].uuid);
}

/** 选一个输出文件夹 */
async function pickFolder() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("serverInstance.folder"),
      directory: true,
      multiple: false,
    });
    if (typeof picked === "string") folder.value = picked;
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 开始生成，进度经 server-instance-progress 上报 */
async function generate() {
  if (busy.value || !ready.value) return;
  busy.value = true;
  error.value = "";
  result.value = null;
  progress.value = null;
  section.value = "start";
  try {
    result.value = await api.generateServerInstance(currentUuid.value, {
      format: format.value,
      folder: folder.value.trim(),
      url: url.value.trim(),
      name: name.value.trim(),
      version: version.value.trim(),
      text: text.value.trim(),
      mods: [...pickedMods.value].map((path) => ({
        path,
        exclude: false,
        url: urlOf(path),
      })),
      configs: configs.value.map((item) => ({ dir: item.dir, mode: item.mode })),
      resourcePacks: [...pickedPacks.value],
    });
    showToast(t("serverInstance.done", { path: folder.value.trim() }));
  } catch (e) {
    error.value = tErr(e);
  } finally {
    busy.value = false;
  }
}

// 单窗口模式：本页被 KeepAlive 缓存，切回不会重新挂载，自己补一次列表与定位
useWindowRefresh(() => {
  if (!busy.value) {
    result.value = null;
    progress.value = null;
  }
  return syncInstances();
});

// 实例在别的窗口里被增删改（删掉的那个）→ 下拉与勾选跟着变
useInstanceChange(syncInstances);

// 已经停在本窗口时又被打开：openWindow 只更新参数、不换组件，跟着参数换实例
watch(() => windowParams.value.uuid, applyTarget);

// 下拉换目录时把同步方式拨到该目录的常见做法，用户仍可自己改
watch(newDir, (dir) => {
  if (dir) newMode.value = FILES_DIRS.includes(dir) ? "files" : "archive";
});

onMounted(async () => {
  await syncInstances();

  unlistenProgress = await onServerInstanceProgress((p) => {
    progress.value = p;
  });
  unlistenFocus = await onServerInstanceFocus((uuid) => {
    if (instances.value.some((i) => i.uuid === uuid)) focusInstance(uuid);
  });
});

onBeforeUnmount(() => {
  unlistenProgress?.();
  unlistenFocus?.();
});
</script>

<template>
  <WindowFrame :title="t('winTitle.serverInstance')" body-fill body-flush @close="emit('close')">
    <!-- 实例放标题栏：换实例不用回到总体设定 -->
    <template #head-right>
      <select v-model="currentUuid" class="field-select head-select" :disabled="busy"
        @change="onInstanceChange">
        <option value="" disabled>{{ t("winExport.instance") }}</option>
        <option v-for="i in instances" :key="i.uuid" :value="i.uuid">{{ i.name }}</option>
      </select>
    </template>

    <div class="si-layout">
      <!-- 左侧分页 -->
      <nav class="si-rail">
        <button v-for="item in sections" :key="item.id" class="si-rail-item"
          :class="{ active: section === item.id }" :disabled="busy && item.id === 'start'"
          @click="section = item.id">
          <span class="si-rail-icon">
            <GlyphIcon :name="item.icon" :size="16" :weight="1.8" />
          </span>
          <span class="si-rail-text">
            <span class="si-rail-title">{{ t(item.labelKey) }}</span>
            <span class="si-rail-desc">{{ t(`serverInstance.tabDesc.${item.id}`) }}</span>
          </span>
        </button>
      </nav>

      <div class="si-content">
        <!-- 当前页大标题 + 说明，与设置窗口同款 -->
        <header class="si-head">
          <h2 class="si-title">{{ t(`serverInstance.tab.${section}`) }}</h2>
          <p class="si-sub">{{ t(`serverInstance.tabDesc.${section}`) }}</p>
        </header>

        <p v-if="!instances.length" class="empty-tip">{{ t("serverInstance.noInstance") }}</p>

        <template v-else>
          <!-- ===== 总体设定 ===== -->
          <section v-if="section === 'general'" class="si-section">
            <div class="si-row">
              <span class="field-label">{{ t("serverInstance.format") }}</span>
              <SegmentedTabs :options="formatOptions" :model-value="format" :disabled="busy"
                @update:model-value="format = $event as Format" />
            </div>
            <p class="field-hint">{{ hint }}</p>

            <div class="field-row-2">
              <div>
                <label class="field-label" for="si-name">{{ t("serverInstance.name") }}</label>
                <input id="si-name" v-model="name" class="field-input" :disabled="busy" spellcheck="false" />
              </div>
              <div>
                <label class="field-label" for="si-version">{{ t("serverInstance.version") }}</label>
                <input id="si-version" v-model="version" class="field-input" :disabled="busy"
                  spellcheck="false" />
              </div>
            </div>

            <label class="field-label" for="si-text">{{ t("serverInstance.desc") }}</label>
            <input id="si-text" v-model="text" class="field-input" :disabled="busy" spellcheck="false"
              :placeholder="t('serverInstance.descPlaceholder')" />

            <label class="field-label" for="si-url">{{ t("serverInstance.url") }}</label>
            <input id="si-url" v-model="url" class="field-input" :disabled="busy" spellcheck="false"
              :placeholder="t('serverInstance.urlPlaceholder')" />
            <p class="field-hint">{{ t("serverInstance.urlHint") }}</p>

            <label class="field-label" for="si-folder">{{ t("serverInstance.folder") }}</label>
            <div class="path-row">
              <input id="si-folder" v-model="folder" class="field-input" :disabled="busy" spellcheck="false"
                :placeholder="t('serverInstance.folderPlaceholder')" />
              <BaseButton :disabled="busy" @click="pickFolder">{{ t("serverInstance.pick") }}</BaseButton>
            </div>
            <p class="field-hint">{{ t("serverInstance.folderHint") }}</p>
          </section>

          <!-- ===== 模组 ===== -->
          <section v-else-if="section === 'mods'" class="si-section stretch">
            <div class="si-section-head">
              <span class="si-counter">{{ pickedMods.size }} / {{ mods.length }}</span>
              <BaseButton size="sm" :disabled="busy" @click="pickAllMods(true)">
                {{ t("serverInstance.pickAll") }}
              </BaseButton>
              <BaseButton size="sm" :disabled="busy" @click="pickAllMods(false)">
                {{ t("serverInstance.pickNone") }}
              </BaseButton>
            </div>
            <!-- 一张表：勾选 / 文件名 / 项目编号 / 文件编号 / 下载地址 / 哈希值
                 编号是实例记录的，只读；只有下载地址能改。
                 几百个模组逐个渲染太重，行虚拟化只画视口附近那些 -->
            <VirtualList class="si-mod-table" :count="mods.length" :row-h="MOD_ROW_H" :row-gap="0">
              <template #header>
                <div class="si-mod-head">
                  <span>{{ t("serverInstance.colSync") }}</span>
                  <span>{{ t("serverInstance.colFile") }}</span>
                  <span>{{ t("serverInstance.colProject") }}</span>
                  <span>{{ t("serverInstance.colFileId") }}</span>
                  <span>{{ t("serverInstance.colUrl") }}</span>
                  <span>{{ t("serverInstance.colSha1") }}</span>
                </div>
              </template>
              <template #default="{ start, end }">
                <p v-if="!mods.length" class="empty-tip">{{ t("serverInstance.noMods") }}</p>
                <div v-for="row in mods.slice(start, end)" :key="row.path" class="si-mod-row">
                  <span class="si-cell-check">
                    <input type="checkbox" class="si-box" :checked="pickedMods.has(row.path)" :disabled="busy"
                      @change="pickedMods = toggled(pickedMods, row.path)" />
                  </span>
                  <span class="si-cell-name" :title="row.path">{{ baseName(row.path) }}</span>
                  <span class="si-cell-id">{{ row.modId || "—" }}</span>
                  <span class="si-cell-id">{{ row.fileId || "—" }}</span>
                  <input class="field-input si-cell-control" :value="urlOf(row.path)" :disabled="busy"
                    spellcheck="false" :placeholder="t('serverInstance.cellUrl')"
                    @input="setModUrl(row.path, $event)" />
                  <span class="si-cell-hash" :title="row.sha1">{{ row.sha1 || "—" }}</span>
                </div>
              </template>
            </VirtualList>
          </section>

          <!-- ===== 资源包 ===== -->
          <section v-else-if="section === 'packs'" class="si-section stretch">
            <div class="si-section-head">
              <span class="si-counter">{{ pickedPacks.size }} / {{ packs.length }}</span>
              <BaseButton size="sm" :disabled="busy" @click="pickAllPacks(true)">
                {{ t("serverInstance.pickAll") }}
              </BaseButton>
              <BaseButton size="sm" :disabled="busy" @click="pickAllPacks(false)">
                {{ t("serverInstance.pickNone") }}
              </BaseButton>
            </div>
            <VirtualList class="si-list" :count="packs.length" :row-h="PACK_ROW_H" :row-gap="2">
              <template #default="{ start, end }">
                <p v-if="!packs.length" class="empty-tip">{{ t("serverInstance.noPacks") }}</p>
                <label v-for="row in packs.slice(start, end)" :key="row.path" class="si-row-item">
                  <input type="checkbox" class="si-box" :checked="pickedPacks.has(row.path)" :disabled="busy"
                    @change="pickedPacks = toggled(pickedPacks, row.path)" />
                  <span class="si-name" :title="row.path">{{ baseName(row.path) }}</span>
                  <span v-if="format === 'mml'" class="si-tag" :class="{ on: packOnline(row) }">
                    {{ packOnline(row) ? t("serverInstance.modOnline") : t("serverInstance.noSource") }}
                  </span>
                  <span class="si-size">{{ sizeText(row.size) }}</span>
                </label>
              </template>
            </VirtualList>
          </section>

          <!-- ===== 配置文件 ===== -->
          <section v-else-if="section === 'configs'" class="si-section stretch">
            <!-- 与模组页同一个表头行：左边计数，右边控件；下面的表也是同一副外观 -->
            <div class="si-section-head">
              <span class="si-counter">{{ t("serverInstance.configCount", { n: configs.length }) }}</span>
              <select v-model="newDir" class="field-select si-add-dir"
                :title="t('serverInstance.configSelect')" :disabled="busy || !dirChoices.length">
                <option value="" disabled>
                  {{ dirChoices.length ? t("serverInstance.configPick") : t("serverInstance.configNoMore") }}
                </option>
                <option v-for="dir in dirChoices" :key="dir.path" :value="dir.path">
                  {{ dir.path }}/
                </option>
              </select>
              <select v-model="newMode" class="field-select si-add-mode" :disabled="busy">
                <option value="archive">{{ t("serverInstance.mode.archive") }}</option>
                <option value="files">{{ t("serverInstance.mode.files") }}</option>
              </select>
              <BaseButton size="sm" :disabled="busy || !newDir" @click="addConfig">
                {{ t("serverInstance.configAdd") }}
              </BaseButton>
            </div>
            <p class="field-hint">{{ t("serverInstance.configHint") }}</p>

            <!-- 与模组页同一套表格外观：表头吸顶、行内直接改同步方式 -->
            <div class="si-table">
              <div class="si-table-head">
                <span>{{ t("serverInstance.configName") }}</span>
                <span>{{ t("serverInstance.configMode") }}</span>
                <span></span>
              </div>
              <p v-if="!configs.length" class="empty-tip">{{ t("serverInstance.configEmpty") }}</p>
              <div v-for="item in configs" :key="item.dir" class="si-table-row">
                <span class="si-name" :title="item.dir">{{ item.dir }}/</span>
                <select class="field-select si-cell-control" :value="item.mode" :disabled="busy"
                  @change="setConfigMode(item.dir, ($event.target as HTMLSelectElement).value as ConfigMode)">
                  <option value="archive">{{ t("serverInstance.mode.archive") }}</option>
                  <option value="files">{{ t("serverInstance.mode.files") }}</option>
                </select>
                <button class="si-remove" :disabled="busy" :title="t('serverInstance.configRemove')"
                  @click="removeConfig(item.dir)">
                  <GlyphIcon name="trash" :size="13" />
                </button>
              </div>
            </div>
          </section>

          <!-- ===== 开始生成 ===== -->
          <section v-else class="si-section">
            <div class="si-summary">
              <div class="si-line">
                {{ t("serverInstance.summaryInstance") }}{{ t("common.detailSep") }}
                <code>{{ current?.name }}</code>
              </div>
              <div class="si-line">
                {{ t("serverInstance.summaryFormat") }}{{ t("common.detailSep") }}
                <code>{{ format === "mml" ? "MML" : "HMCL" }}</code>
              </div>
              <div class="si-line">
                {{ t("serverInstance.summaryMods") }}{{ t("common.detailSep") }}
                <code>{{ pickedMods.size }} / {{ mods.length }}</code>
              </div>
              <div class="si-line">
                {{ t("serverInstance.summaryPacks") }}{{ t("common.detailSep") }}
                <code>{{ pickedPacks.size }} / {{ packs.length }}</code>
              </div>
              <div class="si-line">
                {{ t("serverInstance.summaryConfigs") }}{{ t("common.detailSep") }}
                <code>{{ configs.length }}</code>
              </div>
              <div class="si-line">
                {{ t("serverInstance.folder") }}{{ t("common.detailSep") }}
                <code>{{ folder || "—" }}</code>
              </div>
            </div>

            <div class="si-actions">
              <BaseButton variant="primary" size="lg" :disabled="busy || !ready || !pickedCount"
                @click="generate">
                {{ busy ? t("serverInstance.running") : t("serverInstance.generate") }}
              </BaseButton>
            </div>

            <!-- 生成中：进度条 + 当前文件 -->
            <template v-if="busy">
              <div class="progress-track">
                <div class="progress-fill" :style="percent < 0 ? undefined : { width: percent + '%' }" />
              </div>
              <div class="run-info">
                <span v-if="progress && progress.total > 0">
                  {{ progress.now }} / {{ progress.total }}
                </span>
                <span class="cur">{{ progress?.text }}</span>
              </div>
            </template>

            <!-- 结果 -->
            <template v-else-if="result">
              <div class="si-result">
                <div class="si-line">
                  {{ t("serverInstance.manifest") }}{{ t("common.detailSep") }}<code>{{ result.manifest }}</code>
                </div>
                <div v-if="result.files.length" class="si-line">
                  {{ t("serverInstance.files") }}{{ t("common.detailSep") }}
                  <code v-for="item in result.files" :key="item">{{ item }}</code>
                </div>
                <div class="si-line">{{ t("serverInstance.count", { n: result.count }) }}</div>
              </div>
            </template>

            <p v-if="error" class="error">{{ error }}</p>
          </section>
        </template>
      </div>
    </div>
  </WindowFrame>
</template>

<style scoped>
/* 左导航 + 右内容，与设置窗口同一套骨架：外壳 body-fill + body-flush，这里 flex: 1 就占满 */
.si-layout {
  display: flex;
  flex: 1;
  min-height: 0;
}

.si-rail {
  width: 176px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px 10px 14px 16px;
  border-right: 1px solid var(--border);
  background: var(--bg-side);
  overflow-y: auto;
}

.si-rail-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: var(--text-dim);
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.si-rail-item:hover {
  background: var(--bg-card);
  color: var(--text);
}

.si-rail-item.active {
  background: var(--accent-soft);
  color: var(--accent);
}

.si-rail-item:disabled {
  cursor: default;
  opacity: 0.6;
}

.si-rail-icon {
  flex-shrink: 0;
  display: flex;
}

.si-rail-text {
  display: flex;
  flex-direction: column;
  line-height: 1.3;
  min-width: 0;
}

.si-rail-title {
  font-size: 13px;
  font-weight: 600;
}

.si-rail-desc {
  font-size: 10.5px;
  color: var(--text-dim);
  opacity: 0.8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.si-rail-item.active .si-rail-desc {
  color: inherit;
}

/* 右侧内容：每页自己滚。
   右侧**不留内边距**：滚动条是子元素（.si-section）画的，只有让它贴着内容区右缘，
   滚动条才贴着窗口右边；留白改由下面两条按"26px - 9px 槽位"补回来 */
.si-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  /* 自己不出滚动条：滚的都是下面的 .si-section，这样滚动条才贴着窗口右边 */
  overflow: hidden;
  padding: 22px 0 22px 26px;
}

/* 页头不在滚动容器里，没有槽位，直接留 26px，与下方内容的右缘对齐 */
.si-head,
.si-content > .empty-tip {
  padding-right: 26px;
}

.si-head {
  margin-bottom: 14px;
}

.si-title {
  font-size: 18px;
  font-weight: 800;
}

.si-sub {
  font-size: 12.5px;
  color: var(--text-dim);
  margin-top: 4px;
}

/* 每页一块：默认整页滚；带 .stretch 的页把剩余高度交给列表。
   右内边距 = 26 - 9：9px 是常驻的滚动条槽位，内容右缘因此仍在 26px 上，
   而滚动条贴在窗口右边（见 .si-content 的说明） */
.si-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  padding-right: calc(26px - 9px);
}

/* 撑满的页自己不滚（滚动交给里面的列表 / 表格），没有槽位，右缘直接留 26px */
.si-section.stretch {
  overflow: hidden;
  padding-right: 26px;
}

/* 一行：标签与控件并排 */
.si-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.si-row .field-label {
  margin: 0;
  flex-shrink: 0;
}

/* 添加行：两个下拉按固定宽度排在计数右边，高度与行内控件同一档 */
.si-add-dir {
  width: 220px;
}

.si-add-mode {
  width: 140px;
}

.si-section-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* 计数靠左、按钮靠右：第一个元素吃掉剩余宽度 */
.si-section-head > :first-child {
  flex: 1;
}

.si-counter {
  flex-shrink: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

/* 列表：吃掉这一页剩下的高度；滚动与行虚拟化交给 VirtualList，这里只管外观。
   行间距由 :row-gap 传进去，别再写 gap —— 虚拟化按行高算窗口，写死的 gap 会算偏 */
.si-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-card);
  scrollbar-gutter: stable;
}

.si-row-item {
  display: flex;
  align-items: center;
  gap: 8px;
  /* 行高必须与 PACK_ROW_H 一致：虚拟化按下标估算位置 */
  height: 28px;
  padding: 0 8px;
  border-radius: 8px;
  font-size: 12.5px;
  cursor: pointer;
  user-select: none;
}

.si-row-item:hover {
  background: var(--bg-hover);
}

.si-box {
  flex-shrink: 0;
  width: 15px;
  height: 15px;
  margin: 0;
  accent-color: var(--accent);
  cursor: pointer;
}

.si-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.si-tag {
  flex-shrink: 0;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 10.5px;
  color: var(--text-dim);
  background: var(--bg-hover);
}

/* 在线下载：用强调色的浅底，和"本地托管"一眼分得开 */
.si-tag.on {
  color: var(--accent);
  background: var(--accent-soft);
}

.si-size {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}

/* 两张表共用一套外观：模组表（VirtualList，行虚拟化）与配置目录表（本页自绘行）。
   容器的框、表头吸顶、行高与内边距只写在这里，改一处两张表一起变 */
.si-mod-table,
.si-table {
  flex: 1;
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-card);
}

.si-mod-head,
.si-table-head {
  position: sticky;
  top: 0;
  z-index: 1;
  display: grid;
  align-items: center;
  gap: 8px;
  min-height: 32px;
  padding: 0 10px;
  font-size: 11.5px;
  color: var(--text-dim);
  background: var(--bg-card);
  border-bottom: 1px solid var(--border);
}

.si-mod-row,
.si-table-row {
  display: grid;
  align-items: center;
  gap: 8px;
  /* 行高必须与 MOD_ROW_H 一致：虚拟化按下标估算位置 */
  height: 36px;
  padding: 0 10px;
  font-size: 12.5px;
}

.si-mod-row:hover,
.si-table-row:hover {
  background: var(--bg-hover);
}

/* 列宽各表不同，其余外观共用上面几条 */
.si-mod-head,
.si-mod-row {
  grid-template-columns: 44px minmax(180px, 1.2fr) 104px 104px minmax(220px, 1.8fr) 180px;
}

.si-table-head,
.si-table-row {
  grid-template-columns: minmax(160px, 1fr) 180px 40px;
}

.si-cell-check {
  display: flex;
  align-items: center;
}

.si-cell-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 行内控件：输入框与下拉同一个尺寸，26px 才收得进 36px 的行。
   min-height 必须清掉 —— .field-select 的下限是 --ctl-h（32px），不清就比输入框高一截 */
.si-cell-control {
  height: 26px;
  min-height: 0;
  min-width: 0;
  padding: 0 8px;
}

select.si-cell-control {
  /* 箭头那一侧的间距要留住，否则文字压到箭头上 */
  padding-right: 24px;
  background-position: right 6px center;
}

/* 编号是只读的，用等宽字对齐，空值显示 — */
.si-cell-id {
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.si-cell-hash {
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 10.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 配置目录表：名字 | 同步方式 | 删除。容器、表头、行高与模组表共用上面那组规则 */
.si-remove {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.si-remove:hover:not(:disabled) {
  background: color-mix(in srgb, var(--red) 14%, transparent);
  color: var(--red);
}

/* 开始生成页 */
.si-summary {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
}

.si-line {
  font-size: 12.5px;
  line-height: 1.7;
  word-break: break-all;
}

.si-line.dim {
  color: var(--text-dim);
}

.si-line code {
  font-family: "Cascadia Code", Consolas, monospace;
  color: var(--text);
  margin-right: 8px;
}

.si-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  margin-top: 4px;
}

/* 进度条：弹窗内用 8px 档 */
.progress-track {
  height: 8px;
  margin-top: 8px;
  border-radius: 999px;
  overflow: hidden;
  background: var(--bg-hover);
}

.progress-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent-grad, var(--accent));
  transition: width 0.3s;
}

.run-info {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  font-size: 11.5px;
  color: var(--text-dim);
}

.run-info .cur {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.si-result {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 4px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
}

.error {
  margin: 0;
  color: var(--red);
  font-size: 12.5px;
  word-break: break-all;
}

/* 实例下拉比标题栏标准的 170–280 宽一档：实例名长，要留够显示宽度 */
.head-select {
  min-width: 200px;
  max-width: 300px;
}
</style>
