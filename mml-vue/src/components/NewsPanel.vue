<script setup lang="ts">
// 新闻面板：公告卡片列表（分类标签 + 封面 + 标题）与分页，数据由父组件拉取
import { t } from "../lib/i18n";
import { usePageKeys } from "../composables/usePageKeys";
import AsyncImage from "./ui/AsyncImage.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import type { NewsItem } from "../lib/bindings";

const props = withDefaults(
  defineProps<{
    /** 当前这一页的新闻 */
    items: NewsItem[];
    /** 加载中；刷新按钮转圈并禁用 */
    loading?: boolean;
    /** 当前页码，从 1 开始 */
    page?: number;
    /** 是否还有下一页 */
    hasMore?: boolean;
  }>(),
  { loading: false, page: 1, hasMore: true },
);

const emit = defineEmits<{
  /** 重新拉取当前页 */
  (e: "refresh"): void;
  /** 上一页 */
  (e: "prev"): void;
  /** 下一页 */
  (e: "next"): void;
  /** 打开这条新闻的链接 */
  (e: "open", url: string): void;
}>();

// 鼠标侧键翻新闻：后退 = 上一页，前进 = 下一页；与按钮同样的可用条件
usePageKeys({
  prev: () => {
    if (props.page > 1 && !props.loading) emit("prev");
  },
  next: () => {
    if (props.hasMore && !props.loading) emit("next");
  },
});

/** 分类标签配色：按 tag 字符串哈希取色相，同一分类颜色稳定且各不相同 */
function tagColor(tag: string) {
  let h = 0;
  for (const c of tag) h = (h * 31 + c.charCodeAt(0)) % 360;
  return {
    background: `hsla(${h}, 65%, 55%, 0.16)`,
    color: `hsl(${h}, 75%, 70%)`,
  };
}
</script>

<template>
  <div class="news-panel">
    <div class="panel-head">
      <h2>{{ t("news.head") }}</h2>
      <button class="refresh-btn" :class="{ spinning: loading }" :disabled="loading" v-tip="t('news.refresh')"
        @click="emit('refresh')">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 12a9 9 0 1 1-2.64-6.36" />
          <polyline points="21 3 21 9 15 9" />
        </svg>
      </button>
    </div>

    <div class="news-list" :class="{ 'is-loading-dim': loading && items.length > 0 }">
      <!-- 首次加载：骨架卡片，与整合包列表同一套加载观感，形状照着新闻卡 -->
      <template v-if="loading && items.length === 0">
        <div v-for="n in 6" :key="n" class="news-item sk-card">
          <div class="sk sk-banner"></div>
          <div class="sk-meta">
            <div class="sk sk-tag"></div>
            <div class="sk sk-date"></div>
          </div>
          <div class="sk-title">
            <div class="sk sk-line w90"></div>
            <div class="sk sk-line w60"></div>
          </div>
        </div>
      </template>
      <template v-else>
        <div v-for="item in items" :key="item.id" class="news-item" @click="emit('open', item.url)">
          <!-- 新闻配图来自网络，挂了就换成占位（灰底图片图标），不留破图 -->
          <AsyncImage class="banner" :src="item.image" />
          <div class="news-meta">
            <span class="news-tag" :style="tagColor(item.tag)">{{ item.tag }}</span>
            <span class="news-date" v-tip="item.date">{{ item.date }}</span>
          </div>
          <h3 class="news-title">{{ item.title }}</h3>
        </div>
        <div v-if="items.length === 0" class="empty-tip">
          <span class="empty-icon">
            <GlyphIcon name="news" :size="32" :weight="1.6" />
          </span>
          <span>{{ t("news.empty") }}</span>
        </div>
      </template>
    </div>

    <!-- 分页（页码从 1 开始） -->
    <div v-if="items.length > 0" class="news-pager">
      <button class="page-btn" :disabled="page <= 1 || loading" @click="emit('prev')">
        {{ t("news.prev") }}
      </button>
      <span class="pager-info">{{ t("news.page", { n: page }) }}</span>
      <button class="page-btn" :disabled="!hasMore || loading" @click="emit('next')">
        {{ t("news.next") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.news-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.panel-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.panel-head h2 {
  font-size: 15px;
  font-weight: 700;
}

.refresh-btn {
  width: var(--ctl-h);
  height: var(--ctl-h);
  border-radius: var(--ctl-radius);
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text-dim);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.15s;
}

.refresh-btn:hover:not(:disabled) {
  color: var(--accent);
  border-color: var(--accent);
}

.refresh-btn:disabled {
  cursor: default;
  opacity: 0.5;
}

.refresh-btn svg {
  animation: news-rotate 0.9s linear infinite paused;
}

.refresh-btn.spinning svg {
  animation-play-state: running;
}

@keyframes news-rotate {
  to {
    transform: rotate(360deg);
  }
}

.news-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 16px;
}

.news-item {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
  transition: all 0.15s;
  cursor: pointer;
}

.news-item:hover {
  border-color: var(--accent);
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}

/* ---------- 首次加载的骨架卡片 ----------
   形状照着新闻卡：封面 110 高 + 标签/日期一行 + 两行标题。
   占位块本身用全局 .sk，见 styles/skeleton.css，这里只拼形状 */
.sk-card {
  cursor: default;
}

.sk-card:hover {
  /* 骨架不该有悬停反馈 */
  border-color: var(--border);
  transform: none;
  box-shadow: none;
}

.sk-banner {
  width: 100%;
  height: 110px;
  /* 跟着卡片的圆角裁，自己不用圆角 */
  border-radius: 0;
}

.sk-meta {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  padding: 10px 14px 0;
}

.sk-tag {
  width: 56px;
  height: 15px;
  border-radius: 999px;
}

.sk-date {
  width: 72px;
  height: 11px;
}

.sk-title {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px;
}

/* 新闻配图外框，即 AsyncImage 组件根；图片本身的 object-fit 在组件内部 */
.banner {
  width: 100%;
  height: 110px;
  display: block;
}

.news-meta {
  display: flex;
  justify-content: space-between;
  align-items: center;
  /* 保证左侧分类与右侧副标题至少隔开一点，长副标题时不会挤到一起 */
  gap: 10px;
  padding: 10px 14px 0;
}

/* 底色与文字色由 tagColor 按分类现算，见脚本 */
.news-tag {
  font-size: 10.5px;
  padding: 1px 8px;
  border-radius: 999px;
}

.news-date {
  font-size: 11.5px;
  color: var(--text-dim);
  /* 副标题最长 60+ 字，会折行把这一行撑高；标签是垂直居中的，
     于是下方标题与左侧分类的间距每张卡片都不一样。锁成单行 + 省略号，
     完整内容靠 v-tip 悬停看 */
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.news-title {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.5;
  padding: 14px 14px 14px;
}

.news-pager {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 14px;
}

.pager-info {
  font-size: 12.5px;
  color: var(--text-dim);
  min-width: 56px;
  text-align: center;
}

.empty-tip {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 60px 0;
  color: var(--text-dim);
  font-size: 13px;
}

.empty-icon {
  font-size: 32px;
  opacity: 0.55;
}
</style>
