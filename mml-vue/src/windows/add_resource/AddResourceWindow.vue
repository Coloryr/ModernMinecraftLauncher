<script setup lang="ts">
// 添加资源窗口：模组 / 材质包 / 光影包 / 存档 / 数据包 在线搜索 + 下载
//
// 从主界面「添加资源」按钮打开，目标实例取 gui_config.json 里主窗口
// 当前选中的实例（与资源管理窗口一致）。数据包下载前必须先选存档。
// 下载走全局下载器：命令立即返回，进度在下载窗口里看。
//
// 项目详情与下载整合包窗口共用 components/ProjectDetailPage.vue。
import { computed, nextTick, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import InstanceChip from "../../components/ui/InstanceChip.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import AsyncImage from "../../components/ui/AsyncImage.vue";
import AuthorTag from "../../components/ui/AuthorTag.vue";
import ProjectDetailPage from "../../components/ProjectDetailPage.vue";
import { api } from "../../lib/api";
import { useDownloadBadges } from "../../lib/downloadBadges";
import { formatCount, t, tErr } from "../../lib/i18n";
import { svgIcon } from "../../lib/modrinthIcon";
import { cardTags } from "../../lib/projectTags";
import HighlightText from "../../components/ui/HighlightText.vue";
import { showToast } from "../../lib/toast";
import { loadGuiConfig } from "../../lib/guiConfig";
import { useResourceStatus } from "../../lib/resourceTasks";
import { familyInstanceFromLabel, projectParamToItem, targetProject, targetUuid, type WindowProjectParam } from "../windowManager";
import type {
  FileListItemDto,
  ProjectItemDto,
  ResourceDependencyDto,
  ResourceDependencyFileDto,
  ResourceSaveDto,
} from "../../lib/bindings";

defineEmits<{
  /** 请求关闭本窗口 */
  (e: "close"): void;
}>();

/** 下载任务状态（本窗口是"负责显示"的窗口） */
const { status: resourceStatus, init: initResourceStatus } = useResourceStatus(true);

/** 后端列表每页 20 个项目 */
const PAGE_SIZE = 20;

/** 资源类型，取值是后端 FileType 线串，显示标签走 i18n */
const TYPES = [
  { value: "mod", label: t("addResource.mods") },
  { value: "resourcepack", label: t("addResource.resourcepacks") },
  { value: "shaderpack", label: t("addResource.shaders") },
  { value: "save", label: t("addResource.saves") },
  { value: "dataPacks", label: t("addResource.datapacks") },
];

const SOURCE_LABELS: Record<string, string> = {
  curseforge: "modpack.curseforge",
  modrinth: "modpack.modrinth",
};

/** 模组可按加载器过滤（normal = 全部） */
const LOADERS = ["normal", "forge", "fabric", "quilt", "neoforge"];

/** 目标实例（uuid + 显示名），空 = 未选择 */
const instanceUuid = ref("");
const instanceName = ref("");
const instanceVersion = ref("");

/** 详情是否展开：详情铺满窗口后，返回键放到标题栏 */
const detailOpen = ref(false);

/** 标题栏 chip 的数据；没有目标实例时标题栏换成一句提示 */
const instanceChip = computed(() =>
  instanceUuid.value
    ? { name: instanceName.value || instanceUuid.value, version: instanceVersion.value }
    : null,
);

const type = ref("mod");
const sources = ref<Array<{ value: string; label: string }>>([]);
const source = ref("");

const sorts = ref<string[]>([]);
const sort = ref("");
const versions = ref<string[]>([]);
const version = ref("");
/** 分类，"" = 全部。键 = 传给后端的分类值，值 = 显示名 */
const categories = ref<Array<{ value: string; label: string }>>([]);
const category = ref("");
/** 加载器，仅模组类型有意义；normal 表示全部 */
const loader = ref("normal");
const filter = ref("");

// 离开本窗口时清掉搜索词：来源 / 类型 / 版本等**筛选**留着，那是浏览上下文，
// 搜索词只是"找某样东西"的一次性输入。
// 单窗口模式下这一页被 KeepAlive 缓存，切走不销毁，不清的话再打开时输入框里
// 还压着上次搜的词，用户早忘了自己搜过什么，只会觉得界面不对劲。
onDeactivated(() => {
  filter.value = "";
});

const items = ref<ProjectItemDto[]>([]);
const total = ref(0);
const page = ref(0);
const searching = ref(false);
const error = ref("");

/** 详情页展示的项目；null = 收起。详情与版本列表的状态都在共用组件里 */
const detailItem = ref<ProjectItemDto | null>(null);

/** 存档选择弹窗，数据包下载前用：待装的文件 + 存档列表 */
const savePickFile = ref<FileListItemDto | null>(null);
const saves = ref<ResourceSaveDto[]>([]);
const savesLoading = ref(false);
const pickedSave = ref("");

/** 前置勾选弹窗：正在下载的主文件 + 解析出的前置（已剔除实例里装了的） */
const depFile = ref<FileListItemDto | null>(null);
const deps = ref<ResourceDependencyDto[]>([]);
/** 勾上的前置：pid → 要下载的文件 ID；不在表里就是没勾，默认填自动选中的版本 */
const depsPick = ref(new Map<string, string>());
const depsLoading = ref(false);
/** 「不下载前置」的二次确认 */
const depSkipAsk = ref(false);

/** 内容区根节点，翻页时用它把窗口滚回顶部 */
const bodyRef = ref<HTMLElement | null>(null);

const maxPage = computed(() => Math.max(0, Math.ceil(total.value / PAGE_SIZE) - 1));

/** 存档下载只有 CurseForge 有，Modrinth 没有 world 类型 */
const sourceOptions = computed(() =>
  type.value === "save"
    ? sources.value.filter((s) => s.value === "curseforge")
    : sources.value,
);

const isSaveType = computed(() => type.value === "save");

/** 拉取下载源列表并选中第一个 */
async function loadSources() {
  try {
    sources.value = (await api.getModpackSources()).map((value) => ({
      value,
      label: t(SOURCE_LABELS[value] ?? value),
    }));
  } catch (e) {
    error.value = tErr(e);
    return;
  }
  if (sources.value.length === 0) {
    error.value = t("modpack.sourceFail");
    return;
  }
  source.value = sourceOptions.value[0]?.value ?? sources.value[0].value;
}

/**
 * 重载代次：换源 / 换类型会连着跑几发 loadSource（收藏跳转会同时改源与类型），
 * 旧的一发**后回来**会把新的排序表覆盖掉 —— 后续搜索就会带着上一个源的排序名
 * 发出去，后端报 err.sortTypeNotFound。每发记下编号，回来时不是最新就丢掉。
 */
let sourceSeq = 0;

/** 切换类型 / 下载源：清空搜索词与结果，重新拉排序 / 版本 / 分类，然后搜第一页 */
async function loadSource() {
  const mine = ++sourceSeq;
  items.value = [];
  total.value = 0;
  page.value = 0;
  error.value = "";
  closeDetail();
  filter.value = "";
  version.value = "";
  category.value = "";
  loader.value = "normal";
  sort.value = "";
  sorts.value = [];
  versions.value = [];
  categories.value = [];

  searching.value = true;
  try {
    const [sortList, versionList, categoryMap] = await Promise.all([
      api.getResourceSorts(source.value),
      api.getResourceVersions(source.value),
      api.getResourceCategories(source.value, type.value),
    ]);

    // 已经换源 / 换类型了：这一发的排序 / 版本 / 分类都是上一个的，丢
    if (mine !== sourceSeq) return;

    sorts.value = sortList;
    sort.value = sortList[0] ?? "";
    versions.value = versionList.filter((item) => item !== "");
    // 游戏版本默认落在实例自己的版本上：本窗口是"装进哪个实例"的上下文，默认「全部版本」
    // 会把不兼容的资源一起列出来。实例版本不在候选里（比如快照对不上）才退回全部
    version.value = versions.value.includes(instanceVersion.value) ? instanceVersion.value : "";
    categories.value = Object.entries(categoryMap)
      .map(([value, label]) => ({ value, label }))
      .sort((a, b) => a.label.localeCompare(b.label));
  } catch (e) {
    if (mine !== sourceSeq) return;
    error.value = tErr(e);
    searching.value = false;
    return;
  }

  if (mine !== sourceSeq) return;
  searching.value = false;

  await search();
}

/** 搜索代次：新的一发开始后，旧一发的返回全部丢弃 */
let searchSeq = 0;

async function search() {
  if (!instanceUuid.value) {
    error.value = t("addResource.noInstance");
    return;
  }
  // 排序列表还没拉回来时别发请求：换下载源 / 换类型时它是**异步**拉回来的
  // loadSource 里 await 三个请求，这中间发出去必然带不上合法排序
  // —— 空串后端也不认，报 err.sortTypeNotFound。那次 loadSource 拉完会自己再搜一次。
  if (sorts.value.length === 0) {
    return;
  }
  if (!sorts.value.includes(sort.value)) {
    sort.value = sorts.value[0];
  }
  const mine = ++searchSeq;
  searching.value = true;
  try {
    const res = await api.searchResources(
      instanceUuid.value,
      source.value,
      type.value,
      page.value,
      sort.value,
      category.value || null,
      filter.value.trim() || null,
      version.value || null,
      type.value === "mod" && LOADERS.includes(loader.value) ? loader.value : null,
    );
    if (mine !== searchSeq) return;
    items.value = res.items;
    total.value = res.count;
    error.value = "";
  } catch (e) {
    if (mine !== searchSeq) return;
    // 后端不认排序 → 换成该类型的第一个可用排序重试一次
    if (String(e).includes("err.sortTypeNotFound")) {
      // 只在**真的换成了别的值**时重试一次，否则同一个值会来回刷成死循环
      const fallback = sorts.value[0];
      if (fallback && fallback !== sort.value) {
        sort.value = fallback;
        searching.value = false;
        await search();
        return;
      }
    }
    items.value = [];
    total.value = 0;
    error.value = tErr(e);
  } finally {
    if (mine === searchSeq) searching.value = false;
  }
}

function submitSearch() {
  page.value = 0;
  void search();
}

function turnPage(delta: number) {
  const next = page.value + delta;
  if (next < 0 || next > maxPage.value) return;
  page.value = next;
  // 翻页先回到内容顶部：整页滚动的窗口里，用户此刻停在列表底部，不拉回去就看不到
  // 新一页的头几条。scrollIntoView 会自己找最近的滚动容器（这里是窗口框架的 .frame-body）
  bodyRef.value?.scrollIntoView({ behavior: "smooth", block: "start" });
  void search();
}

/** 列表卡片上的「已下载 / 下载中」角标，与整合包窗口、详情页共用一份判定 */
const { projectInstalled, projectRunning } = useDownloadBadges(resourceStatus);

/** 打开详情：正文 / 截图与版本列表由共用组件并行加载 */
function openDetail(item: ProjectItemDto) {
  detailItem.value = item;
  detailOpen.value = true;
}

/** 收起详情；标题栏上的返回键与 Esc 都走这里 */
function closeDetail() {
  detailItem.value = null;
  detailOpen.value = false;
}

/** 详情页取数：资源接口按实例取，加载器过滤留给列表页 */
function loadDetail(pid: string) {
  return api.getResourceDetail(source.value, pid);
}

function loadFiles(pid: string, pageNo: number, fileVersion: string | null) {
  return api.getResourceFiles(
    instanceUuid.value,
    source.value,
    pid,
    type.value,
    pageNo,
    fileVersion,
    null,
  );
}

/** 详情页点下载：数据包先选存档，模组先解析前置，其余类型直接下载 */
async function download(file: FileListItemDto) {
  if (!detailItem.value || !instanceUuid.value) return;
  const pid = detailItem.value.source.pid;

  if (type.value === "dataPacks") {
    savePickFile.value = file;
    pickedSave.value = "";
    savesLoading.value = true;
    saves.value = [];
    try {
      saves.value = await api.getResourceSaves(instanceUuid.value);
    } catch (e) {
      showToast(tErr(e));
    } finally {
      savesLoading.value = false;
    }
    return;
  }

  // 前置只有模组才有：其余类型直接下载，不为一张空表白跑一次解析
  if (type.value !== "mod") {
    await doDownload(pid, file);
    return;
  }

  await openDeps(pid, file);
}

/**
 * 打开前置勾选弹窗
 *
 * 解析在后端做（递归拉前置的前置，并剔除实例里已经装了的），这里只负责把它列出来：
 * 必选默认勾上、可选默认不勾，版本默认用后端自动挑的那个、可以改选。
 * 一个前置都没有时不弹空窗，直接下主文件。
 *
 * - `pid`: 主文件的项目 ID
 * - `file`: 主文件
 */
/** 前置解析代次：关掉弹窗或换一个前置时，旧一发的返回丢掉 */
let depsSeq = 0;

async function openDeps(pid: string, file: FileListItemDto) {
  const mine = ++depsSeq;
  depFile.value = file;
  deps.value = [];
  depsPick.value = new Map();
  depsLoading.value = true;

  try {
    const list = await api.getResourceDependencies(
      instanceUuid.value,
      source.value,
      pid,
      file.source.fid,
      type.value,
    );
    if (mine !== depsSeq) return;
    deps.value = list;
    // 必选的默认勾上，版本用后端自动挑的那个（files 的第一个）；可选的默认不勾
    depsPick.value = new Map(
      list.filter((dep) => dep.fid && !dep.optional).map((dep) => [dep.pid, dep.fid]),
    );
  } catch (e) {
    if (mine !== depsSeq) return;
    showToast(tErr(e));
  } finally {
    if (mine === depsSeq) depsLoading.value = false;
  }

  if (!deps.value.length) {
    closeDeps();
    await doDownload(pid, file);
  }
}

/** 勾选 / 取消一个前置；勾上时用它自动选中的版本 */
function toggleDep(pid: string, on: boolean, fid: string) {
  const map = new Map(depsPick.value);
  if (on) {
    map.set(pid, fid);
  } else {
    map.delete(pid);
  }
  depsPick.value = map;
}

/** 改选某个前置的版本；选了版本就顺手把它勾上（必选本来就在表里） */
function pickDepFile(pid: string, fid: string) {
  const map = new Map(depsPick.value);
  map.set(pid, fid);
  depsPick.value = map;
}

/** 版本下拉里的一行文案：版本名 + 日期 */
function depFileLabel(file: ResourceDependencyFileDto): string {
  return file.time ? `${file.name} · ${file.time.slice(0, 10)}` : file.name;
}

/** 有可选前置时才有「下载所选」这一档：全是必选时它跟「全部下载」是同一件事 */
const hasOptional = computed(() => deps.value.some((dep) => dep.optional));

/** 收起前置弹窗：没点确认就什么都不下 */
function closeDeps() {
  // 代次 +1：在途的那次解析回来时丢掉，免得把状态写回已关掉的弹窗
  depsSeq++;
  depFile.value = null;
  deps.value = [];
  depsPick.value = new Map();
  depsLoading.value = false;
  depSkipAsk.value = false;
}

/**
 * 收起弹窗并把主文件与给定前置一起排队
 *
 * - `picked`: 要一起下的前置；空表就是"不下载前置"
 */
async function startDownloads(picked: { pid: string; fid: string }[]) {
  const file = depFile.value;
  const pid = detailItem.value?.source.pid;
  if (!file || !pid || !instanceUuid.value) {
    closeDeps();
    return;
  }
  closeDeps();

  await doDownload(pid, file);

  let queued = 0;
  for (const dep of picked) {
    try {
      await api.downloadResource(
        instanceUuid.value,
        source.value,
        dep.pid,
        dep.fid,
        type.value,
        null,
      );
      queued += 1;
    } catch (e) {
      showToast(tErr(e));
    }
  }
  if (queued) {
    showToast(t("addResource.depsQueued", { n: queued }));
  }
}

/** 全部下载：主文件 + 列表里每一个前置，各自用下拉里显示的那个版本 */
async function confirmAllDeps() {
  const picked = deps.value
    .filter((dep) => dep.fid)
    .map((dep) => ({ pid: dep.pid, fid: depsPick.value.get(dep.pid) ?? dep.fid }));
  await startDownloads(picked);
}

/** 下载所选：只下勾上的那些（必选一定在勾选表里，取消不了） */
async function confirmSelectedDeps() {
  const picked = deps.value
    .filter((dep) => dep.fid && depsPick.value.has(dep.pid))
    .map((dep) => ({ pid: dep.pid, fid: depsPick.value.get(dep.pid) ?? dep.fid }));
  await startDownloads(picked);
}

/** 点「不下载前置」：先弹一句确认，免得误点把前置漏掉 */
function askSkipDeps() {
  depSkipAsk.value = true;
}

/** 确认跳过：只下主文件 */
async function skipDeps() {
  depSkipAsk.value = false;
  await startDownloads([]);
}

/** 存档选择确认：真正发起数据包下载 */
async function confirmSave() {
  if (!savePickFile.value || !detailItem.value || !pickedSave.value) return;
  const pid = detailItem.value.source.pid;
  const file = savePickFile.value;
  savePickFile.value = null;
  await doDownload(pid, file, pickedSave.value);
}

async function doDownload(pid: string, file: FileListItemDto, world: string | null = null) {
  try {
    await api.downloadResource(
      instanceUuid.value,
      source.value,
      pid,
      file.source.fid,
      type.value,
      world,
    );
    showToast(t("addResource.downloadOk", { name: file.name }));
    closeDetail();
  } catch (e) {
    showToast(tErr(e));
  }
}

function formatDate(date: string): string {
  return date ? date.slice(0, 10) : "";
}

/** 收藏 / 取消收藏 */
async function toggleStar(item: ProjectItemDto) {
  const star = !item.isStar;
  try {
    await api.collectStar(
      item.source.source,
      item.source.fileType,
      item.source.pid,
      item.name,
      item.image,
      item.url,
      star,
    );
    item.isStar = star;
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 类型 / 下载源变化：换源重载；存档类型强制 CurseForge */
watch([source, type], () => {
  if (source.value) loadSource();
});

watch(type, () => {
  // Modrinth 没有存档类型，切到存档时把源换到 CurseForge
  if (isSaveType.value && source.value === "modrinth") {
    source.value = "curseforge";
  }
});

onMounted(async () => {
  const unlisten = await initResourceStatus();
  onUnmounted(unlisten);

  await refreshInstance();
  await loadSources();
  consumeTargetProject();
});

/**
 * 读本窗口该锁定的实例，更新 uuid 与标题栏 chip 用的名字 / 版本
 *
 * 多窗口模式按窗口标签，那是窗口身份的一部分；单窗口 / 浏览器按本次跳转参数，收藏窗口
 * 点非整合包卡片时那边会先让用户选好实例再跳过来。都没有时退回主窗口选中的那个 ——
 * 单窗口模式下从别的页面返回本页时跳转参数已被清掉，读它就是改造前的行为。
 */
async function refreshInstance() {
  const locked = familyInstanceFromLabel("add_resource") ?? targetUuid() ?? "";
  try {
    instanceUuid.value = locked || (await loadGuiConfig())?.mainWindow.selectedInstance || "";
  } catch {
    instanceUuid.value = locked;
  }

  if (instanceUuid.value) {
    try {
      const list = await api.getInstances();
      const found = list.find((i) => i.uuid === instanceUuid.value);
      instanceName.value = found?.name ?? "";
      instanceVersion.value = found?.version ?? "";
    } catch {
      instanceName.value = "";
      instanceVersion.value = "";
    }
  }
}

/**
 * 收藏窗口点「下载」跳进来的项目
 *
 * 本窗口是"按实例取数"的（没实例拉不出版本列表），所以参数先存着：
 * 实例已选就直接打开详情，没选就等用户选好实例再打开（见下面的 watch）。
 *
 * 进去之前**先把上一次留下的视图状态清掉**：这页在单窗口模式下是 KeepAlive 缓存的，
 * 不清的话会带着上个项目的版本列表 / 搜索词跳转（看着就像"跳错了/没跳"）。
 * 清完还要等一轮 —— 改 type / source 会触发各自的 watch 去刷新列表，
 * 紧接着就开详情的话可能被那批刷新顺手盖掉。
 */
const pendingProject = ref<WindowProjectParam | null>(null);

async function consumeTargetProject() {
  const p = targetProject();
  if (!p || p.fileType === "modpack") {
    return;
  }
  closeDetail();
  filter.value = "";
  // 换源会让排序表作废：**同步**清掉，否则中间那几发搜索会带着上一个源的排序名
  sorts.value = [];
  sort.value = "";
  type.value = p.fileType;
  source.value = p.source;

  await nextTick();

  // 精简条目只够定位项目（下载次数等是空的）→ 现取一次真的，取不到就退回精简条目
  let item = projectParamToItem(p);
  try {
    const real = await api.collectProjectItem(p.source, p.pid, p.fileType);
    if (real) {
      item = real;
    }
  } catch {
    // 用精简条目兜底
  }

  if (instanceUuid.value) {
    openDetail(item);
  } else {
    pendingProject.value = p;
  }
}

watch(instanceUuid, async () => {
  if (instanceUuid.value && pendingProject.value) {
    const p = pendingProject.value;
    pendingProject.value = null;
    await nextTick();
    openDetail(projectParamToItem(p));
  }
});

// 单窗口模式下本页被 KeepAlive 缓存：切回来不会重新挂载，锁定实例与目标项目都得再取一次
onActivated(async () => {
  await refreshInstance();
  await consumeTargetProject();
});
</script>

<template>
  <WindowFrame :title="t('winTitle.addResource')" :back="detailOpen ? t('winCommon.back') : ''" body-gutter
    @close="$emit('close')" @back="closeDetail()">
    <!-- 标题栏：目标实例。本窗口是"装进哪个实例"的上下文，没实例就直说 -->
    <template #head-right>
      <InstanceChip v-if="instanceChip" :instance="instanceChip" />
      <span v-else class="head-warn">{{ t("addResource.noInstance") }}</span>
    </template>

    <div class="res-mode" ref="bodyRef">

      <!-- 资源类型 + 下载源 + 版本 -->
      <div class="res-top">
        <div class="res-filters">
          <SegmentedTabs v-model="type" :options="TYPES" :disabled="searching" />
          <!-- 存档只有 CurseForge 有：锁定源，不显示页签 -->
          <SegmentedTabs v-if="!isSaveType" v-model="source" :options="sourceOptions" :disabled="searching" />
          <div v-else class="res-source-lock">
            <span class="res-source-name">{{ t("modpack.curseforge") }}</span>
            <span class="res-source-hint">{{ t("addResource.saveSourceLimit") }}</span>
          </div>
          <select v-model="version" class="field-select sel-filter sel-version" :disabled="searching"
            @change="submitSearch">
            <option value="">{{ t("modpack.allVersions") }}</option>
            <option v-for="v in versions" :key="v" :value="v">{{ v }}</option>
          </select>
        </div>

        <!-- 过滤条件 + 搜索同一行：加载期间整条锁死，避免半途改条件与在途请求打架 -->
        <div class="res-filters">
          <select v-model="sort" class="field-select sel-filter" :disabled="searching" @change="submitSearch">
            <option v-for="s in sorts" :key="s" :value="s">{{ t(`modpack.sort.${s}`) }}</option>
          </select>
          <select v-if="categories.length" v-model="category" class="field-select sel-filter" :disabled="searching"
            @change="submitSearch">
            <option value="">{{ t("modpack.allCategories") }}</option>
            <option v-for="c in categories" :key="c.value" :value="c.value">{{ c.label }}</option>
          </select>
          <select v-if="type === 'mod'" v-model="loader" class="field-select sel-filter" :disabled="searching"
            @change="submitSearch">
            <option v-for="l in LOADERS" :key="l" :value="l">
              {{ l === "normal" ? t("addResource.loader.normal") : l }}
            </option>
          </select>

          <div class="res-search">
            <input v-model="filter" class="field-input search-input" :placeholder="t('addResource.searchHint')"
              spellcheck="false" :disabled="searching" @keydown.enter="submitSearch" />
            <button class="search-btn" :disabled="searching" @click="submitSearch">
              <span v-if="searching" class="btn-spinner"></span>
              {{ t("modpack.search") }}
            </button>
          </div>
        </div>
      </div>

      <!-- 列表加载指示：翻页 / 换筛选只在结果区上方走一条细进度，不再全窗口遮罩 -->
      <div class="list-bar" :class="{ on: searching }" aria-hidden="true"><span /></div>

      <!-- 结果列表：加载状态内联在本区，旧内容保留并压暗 -->
      <div class="res-list" :class="{ 'is-loading-dim': searching }">
        <!-- 首次加载（还没有内容）：骨架行 -->
        <template v-if="searching && items.length === 0">
          <div v-for="n in 6" :key="n" class="sk-row">
            <div class="sk sk-icon"></div>
            <div class="sk-lines">
              <div class="sk sk-line w60"></div>
              <div class="sk sk-line w90"></div>
              <div class="sk sk-line w40"></div>
            </div>
          </div>
        </template>
        <div v-else-if="error" class="empty-tip">{{ error }}</div>
        <div v-else-if="items.length === 0" class="empty-tip">{{ t("addResource.empty") }}</div>
        <template v-else>
          <div v-for="item in items" :key="item.source.pid" class="pack-item">
            <!-- 收藏星标（右上角）：点击收藏 / 取消收藏 -->
            <button class="pack-star" :class="{ on: item.isStar }"
              v-tip="item.isStar ? t('modpack.unstar') : t('modpack.star')" @click.stop="toggleStar(item)">
              <svg viewBox="0 0 24 24" width="17" height="17" stroke-width="2" stroke-linejoin="round">
                <path
                  d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
              </svg>
            </button>
            <div class="pack-main" @click="openDetail(item)">
              <AsyncImage v-if="item.image" class="pack-icon" :src="item.image" alt="" />
              <div v-else class="pack-icon pack-icon-fallback">{{ item.name.slice(0, 1).toUpperCase() }}</div>
              <div class="pack-info">
                <div class="pack-name">
                  <HighlightText :text="item.name" :query="filter" />
                  <!-- 作者：头像 + 名字的胶囊，与标题明确分开 -->
                  <AuthorTag v-if="item.authors.length" :authors="item.authors" />
                  <span v-if="projectInstalled(item)" class="pack-badge">{{ t("modpack.installed") }}</span>
                  <span v-else-if="projectRunning(item)" class="pack-badge busy">{{ t("modpack.downloading") }}</span>
                </div>
                <div class="pack-desc">{{ item.summary }}</div>
                <div v-if="item.tag.length" class="pack-tags">
                  <span v-for="tag in cardTags(item.tag).shown" :key="tag.name" class="pack-tag">
                    <span v-if="tag.svg" class="tag-svg" v-html="svgIcon(tag.svg)"></span>
                    <img v-else-if="tag.logo" class="pack-tag-icon" :src="tag.logo" loading="lazy" alt="" />
                    {{ tag.name }}
                  </span>
                  <!-- 超过上限的标签用省略号顶上，别把最后一个切掉一半 -->
                  <span v-if="cardTags(item.tag).more" class="pack-tag">…</span>
                </div>
                <div class="pack-meta">
                  {{ t("modpack.downloads", { n: formatCount(item.downloadCount) }) }}
                  <template v-if="item.date"> · {{ formatDate(item.date) }}</template>
                </div>
              </div>
            </div>
          </div>
        </template>
      </div>

      <!-- 分页 -->
      <div v-if="maxPage > 0" class="res-page">
        <button class="page-btn" :disabled="page === 0 || searching" @click="turnPage(-1)">
          {{ t("modpack.prevPage") }}
        </button>
        <span class="page-num">{{ page + 1 }} / {{ maxPage + 1 }} · {{ t("modpack.totalItems", { n: total }) }}</span>
        <button class="page-btn" :disabled="page >= maxPage || searching" @click="turnPage(1)">
          {{ t("modpack.nextPage") }}
        </button>
      </div>

      <!-- 存档选择弹窗（数据包下载前）：标题、提示、按钮栏都用全仓那套
           BaseModal + .modal-text + 全局 .modal-actions + BaseButton，与下面两个弹窗一致 -->
      <BaseModal v-if="savePickFile" :title="t('addResource.chooseSave')" :width="420" below-titlebar :closable="false"
        @close="savePickFile = null">
        <p class="modal-text">{{ t("addResource.chooseSaveHint") }}</p>
        <div class="save-list">
          <div v-if="savesLoading" class="empty-tip">{{ t("resource.loading") }}</div>
          <div v-else-if="saves.length === 0" class="empty-tip">{{ t("resource.empty") }}</div>
          <label v-for="save in saves" :key="save.dir" class="save-row" :class="{ on: pickedSave === save.dir }">
            <input v-model="pickedSave" type="radio" name="save-pick" :value="save.dir" />
            <span class="save-name">{{ save.name }}</span>
          </label>
        </div>
        <div class="modal-actions">
          <BaseButton @click="savePickFile = null">{{ t("actions.cancel") }}</BaseButton>
          <BaseButton variant="accent" :disabled="!pickedSave || savesLoading" @click="confirmSave">
            {{ t("addResource.download") }}
          </BaseButton>
        </div>
      </BaseModal>

      <!-- 前置勾选弹窗（下载模组前）：必选默认勾上且取消不了，可选的自己挑；右上角有关闭按钮 -->
      <BaseModal v-if="depFile" :title="t('addResource.depsTitle')" :width="640" fixed-height="520px" below-titlebar
        @close="closeDeps">
        <div class="deps-hint">{{ t("addResource.depsHint") }}</div>
        <div class="deps-list">
          <div v-if="depsLoading" class="deps-loading">
            <span class="deps-spinner"></span>
            {{ t("addResource.depsLoading") }}
          </div>
          <template v-else>
            <div v-for="dep in deps" :key="dep.pid" class="dep-row" :class="{ off: !dep.fid }">
              <!-- 必选取消不了：勾上且禁用，要跳过前置只能走下面那个「不下载前置」。
                   勾选框挂在整行上（不放进第一行），这样它按整行高度上下居中 -->
              <input :id="`dep-${dep.pid}`" class="dep-check" type="checkbox" :checked="depsPick.has(dep.pid)"
                :disabled="!dep.fid || !dep.optional"
                @change="toggleDep(dep.pid, ($event.target as HTMLInputElement).checked, dep.fid)" />
              <div class="dep-body">
                <label class="dep-main" :for="`dep-${dep.pid}`">
                  <AsyncImage v-if="dep.icon" class="dep-icon" :src="dep.icon" alt="" />
                  <span class="dep-name">{{ dep.name || dep.pid }}</span>
                  <span class="dep-tag">
                    {{ dep.optional ? t("addResource.depsOptional") : t("addResource.depsRequired") }}
                  </span>
                </label>
                <!-- 版本：默认是自动挑的那个，可以改选 -->
                <div v-if="dep.files.length" class="dep-files">
                  <span class="dep-files-label">{{ t("addResource.depsVersion") }}</span>
                  <select class="field-select dep-select" :value="depsPick.get(dep.pid) ?? dep.fid"
                    @change="pickDepFile(dep.pid, ($event.target as HTMLSelectElement).value)">
                    <option v-for="file in dep.files" :key="file.fid" :value="file.fid">
                      {{ depFileLabel(file) }}
                    </option>
                  </select>
                </div>
                <span v-else class="dep-none">{{ t("addResource.depsNoFile") }}</span>
              </div>
            </div>
          </template>
        </div>
        <div class="modal-actions">
          <BaseButton :disabled="depsLoading" @click="askSkipDeps">
            {{ t("addResource.depsSkip") }}
          </BaseButton>
          <BaseButton variant="accent" :disabled="depsLoading" @click="confirmAllDeps">
            {{ t("addResource.depsAll") }}
          </BaseButton>
          <!-- 全是必选时它跟「全部下载」是同一件事，不显示 -->
          <BaseButton v-if="hasOptional" variant="accent" :disabled="depsLoading" @click="confirmSelectedDeps">
            {{ t("addResource.depsSelected") }}
          </BaseButton>
        </div>
      </BaseModal>

      <!-- 跳过前置的二次确认 -->
      <BaseModal v-if="depSkipAsk" :title="t('addResource.depsSkipTitle')" :width="380" below-titlebar :closable="false"
        @close="depSkipAsk = false">
        <p class="deps-hint">{{ t("addResource.depsSkipHint") }}</p>
        <div class="modal-actions">
          <BaseButton @click="depSkipAsk = false">{{ t("actions.cancel") }}</BaseButton>
          <BaseButton variant="accent" @click="skipDeps">{{ t("addResource.depsSkipConfirm") }}</BaseButton>
        </div>
      </BaseModal>

      <!-- 项目详情：与下载整合包窗口共用同一份实现 -->
      <ProjectDetailPage :item="detailItem" :versions="versions" :initial-version="version"
        :action-label="t('addResource.download')" :status="resourceStatus" :load-detail="loadDetail"
        :load-files="loadFiles" @close="closeDetail" @download="download" />
    </div>
  </WindowFrame>
</template>

<style scoped src="./add-resource-window.css"></style>
