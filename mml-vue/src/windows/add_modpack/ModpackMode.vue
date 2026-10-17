<script setup lang="ts">
// 整合包模式：CurseForge / Modrinth 在线搜索 + 选版本安装
// 实例名取自整合包元数据（安装后端自动处理），分组沿用窗口顶部的分组输入框
// 项目详情是共用的 components/ProjectDetailPage.vue（与资源窗口同一份）
import { computed, nextTick, onActivated, onDeactivated, onMounted, ref, toRef, watch } from "vue";
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
import { projectParamToItem, targetProject } from "../windowManager";
import type { FileListItemDto, GroupDto, ModPackStatusDto, ProjectItemDto } from "../../lib/bindings";

const props = defineProps<{
  /** 安装到的分组名；空 = 默认分组，提交时由上层转成分组 uuid */
  group: string;
  /** 已有分组候选，uuid + 名字；下拉显示名字 */
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

/** 下载源，取值是后端线串，显示的标签走 i18n */
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

/** 详情页展示的项目；null = 收起。详情与版本列表的状态都在共用组件里 */
const detailItem = ref<ProjectItemDto | null>(null);

/** 内容区根节点，翻页时用它把窗口滚回顶部 */
const bodyRef = ref<HTMLElement | null>(null);

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
  closeDetail();
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
  // loadSource 里 await 若干请求，这中间发出去必然带不上合法排序
  // —— 空串后端也不认，会报 err.sortTypeNotFound。那次 loadSource 拉完会自己再搜一次。
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
  // 翻页先回到内容顶部，与资源窗口同一口径：不拉回去就停在列表底部，看不到新一页的头几条
  bodyRef.value?.scrollIntoView({ behavior: "smooth", block: "start" });
  void search();
}

/** 列表卡片上的「已安装 / 下载中」角标，与资源窗口、详情页共用一份判定 */
const { projectInstalled, projectRunning } = useDownloadBadges(toRef(props, "status"));

/** 打开详情：正文 / 截图与版本列表由共用组件并行加载 */
function openDetail(item: ProjectItemDto) {
  detailItem.value = item;
  emit("detail", true);
}

/** 收起详情；标题栏上的返回键通过 `defineExpose` 调这里 */
function closeDetail() {
  detailItem.value = null;
  emit("detail", false);
}

// 标题栏上的返回键由上层 AddModpackWindow 持有，通过 ref 调这里
defineExpose({ closeDetail });

/** 详情页取数：整合包接口不带资源类型与加载器 */
function loadDetail(pid: string) {
  return api.getModpackDetail(source.value, pid);
}

function loadFiles(pid: string, pageNo: number, fileVersion: string | null) {
  return api.getModpackFiles(source.value, pid, pageNo, fileVersion);
}

/** 详情页点下载：按当前项目安装，装完由 install 收起详情 */
function onDownload(file: FileListItemDto) {
  if (detailItem.value) {
    install(detailItem.value, file);
  }
}

function install(item: ProjectItemDto, file: FileListItemDto) {
  emit("install", {
    source: item.source.source,
    projectId: item.source.pid,
    fileId: file.source.fid,
    fileName: item.name,
  });
  closeDetail();
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

function formatDate(date: string): string {
  return date ? date.slice(0, 10) : "";
}

onMounted(async () => {
  await loadSources();
  consumeTargetProject();
});

/**
 * 收藏窗口点「下载」跳进来的整合包：进页面就直接打开它的详情（版本列表在里面）
 *
 * 与下载资源窗口同理 —— 本页在单窗口模式下也被 KeepAlive 缓存，先把上一次留下的
 * 视图状态清掉再打开新项目：`closeDetail()` 同时把上层的"返回"状态复位，
 * 改完 source 会触发列表刷新，所以等一轮再开详情，免得被那批刷新盖掉。
 */
async function consumeTargetProject() {
  const p = targetProject();
  if (!p || p.fileType !== "modpack") {
    return;
  }
  closeDetail();
  filter.value = "";
  // 换源会让排序表作废：**同步**清掉，不能等 watch 里那次异步重载，
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

  openDetail(item);
}

onActivated(consumeTargetProject);

watch(source, loadSource);
</script>

<template>
  <div class="modpack-mode" ref="bodyRef">
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
    <div v-if="maxPage > 0" class="modpack-page">
      <button class="page-btn" :disabled="page === 0 || searching" @click="turnPage(-1)">
        {{ t("modpack.prevPage") }}
      </button>
      <span class="page-num">{{ page + 1 }} / {{ maxPage + 1 }} · {{ t("modpack.totalItems", { n: total }) }}</span>
      <button class="page-btn" :disabled="page >= maxPage || searching" @click="turnPage(1)">
        {{ t("modpack.nextPage") }}
      </button>
    </div>

    <!-- 项目详情：与资源窗口共用同一份实现，打开哪个项目由 detailItem 决定 -->
    <ProjectDetailPage :item="detailItem" :versions="versions" :initial-version="version"
      :action-label="t('modpack.install')" :status="props.status" :load-detail="loadDetail" :load-files="loadFiles"
      @close="closeDetail" @download="onDownload" />
  </div>
</template>

<style scoped src="./modpack-mode.css"></style>
<style scoped src="../../styles/parts/group-combo.css"></style>
