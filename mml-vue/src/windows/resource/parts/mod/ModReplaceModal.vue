<script setup lang="ts">
// 替换版本弹窗：列出这个模组在平台上的版本，点一条就把当前 jar 换成那一份
//
// 只负责拉列表与选择：过滤口径（当前实例的游戏版本 + 加载器）与真正换文件都在 ModPane ——
// 取数走它传进来的 `load`，选中后抛回 `pick`，换文件复用「更新下载」那一条链路。
import { onMounted, ref } from "vue";
import { t, tErr } from "../../../../lib/i18n";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import type { FileListDto, FileListItemDto, ModItemDto } from "../../../../lib/bindings";

const props = defineProps<{
  /** 要换版本的那个模组 */
  target: ModItemDto;
  /** 取某一页版本；入参是目标模组与页码，从 0 起，失败抛错 */
  load: (item: ModItemDto, page: number) => Promise<FileListDto>;
}>();

const emit = defineEmits<{
  /** 关掉弹窗 */
  (e: "close"): void;
  /** 选中某一版：把那条文件抛回 ModPane */
  (e: "pick", chosen: FileListItemDto): void;
}>();

/** 已取到的版本，按平台返回顺序，一般是新的在前 */
const list = ref<FileListItemDto[]>([]);
/** 已经取到第几页，从 0 起 */
const page = ref(0);
/** 还有下一页 */
const maxPage = ref(0);
/** 正在取：首屏与「加载更多」共用 */
const loading = ref(false);
/** 出错文案；非空时列表位置只显示它 */
const error = ref("");

/**
 * 取一页
 *
 * - `more`: 点「加载更多」为真，追加到现有列表；首屏为假，整份换掉
 */
async function fetchPage(more: boolean) {
  if (loading.value) return;
  loading.value = true;
  error.value = "";
  const next = more ? page.value + 1 : 0;
  try {
    const res = await props.load(props.target, next);
    list.value = more ? [...list.value, ...res.list] : res.list;
    maxPage.value = res.maxPage;
    page.value = next;
  } catch (e) {
    error.value = tErr(e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => fetchPage(false));

/** 这一条就是当前装着的那份，换上没意义 */
function isCurrent(item: FileListItemDto): boolean {
  return item.isDownload;
}

/** 点一行：当前那一份不给换 */
function pick(item: FileListItemDto) {
  if (isCurrent(item)) return;
  emit("pick", item);
}

/**
 * 发布时间只取日期那一段
 *
 * - `iso`: ISO 8601 字符串，形如 `2024-03-01T12:00:00Z`
 *
 * # 返回值
 *
 * `2024-03-01`；不是这个形态时原样返回
 */
function dateOf(iso: string): string {
  return /^\d{4}-\d{2}-\d{2}/.test(iso) ? iso.slice(0, 10) : iso;
}

/**
 * 文件体积
 *
 * - `bytes`: 字节数
 *
 * # 返回值
 *
 * 人类可读的大小，如 `1.2 MB`
 */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <BaseModal :title="t('resource.replaceVersion')" :width="560" fixed-height="520px" :closable="false"
    @close="emit('close')">
    <p class="rep-title">{{ target.name || target.file }}</p>
    <p v-if="target.version" class="rep-sub">{{ t("resource.updateCurrent", { version: target.version }) }}</p>

    <p v-if="error" class="error-text">{{ error }}</p>
    <p v-else-if="!list.length" class="empty-tip">
      {{ loading ? t("resource.replaceWait") : t("resource.replaceEmpty") }}
    </p>

    <div v-else class="rep-list">
      <button v-for="item in list" :key="item.source.fid" class="rep-row"
        :class="{ 'rep-row-current': isCurrent(item) }" :disabled="isCurrent(item)" @click="pick(item)">
        <span class="rep-body">
          <span class="rep-head">
            <span class="rep-name">{{ item.name }}</span>
            <span v-if="isCurrent(item)" class="rep-tag">{{ t("resource.replaceCurrent") }}</span>
          </span>
          <span class="rep-meta">
            <span v-if="item.time">{{ t("resource.updateDate", { date: dateOf(item.time) }) }}</span>
            <span v-if="item.size > 0">{{ formatSize(item.size) }}</span>
          </span>
        </span>
      </button>
    </div>

    <div v-if="!error && page < maxPage" class="rep-more">
      <BaseButton size="sm" :disabled="loading" @click="fetchPage(true)">
        {{ t("resource.replaceMore") }}
      </BaseButton>
    </div>

    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("resource.cancel") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 弹窗在 body 下，样式走 scoped（与 ModUpdateModal / ModDepsModal 同一处理） */
.rep-title {
  margin: 0;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  word-break: break-all;
}

.rep-sub {
  margin: 3px 0 0;
  color: var(--text-dim);
  font-size: 12px;
}

/* 加载中 / 出错 / 空：这几档没有可滚的列表，让提示吃掉剩余高度，
   动作按钮才会贴底，而不是浮在面板中间 */
.modal-body > .empty-tip,
.modal-body > .error-text {
  flex: 1;
}

/* 弹窗高度被 fixed-height 锁死，这里吃掉剩余高度自己滚 ——
   **不能再给 max-height**：那样面板与列表会各滚一层，右边出现两条滚动条 */
.rep-list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 12px;
  overflow-y: auto;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行的右边缘仍落在弹窗内容区里，滚动条画在这 8px 上 */
  margin-right: -8px;
  padding-right: 8px;
}

/* 一行一个版本，整行可点 */
.rep-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  width: 100%;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-card);
  color: var(--text);
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: all 0.12s;
}

.rep-row:hover:not(:disabled) {
  border-color: var(--accent);
  background: var(--accent-soft);
}

/* 当前装着的那份：点不动，也别看着像个能点的行 */
.rep-row-current {
  opacity: 0.6;
  cursor: default;
}

.rep-body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.rep-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 3px 10px;
}

.rep-name {
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  word-break: break-all;
}

.rep-tag {
  padding: 0 6px;
  border: 1px solid var(--accent-border);
  border-radius: 6px;
  color: var(--accent);
  font-size: 11px;
}

.rep-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 3px 10px;
  margin-top: 3px;
  color: var(--text-dim);
  font-size: 11.5px;
}

.rep-more {
  margin-top: 10px;
  text-align: center;
}
</style>
