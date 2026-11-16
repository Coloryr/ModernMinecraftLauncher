<script setup lang="ts">
// 已下载资源窗口：按项目 ID 排序的表格，搜索与翻页都走后端
//
// 记录有几千份，一次性推给前端太慢；窗口只拿当前页，同一项目的多个版本由后端
// 排在一起。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { t } from "../../lib/i18n";
import {
  commands,
  type LinkResourceDto,
  type LinkResourcePageDto,
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
/** 搜索词，匹配项目 ID、显示名、文件名与 SHA1 */
const keyword = ref("");
/** 请求页码 */
const page = ref(1);

/** 只认最后一次请求的结果，慢请求不能盖掉新搜索 */
let requestSeq = 0;
/** 搜索防抖定时器 */
let searchTimer: ReturnType<typeof setTimeout> | undefined;

/** 当前页的记录 */
const items = computed(() => data.value?.items ?? []);
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

/**
 * 这一行是否换到了另一个项目
 *
 * - `index`: 记录在当前页里的下标
 *
 * # 返回值
 *
 * 是则返回 `true`，表格上画一道组间留白
 */
function isGroupStart(index: number): boolean {
  if (index === 0) return false;
  const prev = items.value[index - 1];
  const curr = items.value[index];
  return prev.pid !== curr.pid || prev.kind !== curr.kind;
}

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
    <div class="res-body">
      <div class="res-toolbar">
        <span class="res-count">{{ headText }}</span>
        <BaseButton size="sm" :disabled="loading" @click="load">
          {{ t("winSettings.resourceLinkRefresh") }}
        </BaseButton>
        <input v-model="keyword" class="field-input res-search"
          :placeholder="t('winSettings.resourceLinkListSearch')" spellcheck="false" />
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
            <col class="res-col-kind" />
            <col class="res-col-pid" />
            <col />
            <col class="res-col-hash" />
            <col class="res-col-size" />
            <col class="res-col-action" />
          </colgroup>
          <thead>
            <tr>
              <th>{{ t("winSettings.resourceLinkColKind") }}</th>
              <th>{{ t("winSettings.resourceLinkColPid") }}</th>
              <th>{{ t("winSettings.resourceLinkColName") }}</th>
              <th>{{ t("winSettings.resourceLinkColHash") }}</th>
              <th class="is-right">{{ t("winSettings.resourceLinkColSize") }}</th>
              <th class="is-right">{{ t("winSettings.resourceLinkColAction") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(item, index) in items" :key="item.kind + item.hash" class="res-row"
              :class="{ 'is-missing': !item.exists, 'is-group-start': isGroupStart(index) }">
              <td><span class="res-kind">{{ kindLabel(item.kind) }}</span></td>
              <td><span class="res-pid" v-tip="item.pid">{{ item.pid || "—" }}</span></td>
              <td class="res-name" v-tip="item.file">{{ item.name || item.file }}</td>
              <td class="res-hash" v-tip="item.hash">{{ shortHash(item.hash) }}</td>
              <td class="is-right res-size">
                {{ item.exists ? formatSize(item.size) : t("winSettings.resourceLinkListMissing") }}
              </td>
              <td class="is-right">
                <BaseButton size="sm" @click="openItem(item)">{{ t("winSettings.resourceLinkListOpen") }}</BaseButton>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div v-if="data && total > 0" class="res-pager">
        <BaseButton size="sm" :disabled="loading || currentPage <= 1" @click="goPage(currentPage - 1)">
          {{ t("winSettings.resourceLinkPrev") }}
        </BaseButton>
        <span class="res-page">{{ t("winSettings.resourceLinkPage", { page: currentPage, pages: pageCount }) }}</span>
        <BaseButton size="sm" :disabled="loading || currentPage >= pageCount" @click="goPage(currentPage + 1)">
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

.res-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}

.res-count {
  flex: 1;
  font-size: 11.5px;
  color: var(--text-dim);
}

.res-search {
  width: 280px;
}

/* 表格只在这里滚，表头跟着粘在容器顶部 */
.res-scroll {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  overflow-y: auto;
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

.res-col-kind {
  width: 60px;
}

.res-col-pid {
  width: 140px;
}

.res-col-hash {
  width: 76px;
}

.res-col-size {
  width: 84px;
}

.res-col-action {
  width: 96px;
}

.res-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 8px;
  background: var(--bg-card);
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
  font-size: 11.5px;
  font-weight: 500;
  text-align: left;
}

.res-table td {
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  font-size: 12.5px;
}

/* 换项目时拉开一条缝，读的时候能看出分组 */
.res-row.is-group-start td {
  border-top: 8px solid var(--bg);
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

.res-pid {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 11.5px;
}

.res-row:hover td {
  background: var(--bg-hover);
}

.res-row.is-missing {
  opacity: 0.55;
}

.res-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
