<script setup lang="ts">
// 已下载资源窗口：按项目分组合并的表格，搜索与翻页都走后端
//
// 记录有几千份，一次性推给前端太慢；窗口只拿当前页，同一项目的多个版本由后端
// 排在一起，这里用 rowspan 把项目一格合并起来。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import SearchBox from "../../components/ui/SearchBox.vue";
import RowMenu, { type RowMenuItem } from "../../components/ui/RowMenu.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { t, tErr } from "../../lib/i18n";
import { LinkResourceTidyProgress } from "../../lib/listens";
import {
  commands,
  type LinkResourceDto,
  type LinkResourcePageDto,
  type LinkResourceTidyProgressDto,
} from "../../lib/bindings";
import { formatSize } from "../../lib/format";
import { KEYS, readJson, writeJson } from "../../lib/storage";
import { showToast } from "../../lib/toast";
import { openWindow } from "../windowManager";
import { selectedInstance } from "../../lib/settings";

const emit = defineEmits<{
  /** 请求关闭本窗口 */
  (e: "close"): void;
}>();

/** 每页条数，单位条 */
const PAGE_SIZE = 20;

/** 表格列 key */
type ColKey = "project" | "file" | "fid" | "hash" | "size";
/** 可排序的列 key */
type SortKey = ColKey;

/** 表头列 */
interface HeadCol {
  /** 列 key，同时是 colgroup / 列宽 / 排序的键 */
  key: ColKey;
  /** 表头文案的 i18n 键 */
  labelKey: string;
  /** 是否居中 */
  center?: boolean;
  /** 是否靠右对齐 */
  right?: boolean;
  /** 排序键；没有这一项表示该列不排序 */
  sortKey?: SortKey;
}

/** 表头：顺序即列序 */
const HEAD: HeadCol[] = [
  { key: "project", labelKey: "winSettings.resourceLinkColProject", sortKey: "project" },
  { key: "file", labelKey: "winSettings.resourceLinkColFile", sortKey: "file" },
  { key: "fid", labelKey: "winSettings.resourceLinkColFid", center: true, sortKey: "fid" },
  { key: "hash", labelKey: "winSettings.resourceLinkColHash", center: true, sortKey: "hash" },
  { key: "size", labelKey: "winSettings.resourceLinkColSize", center: true, sortKey: "size" },

];

/** 每列的默认宽度与可拖到的最小宽度，单位像素 */
/** 每列的默认宽度与可拖到的最小宽度，单位像素；默认值按最小窗口能放下全部列分 */
const COL_SIZE: Record<ColKey, { width: number; min: number }> = {
  project: { width: 200, min: 120 },
  file: { width: 300, min: 160 },
  fid: { width: 92, min: 72 },
  hash: { width: 78, min: 64 },
  size: { width: 76, min: 64 },
};

/** 当前页数据；还没加载到时为 null */
const data = ref<LinkResourcePageDto | null>(null);
/** 正在读取 */
const loading = ref(true);
/** 搜索词，匹配项目、项目 ID、文件 ID、文件名与 SHA1 */
const keyword = ref("");
/** 请求页码 */
const page = ref(1);
/** 表头排序；null 表示回到默认的项目分组顺序 */
const sort = ref<{ key: SortKey; dir: "asc" | "desc" } | null>(null);
/** 各列宽度，单位像素；拖动表头右缘改，存 lib/storage 的本地缓存 */
const colWidths = ref<Record<ColKey, number>>(
  (Object.keys(COL_SIZE) as ColKey[]).reduce(
    (acc, key) => ({ ...acc, [key]: COL_SIZE[key].width }),
    {} as Record<ColKey, number>,
  ),
);


// 上次拖出来的列宽：只认合法数字，且不小于该列的最小宽度
const savedCols = readJson<Record<ColKey, number>>(KEYS.downloadedResCols);
for (const key of Object.keys(COL_SIZE) as ColKey[]) {
  const width = savedCols[key];
  if (typeof width === "number" && Number.isFinite(width)) {
    colWidths.value[key] = Math.max(COL_SIZE[key].min, Math.round(width));
  }
}
/** 是否正在整理项目名 */
const tidying = ref(false);
/** 整理进度；还没收到事件时为 null */
const tidyProgress = ref<LinkResourceTidyProgressDto | null>(null);

/** 只认最后一次请求的结果，慢请求不能盖掉新搜索 */
let requestSeq = 0;
/** 搜索防抖定时器 */
let searchTimer: ReturnType<typeof setTimeout> | undefined;

/** 当前页的记录 */
const items = computed(() => data.value?.items ?? []);
/** 同一项目的多条记录，表格里合并成一组 */
interface ResourceGroup {
  /** 分组键：类型 + 项目 ID */
  key: string;
  /** 项目 ID */
  pid: string;
  /** 项目名；索引里还没记过时为空串 */
  project: string;
  /** 资源类型：mod / resourcepack */
  kind: string;
  /** 这一组里的文件 */
  files: LinkResourceDto[];
}

/** 当前页按项目分好的组 */
const groups = computed(() => {
  const out: ResourceGroup[] = [];
  for (const item of items.value) {
    const key = `${item.kind}\u0000${item.pid}`;
    const last = out[out.length - 1];
    if (last?.key === key) {
      last.files.push(item);
    } else {
      out.push({
        key,
        pid: item.pid,
        project: item.project,
        kind: item.kind,
        files: [item],
      });
    }
  }
  return out;
});
/** 筛选后的总条数，单位条 */
const total = computed(() => data.value?.total ?? 0);
/** 筛选后仍在磁盘上的总占用，单位字节 */
const totalSize = computed(() => data.value?.totalSize ?? 0);
/** 当前页码 */
const currentPage = computed(() => data.value?.page ?? 1);
/** 总页数 */
const pageCount = computed(() => data.value?.pages ?? 1);

/** 顶部那行统计 */
const headText = computed(() =>
  t("winSettings.resourceLinkListCount", {
    count: total.value,
    size: formatSize(totalSize.value),
  }),
);

/** 整理进度百分比；总数未知或还没开始时为 null */
const tidyPercent = computed(() => {
  const progress = tidyProgress.value;
  if (!progress || progress.total <= 0) return null;
  return Math.min(100, (progress.done / progress.total) * 100);
});
/** 进度条是否走无限滚动：还没开始、总数未知或第一项还没处理完 */
const tidyRolling = computed(
  () => !tidyProgress.value || tidyProgress.value.total <= 0 || tidyProgress.value.done === 0,
);
/** 整理时进度条右侧的文字 */
const tidyText = computed(() => {
  if (!tidyProgress.value) return t("winSettings.resourceLinkTidying");
  return t("winSettings.resourceLinkTidyProgress", {
    done: tidyProgress.value.done,
    total: tidyProgress.value.total,
  });
});

/** 拉当前页 */
async function load() {
  const mine = ++requestSeq;
  loading.value = true;
  try {
    const res = await commands.settings.listLinkResources({
      keyword: keyword.value.trim(),
      page: page.value,
      pageSize: PAGE_SIZE,
      sort: sort.value?.key ?? "",
      desc: sort.value?.dir === "desc",
    });
    if (mine !== requestSeq) return;
    data.value = res;
    page.value = res.page;
    // 索引里还没记过项目名时先补一次：不补的话项目列只能显示项目 ID
    if (!autoTidyTried && res.items.some((item) => !item.project)) {
      autoTidyTried = true;
      void tidy(true);
    }
  } catch (e) {
    if (mine === requestSeq) showToast(String(e));
  } finally {
    if (mine === requestSeq) loading.value = false;
  }
}

/** 这次打开窗口是否已经自动补过项目名 */
let autoTidyTried = false;

/**
 * 整理缺失的项目名，完成后刷新当前列表
 *
 * - `silent`: 自动补全时不在界面上弹结果提示，失败也不打扰
 */
async function tidy(silent = false) {
  if (tidying.value) return;
  tidying.value = true;
  tidyProgress.value = null;
  let unlisten: UnlistenFn | null = null;
  try {
    unlisten = await listen<LinkResourceTidyProgressDto>(LinkResourceTidyProgress, (event) => {
      tidyProgress.value = event.payload;
    });
    const result = await commands.settings.tidyLinkResources();
    if (result.total === 0) {
      if (!silent) showToast(t("winSettings.resourceLinkTidyNothing"));
    } else {
      if (!silent) {
        showToast(
          t("winSettings.resourceLinkTidyDone", {
            filled: result.filled,
            failed: result.failed,
          }),
        );
        page.value = 1;
      }
      await load();
    }
  } catch (e) {
    if (!silent) showToast(tErr(e));
  } finally {
    unlisten?.();
    tidying.value = false;
    tidyProgress.value = null;
  }
}

/**
 * 点表头：正序 → 倒序 → 不排序
 *
 * - `key`: 排序键
 */
function cycleSort(key: SortKey) {
  const cur = sort.value;
  if (!cur || cur.key !== key) {
    sort.value = { key, dir: "asc" };
  } else if (cur.dir === "asc") {
    sort.value = { key, dir: "desc" };
  } else {
    sort.value = null;
  }
  page.value = 1;
  void load();
}

/** 表头点击：只有可排序的列才动排序，读取 / 整理期间不响应 */
function onHeadClick(col: HeadCol) {
  if (col.sortKey && !loading.value && !tidying.value) cycleSort(col.sortKey);
}

/** 表头这一列的排序箭头；没在排序时为空串 */
function sortMark(key: ColKey): string {
  const cur = sort.value;
  if (!cur || cur.key !== key) return "";
  return cur.dir === "asc" ? "↑" : "↓";
}

/** 表头的 aria-sort 取值 */
function ariaSort(key: ColKey): "ascending" | "descending" | "none" {
  const cur = sort.value;
  if (!cur || cur.key !== key) return "none";
  return cur.dir === "asc" ? "ascending" : "descending";
}

/**
 * 按住表头右缘拖动改列宽
 *
 * 监听挂在 window 上：指针移出表头甚至移出窗口也不会丢；松手时才把宽度写进本地缓存。
 *
 * - `event`: 按下时的指针事件
 * - `key`: 哪一列
 */
function startResize(event: PointerEvent, key: ColKey) {
  const startX = event.clientX;
  const startWidth = colWidths.value[key];
  const min = COL_SIZE[key].min;
  const move = (e: PointerEvent) => {
    colWidths.value[key] = Math.max(min, Math.round(startWidth + e.clientX - startX));
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", up);
    writeJson(KEYS.downloadedResCols, colWidths.value);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("pointercancel", up);
}

/**
 * 翻到某一页
 *
 * - `target`: 页码，从 1 开始
 */
function goPage(target: number) {
  if (loading.value || target === page.value) return;
  page.value = target;
  void load();
}

onMounted(() => void load());

// 单窗口模式下窗口被 KeepAlive 缓存，切回不会重新挂载，回来时补拉一次
useWindowRefresh(load);

// 输入即搜，抖一下再发请求，避免每敲一个字符扫一遍索引
watch(keyword, () => {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    page.value = 1;
    void load();
  }, 250);
});

onBeforeUnmount(() => {
  if (searchTimer) clearTimeout(searchTimer);
  requestSeq += 1;
});

/** 右键项目格时的菜单；null 表示没开 */
const menu = ref<{ x: number; y: number; group: ResourceGroup } | null>(null);

/** 菜单项：在这个项目上能做的事 */
const menuItems = computed<RowMenuItem[]>(() => [
  { id: "open-project", label: t("winSettings.resourceLinkOpenProject") },
  { id: "open-location", label: t("winSettings.resourceLinkOpenLocation") },
]);

/**
 * 在指针处弹项目菜单
 *
 * - `event`: 右键事件，菜单按它的坐标定位
 * - `group`: 右键的那一组
 */
function openMenu(event: MouseEvent, group: ResourceGroup) {
  menu.value = { x: event.clientX, y: event.clientY, group };
}

/** 项目来自哪个平台：纯数字项目 ID 是 CurseForge，其余是 Modrinth */
function sourceOf(pid: string): string {
  return /^\d+$/.test(pid) ? "curseforge" : "modrinth";
}

/**
 * 菜单动作
 *
 * - `id`: 菜单项 id，见 {@link menuItems}
 */
async function onMenuPick(id: string) {
  const group = menu.value?.group;
  menu.value = null;
  if (!group) return;
  if (id === "open-project") {
    openWindow("add_resource", {
      uuid: selectedInstance.value || undefined,
      project: {
        source: sourceOf(group.pid),
        pid: group.pid,
        fileType: group.kind,
        name: group.project || group.pid,
        icon: null,
        url: "",
      },
    });
    return;
  }
  if (id === "open-location") {
    const first = group.files[0];
    if (first) await openItem(first);
  }
}

/**
 * 在资源管理器里定位一份资源
 *
 * - `item`: 列表里的一行
 */
async function openItem(item: LinkResourceDto) {
  try {
    await commands.settings.openLinkResource(item.kind, item.file);
  } catch (e) {
    showToast(String(e));
  }
}

/** 类型标签文案 */
function kindLabel(kind: string): string {
  return kind === "mod" ? t("winSettings.resourceLinkKindMod") : t("winSettings.resourceLinkKindPack");
}

/** 哈希缩略：前 8 位就够区分，完整值放 tooltip */
function shortHash(hash: string): string {
  return hash.slice(0, 8) || "—";
}
</script>

<template>
  <WindowFrame body-fill body-flush :title="t('winTitle.downloadedResources')" @close="emit('close')">
    <template #head-right>
      <!-- 共 XX 份 · 占用 XX：放在标题栏，正文整块留给表格；整理时让位给进度条 -->
      <span v-if="data && !tidying" class="res-head-count">{{ headText }}</span>
      <span class="res-head-search" :class="{ 'is-busy': tidying }" :inert="tidying || undefined">
        <SearchBox v-model="keyword" width="200px"
          :placeholder="t('winSettings.resourceLinkListSearch')" />
      </span>
      <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="loading || tidying"
        v-tip="t('winSettings.resourceLinkTidyTip')" :aria-label="t('winSettings.resourceLinkTidyTip')"
        @click="tidy()">
        <GlyphIcon name="wrench" :size="15" :weight="1.8" />
      </BaseButton>
      <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="loading || tidying"
        v-tip="t('winSettings.resourceLinkRefresh')" :aria-label="t('winSettings.resourceLinkRefresh')"
        @click="load">
        <GlyphIcon name="refresh" :size="15" :weight="1.8" class="res-refresh-icon"
          :class="{ 'is-spinning': loading }" />
      </BaseButton>
    </template>

    <div class="res-body">
      <div v-if="tidying" class="res-status">
        <div class="res-progress-track" role="progressbar" aria-valuemin="0"
          :aria-label="t('winSettings.resourceLinkTidyTip')"
          :aria-valuemax="tidyProgress?.total || 1" :aria-valuenow="tidyProgress?.done || 0">
          <div class="res-progress-fill progress-fill" :class="{ 'is-roll': tidyRolling }"
            :style="tidyRolling ? undefined : { width: (tidyPercent ?? 0) + '%' }" />
        </div>
        <span class="res-progress-text">
          <span class="res-spinner" aria-hidden="true" />
          {{ tidyText }}
        </span>
      </div>

      <div v-if="loading && !data" class="res-state empty-tip">
        {{ t("winSettings.resourceLinkListLoading") }}
      </div>
      <div v-else-if="total === 0" class="res-state empty-tip">
        {{ keyword.trim() ? t("winSettings.resourceLinkListNoMatch") : t("winSettings.resourceLinkListEmpty") }}
      </div>
      <template v-else>
        <!-- 表头单独一层：竖向滚动条从表头下面开始，不会跨过表头；横向滚动时跟着平移 -->
        <div class="res-head">
          <table class="res-table">
            <!-- 列宽由 colgroup 给：拖表头右缘改，列宽存本地缓存；表头与行区共用同一份 -->
            <colgroup>
              <col v-for="col in HEAD" :key="col.key" :style="{ width: colWidths[col.key] + 'px' }" />
            </colgroup>
            <thead>
              <tr>
                <th v-for="col in HEAD" :key="col.key" :class="{ 'is-right': col.right, 'is-center': col.center }"
                  :aria-sort="col.sortKey ? ariaSort(col.key) : undefined">
                  <button v-if="col.sortKey" class="res-th-btn" :disabled="loading || tidying"
                    @click="onHeadClick(col)">
                    {{ t(col.labelKey) }}
                    <span class="res-sort-mark">{{ sortMark(col.key) }}</span>
                  </button>
                  <span v-else class="res-th-label">{{ t(col.labelKey) }}</span>
                  <span class="res-grip" @click.stop @pointerdown.stop.prevent="startResize($event, col.key)" />
                </th>
              </tr>
            </thead>
          </table>
        </div>

        <div class="res-scroll" :class="{ 'is-loading': loading }" :aria-busy="loading">
          <table class="res-table res-table-body">
            <colgroup>
              <col v-for="col in HEAD" :key="col.key" :style="{ width: colWidths[col.key] + 'px' }" />
            </colgroup>
            <tbody>
              <template v-for="group in groups" :key="group.key">
                <tr v-for="(item, fileIndex) in group.files" :key="item.kind + item.pid + item.hash" class="res-row"
                  :class="{ 'is-missing': !item.exists }">
                  <td v-if="fileIndex === 0" :rowspan="group.files.length" class="res-project-cell"
                    @contextmenu.prevent="openMenu($event, group)">
                    <div class="res-project">
                      <span class="res-kind">{{ kindLabel(group.kind) }}</span>
                      <span class="res-project-name" v-tip="group.project || group.pid">
                        {{ group.project || group.pid || "—" }}
                      </span>
                      <span v-if="group.project" class="res-project-id" v-tip="group.pid">{{ group.pid }}</span>
                    </div>
                  </td>
                  <td class="res-file">
                    <span class="res-file-name" v-tip="item.name || item.file">{{ item.name || item.file }}</span>
                    <span class="res-file-path" v-tip="item.file">{{ item.file }}</span>
                  </td>
                  <td class="res-fid is-center" v-tip="item.fid">{{ item.fid || "—" }}</td>
                  <td class="res-hash is-center" v-tip="item.hash">{{ shortHash(item.hash) }}</td>
                  <td class="is-center res-size">
                    {{ item.exists ? formatSize(item.size) : t("winSettings.resourceLinkListMissing") }}
                  </td>

                </tr>
              </template>
            </tbody>
          </table>
        </div>
      </template>

      <!-- 分页浮在表格右下角：不占正文高度，表格才能铺满整窗 -->
      <div v-if="data && total > 0" class="res-pager">
        <button class="res-pager-btn" :disabled="loading || tidying || currentPage <= 1"
          :aria-label="t('winSettings.resourceLinkPrev')" v-tip="t('winSettings.resourceLinkPrev')"
          @click="goPage(currentPage - 1)">
          <GlyphIcon name="chevron-left" :size="14" :weight="2.2" />
        </button>
        <span class="res-page">{{ t("winSettings.resourceLinkPage", { page: currentPage, pages: pageCount }) }}</span>
        <button class="res-pager-btn" :disabled="loading || tidying || currentPage >= pageCount"
          :aria-label="t('winSettings.resourceLinkNext')" v-tip="t('winSettings.resourceLinkNext')"
          @click="goPage(currentPage + 1)">
          <GlyphIcon name="chevron-right" :size="14" :weight="2.2" />
        </button>
      </div>
    </div>

    <RowMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @pick="onMenuPick" @close="menu = null" />
  </WindowFrame>
</template>

<style scoped>
/* 整窗满铺：正文没有外边距，分页是浮层，定位参照在这里 */
.res-body {
  position: relative;
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
}

/* 标题栏搜索：整理期间压暗并停止交互 */
.res-head-search {
  display: inline-flex;
}

.res-head-search.is-busy {
  opacity: 0.55;
  pointer-events: none;
}

.res-status {
  display: flex;
  flex: none;
  align-items: center;
  gap: 10px;
  min-height: 20px;
  padding: 10px 14px 0;
}

/* 标题栏里的份数：与其它窗口标题栏的状态文字同一档 */
.res-head-count {
  font-size: 11.5px;
  color: var(--text-dim);
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.res-progress-track {
  flex: 1;
  height: 6px;
  border-radius: 999px;
  background: var(--bg-hover);
  overflow: hidden;
}

.res-progress-fill {
  position: relative;
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  overflow: hidden;
  transition: width 0.3s var(--ease-out);
}

/* 进度条上的流光：总数已知时宽度推进，这层高光一直在跑，读得出"还在整理" */
.res-progress-fill::after {
  content: "";
  position: absolute;
  inset: 0;
  background: linear-gradient(
    90deg,
    transparent,
    color-mix(in srgb, var(--text) 28%, transparent),
    transparent
  );
  transform: translateX(-100%);
  animation: res-shine 1.2s linear infinite;
}

@keyframes res-shine {
  to {
    transform: translateX(100%);
  }
}

.res-progress-text {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  color: var(--text-dim);
  white-space: nowrap;
}

.res-refresh-icon.is-spinning {
  animation: res-spin 0.8s linear infinite;
}

.res-spinner {
  width: 12px;
  height: 12px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: res-spin 0.8s linear infinite;
}

@keyframes res-spin {
  to {
    transform: rotate(360deg);
  }
}

/* 表头单独一层：不参与竖向滚动，滚动条只出现在行区（从表头下面开始）。
   overflow: hidden 也会占住 9px 滚动条槽，两边列宽才能对齐 */
.res-head {
  flex: none;
  overflow: hidden;
  scrollbar-gutter: stable;
}

/* 表格铺满整个内容区，只在这里滚 */
.res-scroll {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  overflow-y: auto;
  overflow-x: hidden;
  /* 常驻预留滚动条槽：有没有滚动条，列宽都和表头对齐 */
  scrollbar-gutter: stable;
}

.res-scroll.is-loading {
  opacity: 0.6;
  pointer-events: none;
}

.res-table {
  flex: none;
  /* 永远铺满容器：列宽按 px 写，总和超出时按比例压缩，最小窗口也不出横向滚动条 */
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  table-layout: fixed;
}


/* 行区那张表：滚到底时最后一行不被分页浮层压住 */
.res-table-body {
  margin-bottom: 56px;
}

.res-table th {
  padding: 8px 10px;
  background: var(--bg-card);
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
  font-size: 11.5px;
  font-weight: 500;
  text-align: left;
  user-select: none;
  white-space: nowrap;
}

/* 表头按钮：点一下切排序；外观跟着表头走，不另做一套 */
.res-th-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  cursor: pointer;
}

.res-th-btn:hover:not(:disabled) {
  color: var(--text);
}

.res-th-btn:disabled {
  cursor: default;
}

/* 排序箭头占固定宽：换列排序时表头文字不会左右跳 */
.res-sort-mark {
  display: inline-block;
  width: 10px;
  font-size: 10px;
  color: var(--accent);
}

/* 列宽把手：贴右缘的 8px 热区，悬停表头时才画出那条细线 */
.res-grip {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 8px;
  cursor: col-resize;
}

.res-grip::after {
  content: "";
  position: absolute;
  top: 25%;
  bottom: 25%;
  right: 3px;
  width: 1px;
  background: var(--border);
  opacity: 0;
  transition: opacity 0.12s;
}

.res-table th:hover .res-grip::after {
  opacity: 1;
}

.res-table td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--border);
  font-size: 12.5px;
  vertical-align: middle;
}

/* 满铺后首尾列直接贴窗口边缘，补一点留白 */
.res-table th:first-child,
.res-table td:first-child {
  padding-left: 14px;
}

.res-table th:last-child,
.res-table td:last-child {
  padding-right: 14px;
}

/* 项目一格合并整个项目：底色把同项目的版本圈在一起 */
.res-project-cell {
  vertical-align: top;
  padding: 10px 12px;
  background: var(--bg-card);
  border-right: 1px solid var(--border);
}

.res-project {
  display: flex;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
}

.res-kind {
  display: inline-block;
  padding: 1px 5px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 10.5px;
  line-height: 1.4;
}

.res-project-name {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.35;
  overflow-wrap: anywhere;
}

.res-project-id {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-dim);
  overflow-wrap: anywhere;
}

.res-file {
  min-width: 0;
}

.res-file-name {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.res-file-path {
  display: block;
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-dim);
}

.res-fid {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 11.5px;
}

.res-hash {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-dim);
}

.res-size {
  color: var(--text-dim);
  font-size: 11.5px;
  font-variant-numeric: tabular-nums;
}

/* 合并格不跟着单行悬停，避免项目底色闪一下 */
.res-row:hover td:not(.res-project-cell) {
  background: var(--bg-hover);
}

.res-row.is-missing td:not(.res-project-cell) {
  opacity: 0.55;
}

.res-table .is-right {
  text-align: right;
}

.res-table .is-center {
  text-align: center;
}

.res-state {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
}

/* 分页浮在表格右下角：不占正文高度，表格才能铺满整窗 */
.res-pager {
  position: absolute;
  right: 12px;
  bottom: 12px;
  z-index: 20;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 6px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-card);
  box-shadow: var(--shadow-md);
}

.res-pager-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.res-pager-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text);
}

.res-pager-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.res-page {
  padding: 0 4px;
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}
</style>
