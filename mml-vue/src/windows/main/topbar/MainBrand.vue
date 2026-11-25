<script setup lang="ts">
// 主窗口品牌块：主界面顶栏与加载期间的标题栏共用同一份，别各写一遍。
// 加载期间的标题栏原来只有窗口按钮，ColorMC 迁移弹窗会盖住启动画面中央的程序名，
// 窗口看上去就像没标题，所以这里也放一份，见 MainWindow 的 .boot-head
//
// 外观全部用工具类（Tailwind），尺寸档位取 styles/tailwind.css 里的 token：
// 字号只有 §16.3 那六档，颜色只取主题变量，别在模板里写字面色值。
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

/** 徽章配色：预发布黄、前端调试绿、测试版强调色 */
const badgeColor = computed(() => {
  if (buildChannel.value === "prerelease") return "text-yellow";
  if (buildChannel.value === "frontendDebug") return "text-green";
  return "text-accent";
});
</script>

<template>
  <div class="brand flex shrink-0 items-center gap-2.5">
    <div class="flex flex-col leading-[1.2]">
      <div class="font-bold" :class="compact ? 'text-title' : 'text-heading'">
        {{ t("app.name") }}
        <!-- 测试版 / 预发布版徽章：顶栏一直可见，切到实例列表时也知道自己在用什么版本 -->
        <span v-if="isTestBuild"
          class="ml-1.5 inline-block rounded-pill border border-current px-1.5 py-px align-[2px] text-caption font-semibold"
          :class="badgeColor">{{ t(buildBadgeKey) }}</span>
      </div>
      <div v-if="!compact" class="text-meta text-dim">{{ t("app.sub") }}</div>
    </div>
  </div>
</template>
