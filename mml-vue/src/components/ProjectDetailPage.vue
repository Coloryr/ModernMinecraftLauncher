<script setup lang="ts">
// 项目详情页：整合包窗口与资源窗口共用
//
// 展示哪个项目由父组件决定（`item`），本组件自己管详情与版本列表的加载、分页、截图预览；
// 下载只发事件 —— 整合包是建实例、资源是放进实例（数据包还要先选存档），那是父组件的事。
//
// 常驻挂载、内部按 `current` 显示，而不是父组件用 v-if 在打开时才创建：那样每次打开都会
// 重置 lib/downloadBadges 里那张「本次会话已下载」累计表，重新打开详情角标就没了。
import { computed, onUnmounted, ref, toRef, watch } from "vue";
import { marked } from "marked";
import DOMPurify from "dompurify";
import AsyncImage from "./ui/AsyncImage.vue";
import AuthorTag from "./ui/AuthorTag.vue";
import ImagePreview from "./ui/ImagePreview.vue";
import { api } from "../lib/api";
import { useDownloadBadges } from "../lib/downloadBadges";
import { formatCount, t, tErr } from "../lib/i18n";
import { svgIcon } from "../lib/modrinthIcon";
import { usePageActive } from "../lib/pageActive";
import { showToast } from "../lib/toast";
import type {
  FileListItemDto,
  FileListDto,
  PicDto,
  ProjectDetailDto,
  ProjectItemDto,
} from "../lib/bindings";

/** 任务条目与快照的最小契约，两个窗口的任务 DTO 都满足 */
type TaskItem = { pid: string; fid: string; done: boolean; failed: boolean; cancelled?: boolean };

const props = defineProps<{
  /** 要展示的项目；null = 收起。组件仍常驻，累计的角标状态不丢 */
  item: ProjectItemDto | null;
  /** 版本列表的游戏版本候选，空串由本组件自己补成「全部」 */
  versions: string[];
  /** 打开时的游戏版本筛选，取自列表页当前选中的版本 */
  initialVersion: string;
  /** 版本行按钮的文案：整合包是「安装」，资源是「下载」 */
  actionLabel: string;
  /** 本窗口的下载任务快照，用来给角标同步「已安装 / 下载中」 */
  status: { tasks: TaskItem[] } | null;
  /** 取项目详情 */
  loadDetail: (pid: string) => Promise<ProjectDetailDto>;
  /** 取版本列表，`version` 为 null 表示不限 */
  loadFiles: (pid: string, page: number, version: string | null) => Promise<FileListDto>;
}>();

const emit = defineEmits<{
  /** 请求关闭详情（Esc），由父组件收起 */
  (e: "close"): void;
  /** 下载某个版本；头部「下载最新版」也走这条 */
  (e: "download", file: FileListItemDto): void;
}>();

/** 后端版本列表每页 50 条，与 CurseForge 后端分页一致 */
const FILE_PAGE_SIZE = 50;

/** 当前展示的项目；父组件把 `item` 置 null 后仍留着，退场动画那一下还要用 */
const current = ref<ProjectItemDto | null>(null);

const detail = ref<ProjectDetailDto | null>(null);
const detailLoading = ref(false);
const detailError = ref("");
/** CurseForge：后端翻页，当前页结果；Modrinth：一次拿全部，本地切片 */
const files = ref<FileListItemDto[]>([]);
const allFiles = ref<FileListItemDto[]>([]);
const filesLoading = ref(false);
/** 版本列表加载失败的原因：失败时列表区显示错误 + 重试，而不是当成"没有版本" */
const filesError = ref("");
const filePage = ref(0);
const fileTotal = ref(0);
/** 最新版本：头部「下载」按钮用，取 page 0 的第一条 */
const latestFile = ref<FileListItemDto | null>(null);
/** 版本列表的游戏版本筛选，空串 = 全部 */
const fileVersion = ref("");

/** 详情代次：连续开关时丢弃旧一发的返回 */
let detailSeq = 0;
/** 版本列表代次：换筛选 / 换页 / 换项目时丢弃旧一发的返回 */
let fileSeq = 0;

const isModrinth = computed(() => current.value?.source.source === "modrinth");

/**
 * 详情页头部显示的作者
 *
 * 优先用列表项带来的那份（与卡片上一致）；列表项没有时退回**详情接口**的那份。
 * 从「资源收藏」进来的条目只有名字 / 图标 / 网址，作者是空的，而详情接口一直带着作者 ——
 * 不回退的话详情页一个作者都不显示。空名字的条目直接丢掉，别画出空胶囊。
 */
const authorList = computed(() => {
  /** 空名字的条目直接丢掉，别画出空胶囊 */
  const pick = (list: PicDto[]) => list.filter((item) => !!item.name);
  const fromItem = pick(current.value?.authors ?? []);
  return fromItem.length ? fromItem : pick(detail.value?.authors ?? []);
});

const pageFiles = computed(() =>
  isModrinth.value
    ? allFiles.value.slice(filePage.value * FILE_PAGE_SIZE, (filePage.value + 1) * FILE_PAGE_SIZE)
    : files.value,
);

const fileMaxPage = computed(() => Math.max(1, Math.ceil(fileTotal.value / FILE_PAGE_SIZE)));

const { projectInstalled, projectRunning, fileInstalled, fileRunning } = useDownloadBadges(
  toRef(props, "status"),
);

/** 所属页面是否在前台；单窗口模式下被 KeepAlive 切走时，这份 Teleport 浮层不能留在别的页面上 */
const active = usePageActive();

/** 打开项目：详情与版本列表并行加载，版本筛选沿用列表页选中的游戏版本 */
async function open(item: ProjectItemDto) {
  const mine = ++detailSeq;
  const pid = item.source.pid;

  detail.value = null;
  detailError.value = "";
  files.value = [];
  allFiles.value = [];
  latestFile.value = null;
  filesError.value = "";
  filePage.value = 0;
  fileTotal.value = 0;
  fileVersion.value = props.initialVersion;
  detailLoading.value = true;

  void fetchFiles(pid, 0);

  try {
    const res = await props.loadDetail(pid);
    if (mine !== detailSeq) return;
    detail.value = res;
  } catch (e) {
    if (mine !== detailSeq) return;
    detailError.value = tErr(e);
  } finally {
    if (mine === detailSeq) detailLoading.value = false;
  }
}

async function fetchFiles(pid: string, page: number) {
  const mine = ++fileSeq;
  filesLoading.value = true;
  filesError.value = "";
  try {
    const res = await props.loadFiles(pid, page, fileVersion.value || null);
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
  if (current.value) {
    fetchFiles(current.value.source.pid, filePage.value);
  }
}

/** 版本筛选变化：回到第一页重新拉取 */
function onFileVersionChange() {
  filePage.value = 0;
  if (current.value) {
    fetchFiles(current.value.source.pid, 0);
  }
}

/** 版本列表翻页：Modrinth 本地切片，CurseForge 向后端取页 */
function turnFilePage(delta: number) {
  const next = filePage.value + delta;
  if (next < 0 || next >= fileMaxPage.value) return;
  filePage.value = next;
  // 翻页先把版本列表拉回它的开头：停在上一页的滚动位置就看不到新一页的头几条
  scrollToVersions();
  if (isModrinth.value || !current.value) return;

  fetchFiles(current.value.source.pid, next);
}

/** 下载最新版本（page 0 的第一条） */
function downloadLatest() {
  if (latestFile.value) {
    emit("download", latestFile.value);
  }
}

/** 收藏 / 取消收藏；`item` 与列表项是同一个对象，改它即改列表上的星标 */
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

/** Modrinth 正文（markdown）渲染成 HTML；CurseForge 没有 body，显示简介 */
const bodyHtml = computed(() => {
  const body = detail.value?.body;
  if (!body) return "";
  return DOMPurify.sanitize(marked.parse(body, { async: false }));
});

/**
 * 截图放大预览的图片地址，空 = 关闭
 *
 * 缩放 / 平移 / 点空白关闭都在共用的 `ImagePreview` 里；这里只留"开哪张 / 关掉"，
 * 以及 Esc 的分层处理：先关预览、再关详情页，见 [`onKey`]。
 */
const preview = ref("");

function openPreview(url: string) {
  preview.value = url;
}

function closePreview() {
  preview.value = "";
}

/** Esc：先关截图预览，再关详情页 */
function onKey(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  if (preview.value) {
    closePreview();
    return;
  }
  emit("close");
}

watch(
  [() => props.item, active],
  ([item, on]) => {
    if (item && on) window.addEventListener("keydown", onKey);
    else window.removeEventListener("keydown", onKey);
  },
  { immediate: true },
);

onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
});

watch(
  () => props.item,
  (item) => {
    // 截图预览在详情之外（z-index 更高）：换项目或收起时先收掉，免得留在别的界面上
    preview.value = "";
    if (!item) return;
    current.value = item;
    void open(item);
  },
  { immediate: true },
);

function formatDate(date: string): string {
  return date ? date.slice(0, 10) : "";
}

function formatSize(size: number): string {
  if (!size) return t("modpack.unknownSize");
  if (size >= 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(size / 1024))} KB`;
}

/** 版本列表锚点：头部「版本列表」按钮滚动到这里 */
const versionsRef = ref<HTMLElement | null>(null);

function scrollToVersions() {
  versionsRef.value?.scrollIntoView({ behavior: "smooth", block: "start" });
}
</script>

<template>
  <!--
    截图放大预览：滚轮缩放、拖动平移、点空白处或 Esc 关闭。
    UI 与行为都在共用的 `ImagePreview` 里，资源窗口的截图预览用的是同一份。
    Esc 的"先关预览、再关详情页"层次由本组件处理，见 onKey
  -->
  <ImagePreview v-if="preview" :src="preview" @close="closePreview" />

  <Teleport to="body">
    <transition name="detail-page">
      <!-- 显示与否跟父组件的 item 走：收起时立刻进退场动画。
           退场那一下的内容仍取 current —— 它留着最后一次打开的项目，
           所以动画期间画面不会变空；只按 current 判断的话详情页永远关不掉 -->
      <div v-if="active && props.item && current" class="detail-page">
        <!-- 头部固定、内容滚动；没有外层卡片，直接用窗口底色 -->
        <div class="detail-panel">
          <div class="detail-head">
            <AsyncImage v-if="current.image" class="detail-icon" :src="current.image" alt="" />
            <div v-else class="detail-icon detail-icon-fallback">
              {{ current.name.slice(0, 1).toUpperCase() }}
            </div>
            <div class="detail-title">
              <div class="detail-name">
                {{ current.name }}
                <span v-if="projectInstalled(current)" class="pack-badge">{{ t("modpack.installed") }}</span>
                <span v-else-if="projectRunning(current)" class="pack-badge busy">{{ t("modpack.downloading")
                }}</span>
              </div>
              <div class="detail-sub">
                <template v-if="authorList.length">
                  <AuthorTag :authors="authorList" :max="4" />
                  <span> · </span>
                </template>
                <span>{{ t("modpack.downloads", { n: formatCount(current.downloadCount) }) }}</span>
                <span v-if="current.date"> · {{ formatDate(current.date) }}</span>
              </div>
              <div v-if="detail?.tag.length || current.tag.length" class="pack-tags detail-tags">
                <span v-for="tag in detail?.tag.length ? detail.tag : current.tag" :key="tag.name" class="pack-tag">
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
            <button class="detail-star" :class="{ on: current.isStar }"
              v-tip="current.isStar ? t('modpack.unstar') : t('modpack.star')" @click="toggleStar(current)">
              <svg viewBox="0 0 24 24" width="20" height="20" stroke-width="2" stroke-linejoin="round">
                <path
                  d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
              </svg>
            </button>
            <button v-if="current.url" class="detail-link" v-tip="t('modpack.openPage')"
              @click="api.openUrl(current.url)">
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
                    <AsyncImage v-for="shot in detail.screenshots" :key="shot.logo" class="detail-shot" :src="shot.logo"
                      v-tip="shot.name || shot.description" @click="openPreview(shot.logo)" />
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
                        {
                          n:
                            fileTotal
                        }) }}</span>
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
                            n: formatCount(file.download),
                            date: formatDate(file.time),
                            size: formatSize(file.size),
                          }) }}
                        </span>
                      </div>
                      <button class="install-btn" @click="emit('download', file)">
                        {{ props.actionLabel }}
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
</template>

<style scoped src="../styles/parts/project-detail.css"></style>
