<script setup lang="ts">
// 整合包模式：CurseForge / Modrinth 在线搜索 + 选版本安装
// 实例名取自整合包元数据（安装后端自动处理），分组沿用窗口顶部的分组输入框
import { computed, nextTick, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from "vue";
import { marked } from "marked";
import DOMPurify from "dompurify";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import AsyncImage from "../../components/ui/AsyncImage.vue";
import ImagePreview from "../../components/ui/ImagePreview.vue";
import { api } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { projectParamToItem, targetProject } from "../windowManager";
import type { FileListItemDto, GroupDto, ModPackStatusDto, ModPackTaskDto, ProjectDetailDto, ProjectItemDto } from "../../lib/bindings";

const props = defineProps<{
  /** 安装到的分组名；空 = 默认分组，提交时由上层转成分组 uuid */
  group: string;
  /** 已有分组候选（uuid + 名字；下拉显示名字） */
  groups: GroupDto[];
  /** 安装任务状态，用来给列表同步「已安装 / 安装中」角标 */
  status: ModPackStatusDto | null;
}>();

const emit = defineEmits<{
  /** 安装指定版本：payload 为下载源、项目 ID、文件 ID 与实例名 */
  (e: "install", payload: { source: string; projectId: string; fileId: string; fileName: string }): void;
  /** 安装到的分组名变化 */
  (e: "update:group", value: string): void;
  /** 详情展开 / 收起；返回键在标题栏上，需要知道当前有没有详情 */
  (e: "detail", open: boolean): void;
}>();

/** 后端列表每页 20 个项目 */
const PAGE_SIZE = 20;

const SOURCE_LABELS: Record<string, string> = {
  curseforge: "modpack.curseforge",
  modrinth: "modpack.modrinth",
};

/** 下载源（取值是后端线串，显示的标签走 i18n） */
const sources = ref<Array<{ value: string; label: string }>>([]);
const source = ref("");

/** 排序方式：取值就是后端枚举线串，原样回传 */
const sorts = ref<string[]>([]);
const sort = ref("");

/** 游戏版本，"" = 全部 */
const versions = ref<string[]>([]);
const version = ref("");

/** 分类，"" = 全部。键 = 传给后端的分类值，值 = 显示名 */
const categories = ref<Array<{ value: string; label: string }>>([]);
const category = ref("");

/** 搜索文本 */
const filter = ref("");

/** 分组组合框：点击输入框即展开已有分组（与添加实例窗口一致） */
const groupOpen = ref(false);
const groupQuery = computed(() =>
  props.groups.filter((g) => g.name.toLowerCase().includes(props.group.trim().toLowerCase())),
);

// 离开本窗口时清掉搜索词与分组下拉：来源 / 版本 / 排序 / 分类等筛选留着，那是浏览上下文。
// 单窗口模式下这一页被 KeepAlive 缓存，切走不销毁，不清的话再打开时输入框里
// 还压着上次搜的词，用户早忘了自己搜过什么，只会觉得界面不对劲。
onDeactivated(() => {
  filter.value = "";
  groupOpen.value = false;
});

function onGroupInput(e: Event) {
  emit("update:group", (e.target as HTMLInputElement).value);
  groupOpen.value = true;
}

function pickGroup(name: string) {
  emit("update:group", name);
  groupOpen.value = false;
}

const items = ref<ProjectItemDto[]>([]);
const total = ref(0);
const page = ref(0);
const searching = ref(false);
const error = ref("");

/** 详情页：当前项目 + 详情数据 + 版本列表 */
const detailItem = ref<ProjectItemDto | null>(null);
const detail = ref<ProjectDetailDto | null>(null);
const detailLoading = ref(false);
const detailError = ref("");
/** CurseForge：后端翻页，当前页结果；Modrinth：一次拿全部，本地切片 */
const files = ref<FileListItemDto[]>([]);
const allFiles = ref<FileListItemDto[]>([]);
const filesLoading = ref(false);
/** 版本列表加载失败的原因：失败时列表区显示错误 + 重试，而不是当成"没有版本" */
const filesError = ref("");
/** 版本列表页码（0 起）与总数 */
const filePage = ref(0);
const fileTotal = ref(0);
/** 最新版本：详情页「下载」按钮用，取 page 0 的第一条 */
const latestFile = ref<FileListItemDto | null>(null);
/** 详情页版本列表的游戏版本筛选，空串 = 全部；打开详情时沿用列表页的筛选 */
const fileVersion = ref("");

/** 版本列表每页条数（与 CurseForge 后端分页一致） */
const FILE_PAGE_SIZE = 50;

const isModrinth = computed(() => source.value === "modrinth");

/** 当前页显示的版本列表 */
const pageFiles = computed(() =>
  isModrinth.value
    ? allFiles.value.slice(filePage.value * FILE_PAGE_SIZE, (filePage.value + 1) * FILE_PAGE_SIZE)
    : files.value,
);

const fileMaxPage = computed(() => Math.max(1, Math.ceil(fileTotal.value / FILE_PAGE_SIZE)));

const maxPage = computed(() => Math.max(0, Math.ceil(total.value / PAGE_SIZE) - 1));

/** 拉取下载源列表并选中第一个；后续筛选数据由 watch(source) 加载 */
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
  source.value = sources.value[0].value;
}

/**
 * 重载代次：换下载源会连着跑几发 loadSource（比如收藏跳转同时改源与类型），
 * 旧的一发**后回来**会把新的排序表覆盖掉 —— 于是后续搜索带着上一个源的排序名
 * 发出去，后端报 err.sortTypeNotFound。每发记下自己的编号，回来时不是最新就丢掉。
 */
let sourceSeq = 0;

/** 切换下载源：清空搜索词与结果，重新拉排序 / 版本 / 分类，然后搜第一页 */
async function loadSource() {
  const mine = ++sourceSeq;
  items.value = [];
  total.value = 0;
  page.value = 0;
  error.value = "";
  detailItem.value = null;
  detail.value = null;
  files.value = [];
  allFiles.value = [];
  latestFile.value = null;
  fileVersion.value = "";
  filter.value = "";
  version.value = "";
  category.value = "";
  sort.value = "";
  sorts.value = [];
  versions.value = [];
  categories.value = [];

  searching.value = true;
  try {
    const [sortList, versionList, categoryMap] = await Promise.all([
      api.getModpackSorts(source.value),
      api.getModpackVersions(source.value),
      api.getModpackCategories(source.value),
    ]);

    // 已经换源了：这一发的排序 / 版本 / 分类都是上一个源的，丢
    if (mine !== sourceSeq) return;

    sorts.value = sortList;
    sort.value = sortList[0] ?? "";
    // 版本列表里后端已经插了一个空串表示“全部”，这里统一由前端的选项提供
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

/** 搜索代次：新的一发开始后，旧一发的返回（含被后端取消的）全部丢弃 */
let searchSeq = 0;

/** 搜索期间置 `searching`（弹窗锁住窗口），成败都清掉 */
async function search() {
  // 排序列表还没拉回来时别发请求：换下载源时它是**异步**拉回来的
  // （loadSource 里 await 若干请求），这中间发出去必然带不上合法排序
  // —— 空串后端也不认（err.sortTypeNotFound）。那次 loadSource 拉完会自己再搜一次。
  if (sorts.value.length === 0) {
    return;
  }
  if (!sorts.value.includes(sort.value)) {
    sort.value = sorts.value[0];
  }

  const mine = ++searchSeq;
  searching.value = true;
  try {
    const res = await api.searchModpacks(
      source.value,
      page.value,
      sort.value,
      category.value || null,
      filter.value.trim() || null,
      version.value || null,
    );
    if (mine !== searchSeq) return;
    items.value = res.items;
    total.value = res.count;
    error.value = "";
  } catch (e) {
    // 被更新的一发顶掉、或窗口关闭时后端取消，都不算失败，不弹错
    if (mine !== searchSeq || String(e) === "err.cancelled") return;
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
    // 只有最新一发负责收掉锁定弹窗
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

/** 详情代次：连续开关弹窗时丢弃旧一发的返回 */
let detailSeq = 0;

/** 版本列表代次：换筛选 / 换页时丢弃旧一发的返回 */
let fileSeq = 0;

/** 拉取版本列表：按 `fileVersion` 过滤并取指定页；Modrinth 一次拿全部本地切片，CurseForge 走后端分页 */
async function loadFiles(pid: string, page: number) {
  const mine = ++fileSeq;
  filesLoading.value = true;
  filesError.value = "";
  try {
    const res = await api.getModpackFiles(source.value, pid, page, fileVersion.value || null);
    if (mine !== fileSeq) return;
    fileTotal.value = res.count || res.list.length;
    // Modrinth 一次返回全部版本，本地切片翻页；CurseForge 走后端分页
    if (isModrinth.value) {
      allFiles.value = res.list;
    } else {
      files.value = res.list;
    }
    if (page === 0) {
      latestFile.value = res.list[0] ?? null;
    }
  } catch (e) {
    // 旧一发的失败、或窗口关闭时后端取消，都不算失败
    if (mine !== fileSeq || String(e) === "err.cancelled") return;
    // 没有这个 catch 时，异常会变成未处理的 rejection，界面只剩"没有可用版本"
    files.value = [];
    allFiles.value = [];
    fileTotal.value = 0;
    latestFile.value = null;
    filesError.value = tErr(e);
  } finally {
    if (mine === fileSeq) filesLoading.value = false;
  }
}

/** 版本列表重试（失败提示里的按钮） */
function reloadFiles() {
  if (detailItem.value) {
    loadFiles(detailItem.value.source.pid, filePage.value);
  }
}

/** 双击项目：打开详情页，正文/截图与版本列表并行加载 */
async function openDetail(item: ProjectItemDto) {
  const mine = ++detailSeq;
  const pid = item.source.pid;

  detailItem.value = item;
  detail.value = null;
  detailError.value = "";
  files.value = [];
  allFiles.value = [];
  latestFile.value = null;
  filePage.value = 0;
  fileTotal.value = 0;
  emit("detail", true);
  // 版本列表的筛选沿用列表页选中的游戏版本
  fileVersion.value = version.value;
  detailLoading.value = true;

  loadFiles(pid, 0);

  try {
    const res = await api.getModpackDetail(source.value, pid);
    if (mine !== detailSeq) return;
    detail.value = res;
  } catch (e) {
    if (mine !== detailSeq) return;
    detailError.value = tErr(e);
  } finally {
    if (mine === detailSeq) detailLoading.value = false;
  }
}

/** 版本列表筛选变化：回到第一页重新拉取 */
function onFileVersionChange() {
  filePage.value = 0;
  if (detailItem.value) {
    loadFiles(detailItem.value.source.pid, 0);
  }
}

/** 版本列表翻页：Modrinth 本地切片，CurseForge 向后端取页 */
function turnFilePage(delta: number) {
  const next = filePage.value + delta;
  if (next < 0 || next >= fileMaxPage.value) return;
  filePage.value = next;
  if (isModrinth.value || !detailItem.value) return;

  loadFiles(detailItem.value.source.pid, next);
}

/** 下载最新版本（page 0 的第一条） */
function downloadLatest() {
  if (detailItem.value && latestFile.value) {
    install(detailItem.value, latestFile.value);
  }
}

/** 版本列表锚点：头部「版本列表」按钮滚动到这里 */
const versionsRef = ref<HTMLElement | null>(null);

function scrollToVersions() {
  versionsRef.value?.scrollIntoView({ behavior: "smooth", block: "start" });
}

/** 收藏 / 取消收藏（列表星标与详情页星标共用） */
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

function closeDetail() {
  detailSeq++;
  detailItem.value = null;
  detail.value = null;
  files.value = [];
  allFiles.value = [];
  latestFile.value = null;
  detailError.value = "";
  emit("detail", false);
}

// 标题栏上的返回键由上层（AddModpackWindow）持有，通过 ref 调这里
defineExpose({ closeDetail });

function install(item: ProjectItemDto, file: FileListItemDto) {
  emit("install", {
    source: item.source.source,
    projectId: item.source.pid,
    fileId: file.source.fid,
    fileName: item.name,
  });
  closeDetail();
}

/** Modrinth 正文（markdown）渲染成 HTML；CurseForge 没有 body，显示简介 */
const bodyHtml = computed(() => {
  const body = detail.value?.body;
  if (!body) return "";
  return DOMPurify.sanitize(marked.parse(body, { async: false }));
});

/**
 * Modrinth 分类图标：icon 字段是内联 svg 源码（stroke=currentColor），消毒后内联渲染
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

/**
 * 截图放大预览（当前预览的图片地址，空 = 关闭）
 *
 * 缩放 / 平移 / 点空白关闭都在共用的 `ImagePreview` 里；这里只留"开哪张 / 关掉"，
 * 以及 Esc 的分层处理（先关预览、再关详情页，见 [`onDetailKey`]）。
 */
const preview = ref("");

function openPreview(url: string) {
  preview.value = url;
}

function closePreview() {
  preview.value = "";
}

/** Esc：先关截图预览，再关详情页 */
function onDetailKey(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  if (preview.value) {
    closePreview();
    return;
  }
  closeDetail();
}

watch(detailItem, (val) => {
  if (val) window.addEventListener("keydown", onDetailKey);
  else window.removeEventListener("keydown", onDetailKey);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onDetailKey);
});

function formatDate(date: string): string {
  return date ? date.slice(0, 10) : "";
}

/**
 * 本次会话里见过的「安装完成」标记（只增不减）
 *
 * 任务表的条目会被清掉（用户点「清除已完成」），但实例确实已经建好了——
 * 「已安装」角标不能跟着任务一起消失。只往集合里加，不做任何复位。
 */
const donePids = ref(new Set<string>());
const doneFileKeys = ref(new Set<string>());

watch(
  () => props.status,
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
 * 由 `props.status` 直接**派生**，不再往列表项里写回 `download` / `downloadNow`：
 * 写回是单向的（只置 true 不复位），任务取消、失败或清除之后角标会一直挂着。
 * 「已安装」取实例表来源（`item.download` / `file.isDownload`）与上面那张累计表的「或」。
 */
const taskState = computed(() => {
  const running = (task: ModPackTaskDto) => !task.done && !task.failed && !task.cancelled;
  const tasks = props.status?.tasks ?? [];
  return {
    runningProjects: new Set(tasks.filter(running).map((task) => task.pid)),
    runningFiles: new Set(tasks.filter(running).map((task) => `${task.pid}|${task.fid}`)),
  };
});

function fileKey(file: FileListItemDto): string {
  return `${file.source.pid}|${file.source.fid}`;
}

/** 项目级「已安装」（实例表已装 + 本次会话装完） */
function projectInstalled(item: ProjectItemDto): boolean {
  return item.download || donePids.value.has(item.source.pid);
}

/** 项目级「下载中」：只看任务表——DTO 里那份是取列表时的快照，用它角标会粘住 */
function projectRunning(item: ProjectItemDto): boolean {
  return taskState.value.runningProjects.has(item.source.pid);
}

/** 版本级「已安装」 */
function fileInstalled(file: FileListItemDto): boolean {
  return file.isDownload || doneFileKeys.value.has(fileKey(file));
}

/** 版本级「下载中」 */
function fileRunning(file: FileListItemDto): boolean {
  return taskState.value.runningFiles.has(fileKey(file));
}

function formatSize(size: number): string {
  if (!size) return t("modpack.unknownSize");
  if (size >= 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(size / 1024))} KB`;
}

onMounted(async () => {
  await loadSources();
  consumeTargetProject();
});

/**
 * 收藏窗口点「下载」跳进来的整合包：进页面就直接打开它的详情（版本列表在里面）
 *
 * 与下载资源窗口同理 —— 本页在单窗口模式下也被 KeepAlive 缓存，先把上一次留下的
 * 视图状态清掉再打开新项目（`closeDetail()` 同时把上层的"返回"状态复位），
 * 改完 source 会触发列表刷新，所以等一轮再开详情，免得被那批刷新盖掉。
 */
async function consumeTargetProject() {
  const p = targetProject();
  if (!p || p.fileType !== "modpack") {
    return;
  }
  closeDetail();
  filter.value = "";
  // 换源会让排序表作废：**同步**清掉（不能等 watch 里那次异步重载），
  // 否则中间那几发搜索会带着上一个源的排序名发出去（后端不认 → 报未知排序）
  sorts.value = [];
  sort.value = "";
  if (p.source) {
    source.value = p.source;
  }

  await nextTick();

  // 精简条目只够定位项目，下载次数 / 更新时间 / 收藏状态都是空的，所以现取一次真的；
  // 取不到（离线等）就退回精简条目，至少详情能打开
  let item = projectParamToItem(p);
  try {
    const real = await api.collectProjectItem(p.source, p.pid, p.fileType);
    if (real) {
      item = real;
    }
  } catch {
    // 用精简条目兜底
  }

  void openDetail(item);
}

onActivated(consumeTargetProject);

watch(source, loadSource);
</script>

<template>
  <div class="modpack-mode">
    <!-- 过滤条件 + 搜索：同一块白底卡片。加载期间整条锁死，避免半途改条件与在途请求打架 -->
    <div class="modpack-top">
      <div class="modpack-filters">
        <SegmentedTabs v-model="source" :options="sources" :disabled="searching" />
        <select v-model="version" class="field-select sel-version" :disabled="searching" @change="submitSearch">
          <option value="">{{ t("modpack.allVersions") }}</option>
          <option v-for="v in versions" :key="v" :value="v">{{ v }}</option>
        </select>
        <select v-model="sort" class="field-select sel-sort" :disabled="searching" @change="submitSearch">
          <option v-for="s in sorts" :key="s" :value="s">{{ t(`modpack.sort.${s}`) }}</option>
        </select>
        <select v-model="category" class="field-select sel-category" :disabled="searching" @change="submitSearch">
          <option value="">{{ t("modpack.allCategories") }}</option>
          <option v-for="c in categories" :key="c.value" :value="c.value">{{ c.label }}</option>
        </select>
        <div class="group-combo sel-group">
          <input :value="props.group" class="field-input" :placeholder="t('modpack.groupPlaceholder')"
            spellcheck="false" :disabled="searching" @focus="groupOpen = true" @input="onGroupInput"
            @blur="groupOpen = false" />
          <div v-if="groupOpen" class="group-drop">
            <button v-for="g in groupQuery" :key="g.uuid" class="group-opt" @mousedown.prevent
              @click="pickGroup(g.name)">
              {{ g.name }}
            </button>
            <div v-if="!groupQuery.length" class="empty-tip">{{ t("add.groupNone") }}</div>
          </div>
        </div>
      </div>

      <!-- 搜索 -->
      <div class="modpack-search">
        <input v-model="filter" class="field-input search-input" :placeholder="t('modpack.searchHint')"
          spellcheck="false" :disabled="searching" @keydown.enter="submitSearch" />
        <button class="search-btn" :disabled="searching" @click="submitSearch">
          <span v-if="searching" class="btn-spinner"></span>
          {{ t("modpack.search") }}
        </button>
      </div>
    </div>

    <!-- 列表加载指示：翻页 / 换筛选只在结果区上方走一条细进度，不再全窗口遮罩 -->
    <div class="list-bar" :class="{ on: searching }" aria-hidden="true"><span /></div>

    <!-- 结果列表：加载状态内联在本区，旧内容保留并压暗 -->
    <div class="modpack-list" :class="{ 'is-loading-dim': searching }">
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
      <div v-else-if="items.length === 0" class="empty-tip">{{ t("modpack.empty") }}</div>
      <template v-else>
        <div v-for="item in items" :key="item.source.pid" class="pack-item">
          <!-- 收藏星标（右上角）：点击收藏 / 取消收藏 -->
          <button class="pack-star" :class="{ on: item.isStar }"
            v-tip="item.isStar ? t('modpack.unstar') : t('modpack.star')" @click.stop="toggleStar(item)">
            <svg viewBox="0 0 24 24" width="17" height="17" stroke-width="2" stroke-linejoin="round">
              <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
            </svg>
          </button>
          <div class="pack-main" @click="openDetail(item)">
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
    <div v-if="maxPage > 0" class="modpack-page">
      <button class="page-btn" :disabled="page === 0 || searching" @click="turnPage(-1)">
        {{ t("modpack.prevPage") }}
      </button>
      <span class="page-num">{{ page + 1 }} / {{ maxPage + 1 }} · {{ t("modpack.totalItems", { n: total }) }}</span>
      <button class="page-btn" :disabled="page >= maxPage || searching" @click="turnPage(1)">
        {{ t("modpack.nextPage") }}
      </button>
    </div>

    <!--
      截图放大预览：滚轮缩放、拖动平移、点空白处或 Esc 关闭。
      UI 与行为都在共用的 `ImagePreview` 里，资源窗口的截图预览用的是同一份。
      Esc 仍由本页处理（见 onDetailKey）：这里还有"先关预览、再关详情页"的层次
    -->
    <ImagePreview v-if="preview" :src="preview" @close="closePreview" />

    <!-- 项目详情：单击列表项打开，铺满标题栏以下的整个窗口，返回键在标题栏上 -->
    <Teleport to="body">
      <transition name="detail-page">
        <div v-if="detailItem" class="detail-page">
          <!-- 头部固定、内容滚动；没有外层卡片，直接用窗口底色 -->
          <div class="detail-panel">
            <div class="detail-head">
              <AsyncImage v-if="detailItem.image" class="detail-icon" :src="detailItem.image" alt="" />
              <div v-else class="detail-icon detail-icon-fallback">
                {{ detailItem.name.slice(0, 1).toUpperCase() }}
              </div>
              <div class="detail-title">
                <div class="detail-name">
                  {{ detailItem.name }}
                  <span v-if="projectInstalled(detailItem)" class="pack-badge">{{ t("modpack.installed") }}</span>
                  <span v-else-if="projectRunning(detailItem)" class="pack-badge busy">{{ t("modpack.downloading")
                    }}</span>
                </div>
                <div class="detail-sub">
                  <span v-if="detailItem.authors.length">{{detailItem.authors.map((a) => a.name).join(", ")}}</span>
                  <span> · {{ t("modpack.downloads", { n: detailItem.downloadCount.toLocaleString() }) }}</span>
                  <span v-if="detailItem.date"> · {{ formatDate(detailItem.date) }}</span>
                </div>
                <div v-if="detail?.tag.length || detailItem.tag.length" class="pack-tags detail-tags">
                  <span v-for="tag in detail?.tag.length ? detail.tag : detailItem.tag" :key="tag.name"
                    class="pack-tag">
                    <span v-if="tag.svg" class="tag-svg" v-html="svgIcon(tag.svg)"></span>
                    <img v-else-if="tag.logo" class="pack-tag-icon" :src="tag.logo" loading="lazy" alt="" />
                    {{ tag.name }}
                  </span>
                </div>
              </div>
              <button class="detail-jump" @click="scrollToVersions">
                {{ t("modpack.versions") }}
              </button>
              <button class="detail-download" :disabled="filesLoading || !latestFile" @click="downloadLatest">
                {{ t("modpack.download") }}
              </button>
              <button class="detail-star" :class="{ on: detailItem.isStar }"
                v-tip="detailItem.isStar ? t('modpack.unstar') : t('modpack.star')" @click="toggleStar(detailItem)">
                <svg viewBox="0 0 24 24" width="20" height="20" stroke-width="2" stroke-linejoin="round">
                  <path
                    d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
                </svg>
              </button>
              <button v-if="detailItem.url" class="detail-link" v-tip="t('modpack.openPage')"
                @click="api.openUrl(detailItem.url)">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"
                  stroke-linecap="round" stroke-linejoin="round">
                  <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                  <polyline points="15 3 21 3 21 9" />
                  <line x1="10" y1="14" x2="21" y2="3" />
                </svg>
              </button>
            </div>

            <div class="detail-scroll">
              <!-- 详情正文的加载 / 失败只在这里提示一次；版本列表是并行的另一份加载，自己管自己 -->
              <div v-if="detailLoading" class="empty-tip">{{ t("modpack.loadingDetail") }}</div>
              <div v-else-if="detailError" class="empty-tip">{{ detailError }}</div>
              <!-- 简介 / 截图 / 版本列表合并为一块大卡片，分区间距隔开，避免多框割裂 -->
              <div class="detail-card">
                <template v-if="detail">
                  <!-- Modrinth：正文；CurseForge：简介。标题都用「项目简介」 -->
                  <div class="detail-section">{{ t("modpack.summary") }}</div>
                  <article v-if="bodyHtml" class="detail-md" v-html="bodyHtml"></article>
                  <div v-else class="detail-summary">{{ detail.summary }}</div>

                  <template v-if="detail.screenshots.length">
                    <div class="detail-section detail-block">{{ t("modpack.screenshots") }}</div>
                    <div class="detail-shots">
                      <AsyncImage v-for="shot in detail.screenshots" :key="shot.logo" class="detail-shot"
                        :src="shot.logo" v-tip="shot.name || shot.description" @click="openPreview(shot.logo)" />
                    </div>
                  </template>
                </template>

                <div ref="versionsRef" class="detail-block versions-anchor">
                  <div class="detail-section detail-versions-head">
                    <span>{{ t("modpack.versions") }}</span>
                    <div class="version-tools">
                      <select v-model="fileVersion" class="field-select sel-file-version" :disabled="filesLoading"
                        @change="onFileVersionChange">
                        <option value="">{{ t("modpack.allVersions") }}</option>
                        <option v-for="v in versions" :key="v" :value="v">{{ v }}</option>
                      </select>
                      <div v-if="fileMaxPage > 1" class="file-page">
                        <button class="page-btn" :disabled="filePage === 0 || filesLoading" @click="turnFilePage(-1)">
                          {{ t("modpack.prevPage") }}
                        </button>
                        <span class="file-page-num">{{ filePage + 1 }} / {{ fileMaxPage }} · {{ t("modpack.totalItems",
                          { n:
                          fileTotal }) }}</span>
                        <button class="page-btn" :disabled="filePage >= fileMaxPage - 1 || filesLoading"
                          @click="turnFilePage(1)">
                          {{ t("modpack.nextPage") }}
                        </button>
                      </div>
                    </div>
                  </div>
                  <div class="detail-files" :class="{ 'is-loading-dim': filesLoading }">
                    <!-- 首次加载用骨架；翻页时保留旧内容并压暗（结果到了才整体替换） -->
                    <template v-if="filesLoading && pageFiles.length === 0">
                      <div v-for="n in 5" :key="n" class="sk-row compact">
                        <div class="sk-lines">
                          <div class="sk sk-line w70"></div>
                          <div class="sk sk-line w45"></div>
                        </div>
                      </div>
                    </template>
                    <!-- 失败与"没有版本"必须分开：前者可重试，后者是空结果 -->
                    <div v-else-if="filesError" class="empty-tip">
                      {{ t("modpack.versionFail") }}
                      <button class="retry-btn" @click="reloadFiles">{{ t("modpack.retry") }}</button>
                    </div>
                    <div v-else-if="pageFiles.length === 0" class="empty-tip">{{ t("modpack.noVersions") }}</div>
                    <template v-else>
                      <div v-for="file in pageFiles" :key="file.source.fid" class="file-row">
                        <div class="file-info">
                          <span class="file-name">
                            {{ file.name }}
                            <span v-if="fileInstalled(file)" class="pack-badge">{{ t("modpack.installed") }}</span>
                            <span v-else-if="fileRunning(file)" class="pack-badge busy">{{ t("modpack.downloading")
                              }}</span>
                          </span>
                          <span class="file-meta">
                            {{ t("modpack.fileMeta", {
                              n: file.download.toLocaleString(),
                              date: formatDate(file.time),
                              size: formatSize(file.size),
                            }) }}
                          </span>
                        </div>
                        <button class="install-btn" @click="install(detailItem, file)">
                          {{ t("modpack.install") }}
                        </button>
                      </div>
                    </template>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </transition>
    </Teleport>
  </div>
</template>

<style scoped src="./modpack-mode.css"></style>
<style scoped src="../../styles/parts/group-combo.css"></style>
