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
import { showToast } from "../../lib/toast";

const emit = defineEmits<{
  /** 请求关闭本窗口 */
  (e: "close"): void;
}>();

/** 每页条数，单位条 */
const PAGE_SIZE = 20;

/** 当前页数据；还没加载到时为 null */
const data = ref<LinkResourcePageDto | null>(null);
/** 正在读取 */
const loading = ref(true);
/** 搜索词，匹配项目、项目 ID、文件 ID、文件名与 SHA1 */
const keyword = ref("");
/** 请求页码 */
const page = ref(1);
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
/** 当前页按项目分好的组 */
const groups = computed(() => {
  const out: {
    key: string;
    pid: string;
    project: string;
    kind: string;
    files: LinkResourceDto[];
  }[] = [];
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
    });
    if (mine !== requestSeq) return;
    data.value = res;
    page.value = res.page;
  } catch (e) {
    if (mine === requestSeq) showToast(String(e));
  } finally {
    if (mine === requestSeq) loading.value = false;
  }
}

/** 整理缺失的项目名，完成后刷新当前列表 */
async function tidy() {
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
      showToast(t("winSettings.resourceLinkTidyNothing"));
    } else {
      showToast(
        t("winSettings.resourceLinkTidyDone", {
          filled: result.filled,
          failed: result.failed,
        }),
      );
      page.value = 1;
      await load();
    }
  } catch (e) {
    showToast(tErr(e));
  } finally {
    unlisten?.();
    tidying.value = false;
    tidyProgress.value = null;
  }
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
  <WindowFrame body-fill :title="t('winTitle.downloadedResources')" @close="emit('close')">
    <template #head-right>
      <span class="res-head-search" :class="{ 'is-busy': tidying }" :inert="tidying || undefined">
        <SearchBox v-model="keyword" width="200px"
          :placeholder="t('winSettings.resourceLinkListSearch')" />
      </span>
      <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="loading || tidying"
        v-tip="t('winSettings.resourceLinkTidyTip')" :aria-label="t('winSettings.resourceLinkTidyTip')"
        @click="tidy">
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
      <div class="res-status">
        <template v-if="tidying">
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
        </template>
        <span v-else class="res-count">{{ headText }}</span>
      </div>

      <div class="res-scroll" :class="{ 'is-loading': loading && data }" :aria-busy="loading">
        <div v-if="loading && !data" class="res-state empty-tip">
          {{ t("winSettings.resourceLinkListLoading") }}
        </div>
        <div v-else-if="total === 0" class="res-state empty-tip">
          {{ keyword.trim() ? t("winSettings.resourceLinkListNoMatch") : t("winSettings.resourceLinkListEmpty") }}
        </div>
        <table v-else class="res-table">
          <colgroup>
            <col class="res-col-project" />
            <col />
            <col class="res-col-fid" />
            <col class="res-col-hash" />
            <col class="res-col-size" />
            <col class="res-col-action" />
          </colgroup>
          <thead>
            <tr>
              <th>{{ t("winSettings.resourceLinkColProject") }}</th>
              <th>{{ t("winSettings.resourceLinkColFile") }}</th>
              <th>{{ t("winSettings.resourceLinkColFid") }}</th>
              <th>{{ t("winSettings.resourceLinkColHash") }}</th>
              <th class="is-right">{{ t("winSettings.resourceLinkColSize") }}</th>
              <th class="is-right">{{ t("winSettings.resourceLinkColAction") }}</th>
            </tr>
          </thead>
          <tbody>
            <template v-for="group in groups" :key="group.key">
              <tr v-for="(item, fileIndex) in group.files" :key="item.kind + item.pid + item.hash" class="res-row"
                :class="{ 'is-missing': !item.exists }">
                <td v-if="fileIndex === 0" :rowspan="group.files.length" class="res-project-cell">
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
                <td class="res-fid" v-tip="item.fid">{{ item.fid || "—" }}</td>
                <td class="res-hash" v-tip="item.hash">{{ shortHash(item.hash) }}</td>
                <td class="is-right res-size">
                  {{ item.exists ? formatSize(item.size) : t("winSettings.resourceLinkListMissing") }}
                </td>
                <td class="is-right">
                  <BaseButton size="sm" @click="openItem(item)">{{ t("winSettings.resourceLinkListOpen") }}</BaseButton>
                </td>
              </tr>
            </template>
          </tbody>
        </table>
      </div>

      <div v-if="data && total > 0" class="res-pager">
        <BaseButton size="sm" :disabled="loading || tidying || currentPage <= 1"
          @click="goPage(currentPage - 1)">
          {{ t("winSettings.resourceLinkPrev") }}
        </BaseButton>
        <span class="res-page">{{ t("winSettings.resourceLinkPage", { page: currentPage, pages: pageCount }) }}</span>
        <BaseButton size="sm" :disabled="loading || tidying || currentPage >= pageCount"
          @click="goPage(currentPage + 1)">
          {{ t("winSettings.resourceLinkNext") }}
        </BaseButton>
      </div>
    </div>
  </WindowFrame>
</template>

<style scoped>
.res-body {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  gap: 12px;
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
}

.res-count {
  font-size: 11.5px;
  color: var(--text-dim);
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

/* 表格只在这里滚；外框给表格一个面板，表头跟着粘在面板顶部 */
.res-scroll {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 12px;
}

.res-scroll.is-loading {
  opacity: 0.6;
  pointer-events: none;
}

.res-table {
  flex: none;
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  table-layout: fixed;
}

.res-col-project {
  width: 220px;
}

.res-col-fid {
  width: 100px;
}

.res-col-hash {
  width: 72px;
}

.res-col-size {
  width: 80px;
}

.res-col-action {
  width: 104px;
}

.res-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 8px 10px;
  background: var(--bg-card);
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
  font-size: 11.5px;
  font-weight: 500;
  text-align: left;
}

.res-table td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--border);
  font-size: 12.5px;
  vertical-align: middle;
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

.res-state {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
}

.res-pager {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
}

.res-page {
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}
</style>
