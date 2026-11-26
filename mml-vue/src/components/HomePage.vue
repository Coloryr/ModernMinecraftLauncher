<script setup lang="ts">
// 启动器主页：上次启动实例 / 联机大厅 / 方块列表入口 + Minecraft 新闻
// empty 为 true 时在内容区顶部展示空实例引导块，含添加账户 / 添加游戏实例两个按钮
import { computed } from "vue";
import { t } from "../lib/i18n";
import { buildChannelClass, buildNoticeKey, isSourceBuild, isTestBuild } from "../lib/buildInfo";
import { showToast } from "../lib/toast";
import { openWindow } from "../windows/windowManager";
import type { InstanceInfoDto, NewsItem } from "../lib/bindings";
import NewsPanel from "./NewsPanel.vue";
import InstanceIcon from "./InstanceIcon.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import LaunchProgress from "./LaunchProgress.vue";

const props = withDefaults(
  defineProps<{
    /** 当前这一页的新闻 */
    items: NewsItem[];
    /** 上次启动的实例；`null` 表示没有，那张卡片不显示 */
    currentInstance: InstanceInfoDto | null;
    /** 这个实例正在启动流程里（进程还没起来）；按钮里换成进度，点它是取消启动 */
    launching?: boolean;
    /** 启动 / 补全阶段文案，由调用方按阶段算好 */
    launchStageText?: string;
    /** 启动进度；null 表示这一阶段没有确定值，进度条走滚动 */
    launchPercent?: number | null;
    /** 启动方式下拉能不能点 */
    canLaunchMenu?: boolean;
    /** 启动方式下拉是否展开（只用于 aria-expanded） */
    launchMenuOpen?: boolean;
    /** 实例离线且设置了禁止离线启动 */
    offlineBlocked?: boolean;
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
     * 返回的去处随视图模式而变，文案由调用方决定，见 MainWindow：列表模式回到实例列表，
     * 分组模式回到被盖住的实例设置详情。写死文案会出现"点了返回实例列表却回到实例设置"。
     */
    backLabelKey?: string;
  }>(),
  {
    loading: false,
    page: 1,
    hasMore: true,
    backLabelKey: "home.backToList",
    launching: false,
    launchStageText: "",
    launchPercent: null,
    canLaunchMenu: false,
    launchMenuOpen: false,
    offlineBlocked: false,
  },
);

/** 补全文件阶段（建实例后的自动下载） */
const preparing = computed(() => !!props.currentInstance?.preparing);
/** 进程已经起来了 */
const running = computed(() => !!props.currentInstance?.running);
/** 启动流程里：启动中 / 补全中 / 运行中，这几种点按钮都是"取消 / 关闭" */
const busy = computed(() => props.launching || preparing.value || running.value);

/** 能不能点：没有实例、或离线又禁止离线启动时按住；忙的时候要能点来取消 */
const playDisabled = computed(() => !props.currentInstance || (!busy.value && props.offlineBlocked));

/** 悬停时替换显示的动作：取消启动 / 取消补全 / 关闭游戏 */
const actionText = computed(() => {
  if (props.launching) return t("launch.cancel");
  if (preparing.value) return t("launch.cancelPrepare");
  return t("launch.stop");
});

/** 动作图标：取消类是叉，关闭游戏是电源 */
const actionIcon = computed(() => (props.launching || preparing.value ? "close" : "power"));

const emit = defineEmits<{
  /** 启动按钮：闲时启动，启动中 / 补全中取消、运行中弹关闭确认（都由外壳判） */
  (e: "quick-launch"): void;
  /** 点右侧下拉：出启动方式菜单（调试启动 / 导出启动参数） */
  (e: "launch-menu", event: MouseEvent): void;
  /** 点击页头返回按钮：返回目标由调用方决定，见 backLabelKey */
  (e: "back"): void;
  /** 空实例引导块里的"添加实例" */
  (e: "add-instance"): void;
  /** 空实例引导块里的"添加账户" */
  (e: "add-account"): void;
  /** 重新拉取新闻 */
  (e: "refresh"): void;
  /** 新闻上一页 */
  (e: "prev"): void;
  /** 新闻下一页 */
  (e: "next"): void;
  /** 打开新闻里的链接 */
  (e: "open", url: string): void;
}>();

/** 联机大厅暂未开放：功能入口还在，点了只提示「制作中」 */
function onLobby() {
  showToast(t("actions.comingSoon", { name: t("home.lobby") }));
}

</script>

<template>
  <div class="home-page">
    <!-- 测试版 / 预发布版提示：压在主页最上方，打开启动器第一眼就能看到 -->
    <div v-if="isTestBuild" class="build-notice" :class="buildChannelClass">
      <svg class="build-notice-icon" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor"
        stroke-width="2" stroke-linecap="round">
        <path d="M12 8v5M12 16.5h.01" />
        <path d="M10.3 3.9 2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
      </svg>
      <span>{{ t(buildNoticeKey) }}</span>
    </div>
    <!-- 页头：主页标题 + 返回，主页自己的页面级控件，不占卡片槽位 -->
    <div v-if="!empty" class="page-head">
      <h2 class="page-title">{{ t("home.entry") }}</h2>
      <button class="page-back" @click="emit('back')">
        <GlyphIcon class="page-back-arrow" name="chevron-left" :size="16" />
        {{ t(backLabelKey) }}
      </button>
    </div>

    <!-- 无实例：空实例引导块 -->
    <div v-if="empty" class="empty-block">
      <div class="empty-block-icon">
        <!-- 图标跟随所在色块的文字色，.empty-block-icon 已把 color 定为白色 -->
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
        <button class="empty-block-btn" @click="emit('add-account')">
          {{ t("empty.addAccount") }}
        </button>
        <button class="empty-block-btn primary" @click="emit('add-instance')">
          <GlyphIcon name="plus" :size="14" :weight="2.2" /> {{ t("empty.addInstance") }}
        </button>
      </div>
    </div>

    <!-- 当前选中实例：仅在选中时显示（未选中时该槽位留空）。
         启动按钮与实例设置里那颗同一套：跑起来时按钮里是进度、点它是取消 / 关闭，
         右端贴一个下拉箭头出启动方式菜单 -->
    <div v-if="currentInstance" class="last-card">
      <InstanceIcon :name="currentInstance.name" :uuid="currentInstance.uuid" :loader="currentInstance.loader"
        :size="52" />
      <span class="last-name">{{ currentInstance.name }}</span>
      <div class="last-launch">
        <button class="last-play" :class="{ running: busy }" :disabled="playDisabled"
          @click="emit('quick-launch')">
          <!-- 启动 / 补全中：阶段 + 进度；悬停换成取消动作 -->
          <span v-if="busy" class="last-busy">
            <span v-if="launching || preparing" class="last-progress">
              <LaunchProgress :text="launchStageText" :percent="launchPercent" />
            </span>
            <span v-else class="last-running">{{ t("launch.running") }}</span>
            <span class="last-hover">
              <GlyphIcon :name="actionIcon" :size="12" />
              {{ actionText }}
            </span>
          </span>
          <template v-else>
            <GlyphIcon name="play" :size="12" /> {{ t("home.lastPlay") }}
          </template>
        </button>
        <button type="button" class="last-drop" :disabled="!canLaunchMenu" v-tip="t('launchMenu.more')"
          :aria-label="t('launchMenu.more')" :aria-expanded="launchMenuOpen"
          @click="emit('launch-menu', $event)">
          <GlyphIcon name="chevron-down" :size="14" :weight="2.2" />
        </button>
      </div>
    </div>

    <div class="entry-cards">
      <!-- 联机大厅：入口保留，点了只提示「制作中」 -->
      <!-- 联机大厅：还没上线，只在源码启动（测试版）时露出来 -->
      <button v-if="isSourceBuild" class="entry-card lobby" @click="onLobby">
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
/* 构建通道提示：整条铺满内容区，预发布版黄、测试版强调色，与顶栏徽章同一套色 */
.build-notice {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 10px;
  font-size: 12.5px;
  line-height: 1.5;
  border: 1px solid currentColor;
}

.build-notice.prerelease {
  color: var(--yellow);
  background: color-mix(in srgb, var(--yellow) 12%, transparent);
}

.build-notice.debug {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.build-notice.frontend-debug {
  color: var(--green);
  background: color-mix(in srgb, var(--green) 12%, transparent);
}

.build-notice-icon {
  flex-shrink: 0;
}

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
  font-size: 15px;
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
  border-radius: 8px;
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
  border-radius: 12px;
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
  color: var(--on-accent);
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
  font-size: 18px;
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
  font-size: 13px;
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
  color: var(--on-accent);
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
  border-radius: 12px;
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

/* 启动按钮 + 右端下拉：整体仍是一颗按钮，箭头压在内侧右端（与实例设置里那颗同一做法） */
.last-launch {
  position: relative;
  display: flex;
  align-items: stretch;
  flex-shrink: 0;
}

.last-play {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: var(--ctl-h);
  /* 定一个下限：进度那两行（阶段 + 进度条）要放得开，标签长短也不让卡片左右跳 */
  min-width: 140px;
  /* 右边留出下拉箭头的热区 */
  padding: 0 34px 0 16px;
  border-radius: 8px;
  border: none;
  background: var(--accent);
  color: var(--on-accent);
  font-size: 13px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: filter 0.15s, background 0.15s, color 0.15s;
  white-space: nowrap;
  overflow: hidden;
  /* 进度条比主界面那颗窄一档：按钮本身窄，110px 会把阶段文字挤没 */
  --launch-bar-w: 88px;
}

.last-play:hover:not(:disabled) {
  filter: brightness(1.12);
}

/* 启动中悬停是取消启动、运行中悬停是关闭游戏，都是危险动作：整颗转红底红字 */
.last-play.running:hover:not(:disabled) {
  filter: none;
  background: color-mix(in srgb, var(--red) 20%, var(--bg-card));
  color: var(--red);
}

.last-play:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

/* 按钮里平时是进度、「运行中」，悬停换成动作：两层叠在同一位置，只切透明度 */
.last-busy {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
}

.last-progress,
.last-running {
  transition: opacity 0.12s;
}

.last-running {
  font-size: 12.5px;
  font-weight: 600;
}

.last-hover {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  /* 点击交给按钮本身 */
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.12s;
  white-space: nowrap;
}

.last-play:hover .last-progress,
.last-play:hover .last-running {
  opacity: 0;
}

.last-play:hover .last-hover {
  opacity: 1;
}

/* 启动方式下拉：只做一小块悬停底，不画竖线，视觉上还是同一颗按钮 */
.last-drop {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  padding: 0;
  border: none;
  border-radius: 0 8px 8px 0;
  background: transparent;
  color: var(--on-accent);
  cursor: pointer;
  transition: background 0.12s;
}

.last-drop:hover:not(:disabled) {
  background: color-mix(in srgb, var(--on-accent) 14%, transparent);
}

.last-drop:disabled {
  opacity: 0.55;
  cursor: not-allowed;
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
  border-radius: 12px;
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
  background: var(--star-soft);
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
  background: var(--star-soft-strong);
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
  font-size: 12.5px;
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
