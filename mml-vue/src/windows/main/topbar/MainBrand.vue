<script setup lang="ts">
// 主窗口品牌块：主界面顶栏与加载期间的标题栏共用同一份，别各写一遍。
// 加载期间的标题栏原来只有窗口按钮，ColorMC 迁移弹窗会盖住启动画面中央的程序名，
// 窗口看上去就像没标题，所以这里也放一份，见 MainWindow 的 .boot-head
import { computed, onMounted } from "vue";
import { t } from "../../../lib/i18n";
import { buildBadgeKey, buildChannel, isTestBuild, loadBuildInfo } from "../../../lib/buildInfo";
import { detectOs } from "../../../lib/titlebar";
import { useWindowFullscreen } from "../../../lib/useWindowFullscreen";

// 徽章要的构建通道只拉一次，两处用到品牌块，谁先挂载谁发起
onMounted(() => {
  void loadBuildInfo();
});

/** macOS 全屏时标题栏只剩 28px 内容区，副标题要收起来、主标题降一档 */
const fullscreen = useWindowFullscreen();
const compact = computed(() => detectOs() === "macos" && fullscreen.value);
</script>

<template>
  <div class="brand" :class="{ compact }">
    <div class="brand-text">
      <div class="brand-name">
        {{ t("app.name") }}
        <!-- 测试版 / 预发布版徽章：顶栏一直可见，切到实例列表时也知道自己在用什么版本 -->
        <span v-if="isTestBuild" class="build-badge" :class="buildChannel">{{ t(buildBadgeKey) }}</span>
      </div>
      <div class="brand-sub">{{ t("app.sub") }}</div>
    </div>
  </div>
</template>

<style scoped>
.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.brand-text {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
}

.brand-name {
  font-size: 15px;
  font-weight: 700;
}

.brand-sub {
  font-size: 11px;
  color: var(--text-dim);
}

/* 构建通道徽章：预发布版用黄色、测试版用强调色，与状态色的用法一致 */
.build-badge {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  border-radius: 999px;
  font-size: 10.5px;
  font-weight: 600;
  vertical-align: 2px;
  border: 1px solid currentColor;
}

.build-badge.prerelease {
  color: var(--yellow);
}

.build-badge.debug {
  color: var(--accent);
}

/* macOS 全屏：内容区只有 28px，副标题放不下；主标题跟着窗口按钮降到 13px */
.brand.compact .brand-sub {
  display: none;
}

.brand.compact .brand-name {
  font-size: 13px;
}
</style>
