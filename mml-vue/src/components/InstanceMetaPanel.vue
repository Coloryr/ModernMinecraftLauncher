<script setup lang="ts">
// 实例元信息面板：版本类型+版本 / 加载器+加载器版本 / 游戏内语言+日志编码 / 整合包类型
// 改动经 update 事件走 IPC 写入核心实例配置；刷新按钮与添加实例窗口同逻辑
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { t, tErr } from "../lib/i18n";
import { api, modpackCheck, modpackUpdate, onAddLoaderProgress } from "../lib/api";
import { showToast } from "../lib/toast";
import BaseButton from "./ui/BaseButton.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import ModPackUpdateModal from "./ModPackUpdateModal.vue";
import LoaderQueryProgress from "./LoaderQueryProgress.vue";
import type { InstanceInfoDto, ModPackUpdateDto, VersionInfoDto } from "../lib/bindings";

const props = defineProps<{
  /** 当前实例 */
  instance: InstanceInfoDto;
  /** 可选版本清单，供版本类型 / 版本下拉使用 */
  versions: VersionInfoDto[];
  /** 实例锁定生效：只留游戏内语言 / 日志编码，版本与加载器整块隐藏 */
  locked?: boolean;
}>();

const emit = defineEmits<{
  /** 提交改动；只带要改的字段 */
  (e: "update", patch: Partial<InstanceInfoDto>): void;
  /** 版本清单刷新完成：回传新清单 */
  (e: "refreshed", versions: VersionInfoDto[]): void;
}>();

/** 提交一处改动 */
function change(patch: Partial<InstanceInfoDto>) {
  emit("update", patch);
}

/**
 * 是不是整合包实例：整合包类型选了具体平台，"" / "none" 表示不是
 *
 * 整合包的版本与加载器是整合包作者在包里定下的 —— 版本、加载器、加载器版本都由
 * manifest 决定 —— 不该在实例设置里随手改，与"实例锁定"同一个理由，所以那一整块隐藏。
 * 整合包类型本身仍可编辑，那是"这个实例挂在哪条更新线上"。
 */
const isModpack = computed(
  () => !!props.instance.modpackType && props.instance.modpackType !== "none",
);

// 版本类型显示名；独立 ID 走 i18n 的 add.type.*
function typeName(vt: string): string {
  return t(`add.type.${vt}`);
}

// 实例的版本类型粒度是 release / snapshot / other（旧版合并），
// 版本清单里旧版拆成 old_beta / old_alpha，统一归并成 other
function normType(vt: string): string {
  return vt === "old_beta" || vt === "old_alpha" ? "other" : vt;
}

// ================= 整合包更新 =================

/** 这个实例是不是从平台安装的整合包：本地导入的包没有项目号，问不了平台 */
const onlineModpack = computed(
  () => props.instance.modpackType === "curseforge" || props.instance.modpackType === "modrinth",
);

/** 查更新 / 登记升级任务期间置真 */
const packBusy = ref(false);
/** 查到的新版本；null = 没在问 */
const packResult = ref<ModPackUpdateDto | null>(null);

/**
 * 查整合包更新
 *
 * 查到就开确认弹窗（见 ModPackUpdateModal），没查到给一条轻提示，不留空窗。
 * 入口在这一层而不是资源窗口：升级动的是整个实例。
 */
async function checkModpack() {
  if (packBusy.value || !onlineModpack.value) return;
  packBusy.value = true;
  try {
    const result = await modpackCheck(props.instance.uuid);
    if (!result.hasUpdate) {
      showToast(t("detail.modpackUpToDate"));
      return;
    }
    packResult.value = result;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    packBusy.value = false;
  }
}

/**
 * 确认更新：把任务登记到后端，进度在标题栏 / 整合包任务里看
 *
 * - `fileId`: 要升到的那份文件编号，取检查结果里的 `newFileId`
 */
async function applyModpackUpdate(fileId: string) {
  if (packBusy.value) return;
  packBusy.value = true;
  try {
    await modpackUpdate(props.instance.uuid, fileId);
    packResult.value = null;
    showToast(t("detail.modpackUpdating"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    packBusy.value = false;
  }
}

// ================= 版本类型 + 版本 =================

const versionTypes = computed(() => {
  const set = new Set<string>();
  for (const v of props.versions) set.add(normType(v.versionType));
  return [...set];
});

const typeFilter = ref(normType(props.instance.versionType ?? "release"));

// 跟随实例（切换实例 / 更新后同步）
watch(
  () => props.instance.versionType,
  (v) => {
    typeFilter.value = normType(v ?? "release");
  },
);

const filteredVersions = computed(() =>
  props.versions.filter((v) => normType(v.versionType) === typeFilter.value),
);

/**
 * 换游戏版本：加载器类型回到原版
 *
 * 新版本下原来那个加载器多半不存在（加载器版本列表也按版本拉），一起清掉并锁回未拉取状态。
 *
 * - `value`: 选中的版本 id
 */
function onVersionChange(value: string) {
  lvLocked.value = true;
  loaderVersions.value = [];
  change({ version: value, loader: "normal", loaderVersion: null });
}

function onVersionTypeChange(value: string) {
  typeFilter.value = value;
  // 当前版本不在该类型中时，切到该类型第一个版本；换版本同样把加载器带回原版
  const target = props.versions.find((v) => normType(v.versionType) === value);
  if (target && target.id !== props.instance.version) {
    onVersionChange(target.id);
    change({ versionType: value });
  } else {
    change({ versionType: value });
  }
}

// 刷新版本列表：清空后端缓存重新拉取，版本类型 / 版本随列表一起更新
const verLoading = ref(false);

async function onRefreshVersions() {
  verLoading.value = true;
  try {
    emit("refreshed", await api.refreshVersions());
    showToast(t("tip.refreshed"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    verLoading.value = false;
  }
}

// ================= 加载器类型 + 加载器版本 =================

// 加载器显示名；独立 ID 走 i18n 的 add.loader.*
function loaderLabel(id: string): string {
  return t(`add.loader.${id}`);
}

// 默认只有原版 / 当前加载器 / 自定义；点刷新拉取该版本支持的加载器后合并进来
const fetchedLoaders = ref<string[]>([]);
const loadersLoading = ref(false);

const loaderOptions = computed(() => {
  const opts = ["normal"];
  const cur = props.instance.loader;
  if (cur && !opts.includes(cur)) opts.push(cur);
  for (const id of fetchedLoaders.value) {
    if (!opts.includes(id)) opts.push(id);
  }
  if (!opts.includes("custom")) opts.push("custom");
  return opts;
});

// 支持列表查询进度；事件是全局广播，仅本面板查询期间显示
const progressStep = ref(0);
const progressTotal = ref(0);

let unlistenProgress: (() => void) | null = null;
onMounted(async () => {
  unlistenProgress = await onAddLoaderProgress((e) => {
    if (!loadersLoading.value) return;
    progressStep.value = e.step;
    progressTotal.value = e.total;
  });
});
onUnmounted(() => unlistenProgress?.());

async function onRefreshLoaders() {
  if (!props.instance.version) return;
  loadersLoading.value = true;
  progressStep.value = 0;
  try {
    fetchedLoaders.value = await api.addGetSupportLoaders(props.instance.version);
    showToast(t("tip.refreshed"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loadersLoading.value = false;
  }
}

// 加载器版本默认锁死：点刷新按钮（或切换加载器）拉取真实列表后解锁
const loaderVersions = ref<string[]>([]);
const lvLocked = ref(true);
const lvLoading = ref(false);

// 锁死 / 列表为空时也显示当前版本，避免下拉显示空白
const lvOptions = computed(() => {
  const list = [...loaderVersions.value];
  const cur = props.instance.loaderVersion;
  if (cur && !list.includes(cur)) list.unshift(cur);
  return list;
});

/** 拉取指定加载器在当前版本下的版本列表（原版 / 自定义没有版本列表） */
async function fetchLoaderVersions(loaderId: string, notify = false) {
  if (!loaderId || loaderId === "normal" || loaderId === "custom") return;
  if (!props.instance.version) return;
  lvLoading.value = true;
  try {
    loaderVersions.value = await api.addLoaderVersions(loaderId, props.instance.version);
    lvLocked.value = false;
    if (notify) showToast(t("tip.refreshed"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    lvLoading.value = false;
  }
}

async function onRefreshLoaderVersions() {
  await fetchLoaderVersions(props.instance.loader, true);
}

// 自定义加载器：路径选择，与添加实例窗口一致，文件名存入加载器版本字段
const loaderPathInput = ref<HTMLInputElement | null>(null);

function onLoaderPathPick(e: Event) {
  const input = e.target as HTMLInputElement;
  if (input.files?.[0]) change({ loaderVersion: input.files[0].name });
}

async function onLoaderChange(value: string) {
  // 切换加载器后重新锁死并自动拉取新加载器的版本列表
  lvLocked.value = true;
  loaderVersions.value = [];
  change({ loader: value, loaderVersion: null });
  await fetchLoaderVersions(value);
}

// ================= 游戏内语言 + 日志编码 =================

// 语言列表从实例的资源索引 minecraft/lang/*.json 里查；
// 资源未下载 / 查询失败时保留默认（简体中文 + English）
// label 是语言自称，与界面语言无关，取的就是游戏资源里的原文，所以不进 i18n
const DEFAULT_LANGS = [
  { value: "zh_cn", label: "简体中文" },
  { value: "en_us", label: "English" },
];
const langs = ref([...DEFAULT_LANGS]);
// 查询要读整份资源索引，耗时较长；期间下拉框显示"加载中"并禁用，
// 免得用户选到还没被真实列表替换掉的默认项
const langsLoading = ref(false);

async function refreshLangs() {
  langsLoading.value = true;
  try {
    const list = await api.getInstanceLangs(props.instance.uuid);
    if (list.length) {
      // 显示名由后端从该语言文件里读 language.name，如 简体中文；
      // 资源没下全时后端回落成代码，这里再兜一层，反正别显示空串
      langs.value = list.map((item) => ({
        value: item.code,
        label: item.name || item.code,
      }));
    }
  } catch {
    langs.value = [...DEFAULT_LANGS];
  } finally {
    langsLoading.value = false;
  }
}

// 切换实例：重置加载器查询状态并重新查询语言列表
watch(
  () => props.instance.uuid,
  () => {
    fetchedLoaders.value = [];
    loaderVersions.value = [];
    lvLocked.value = true;
    langs.value = [...DEFAULT_LANGS];
    refreshLangs();
  },
  { immediate: true },
);

// ================= 整合包类型（面板最底部） =================

const platforms = [
  { value: "", label: t("meta.platformNone") },
  { value: "curseforge", label: t("add.pack.curseforge") },
  { value: "modrinth", label: t("add.pack.modrinth") },
  { value: "serverpack", label: t("meta.platformOnline") },
];

function onPlatformChange(value: string) {
  // null 必须显式下发，JSON.stringify 会丢掉 undefined，后端才能区分“清空”
  if (!value) {
    change({ modpackType: null, pid: null, fid: null, serverUrl: null });
    return;
  }
  // 在线网络整合包用网址，其余平台用项目 / 文件 ID；切平台时清掉另一套字段
  if (value === "serverpack") {
    change({ modpackType: value, pid: null, fid: null });
  } else {
    change({ modpackType: value, serverUrl: null });
  }
}
</script>

<template>
  <div class="meta-panel">
    <!-- 加载器查询进度不浮在窗口正上方：就地显示在各自字段下面，见下面的
         .meta-progress，与添加实例窗口同一套 LoaderQueryProgress -->

    <!-- 版本与加载器：实例锁定时整块隐藏，避免玩家改掉整合包作者选定的版本；
         整合包实例同样整块隐藏 —— 版本 / 加载器 / 加载器版本都由整合包 manifest
         决定，这里给了也没法改对 —— 版本类型跟着一起隐，它只服务于那个版本下拉 -->
    <template v-if="!locked && !isModpack">
      <!-- 版本类型 + 版本 -->
      <span class="meta-label">{{ t("meta.versionType") }}</span>
      <select class="field-select" :value="typeFilter" :disabled="verLoading || loadersLoading || lvLoading"
        @change="onVersionTypeChange(($event.target as HTMLSelectElement).value)">
        <option v-for="vt in versionTypes" :key="vt" :value="vt">{{ typeName(vt) }}</option>
      </select>
      <span class="meta-label small">{{ t("meta.version") }}</span>
      <div class="select-row">
        <!-- 版本改了加载器就回到原版，见 onVersionChange；三种刷新期间都不给改 -->
        <select class="field-select" :value="instance.version" :disabled="verLoading || loadersLoading || lvLoading"
          @change="onVersionChange(($event.target as HTMLSelectElement).value)">
          <optgroup :label="typeName(typeFilter)">
            <option v-for="v in filteredVersions" :key="v.id" :value="v.id">{{ v.id }}</option>
          </optgroup>
        </select>
        <button class="ui-btn-icon" :disabled="verLoading || loadersLoading || lvLoading"
          v-tip="t('add.versionRefresh')"
          :aria-label="t('add.versionRefresh')" @click="onRefreshVersions">
          <GlyphIcon name="refresh" :size="14" :weight="2" />
        </button>
      </div>

      <!-- 加载器类型 + 加载器版本 -->
      <span class="meta-label">{{ t("meta.loader") }}</span>
      <div class="select-row">
        <!-- 默认只有原版 / 当前 / 自定义，刷新后合并该版本支持的加载器；查询期间锁住 -->
        <select class="field-select" :value="instance.loader" :disabled="loadersLoading || lvLoading"
          @change="onLoaderChange(($event.target as HTMLSelectElement).value)">
          <option v-for="l in loaderOptions" :key="l" :value="l">{{ loaderLabel(l) }}</option>
        </select>
        <button class="ui-btn-icon" :disabled="loadersLoading || !instance.version" v-tip="t('add.loaderRefresh')"
          :aria-label="t('add.loaderRefresh')" @click="onRefreshLoaders">
          <GlyphIcon name="refresh" :size="14" :weight="2" />
        </button>
      </div>
      <span class="meta-label small">{{ instance.loader === "custom" ? t("add.loaderPath") : t("meta.loaderVersion")
      }}</span>
      <!-- 自定义加载器：路径选择，文件名存入加载器版本字段；其余为版本下拉 -->
      <div v-if="instance.loader === 'custom'" class="select-row">
        <input class="field-input" :value="instance.loaderVersion ?? ''" :placeholder="t('add.loaderPathPlaceholder')"
          spellcheck="false" @input="change({ loaderVersion: ($event.target as HTMLInputElement).value || null })" />
        <BaseButton size="sm" variant="accent" @click="loaderPathInput?.click()">…</BaseButton>
        <input ref="loaderPathInput" type="file" accept=".jar" class="hidden-input" @change="onLoaderPathPick" />
      </div>
      <div v-else class="select-row">
        <!-- 默认锁死；点刷新或切换加载器后自动拉取真实版本列表并解锁 -->
        <select class="field-select" :value="instance.loaderVersion ?? ''"
          :disabled="lvLocked || lvLoading || loadersLoading"
          @change="change({ loaderVersion: ($event.target as HTMLSelectElement).value || null })">
          <option v-for="lv in lvOptions" :key="lv" :value="lv">{{ lv }}</option>
        </select>
        <button class="ui-btn-icon"
          :disabled="lvLoading || !instance.version || instance.loader === 'normal' || loadersLoading"
          v-tip="t('add.loaderVerRefresh')"
          :aria-label="t('add.loaderVerRefresh')" @click="onRefreshLoaderVersions">
          <GlyphIcon name="refresh" :size="14" :weight="2" />
        </button>
      </div>

      <!-- 查询进度就地显示在字段下面，原来浮在窗口正上方。
           面板是 4 列网格，所以每行要显式跨满整行，否则会被塞进"加载器版本"那两格里 -->
      <div v-if="loadersLoading" class="meta-progress">
        <LoaderQueryProgress kind="query" :visible="true" :step="progressStep" :total="progressTotal" />
      </div>
      <div v-if="lvLoading" class="meta-progress">
        <LoaderQueryProgress kind="versions" :visible="true" :step="0" :total="0" />
      </div>
    </template>

    <!-- 游戏内语言 + 日志编码 -->
    <span class="meta-label">{{ t("meta.lang") }}</span>
    <select class="field-select" :value="instance.lang ?? 'zh_cn'" :disabled="langsLoading"
      @change="change({ lang: ($event.target as HTMLSelectElement).value })">
      <option v-if="langsLoading" value="">{{ t("meta.langLoading") }}</option>
      <template v-else>
        <option v-for="l in langs" :key="l.value" :value="l.value">{{ l.label }}</option>
      </template>
    </select>
    <span class="meta-label small">{{ t("meta.logEncoding") }}</span>
    <select class="field-select" :value="instance.logEncoding ?? 'utf8'"
      @change="change({ logEncoding: ($event.target as HTMLSelectElement).value })">
      <option value="utf8">{{ t("meta.utf8") }}</option>
      <option value="gbk">{{ t("meta.gbk") }}</option>
    </select>

    <!-- 整合包类型：同样是整合包作者的设定，锁定时整块隐藏 -->
    <template v-if="!locked">
      <!-- 整合包类型：最底部，不占整行；ID 平台填项目 / 文件 ID，在线平台填网址 -->
      <span class="meta-label">{{ t("meta.modpack") }}</span>
      <select class="field-select" :value="instance.modpackType === 'none' ? '' : instance.modpackType ?? ''"
        @change="onPlatformChange(($event.target as HTMLSelectElement).value)">
        <option v-for="p in platforms" :key="p.value" :value="p.value">{{ p.label }}</option>
      </select>
      <!-- 在线网络整合包：网址输入框（隐藏 ID 输入框） -->
      <div v-if="instance.modpackType === 'serverpack'" class="modpack-url">
        <input class="field-input" :value="instance.serverUrl ?? ''" :placeholder="t('add.url')" spellcheck="false"
          @input="change({ serverUrl: ($event.target as HTMLInputElement).value || null })" />
      </div>
      <!-- ID 平台：项目 / 文件 ID 与"检查整合包更新"同一行。
           更新按钮只给从平台安装的整合包（本地导入的包没有项目号，问不了平台），
           只有图标、文案走悬停提示，与这一栏的其它小按钮一致 -->
      <div v-else-if="instance.modpackType && instance.modpackType !== 'none'" class="modpack-ids">
        <input class="field-input" :value="instance.pid ?? ''" :placeholder="t('meta.pid')" spellcheck="false"
          @input="change({ pid: ($event.target as HTMLInputElement).value || null })" />
        <input class="field-input" :value="instance.fid ?? ''" :placeholder="t('meta.fid')" spellcheck="false"
          @input="change({ fid: ($event.target as HTMLInputElement).value || null })" />
        <button class="ui-btn-icon" :disabled="packBusy" v-tip="t('detail.modpackCheck')"
          :aria-label="t('detail.modpackCheck')" @click="checkModpack">
          <GlyphIcon name="update" :size="14" :weight="2" />
        </button>
      </div>
      <!-- 整合包更新的确认窗：查到新版本才开 -->
      <ModPackUpdateModal v-if="packResult" :result="packResult" @update="applyModpackUpdate"
        @close="packResult = null" />
    </template>
  </div>
</template>

<style scoped>
/* 4 列网格：标签 | 下拉 | 标签 | 下拉，保证各下拉框左对齐。
   横向 12px 与启动参数面板的行内间距一致（两个卡片标签列都是 84，控件才能上下对齐）；
   纵向仍是原来的 10px 节奏 */
.meta-panel {
  display: grid;
  grid-template-columns: 84px 1fr 64px 1fr;
  column-gap: 12px;
  row-gap: 10px;
  align-items: center;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px 16px;
}

.meta-label {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-dim);
  white-space: nowrap;
}

.meta-label.small {
  text-align: right;
}

/* 加载器查询进度所在的那一行：跨满整行，跟在字段下面 */
.meta-progress {
  grid-column: 1 / -1;
}

/* 下拉 + 刷新按钮（与添加实例窗口一致的排布） */
.select-row {
  display: flex;
  gap: 6px;
  align-items: center;
  min-width: 0;
}

.select-row .field-select {
  flex: 1;
  min-width: 0;
}

.select-row .field-input {
  flex: 1;
  min-width: 0;
}

/* 隐藏的文件选择框（自定义加载器路径） */
.hidden-input {
  display: none;
}

/* 项目 / 文件 ID 与「检查整合包更新」图标按钮**同一行**（跨右侧两列）：
   两个输入框等宽，按钮按自身宽度收在右端 —— 三列而不是两列，否则按钮会被挤到下一行 */
.modpack-ids {
  grid-column: 3 / 5;
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: 10px;
  min-width: 0;
}

/* 在线网络整合包网址输入框 */

.modpack-url {
  grid-column: 3 / 5;
  display: flex;
  min-width: 0;
}

.modpack-url .field-input {
  flex: 1;
  min-width: 0;
}
</style>
