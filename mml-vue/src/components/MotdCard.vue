<script setup lang="ts">
// 服务器 MOTD 卡片：图标 + 名字 + MOTD（彩色分段、保留换行）+ 人数/版本/延迟
//
// **三处共用同一份**：主窗口底部的 MOTD 悬浮卡、实例详情里的 MOTD 卡、资源窗口服务器
// 列表上方的 MOTD 卡。抽成组件而不是各写一遍，是因为 MOTD 的细节不少 —— 两行要保留
// `\n`、彩色分段要过 `motdSegStyle`、图标可能没有、查不到时要分清"查询中"与"离线" ——
// 三处各写一遍迟早漂移成"一个能上色一个不能"，`lib/motd.ts` 里对同一件事也说过。
//
// 组件**只负责卡片自身的外观**：描边 / 背景 / 内边距 / 圆角 / 阴影 / 刷新按钮；
// 摆在哪里、要不要固定定位由调用方决定，主窗口的悬浮卡自己加 `.motd-float`。
import { computed, ref, watch } from "vue";
import { t } from "../lib/i18n";
import { faviconOf, motdSegStyle } from "../lib/motd";
import type { MotdDto } from "../lib/bindings";

const props = withDefaults(
  defineProps<{
    /** 查询结果；`null` = 还没回来（显示"刷新中…"） */
    motd: MotdDto | null;
    /** 标题：服务器名或地址，由调用方给，不从 MOTD 里取 */
    name: string;
    /**
     * 紧凑版：图标 46px、去阴影、内边距略松
     *
     * 给"嵌在面板里"的场合用（实例详情）；悬浮卡与列表上方那张用默认大小。
     */
    compact?: boolean;
    /** 是否显示右上角的刷新按钮（悬停卡片才出现） */
    refreshable?: boolean;
    /** 刷新中：按钮里的箭头转起来 */
    loading?: boolean;
  }>(),
  { compact: false, refreshable: false, loading: false },
);

defineEmits<{
  /** 点右上角刷新按钮 */
  (e: "refresh"): void;
}>();

/** 在线时的 MOTD 分段；离线 / 查询中时为空数组，两种都走下面的兜底文案 */
const segments = computed(() => (props.motd?.state === "ok" ? props.motd.segments : []));

/** 在线时才有 meta 行（人数 / 版本 / 延迟） */
const online = computed(() => (props.motd?.state === "ok" ? props.motd : null));

/**
 * 图标加载失败过，说明这台服务器的 favicon 有问题
 *
 * 失败就**整块不画**：既不留破图图标，也不让 `lib/imageFallback.ts` 的全局兜底
 * 换成灰底占位图 —— 用户口径是"MOTD 图标不要用占位符显示"。
 * 换一台服务器 / 重新查到结果时清掉，让它有机会再试
 */
const iconFailed = ref(false);
watch(
  () => props.motd,
  () => (iconFailed.value = false),
);

/**
 * 图标地址：**只认服务器查询带回来的 favicon**
 *
 * `servers.dat` 里存的那个图标直接跳过（用户口径）—— 那份是历史遗留的 base64，
 * 与实际在线的服务器未必一致，还得额外猜它的编码格式
 */
const iconUrl = computed(() => (iconFailed.value ? "" : faviconOf(props.motd)));
</script>

<template>
  <div class="motd-card" :class="{ compact }">
    <button v-if="refreshable" class="motd-refresh" v-tip="t('server.refresh')" @click="$emit('refresh')">
      <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2"
        stroke-linecap="round" stroke-linejoin="round" :class="{ spin: loading }">
        <path d="M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6" />
      </svg>
    </button>

    <!-- 图标：**只认服务器查询带回来的 favicon**，`servers.dat` 里存的那个跳过。
         没有 / 加载失败就什么都不画 —— 与资源列表的行同一口径。
         `data-no-fallback` 是必须的：全局兜底 lib/imageFallback.ts 会把加载失败的图
         换成灰底占位图，那正是这里要避免的"占位符" -->
    <img v-if="iconUrl" class="motd-icon" :src="iconUrl" alt="" data-no-fallback @error="iconFailed = true" />

    <div class="motd-info">
      <div class="motd-name">{{ name }}</div>
      <div class="motd-text">
        <template v-if="segments.length">
          <span v-for="(seg, i) in segments" :key="i" :style="motdSegStyle(seg)">{{
            seg.text
          }}</span>
        </template>
        <!-- 在线但没有文字段：**不能说"连不上"**。查询成功、只是这台服务器没给简介，
             或简介是启动器认不出的写法时，以前会落到下面那句"服务器无法连接"，
             与同一张卡片上显示的人数自相矛盾 -->
        <span v-else-if="online">{{ t("server.noMotd") }}</span>
        <span v-else-if="motd">{{ motd.message || t("server.offline") }}</span>
        <span v-else>{{ t("server.refreshing") }}</span>
      </div>
      <div class="motd-meta">
        <!-- 刷新中优先：这一行是"点下去有没有反应"最直观的落点 ——
             原来只有在线时才显示人数/版本/延迟，刷新时整行不动，看着像没点 -->
        <span v-if="loading" class="motd-online">{{ t("server.refreshing") }}</span>
        <template v-else-if="online">
          <span class="motd-online">{{
            t("server.players", { now: online.playersOnline ?? 0, max: online.playersMax ?? 0 })
          }}</span>
          <span class="sep">·</span>
          <span>{{ online.version || t("server.unknown") }}</span>
          <span class="sep">·</span>
          <span>{{ t("server.ping", { ms: online.ping }) }}</span>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 卡片外观。摆位由调用方管，悬浮卡是 position: fixed */
.motd-card {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 14px;
  border: 1px solid var(--border);
  background: var(--bg-card);
  /* 用户口径：MOTD 卡片**不要阴影**，原来悬浮卡那份 --shadow-lg 也去掉了，
     三处保持同一观感 */
  /* 固定高度：MOTD 一行 / 两行、在线 / 离线、刷新中之间切换时卡片不跳动。
     数值按"名字 + 两行 MOTD + meta"三段算出来，行高见下面各条、合计约 78px，
     加上下 12px 内边距、再留几 px 余量；多出来的 MOTD 行由
     `.motd-text` 的 max-height 自己截掉，不会顶破卡片 */
  height: 108px;
  overflow: hidden;
}

/* 紧凑版（实例详情）：内边距松一点、MOTD 字号大一号，高度跟着补 */
.motd-card.compact {
  gap: 14px;
  padding: 14px 16px;
  border-radius: 12px;
  height: 112px;
}

/* 服务器图标：就是 MOTD 里带回来的那个 favicon。
   只负责尺寸 / 圆角 / 裁切 —— 占位方块那套渐变底与字号已去掉（没有图标时整块不渲染） */
.motd-icon {
  width: 60px;
  height: 60px;
  border-radius: 10px;
  object-fit: cover;
  flex-shrink: 0;
}

.motd-card.compact .motd-icon {
  width: 46px;
  height: 46px;
}

.motd-info {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
}

.motd-name {
  font-size: 14px;
  line-height: 1.3;
  font-weight: 700;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.motd-text {
  font-size: 12.5px;
  line-height: 1.35;
  font-weight: 500;
  color: var(--text-dim);
  /* MOTD 可能多行，保留 \n 换行 */
  white-space: pre-wrap;
  word-break: break-all;
  /* 卡片高度固定，这里最多露两行 —— 再多就让**文字**自己截掉，
     免得把下面的 meta 顶出卡片（少数服会写三行以上）。
     用 max-height 而不是 `-webkit-line-clamp`：后者要把 display 改成 `-webkit-box`，
     那会把每个彩色分段当成独立的盒，MOTD 会被拆成"一行一段" */
  max-height: calc(2 * 1.35em);
  overflow: hidden;
}

/* 紧凑版里 MOTD 是主角：大一号、用正文色 */
.motd-card.compact .motd-text {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text);
}

.motd-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  line-height: 1.3;
  color: var(--text-dim);
  white-space: nowrap;
}

.motd-online {
  color: var(--green);
}

.sep {
  opacity: 0.5;
}

/* 右上角刷新：悬停卡片才出现，避免平时喧宾夺主 */
.motd-refresh {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 24px;
  height: 24px;
  border-radius: 7px;
  border: 1px solid var(--border);
  background: var(--bg-side);
  color: var(--text-dim);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition:
    opacity 0.15s,
    color 0.15s;
}

.motd-card:hover .motd-refresh {
  opacity: 1;
}

.motd-refresh:hover {
  color: var(--accent);
}

.motd-refresh .spin {
  animation: motd-spin 0.8s linear infinite;
}

@keyframes motd-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
