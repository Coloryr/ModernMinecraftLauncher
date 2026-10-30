<script setup lang="ts">
// 主窗口顶部栏：同时充当自绘标题栏（整条可拖动，两端放窗口按钮）
// 布局：品牌 + 主页切换 + 功能入口 + 主题切换 + 账户选择
import { computed, onMounted, onUnmounted } from "vue";
import { t } from "../../../lib/i18n";
import AccountSelector from "../../../components/AccountSelector.vue";
import WindowControls from "../../../components/ui/WindowControls.vue";
import ModpackTitleIndicator from "../../../components/ModpackTitleIndicator.vue";
import DownloadTitleIndicator from "../../../components/DownloadTitleIndicator.vue";
import ResourceTitleIndicator from "../../../components/ResourceTitleIndicator.vue";
import { detectOs, onTitleBarPointerDown, titleBarStyle } from "../../../lib/titlebar";
import { useWindowFullscreen } from "../../../lib/useWindowFullscreen";
import { useModpackStatus } from "../../../lib/modpackTasks";
import { pluginDecorated } from "../../../lib/decoration";
import MainBrand from "./MainBrand.vue";
import type { AccountStoreDto } from "../../../lib/bindings";
import type { FeatureId } from "../types";

defineProps<{
  /** 功能入口列表，顺序即显示顺序 */
  features: Array<{ id: FeatureId; icon: string }>;
  /** 启动器主页是否正显示，用来给主页按钮上按下态 */
  newsActive: boolean;
  /** 当前账户；没有账户时为 null */
  currentAccount: AccountStoreDto | null;
  /** 可切换的账户列表，来自共享存储 */
  accounts: AccountStoreDto[];
}>();

// 下载 / 整合包两个进度指示器默认挂在下载整合包窗口，那个窗口关着时才回到这里。
// 单窗口模式下它只是应用内的一页，`windowOpen` 恒为 false，由那一页自己挂
const { status: modpackStatus, init: initModpackStatus } = useModpackStatus(false);
/** 全屏时顶栏要让出 macOS 的刘海 / 菜单栏区域 */
const fullscreen = useWindowFullscreen();
/** 平台判定：安全区这类"按系统"的样式用它，不跟标题栏样式走，否则换样式时外观会反转 */
const isMac = detectOs() === "macos";
/** 原生红黄绿由插件负责摆放：此时不再自绘窗口按钮，左侧按插件给的清空量让位 */
const pluginControls = computed(() => isMac && titleBarStyle === "macos");
let unsubModpack: (() => void) | null = null;

onMounted(async () => {
  unsubModpack = await initModpackStatus();
});

onUnmounted(() => {
  unsubModpack?.();
});

/** 下载整合包窗口是否还开着 */
const modpackWindowOpen = computed(() => !!modpackStatus.value?.windowOpen);

/** 顶栏动作，一律抛给主窗口执行 */
const emit = defineEmits<{
  /** 切换启动器主页 */
  (e: "toggle-news"): void;
  /** 打开某个功能入口 */
  (e: "feature", id: FeatureId): void;
  /** 账户选择器换人 */
  (e: "update:account", acc: AccountStoreDto): void;
}>();
</script>

<template>
  <header class="topbar"
    :class="[titleBarStyle, {
      'is-macos': isMac,
      'is-plugin-decorated': pluginDecorated,
      'is-plugin-controls': pluginControls,
      'is-fullscreen': fullscreen,
    }]"
    @pointerdown="onTitleBarPointerDown">
    <!-- 品牌块与加载期间的标题栏共用，见 MainBrand.vue -->
    <MainBrand />

    <div class="topbar-right">
      <!-- 进度指示器：排在主页按钮左边（整排的最前面）。
           主页面在两种窗口模式下都是这个顶栏，所以两种模式都挂；
           功能页用的是 WindowFrame 那套标题栏，那边也有同名组件，跟着当时的标题栏走。
           没有任务时各自整块不存在，不留空位 -->
      <ModpackTitleIndicator v-if="!modpackWindowOpen" />
      <DownloadTitleIndicator v-if="!modpackWindowOpen" />
      <ResourceTitleIndicator />

      <!-- 启动器主页（切换按钮） -->
      <button class="topbar-icon-btn" :class="{ pressed: newsActive }" v-tip="t('home.entry')"
        @click="emit('toggle-news')">
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="m3 10 9-7 9 7v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V10z" />
          <path d="M9 22V12h6v10" />
        </svg>
      </button>

      <!-- 功能入口 -->
      <button v-for="f in features" :key="f.id" class="topbar-icon-btn" v-tip="t('features.' + f.id)"
        @click="emit('feature', f.id)">
        <svg v-if="f.icon === 'gear'" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor"
          stroke-width="1.8">
          <circle cx="12" cy="12" r="3.2" />
          <path
            d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.03-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.56-1.03 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h.01a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.01a1.7 1.7 0 0 0 1.56 1.03H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.56 1.03z" />
        </svg>
        <svg v-else-if="f.icon === 'chart'" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor"
          stroke-width="1.8" stroke-linecap="round">
          <path d="M18 20V10M12 20V4M6 20v-6" />
        </svg>
        <svg v-else-if="f.icon === 'star'" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor"
          stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="m12 2.6 2.9 5.9 6.5.95-4.7 4.58 1.1 6.47L12 17.44 6.2 20.5l1.1-6.47-4.7-4.58 6.5-.95z" />
        </svg>
        <svg v-else viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round">
          <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
          <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
        </svg>
      </button>

      <AccountSelector :account="currentAccount" :accounts="accounts"
        @update:account="emit('update:account', $event)" />
    </div>

    <!-- 窗口按钮：插件摆原生按钮时不画（macOS 正常形态）；原生被藏掉时
         这里按对调后的样式自己画一套，位置跟着样式走。
         Windows 的贴靠热区由 lib/decoration.ts 把这三个按钮的矩形报给插件 -->
    <WindowControls v-if="!pluginControls" :style="titleBarStyle" />
  </header>
</template>

<style scoped>
/* 高度即标题栏高度：与子窗口框架 WindowFrame 的 .frame-head 共用 --titlebar-h，
   两边必须一致，否则单窗口模式切换页面时标题栏会跳高。
   顶栏是 .main-window 的第一个 flex 子节点，改高度会让顶栏以下的所有内容（含侧栏）整体下移 */
.topbar {
  height: var(--titlebar-h);
  /* 高度锁死：内容比它高时 flex 仍会按内容撑开，实测量出来会多出两三个像素 */
  min-height: var(--titlebar-h);
  max-height: var(--titlebar-h);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 18px;
  background: var(--bg-side);
  border-bottom: 1px solid var(--border);
}

/* 插件接管后原生红黄绿摆在内容上方：按插件发布的清空量让出左侧，
   此时自己的 WindowControls 已被 v-if 掉，不会重复占位。
   全屏时插件把清空量发成 0，用 max 退回常规内边距，免得内容贴到窗口边缘；
   进出全屏是窗口尺寸形变，间距跟着一起过渡，不然退出全屏会先贴边再跳开 */
/* 全屏切换会同时改左内边距、上内边距与高度：三条放同一个过渡里，否则内容会先右移再上移，
   看着像两段动画。时长与窗口进出全屏的尺寸动画同档 */
.topbar.is-macos {
  transition: padding-left 0.3s, padding-top 0.3s, height 0.3s;
}

/* 按系统而不是按标题栏样式：Windows 的插件接管不发布 left-clearance，带上会白多一段内边距 */
.topbar.is-macos.is-plugin-controls {
  padding-left: max(18px, var(--tauri-plugin-decoration-left-clearance, 76px));
}

/* macOS 全屏时窗口铺满整屏，顶栏顶在屏幕最上沿，刘海与菜单栏会挡住内容：
   整体下压一层安全区，顶部露出的仍是顶栏底色，下面的布局跟着一起下去 */
.topbar.is-macos.is-fullscreen {
  /* 顶上 24px 是刘海 / 菜单栏的安全区，剩下 28px 是内容；总高与非全屏一致。
     顶栏内的方形键跟着缩到 26px，否则 32px 的按钮撑出这条 28px 的内容区 */
  --titlebar-actual-h: var(--titlebar-h);
  --ctl-h: 26px;
  box-sizing: border-box;
  height: var(--titlebar-actual-h);
  padding-top: 24px;
}

/* 28px 的内容区：账户按钮按内容定高（头像 + 内边距），这里压到 26px 上下；
   必须穿透到子组件，见 AccountSelector 的 .account-btn。品牌块自己收一档，见 MainBrand */
.topbar.is-macos.is-fullscreen :deep(.account-btn) {
  padding: 2px 6px;
}

.topbar.is-macos.is-fullscreen :deep(.account-btn .avatar) {
  width: 20px;
  height: 20px;
  font-size: 10px;
}

.topbar.is-macos.is-fullscreen :deep(.account-btn .account-name) {
  font-size: 11.5px;
}

/* 全屏时只留账户名一行，类型标签挤不下 */
.topbar.is-macos.is-fullscreen :deep(.account-btn .account-type) {
  display: none;
}

/* windows 样式的窗口按钮是小方块、不贴边，右侧留一点呼吸空间；
   macos 样式的圆点留在左内边距之后，与系统一致留 ~18px 空白。
   注意：**不要**在这里叠加插件的 --tauri-plugin-decoration-right-clearance ——
   插件那套按钮 DOM 已全局隐藏、不参与布局，见 styles/decoration.css，
   视觉与占位都由 M²L 自己的 WindowControls 提供，再让一次宽度会多出一段空白 */
.topbar.windows {
  padding-right: 10px;
}

/* 右侧集群自己顶到最右：窗口按钮在 macos 下插在头部、windows 下插在尾部，
   都不会被布局挤到中间 */
.topbar-right {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

/* 尺寸与圆角走标题栏标准（styles/titlebar.css 的方形图标键），这里只留外观 */
.topbar-icon-btn {
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.topbar-icon-btn:hover {
  background: var(--bg-card);
  border-color: var(--border);
  color: var(--text);
}

.topbar-icon-btn.pressed {
  background: var(--accent-soft);
  border-color: var(--accent-border);
  color: var(--accent);
}
</style>
