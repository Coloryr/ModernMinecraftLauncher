<script setup lang="ts">
// 实例元信息面板：版本类型+版本 / 加载器+加载器版本 / 游戏内语言+日志编码 / 自动打开日志窗口 / 整合包类型
// 元信息改动经 update 事件走 IPC 写入核心实例配置，日志窗口开关直接调日志设置命令；
// 刷新按钮与添加实例窗口同逻辑
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { t, tErr } from "../lib/i18n";
import { api, modpackCheck, modpackUpdate, onAddLoaderProgress } from "../lib/api";
import { showToast } from "../lib/toast";
import BaseButton from "./ui/BaseButton.vue";
import BaseSwitch from "./ui/BaseSwitch.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import ModPackUpdateModal from "./ModPackUpdateModal.vue";
import LoaderQueryProgress from "./LoaderQueryProgress.vue";
import type { InstanceInfoDto, ModPackUpdateDto, VersionInfoDto } from "../lib/bindings";

const props = defineProps<{
  /** 当前实例 */
  instance: InstanceInfoDto;
  /** 可选版本清单，供版本类型 / 版本下拉使用 */
  versions: VersionInfoDto[];
  /** 实例锁定生效：隐藏版本 / 加载器与整合包设置，只留游戏内语言、日志编码与自动打开日志窗口 */
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

// 版本类型显示名；文案键为 add.type.*
function typeName(vt: string): string {
  return t(`add.type.${vt}`);
}

// 实例的版本类型粒度是 release / snapshot / other（旧版合并），
// 版本清单里旧版拆成 old_beta / old_alpha，统一归并成 other
function normType(vt: string): string {
  return vt === "old_beta" || vt === "old_alpha" ? "other" : vt;
}

// ================= 整合包更新 =================

/** 能不能查更新：平台安装的整合包按平台查，在线服务器包按发布地址查 */
const updatableModpack = computed(
  () =>
    props.instance.modpackType === "curseforge" ||
    props.instance.modpackType === "modrinth" ||
    (props.instance.modpackType === "serverpack" && !!props.instance.serverUrl),
);

/**
 * 「检查整合包更新」按钮能不能点
 *
 * 平台整合包还要**项目 ID 与文件 ID 都在**才问得动平台，缺一个都拼不出请求；
 * 在线服务器包只看地址（判据在 updatableModpack 里）。
 * 自动检查仍按 updatableModpack 走：手填 ID 的过程中不该突然弹确认窗。
 */
const canCheckModpack = computed(() => {
  const type = props.instance.modpackType;
  if (type === "curseforge" || type === "modrinth") {
    return !!props.instance.pid && !!props.instance.fid;
  }
  return updatableModpack.value;
});

/** 这次检查针对的是在线服务器包 */
const serverPack = computed(() => props.instance.modpackType === "serverpack");

/** 查更新 / 登记升级任务期间置真 */
const packBusy = ref(false);
/** 查到的新版本；null = 没在问 */
const packResult = ref<ModPackUpdateDto | null>(null);

/**
 * 查整合包 / 服务器包更新
 *
 * 查到就开确认弹窗（见 ModPackUpdateModal），没查到给一条轻提示，不留空窗。
 * 入口在这一层而不是资源窗口：升级动的是整个实例。
 *
 * - `auto`: 选中实例后自动查的那一次；不弹"已是最新"，失败也不提示，免得每次切实例都打扰
 */
async function checkModpack(auto = false) {
  if (packBusy.value || !canCheckModpack.value) return;
  packBusy.value = true;
  try {
    const result = await modpackCheck(props.instance.uuid);
    if (!result.hasUpdate) {
      if (!auto) showToast(t(serverPack.value ? "detail.serverPackUpToDate" : "detail.modpackUpToDate"));
      return;
    }
    packResult.value = result;
  } catch (e) {
    if (!auto) showToast(tErr(e));
  } finally {
    packBusy.value = false;
  }
}

/** 已经自动查过的实例：实例列表刷新会换掉对象，只认 uuid，免得反复查 */
const autoChecked = new Set<string>();

// 选中实例后自动按下「检查更新」：有新版就弹确认窗，没有就静默。
// 同时盯 `updatableModpack`：服务器包的地址是随实例数据异步回来的，来晚一步也要补查一次
watch(
  [() => props.instance.uuid, updatableModpack] as const,
  ([uuid, ok]) => {
    // 锁定的实例整块整合包设置是隐藏的，自动查到了也没地方弹出确认窗
    if (!uuid || props.locked || !ok || autoChecked.has(uuid)) return;
    autoChecked.add(uuid);
    void checkModpack(true);
  },
  { immediate: true },
);

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
 * 一次提交所有改动，避免分两次 IPC 写同一条实例记录。
 *
 * - `value`: 选中的版本 id
 * - `extra`: 顺带提交的其它字段，如版本类型
 */
function onVersionChange(value: string, extra: Partial<InstanceInfoDto> = {}) {
  lvLocked.value = true;
  loaderVersions.value = [];
  change({ version: value, loader: "normal", loaderVersion: null, ...extra });
}

function onVersionTypeChange(value: string) {
  typeFilter.value = value;
  // 当前版本不在该类型中时，切到该类型第一个版本；换版本同样把加载器带回原版
  const target = props.versions.find((v) => normType(v.versionType) === value);
  if (target && target.id !== props.instance.version) {
    onVersionChange(target.id, { versionType: value });
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
let disposed = false;
onMounted(async () => {
  const off = await onAddLoaderProgress((e) => {
    if (!loadersLoading.value) return;
    progressStep.value = e.step;
    progressTotal.value = e.total;
  });
  // 订阅期间组件已卸载：立刻退订，别把监听留到窗口关闭之后
  if (disposed) {
    off();
    return;
  }
  unlistenProgress = off;
});
onUnmounted(() => {
  disposed = true;
  unlistenProgress?.();
  unlistenProgress = null;
});

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

let langsSeq = 0;

async function refreshLangs() {
  const mine = ++langsSeq;
  langsLoading.value = true;
  try {
    const list = await api.getInstanceLangs(props.instance.uuid);
    if (mine !== langsSeq) return;
    if (list.length) {
      // 显示名由后端从该语言文件里读 language.name，如 简体中文；
      // 资源没下全时后端回落成代码，这里再兜一层，反正别显示空串
      langs.value = list.map((item) => ({
        value: item.code,
        label: item.name || item.code,
      }));
    }
  } catch {
    if (mine === langsSeq) langs.value = [...DEFAULT_LANGS];
  } finally {
    if (mine === langsSeq) langsLoading.value = false;
  }
}

// ================= 自动打开日志窗口 =================

/** 启动游戏后自动打开该实例的日志窗口；存在实例的 `gui_setting.json` 里 */
const logAutoShow = ref(false);
/** 设置是否已读到：读到之前开关不可点，免得把默认值写回去 */
const logAutoShowLoaded = ref(false);
/** 保存中，避免连点 */
const logAutoShowBusy = ref(false);
/** 读取序号：切换实例后旧请求的结果直接丢掉 */
let logAutoShowSeq = 0;

/** 读一次日志设置，只取「自动打开」这一项 */
async function refreshLogAutoShow() {
  const seq = ++logAutoShowSeq;
  logAutoShowLoaded.value = false;
  try {
    const setting = await api.getLogSetting(props.instance.uuid);
    if (seq === logAutoShowSeq) logAutoShow.value = setting.autoShow;
  } catch (e) {
    console.error("[InstanceMetaPanel] 读取日志设置失败", e);
    if (seq === logAutoShowSeq) logAutoShow.value = false;
  } finally {
    if (seq === logAutoShowSeq) logAutoShowLoaded.value = true;
  }
}

/**
 * 切换「自动打开日志窗口」
 *
 * - `on`: 新的开关状态
 */
async function onLogAutoShow(on: boolean) {
  if (!logAutoShowLoaded.value || logAutoShowBusy.value) return;
  const before = logAutoShow.value;
  logAutoShow.value = on;
  logAutoShowBusy.value = true;
  try {
    await api.setLogAutoShow(props.instance.uuid, on);
  } catch (e) {
    logAutoShow.value = before;
    showToast(tErr(e));
  } finally {
    logAutoShowBusy.value = false;
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
    refreshLogAutoShow();
  },
  { immediate: true },
);

// ================= 整合包类型（面板最底部） =================

// 放进 computed：切换语言时下拉文案要跟着变，setup 里取一次会停在旧语言
const platforms = computed(() => [
  { value: "", label: t("meta.platformNone") },
  { value: "curseforge", label: t("add.pack.curseforge") },
  { value: "modrinth", label: t("add.pack.modrinth") },
  { value: "serverpack", label: t("meta.platformOnline") },
]);

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
    <!-- 加载器查询进度就地显示在各自字段下面，见 .meta-progress；
         与添加实例窗口同一套 LoaderQueryProgress -->

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
          v-tip="t('add.versionRefresh')" :aria-label="t('add.versionRefresh')" @click="onRefreshVersions">
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
          v-tip="t('add.loaderVerRefresh')" :aria-label="t('add.loaderVerRefresh')" @click="onRefreshLoaderVersions">
          <GlyphIcon name="refresh" :size="14" :weight="2" />
        </button>
      </div>

      <!-- 查询进度就地显示在字段下面。面板是 4 列网格，所以每行要显式跨满整行，
           否则会被塞进"加载器版本"那两格里 -->
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

    <!-- 自动打开日志窗口：实例级行为 -->
    <span class="meta-label">{{ t("meta.logWindow") }}</span>
    <label class="meta-switch">
      <BaseSwitch :model-value="logAutoShow" :disabled="!logAutoShowLoaded || logAutoShowBusy"
        @update:model-value="onLogAutoShow" />
      <span>{{ t("meta.logAutoOpen") }}</span>
    </label>

    <!-- 整合包类型：同样是整合包作者的设定，锁定时整块隐藏 -->
    <template v-if="!locked">
      <!-- 整合包类型：最底部，不占整行；ID 平台填项目 / 文件 ID，在线平台填网址 -->
      <span class="meta-label">{{ t("meta.modpack") }}</span>
      <!-- 查更新期间整块锁住：类型、网址都不能改，按钮转圈表示正在检测 -->
      <select class="field-select" :value="instance.modpackType === 'none' ? '' : instance.modpackType ?? ''"
        :disabled="packBusy" @change="onPlatformChange(($event.target as HTMLSelectElement).value)">
        <option v-for="p in platforms" :key="p.value" :value="p.value">{{ p.label }}</option>
      </select>
      <!-- 在线网络整合包：网址输入框（隐藏 ID 输入框），右侧是同步按钮 -->
      <div v-if="instance.modpackType === 'serverpack'" class="modpack-url">
        <input class="field-input" :value="instance.serverUrl ?? ''" :placeholder="t('add.url')" spellcheck="false"
          :disabled="packBusy" @input="change({ serverUrl: ($event.target as HTMLInputElement).value || null })" />
        <button class="ui-btn-icon" :disabled="packBusy || !instance.serverUrl"
          v-tip="packBusy ? t('detail.modpackChecking') : t('detail.serverPackCheck')"
          :aria-label="packBusy ? t('detail.modpackChecking') : t('detail.serverPackCheck')" @click="checkModpack()">
          <span v-if="packBusy" class="pack-spin" />
          <GlyphIcon v-else name="update" :size="14" :weight="2" />
        </button>
      </div>
      <!-- ID 平台：项目 / 文件 ID 与"检查整合包更新"同一行。
           更新按钮只给从平台安装的整合包（本地导入的包没有项目号，问不了平台），
           只有图标、文案走悬停提示，与这一栏的其它小按钮一致 -->
      <div v-else-if="instance.modpackType && instance.modpackType !== 'none'" class="modpack-ids">
        <input class="field-input" :value="instance.pid ?? ''" :placeholder="t('meta.pid')" spellcheck="false"
          :disabled="packBusy" @input="change({ pid: ($event.target as HTMLInputElement).value || null })" />
        <input class="field-input" :value="instance.fid ?? ''" :placeholder="t('meta.fid')" spellcheck="false"
          :disabled="packBusy" @input="change({ fid: ($event.target as HTMLInputElement).value || null })" />
        <!-- 项目 / 文件 ID 缺一个就问不动平台，按钮直接禁用并说明缺什么 -->
        <button class="ui-btn-icon" :disabled="packBusy || !canCheckModpack"
          v-tip="packBusy ? t('detail.modpackChecking') : !canCheckModpack ? t('detail.modpackNeedIds') : t('detail.modpackCheck')"
          :aria-label="packBusy ? t('detail.modpackChecking') : !canCheckModpack ? t('detail.modpackNeedIds') : t('detail.modpackCheck')"
          @click="checkModpack()">
          <span v-if="packBusy" class="pack-spin" />
          <GlyphIcon v-else name="update" :size="14" :weight="2" />
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

/* 自动打开日志窗口：开关与文案同一行，跨过右侧三列 */
.meta-switch {
  grid-column: 2 / 5;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
  color: var(--text);
  user-select: none;
  cursor: pointer;
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

/* 项目 / 文件 ID 与「检查整合包更新」图标按钮同一行，跨右侧两列：
   两个输入框等宽，按钮按自身宽度收在右端。用三列而不是两列，否则按钮会被挤到下一行 */
.modpack-ids {
  grid-column: 3 / 5;
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: 10px;
  min-width: 0;
}

/* 在线网络整合包网址输入框 + 右侧同步按钮 */
.modpack-url {
  grid-column: 3 / 5;
  display: flex;
  gap: 10px;
  min-width: 0;
}

.modpack-url .field-input {
  flex: 1;
  min-width: 0;
}

/* 检测中的转圈：与按钮里那个更新图标同尺寸，转动表示查询在跑。
   关掉界面动画时会被全局规则停住，这时靠提示文案与禁用态表达 */
.pack-spin {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  animation: pack-spin 0.8s linear infinite;
}

@keyframes pack-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
