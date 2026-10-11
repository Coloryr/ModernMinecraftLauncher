<script setup lang="ts">
// 主窗口顶部栏：同时充当自绘标题栏（整条可拖动，两端放窗口按钮）
// 布局：品牌 + 主页切换 + 功能入口 + 主题切换 + 账户选择
import { t } from "../../../lib/i18n";
import AccountSelector from "../../../components/AccountSelector.vue";
import WindowControls from "../../../components/ui/WindowControls.vue";
import ModpackTitleIndicator from "../../../components/ModpackTitleIndicator.vue";
import DownloadTitleIndicator from "../../../components/DownloadTitleIndicator.vue";
import { onTitleBarPointerDown, titleBarStyle } from "../../../lib/titlebar";
import { pluginDecorated } from "../../../lib/decoration";
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
  <header class="topbar" :class="titleBarStyle" @pointerdown="onTitleBarPointerDown">
    <!-- macos 样式：红黄绿在左端。
         主窗口接插件装饰时，插件在 macOS 上用的是**原生红黄绿**（不是 HTML 按钮），
         所以这里不渲染自己的圆点，免得与原生控件重叠 -->
    <WindowControls v-if="titleBarStyle === 'macos' && !pluginDecorated" :style="titleBarStyle" />

    <div class="brand">
      <div class="brand-logo">MC</div>
      <div class="brand-text">
        <div class="brand-name">{{ t("app.name") }}</div>
        <div class="brand-sub">{{ t("app.sub") }}</div>
      </div>
    </div>

    <div class="topbar-right">
      <!-- 进度指示器：排在主页按钮左边（整排的最前面）。
           主页面在两种窗口模式下都是这个顶栏，所以两种模式都挂；
           功能页用的是 WindowFrame 那套标题栏，那边也有同名组件，跟着当时的标题栏走。
           没有任务时各自整块不存在，不留空位 -->
      <ModpackTitleIndicator />
      <DownloadTitleIndicator />

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

    <!-- windows 样式：最小化 / 最大化 / 关闭在右端。
         主窗口接了插件装饰时，这里**照常渲染**自己的按钮（外观与其它窗口一致），
         插件的按钮被 .topbar 下的样式隐藏成透明占位、只负责给系统量贴靠热区，
         见 <style> 里 data-tauri-decoration-* 那一段 -->
    <WindowControls v-if="titleBarStyle === 'windows'" :style="titleBarStyle" />
  </header>
</template>

<style scoped>
/* 高度即标题栏高度：与子窗口框架 WindowFrame 的 .frame-head 共用 --titlebar-h，
   两边必须一致，否则单窗口模式切换页面时标题栏会跳高 —— 曾经这里写死 64px、那边 60px。
   顶栏是 .main-window 的第一个 flex 子节点，改高度会让顶栏以下的所有内容（含侧栏）整体下移 */
.topbar {
  height: var(--titlebar-h);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 18px;
  background: var(--bg-side);
  border-bottom: 1px solid var(--border);
}

/* windows 样式的窗口按钮是小方块、不贴边，右侧留一点呼吸空间；
   macos 样式的圆点留在左内边距之后，与系统一致留 ~18px 空白。
   注意：**不要**在这里叠加插件的 --tauri-plugin-decoration-right-clearance ——
   插件的按钮已被做成透明占位，见 styles/decoration.css，
   视觉与占位都由 M²L 自己的 WindowControls 提供，再让一次宽度会多出一段空白 */
.topbar.windows {
  padding-right: 10px;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.brand-logo {
  width: 38px;
  height: 38px;
  border-radius: 10px;
  background: linear-gradient(135deg, #4f8cff, #7c5cff);
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 800;
  font-size: 16px;
  color: #fff;
  box-shadow: 0 3px 10px rgba(79, 140, 255, 0.35);
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

/* 右侧集群自己顶到最右：窗口按钮在 macos 下插在头部、windows 下插在尾部，
   都不会被布局挤到中间 */
.topbar-right {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

.topbar-icon-btn {
  width: 36px;
  height: 36px;
  border-radius: 10px;
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
