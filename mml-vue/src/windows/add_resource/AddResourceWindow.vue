<script setup lang="ts">
// 添加资源窗口：模组 / 材质包 / 光影包 / 存档 / 数据包 在线搜索 + 下载
//
// 从主界面「添加资源」按钮打开，目标实例取 gui_config.json 里主窗口
// 当前选中的实例（与资源管理窗口一致）。数据包下载前必须先选存档。
// 下载走全局下载器：命令立即返回，进度在下载窗口里看。
import { computed, nextTick, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from "vue";
import DOMPurify from "dompurify";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import InstanceChip from "../../components/ui/InstanceChip.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import AsyncImage from "../../components/ui/AsyncImage.vue";
import ResourceDownloadBar from "../../components/ResourceDownloadBar.vue";
import { api } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { loadGuiConfig } from "../../lib/guiConfig";
import { useResourceStatus } from "../../lib/resourceTasks";
import { projectParamToItem, targetProject, targetUuid, type WindowProjectParam } from "../windowManager";
import type { FileListItemDto, ProjectItemDto, ResourceSaveDto, ResourceTaskDto } from "../../lib/bindings";

defineEmits<{
  /** 请求关闭本窗口 */
  (e: "close"): void;
}>();

/** 下载任务状态（本窗口是"负责显示"的窗口） */
const { status: resourceStatus, init: initResourceStatus } = useResourceStatus(true);

/** 后端列表每页 20 个项目 */
const PAGE_SIZE = 20;
/** 版本列表每页条数，与 CurseForge 后端分页一致 */
const FILE_PAGE_SIZE = 50;

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

/** 版本选择弹窗：当前项目 + 版本列表 */
const versionItem = ref<ProjectItemDto | null>(null);
const files = ref<FileListItemDto[]>([]);
const allFiles = ref<FileListItemDto[]>([]);
const filesLoading = ref(false);
const filePage = ref(0);
const fileTotal = ref(0);
const fileVersion = ref("");

/** 存档选择弹窗，数据包下载前用：待装的文件 + 存档列表 */
const savePickFile = ref<FileListItemDto | null>(null);
const saves = ref<ResourceSaveDto[]>([]);
const savesLoading = ref(false);
const pickedSave = ref("");

const maxPage = computed(() => Math.max(0, Math.ceil(total.value / PAGE_SIZE) - 1));

const isModrinth = computed(() => source.value === "modrinth");

/** 当前页显示的版本列表：Modrinth 一次拿全部本地切片，CurseForge 后端分页 */
const pageFiles = computed(() =>
  isModrinth.value
    ? allFiles.value.slice(filePage.value * FILE_PAGE_SIZE, (filePage.value + 1) * FILE_PAGE_SIZE)
    : files.value,
);
const fileMaxPage = computed(() => Math.max(1, Math.ceil(fileTotal.value / FILE_PAGE_SIZE)));

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
  void search();
}

/** 版本列表代次：换筛选 / 换页时丢弃旧一发的返回 */
let fileSeq = 0;

async function loadFiles(pid: string, pageNo: number) {
  if (!instanceUuid.value) return;
  const mine = ++fileSeq;
  filesLoading.value = true;
  try {
    const res = await api.getResourceFiles(
      instanceUuid.value,
      source.value,
      pid,
      type.value,
      pageNo,
      fileVersion.value || null,
      null,
    );
    if (mine !== fileSeq) return;
    fileTotal.value = res.count || res.list.length;
    if (isModrinth.value) {
      allFiles.value = res.list;
    } else {
      files.value = res.list;
    }
  } catch (e) {
    if (mine === fileSeq) showToast(tErr(e));
  } finally {
    if (mine === fileSeq) filesLoading.value = false;
  }
}

/** 点击项目：打开版本选择弹窗（筛选沿用列表页选中的游戏版本） */
function openVersions(item: ProjectItemDto) {
  versionItem.value = item;
  files.value = [];
  allFiles.value = [];
  filePage.value = 0;
  fileTotal.value = 0;
  fileVersion.value = version.value;
  loadFiles(item.source.pid, 0);
}

function closeVersions() {
  fileSeq++;
  versionItem.value = null;
  files.value = [];
  allFiles.value = [];
}

function onFileVersionChange() {
  filePage.value = 0;
  if (versionItem.value) {
    loadFiles(versionItem.value.source.pid, 0);
  }
}

function turnFilePage(delta: number) {
  const next = filePage.value + delta;
  if (next < 0 || next >= fileMaxPage.value) return;
  filePage.value = next;
  if (isModrinth.value || !versionItem.value) return;
  loadFiles(versionItem.value.source.pid, next);
}

/** 下载资源：数据包先选存档，其余直接加入下载 */
async function download(file: FileListItemDto) {
  if (!versionItem.value || !instanceUuid.value) return;
  const pid = versionItem.value.source.pid;

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

  await doDownload(pid, file);
}

/** 存档选择确认：真正发起数据包下载 */
async function confirmSave() {
  if (!savePickFile.value || !versionItem.value || !pickedSave.value) return;
  const pid = versionItem.value.source.pid;
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
    closeVersions();
  } catch (e) {
    showToast(tErr(e));
  }
}

/**
 * Modrinth 分类图标：icon 字段是内联 svg 源码，stroke=currentColor，消毒后内联渲染
 *
 * 消毒结果按源码缓存：模板里直接调用函数，每次重渲染都会重跑一遍 DOMPurify，
 * 而同一批分类图标的源码是不变的。
 */
const svgCache = new Map<string, string>();

function svgIcon(svg: string): string {
  const hit = svgCache.get(svg);
  if (hit !== undefined) return hit;
  const safe = DOMPurify.sanitize(svg);
  svgCache.set(svg, safe);
  return safe;
}

function formatDate(date: string): string {
  return date ? date.slice(0, 10) : "";
}

/**
 * 本次会话里见过的「下载完成」标记（只增不减）
 *
 * 任务表的终态条目由后端延时移除，但文件确实已经装进实例了——
 * 「已下载」角标不能跟着任务一起消失。只往集合里加，不做任何复位。
 */
const donePids = ref(new Set<string>());
const doneFileKeys = ref(new Set<string>());

watch(
  resourceStatus,
  (status) => {
    const tasks = status?.tasks ?? [];
    if (!tasks.some((task) => task.done)) return;
    const pids = new Set(donePids.value);
    const files = new Set(doneFileKeys.value);
    for (const task of tasks) {
      if (!task.done) continue;
      pids.add(task.pid);
      files.add(`${task.pid}|${task.fid}`);
    }
    // 只在真新增时换引用：进度事件很频繁，无谓的赋值会带着模板一起重渲染
    if (pids.size !== donePids.value.size) donePids.value = pids;
    if (files.size !== doneFileKeys.value.size) doneFileKeys.value = files;
  },
  { immediate: true },
);

/**
 * 进行中任务派生的角标状态
 *
 * 由 `resourceStatus` 直接**派生**，不再往列表项里写回 `download` / `downloadNow`：
 * 写回是单向的（只置 true 不复位），任务失败或结束后角标会一直挂着。
 * 「已下载」取实例侧来源 `item.download` / `file.isDownload`，与上面那张累计表取「或」。
 */
const taskState = computed(() => {
  const running = (task: ResourceTaskDto) => !task.done && !task.failed;
  const tasks = resourceStatus.value?.tasks ?? [];
  return {
    runningProjects: new Set(tasks.filter(running).map((task) => task.pid)),
    runningFiles: new Set(tasks.filter(running).map((task) => `${task.pid}|${task.fid}`)),
  };
});

function fileKey(file: FileListItemDto): string {
  return `${file.source.pid}|${file.source.fid}`;
}

/** 项目级「已下载」（实例侧已装 + 本次会话装完） */
function projectInstalled(item: ProjectItemDto): boolean {
  return item.download || donePids.value.has(item.source.pid);
}

/** 项目级「下载中」：只看任务表——DTO 里那份是取列表时的快照，用它角标会粘住 */
function projectRunning(item: ProjectItemDto): boolean {
  return taskState.value.runningProjects.has(item.source.pid);
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

/** 文件级「已下载」 */
function fileInstalled(file: FileListItemDto): boolean {
  return file.isDownload || doneFileKeys.value.has(fileKey(file));
}

/** 文件级「下载中」 */
function fileRunning(file: FileListItemDto): boolean {
  return taskState.value.runningFiles.has(fileKey(file));
}

function formatSize(size: number): string {
  if (!size) return t("modpack.unknownSize");
  if (size >= 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(size / 1024))} KB`;
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

  try {
    const cfg = await loadGuiConfig();
    // 目标实例：openWindow 带进来的那个优先，收藏窗口点非整合包卡片时
    // 那边会先让用户选好实例再跳过来；否则用主窗口选中的那个
    instanceUuid.value = targetUuid() ?? cfg?.mainWindow.selectedInstance ?? "";
  } catch {
    instanceUuid.value = targetUuid() ?? "";
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

  await loadSources();
  consumeTargetProject();
});

/**
 * 收藏窗口点「下载」跳进来的项目
 *
 * 本窗口是"按实例取数"的（没实例拉不出版本列表），所以参数先存着：
 * 实例已选就直接打开版本列表，没选就等用户选好实例再打开（见下面的 watch）。
 *
 * 进去之前**先把上一次留下的视图状态清掉**：这页在单窗口模式下是 KeepAlive 缓存的，
 * 不清的话会带着上个项目的版本弹窗 / 文件列表 / 搜索词跳转（看着就像"跳错了/没跳"）。
 * 清完还要等一轮 —— 改 type / source 会触发各自的 watch 去刷新列表，
 * 紧接着就开弹窗的话可能被那批刷新顺手盖掉。
 */
const pendingProject = ref<WindowProjectParam | null>(null);

async function consumeTargetProject() {
  const p = targetProject();
  if (!p || p.fileType === "modpack") {
    return;
  }
  closeVersions();
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
    openVersions(item);
  } else {
    pendingProject.value = p;
  }
}

watch(instanceUuid, async () => {
  if (instanceUuid.value && pendingProject.value) {
    const p = pendingProject.value;
    pendingProject.value = null;
    await nextTick();
    openVersions(projectParamToItem(p));
  }
});

// 单窗口模式下本页被 KeepAlive 缓存：切回来不会重新挂载，得在激活时再取一次参数
onActivated(consumeTargetProject);
</script>

<template>
  <WindowFrame :title="t('winTitle.addResource')" body-gutter @close="$emit('close')">
    <!-- 标题栏：目标实例。本窗口是"装进哪个实例"的上下文，没实例就直说 -->
    <template #head-right>
      <InstanceChip v-if="instanceChip" :instance="instanceChip" />
      <span v-else class="head-warn">{{ t("addResource.noInstance") }}</span>
    </template>

    <div class="res-mode">
      <!-- 下载任务进度条：多任务，完成后短暂停留再自动消失 -->
      <ResourceDownloadBar v-if="resourceStatus?.tasks.length" :status="resourceStatus" />

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
                <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
              </svg>
            </button>
            <div class="pack-main" @click="openVersions(item)">
              <AsyncImage v-if="item.image" class="pack-icon" :src="item.image" alt="" />
              <div v-else class="pack-icon pack-icon-fallback">{{ item.name.slice(0, 1).toUpperCase() }}</div>
              <div class="pack-info">
                <div class="pack-name">
                  <span>{{ item.name }}</span>
                  <span v-if="item.authors.length" class="pack-author">
                    {{item.authors.map((a) => a.name).join(", ")}}
                  </span>
                  <span v-if="projectInstalled(item)" class="pack-badge">{{ t("modpack.installed") }}</span>
                  <span v-else-if="projectRunning(item)" class="pack-badge busy">{{ t("modpack.downloading") }}</span>
                </div>
                <div class="pack-desc">{{ item.summary }}</div>
                <div v-if="item.tag.length" class="pack-tags">
                  <span v-for="tag in item.tag" :key="tag.name" class="pack-tag">
                    <span v-if="tag.svg" class="tag-svg" v-html="svgIcon(tag.svg)"></span>
                    <img v-else-if="tag.logo" class="pack-tag-icon" :src="tag.logo" loading="lazy" alt="" />
                    {{ tag.name }}
                  </span>
                </div>
                <div class="pack-meta">
                  {{ t("modpack.downloads", { n: item.downloadCount.toLocaleString() }) }}
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

      <!-- 版本选择弹窗 -->
      <BaseModal v-if="versionItem" :width="620" below-titlebar @close="closeVersions">
        <div class="ver-head">
          <AsyncImage v-if="versionItem.image" class="ver-icon" :src="versionItem.image" alt="" />
          <div v-else class="ver-icon ver-icon-fallback">
            {{ versionItem.name.slice(0, 1).toUpperCase() }}
          </div>
          <div class="ver-title">
            <div class="ver-name">{{ versionItem.name }}</div>
            <div v-if="versionItem.summary" class="ver-summary">{{ versionItem.summary }}</div>
          </div>
        </div>
        <div class="ver-tools">
          <select v-model="fileVersion" class="field-select sel-file-version" :disabled="filesLoading"
            @change="onFileVersionChange">
            <option value="">{{ t("modpack.allVersions") }}</option>
            <option v-for="v in versions" :key="v" :value="v">{{ v }}</option>
          </select>
          <div v-if="fileMaxPage > 1" class="file-page">
            <button class="page-btn" :disabled="filePage === 0 || filesLoading" @click="turnFilePage(-1)">
              {{ t("modpack.prevPage") }}
            </button>
            <span class="file-page-num">{{ filePage + 1 }} / {{ fileMaxPage }} · {{ t("modpack.totalItems", {
              n:
              fileTotal })
              }}</span>
            <button class="page-btn" :disabled="filePage >= fileMaxPage - 1 || filesLoading" @click="turnFilePage(1)">
              {{ t("modpack.nextPage") }}
            </button>
          </div>
        </div>
        <div class="ver-files" :class="{ 'is-loading-dim': filesLoading }">
          <!-- 首次加载用骨架；翻页时保留旧内容并压暗（结果到了才整体替换） -->
          <template v-if="filesLoading && pageFiles.length === 0">
            <div v-for="n in 4" :key="n" class="sk-row compact">
              <div class="sk-lines">
                <div class="sk sk-line w70"></div>
                <div class="sk sk-line w45"></div>
              </div>
            </div>
          </template>
          <div v-else-if="pageFiles.length === 0" class="empty-tip">{{ t("modpack.noVersions") }}</div>
          <template v-else>
            <div v-for="file in pageFiles" :key="file.source.fid" class="file-row">
              <div class="file-info">
                <span class="file-name">
                  {{ file.name }}
                  <span v-if="fileInstalled(file)" class="pack-badge">{{ t("modpack.installed") }}</span>
                  <span v-else-if="fileRunning(file)" class="pack-badge busy">{{ t("modpack.downloading") }}</span>
                </span>
                <span class="file-meta">
                  {{ t("modpack.fileMeta", {
                    n: file.download.toLocaleString(),
                    date: formatDate(file.time),
                    size: formatSize(file.size),
                  }) }}
                </span>
              </div>
              <button class="install-btn" @click="download(file)">
                {{ t("addResource.download") }}
              </button>
            </div>
          </template>
        </div>
      </BaseModal>

      <!-- 存档选择弹窗（数据包下载前） -->
      <BaseModal v-if="savePickFile" :width="420" below-titlebar @close="savePickFile = null">
        <div class="save-head">{{ t("addResource.chooseSave") }}</div>
        <div class="save-hint">{{ t("addResource.chooseSaveHint") }}</div>
        <div class="save-list">
          <div v-if="savesLoading" class="empty-tip">{{ t("modpack.loading") }}</div>
          <div v-else-if="saves.length === 0" class="empty-tip">{{ t("resource.empty") }}</div>
          <label v-for="save in saves" :key="save.dir" class="save-row" :class="{ on: pickedSave === save.dir }">
            <input v-model="pickedSave" type="radio" name="save-pick" :value="save.dir" />
            <span class="save-name">{{ save.name }}</span>
          </label>
        </div>
        <div class="save-actions">
          <button class="search-btn" :disabled="!pickedSave" @click="confirmSave">
            {{ t("modpack.install") }}
          </button>
        </div>
      </BaseModal>
    </div>
  </WindowFrame>
</template>

<style scoped src="./add-resource-window.css"></style>
