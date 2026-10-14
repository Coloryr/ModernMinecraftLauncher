<script setup lang="ts">
// 启动器主页：上次启动实例 / 联机大厅 / 方块列表 入口 + Minecraft 新闻
// empty=true 时顶部展示空实例引导块，含添加实例按钮；此时一个实例都没有
import { t } from "../lib/i18n";
import { showToast } from "../lib/toast";
import { openWindow } from "../windows/windowManager";
import type { InstanceInfoDto, NewsItem } from "../lib/bindings";
import NewsPanel from "./NewsPanel.vue";
import InstanceIcon from "./InstanceIcon.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";

withDefaults(
  defineProps<{
    /** 当前这一页的新闻 */
    items: NewsItem[];
    /** 上次启动的实例；`null` 表示没有，那张卡片不显示 */
    currentInstance: InstanceInfoDto | null;
    /** 是否一个实例都没有：为 true 时顶部换成空实例引导块 */
    empty?: boolean;
    /** 新闻是否加载中 */
    loading?: boolean;
    /** 新闻页码，从 1 开始 */
    page?: number;
    /** 新闻是否还有下一页 */
    hasMore?: boolean;
    /**
     * 页头返回按钮的文案键
     *
     * 返回的**去处随视图模式而变**，由调用方决定，见 MainWindow：
     * - 列表模式：返回的是"实例列表"那一屏，主页与它同为整屏，属于同级切换；
     * - 分组 / 平铺模式：实例列表是**常驻的侧栏**，主页只是右侧内容区的一屏，
     *   返回的是被它盖住的"实例设置"详情。
     *
     * 所以文案不能写死 —— 写死会出现"点了'返回实例列表'却回到实例设置"。
     */
    backLabelKey?: string;
  }>(),
  { loading: false, page: 1, hasMore: true, backLabelKey: "home.backToList" },
);

const emit = defineEmits<{
  /** 选中该实例 */
  (e: "select", inst: InstanceInfoDto): void;
  /** 快捷启动上次启动的实例 */
  (e: "quick-launch"): void;
  /** 点击页头返回按钮：返回实例列表（关闭启动器主页） */
  (e: "back"): void;
  /** 空实例引导块里的"添加实例" */
  (e: "add-instance"): void;
  /** 空实例引导块里的"添加账户" */
  (e: "add-account"): void;
  /** 空实例引导块里的"设置 Java" */
  (e: "add-java"): void;
  /** 重新拉取新闻 */
  (e: "refresh"): void;
  /** 新闻上一页 */
  (e: "prev"): void;
  /** 新闻下一页 */
  (e: "next"): void;
  /** 打开新闻里的链接 */
  (e: "open", url: string): void;
}>();

/** 未实现功能的占位提示 */
function entry(name: string) {
  showToast(t("actions.wip", { name }));
}
</script>

<template>
  <div class="home-page">
    <!-- 页头：主页标题 + 返回，主页自己的页面级控件，不占卡片槽位。
         按钮文案随视图模式变，返回的去处见 backLabelKey 的说明 -->
    <div v-if="!empty" class="page-head">
      <h2 class="page-title">{{ t("home.entry") }}</h2>
      <button class="page-back" @click="emit('back')">
        <GlyphIcon class="page-back-arrow" name="chevron-left" :size="16" />
        {{ t(backLabelKey) }}
      </button>
    </div>

    <!-- 无实例：空实例引导块（融合空状态设计） -->
    <div v-if="empty" class="empty-block">
      <div class="empty-block-icon">
        <!-- 图标跟随所在色块的文字色，.empty-block-icon 已把 color 定为 #fff -->
        <svg viewBox="0 0 24 24" width="44" height="44" fill="none" stroke="currentColor" stroke-width="1.6"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3 4.5 7.5v9L12 21l7.5-4.5v-9L12 3z" />
          <path d="M4.5 7.5 12 12l7.5-4.5" />
          <path d="M12 12v9" />
        </svg>
        <span class="empty-block-badge">
          <GlyphIcon name="plus" :size="16" :weight="2.6" />
        </span>
      </div>
      <h2 class="empty-block-title">{{ t("empty.title") }}</h2>
      <p class="empty-block-desc">{{ t("empty.desc") }}</p>
      <div class="empty-block-actions">
        <button class="empty-block-btn primary" @click="emit('add-instance')">
          <GlyphIcon name="plus" :size="14" :weight="2.2" /> {{ t("list.add") }}
        </button>
        <button class="empty-block-btn" @click="emit('add-account')">
          {{ t("empty.addAccount") }}
        </button>
        <button class="empty-block-btn" @click="emit('add-java')">
          {{ t("empty.setJava") }}
        </button>
      </div>
    </div>

    <!-- 当前选中实例：仅在选中时显示（未选中时该槽位留空），点一下即可快捷启动 -->
    <div v-if="currentInstance" class="last-card">
      <InstanceIcon :name="currentInstance.name" :uuid="currentInstance.uuid" :size="52" />
      <span class="last-name">{{ currentInstance.name }}</span>
      <button class="last-play" @click="emit('quick-launch')">
        <GlyphIcon name="play" :size="12" /> {{ t("home.lastPlay") }}
      </button>
      <button class="last-open" @click="emit('select', currentInstance)">
        <GlyphIcon name="chevron-right" :size="18" />
      </button>
    </div>

    <div class="entry-cards">
      <!-- 联机大厅 -->
      <button class="entry-card lobby" @click="entry(t('home.lobby'))">
        <span class="entry-icon">
          <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="1.8"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
            <circle cx="9" cy="7" r="4" />
            <path d="M23 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" />
          </svg>
        </span>
        <span class="entry-text">
          <span class="entry-title">{{ t("home.lobby") }}</span>
          <span class="entry-desc">{{ t("home.lobbyDesc") }}</span>
        </span>
        <GlyphIcon class="entry-arrow" name="chevron-right" :size="18" />
      </button>

      <!-- 方块列表：独立窗口 -->
      <button class="entry-card lottery" @click="openWindow('block')">
        <span class="entry-icon">
          <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="1.8"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 3 4.5 7.5v9L12 21l7.5-4.5v-9L12 3z" />
            <path d="M4.5 7.5 12 12l7.5-4.5" />
            <path d="M12 12v9" />
            <path d="M19.5 11v6L12 21" />
          </svg>
        </span>
        <span class="entry-text">
          <span class="entry-title">{{ t("home.blocks") }}</span>
          <span class="entry-desc">{{ t("home.blocksDesc") }}</span>
        </span>
        <GlyphIcon class="entry-arrow" name="chevron-right" :size="18" />
      </button>
    </div>

    <NewsPanel :items="items" :loading="loading" :page="page" :has-more="hasMore" @refresh="emit('refresh')"
      @prev="emit('prev')" @next="emit('next')" @open="(url: string) => emit('open', url)" />
  </div>
</template>

<style scoped>
.home-page {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

/* 页头：主页标题 + 返回实例列表 */
.page-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.page-title {
  font-size: 16px;
  font-weight: 700;
  color: var(--text);
}

.page-back {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: var(--ctl-h);
  padding: 0 13px 0 10px;
  border: 1px solid var(--accent-border);
  border-radius: 9px;
  background: var(--bg-card);
  color: var(--accent);
  font-size: 12.5px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

.page-back:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.page-back-arrow {
  font-size: 16px;
  line-height: 1;
}

/* 空实例引导块 */
.empty-block {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 26px 20px 22px;
  border: 1px dashed var(--accent-border);
  border-radius: 16px;
  background: var(--bg-card);
}

.empty-block-icon {
  position: relative;
  width: 96px;
  height: 96px;
  border-radius: 28px;
  background: var(--accent-grad);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  box-shadow: 0 14px 36px var(--accent-soft);
  margin-bottom: 14px;
}

.empty-block-badge {
  position: absolute;
  right: -8px;
  top: -8px;
  width: 30px;
  height: 30px;
  border-radius: 50%;
  background: var(--bg-card);
  border: 2px solid var(--accent);
  color: var(--accent);
  font-size: 17px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
}

.empty-block-title {
  font-size: 19px;
  font-weight: 700;
}

.empty-block-desc {
  font-size: 13px;
  color: var(--text-dim);
  margin-bottom: 10px;
  text-align: center;
}

.empty-block-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 10px;
  margin-top: 8px;
}

.empty-block-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: var(--ctl-h);
  padding: 0 20px;
  border: 1px solid var(--accent-border);
  border-radius: 10px;
  background: var(--bg-card);
  color: var(--accent);
  font-size: 13.5px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.15s;
}

.empty-block-btn:hover {
  background: var(--accent-soft);
}

.empty-block-btn.primary {
  border: none;
  background: var(--accent);
  color: #fff;
}

.empty-block-btn.primary:hover {
  filter: brightness(1.12);
  background: var(--accent);
}

/* 上次启动实例 */
.last-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 16px;
  border-radius: 14px;
  border: 1px solid var(--accent-border);
  background: var(--accent-soft);
}

.last-name {
  flex: 1;
  min-width: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.last-play {
  height: var(--ctl-h);
  padding: 0 18px;
  border-radius: 9px;
  border: none;
  background: var(--accent);
  color: #fff;
  font-size: 13px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: filter 0.15s;
  white-space: nowrap;
}

.last-play:hover {
  filter: brightness(1.12);
}

.last-open {
  width: var(--ctl-h);
  height: var(--ctl-h);
  border-radius: 9px;
  border: 1px solid var(--accent-border);
  background: var(--bg-raised);
  color: var(--accent);
  font-size: 18px;
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.15s;
}

.last-open:hover {
  background: var(--accent);
  color: #fff;
}

.entry-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 12px;
}

.entry-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 16px 18px;
  border-radius: 14px;
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text);
  cursor: pointer;
  text-align: left;
  font-family: inherit;
  transition: all 0.15s;
}

.entry-card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}

.entry-card.lobby:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.entry-card.lottery:hover {
  border-color: var(--yellow);
  background: rgba(245, 185, 68, 0.12);
}

.entry-icon {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.entry-card.lobby .entry-icon {
  background: var(--accent-soft);
  color: var(--accent);
}

.entry-card.lottery .entry-icon {
  background: rgba(245, 185, 68, 0.14);
  color: var(--yellow);
}

.entry-text {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.entry-title {
  font-size: 15px;
  font-weight: 700;
}

.entry-desc {
  font-size: 12px;
  color: var(--text-dim);
}

.entry-arrow {
  font-size: 20px;
  color: var(--text-dim);
  flex-shrink: 0;
}

.entry-card:hover .entry-arrow {
  color: var(--accent);
}
</style>
