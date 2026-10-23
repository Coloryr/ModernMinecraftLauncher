<script setup lang="ts">
// 根组件：按当前窗口标识（currentKind）挂载对应窗口，附全局 Toast
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref, type Component } from "vue";
import { applyTheme } from "./lib/theme";
import { applyLocale, t } from "./lib/i18n";
import { applyAnimations, bgImage, bgLayerStyle } from "./lib/appearance";
import { customPageOn, customUiPageUrl, resolveWindowPage, windowPageVersion } from "./lib/customUi";
import { api, onCloseBlocked } from "./lib/api";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  closeWindow,
  closeDownloadPopup,
  closeModpackPopup,
  closeResourcePopup,
  currentKind,
  downloadPopupOpen,
  modpackPopupOpen,
  multiWindow,
  quitWindow,
  resourcePopupOpen,
} from "./windows/windowManager";
import type { WindowKind } from "./windows/registry";
import { setModpackFallbackToast, useModpackStatus } from "./lib/modpackTasks";
import { initMusic } from "./lib/music";
import { useResourceStatus } from "./lib/resourceTasks";
import AppToast from "./components/ui/AppToast.vue";
import BaseButton from "./components/ui/BaseButton.vue";
import BaseModal from "./components/ui/BaseModal.vue";
import ModpackPopup from "./components/ModpackPopup.vue";
import ResourcePopup from "./components/ResourcePopup.vue";
import CustomUiPage from "./components/CustomUiPage.vue";
import AccountLockModal from "./components/AccountLockModal.vue";
import AutoSaveModal from "./components/AutoSaveModal.vue";

applyTheme();
applyLocale();
applyAnimations();

/** 下载管理：既是可切换的窗口页，也会在单窗口模式下当悬浮弹窗用（下面单独渲染一份） */
const DownloadWindow = defineAsyncComponent(() => import("./windows/download/DownloadWindow.vue"));

/**
 * 游戏日志：**一个实例一个**的工具型窗口
 *
 * 多窗口模式下每个实例一个真实窗口（标签 `mml-log-<实例uuid>`，身份由后端按实例派生，
 * 实例不可更改）；单窗口模式下它像别的窗口一样应用内跳转，实例由 windowParams 带进来。
 */
const LogWindow = defineAsyncComponent(() => import("./windows/log/LogWindow.vue"));

// 各窗口按需加载：每个窗口编译为独立 chunk，打开时才拉取对应 JS
const windowMap: Record<WindowKind, Component> = {
  main: defineAsyncComponent(() => import("./windows/main/MainWindow.vue")),
  settings: defineAsyncComponent(() => import("./windows/settings/SettingsWindow.vue")),
  stats: defineAsyncComponent(() => import("./windows/stats/StatsWindow.vue")),
  help: defineAsyncComponent(() => import("./windows/help/HelpWindow.vue")),
  resource: defineAsyncComponent(() => import("./windows/resource/ResourceWindow.vue")),
  account: defineAsyncComponent(() => import("./windows/account/AccountWindow.vue")),
  add: defineAsyncComponent(() => import("./windows/add/AddInstanceWindow.vue")),
  add_modpack: defineAsyncComponent(() => import("./windows/add_modpack/AddModpackWindow.vue")),
  add_resource: defineAsyncComponent(() => import("./windows/add_resource/AddResourceWindow.vue")),
  collect: defineAsyncComponent(() => import("./windows/collect/CollectWindow.vue")),
  download: DownloadWindow,
  java_download: defineAsyncComponent(() => import("./windows/java_download/JavaDownloadWindow.vue")),
  block: defineAsyncComponent(() => import("./windows/block/BlockWindow.vue")),
  log: LogWindow,
  export: defineAsyncComponent(() => import("./windows/export/ExportWindow.vue")),
  lobby: defineAsyncComponent(() => import("./windows/lobby/LobbyWindow.vue")),
  config: defineAsyncComponent(() => import("./windows/config/ConfigWindow.vue")),
  // 服务器包导出与实例导出共用一套界面：同一个组件按 kind 锁成服务器包模式
  server_pack: defineAsyncComponent(() => import("./windows/export/ExportWindow.vue")),
};

const currentWindow = computed(() => windowMap[currentKind.value]);

// 整页包里的组件可以直接复用内置窗口：ReplaceWindow 会从 window.__mmlWindows 取 main
;(window as unknown as { __mmlWindows?: Record<string, Component> }).__mmlWindows = windowMap;

/**
 * 整页重写只在主窗口生效
 *
 * 设置等工具窗口保留内置界面，否则用户改错页面地址后没有入口改回来。
 */
const customPage = computed(
  () => customPageOn.value && customUiPageUrl.value !== "" && currentKind.value === "main",
);

/**
 * 整页包按窗口 kind 提供的替换组件
 *
 * 回调在挂载前由 initCustomUi 加载并注册，这里同步就能取到；取不到时继续用内置窗口。
 */
const windowPage = computed(() => {
  void windowPageVersion.value;
  return customPageOn.value ? resolveWindowPage(currentKind.value) : null;
});

/**
 * 整合包安装任务的全局状态
 *
 * 放根组件是因为进度弹窗是全局浮层，标题栏上的入口在各窗口自己那边挂，
 * 而安装跑在后台、用户可能已经离开了"下载整合包"那一页。
 *
 * `own=false` + `setModpackFallbackToast(true)`：主窗口那份订阅也在，为了安装成功后
 * 刷新并选中新实例，所以这里指定"整合包那页不在前台时，终态提示只由根组件弹"，
 * 免得两边各弹一份。
 */
setModpackFallbackToast(true);
const { status: modpackStatus, init: initModpackStatus } = useModpackStatus(false);

/**
 * 资源下载任务的全局状态
 *
 * 与整合包那份同理：进度弹窗是全局浮层，而下载跑在后台、用户可能已经离开了"添加资源"
 * 那一页。标题栏指示器自带订阅，这里这份只喂弹窗。
 */
const { status: resourceStatus, init: initResourceStatus } = useResourceStatus(true);

/** 本窗口标签：关闭被拦截事件只认发给自己的那一条，否则别的窗口也会弹同一句话 */
const myLabel = getCurrentWindow().label;

const closeAsk = ref(false);
/** 这次拦的是"退出应用"还是"关掉下载窗口"——只影响文案 */
const closeAskQuit = ref(false);

/** 自定义音乐的配置订阅（配了音乐才需要） */
let unsubMusic: (() => void) | null = null;
let unsubCloseBlocked: (() => void) | null = null;
let unsubModpackStatus: (() => void) | null = null;
let unsubResourceStatus: (() => void) | null = null;

onMounted(async () => {
  /**
   * 关闭被拦截：还有下载任务在跑
   *
   * Rust 侧对主窗口设了关闭保护，见 src-tauri 的 `close_guarded` —— 没有实例在跑时
   * 关掉主窗口就是退出应用、会连带停掉下载，所以它先拒绝、发一条 `close-blocked`，
   * 由这里问一句。有实例在跑时关窗只是退回后台，下载不停，不会走到这里。
   * 放在根组件而不是下载窗口里，是因为单窗口模式下任何页面点 ✕ 都算退出应用，
   * 而各窗口自己收不到这条事件（下载弹窗也可能根本没开着）。
   */
  unsubCloseBlocked = await onCloseBlocked((blocked) => {
    // 事件带的是被拦下的那个窗口的标签：不是自己就忽略，别在别的窗口也弹一遍
    if (blocked && blocked !== myLabel) return;
    // 单窗口模式：唯一的真实窗口就是主窗口，任何页面都算退出；
    // 多窗口模式：只有主窗口算退出
    closeAskQuit.value = !multiWindow.value || currentKind.value === "main";
    closeAsk.value = true;
  });

  // 整合包安装状态订阅：挂在这里而不是各窗口里，安装跑在后台、进度要能在任何页面显示
  unsubModpackStatus = await initModpackStatus();
  // 资源下载同理：进度弹窗是全局浮层，下载跑在后台
  unsubResourceStatus = await initResourceStatus();
  // 自定义音乐：读一遍配置，配了就起播（自动播放被拦时停在未播放，等标题栏那枚按钮）
  unsubMusic = await initMusic();
});

onUnmounted(() => {
  unsubMusic?.();
  unsubCloseBlocked?.();
  unsubModpackStatus?.();
  unsubResourceStatus?.();
});

/** 确认「停止并关闭 / 停止并退出」：先停掉所有下载，任务清空后 Rust 侧不再拦截，再重放这次关闭 */
async function stopDownloadsAndClose() {
  closeAsk.value = false;
  try {
    await api.stopAllDownloads();
  } catch {
    // 停下载是本地清表，正常不会失败；真出错也让关闭走一遍——
    // 关不掉会再收到一次 close-blocked、弹窗重新出现，不会卡在关不掉的窗口上
  }
  quitWindow();
}

/** 两套文案：退出应用 / 关掉下载窗口 */
const closeAskTitle = computed(() =>
  closeAskQuit.value ? t("winDownload.quitTitle") : t("winDownload.closeTitle"),
);
const closeAskText = computed(() =>
  closeAskQuit.value ? t("winDownload.quitConfirm") : t("winDownload.closeConfirm"),
);
const closeAskButton = computed(() =>
  closeAskQuit.value ? t("winDownload.stopAndQuit") : t("winDownload.stopAndClose"),
);
</script>

<template>
  <!-- 整页包提供组件时，任何窗口都直接换成它 -->
  <component :is="windowPage" :key="currentKind" v-if="windowPage" />

  <!-- 否则主窗口仍可用 iframe 整页；内置界面与浮层都不渲染 -->
  <CustomUiPage v-else-if="customPage" :url="customUiPageUrl" />

  <template v-else>
  <!-- 背景图层：设置里选的本地图片，所有窗口共用 -->
  <div v-if="bgImage" class="app-bg" :style="bgLayerStyle" />
  <!-- 单窗口模式的页面切换动画：多窗口模式下每个窗口是独立 webview、组件不会变，
       所以这里平时不动。动画只作用在内容区，见 base.css 的 .win-page-* 选择器，
       标题栏与右上角三个窗口按钮不参与；:duration 与那边的 0.16s 对应，
       因为根元素自身不产生过渡、Vue 推断不出时长。关掉"界面动画"时全局禁用。 -->
  <Transition name="win-page" mode="out-in" :duration="160">
    <KeepAlive>
      <component :is="currentWindow" :key="currentKind" @close="closeWindow" />
    </KeepAlive>
  </Transition>
  <!-- 单窗口模式：下载管理浮在当前页面之上，属工具窗口，见 windowManager 的
       openWindow 特例；关掉它只是收起弹窗，任务照跑，右下角的入口还在 -->
  <DownloadWindow v-if="!multiWindow && downloadPopupOpen && currentKind !== 'download'" @close="closeDownloadPopup" />
  <!-- 整合包安装进度弹窗：由标题栏上那个指示器点开。单窗口模式下是主窗口顶栏，
       多窗口模式下各窗口自己的标题栏里也有指示器，点开同一个弹窗。
       关掉它只是收起，安装照常跑 -->
  <ModpackPopup v-if="modpackPopupOpen && modpackStatus?.tasks.length" :status="modpackStatus"
    @close="closeModpackPopup" />
  <!-- 资源下载进度弹窗：入口是标题栏上的 ResourceTitleIndicator，关掉只收起、下载照跑 -->
  <ResourcePopup v-if="resourcePopupOpen && resourceStatus?.tasks.length" :status="resourceStatus"
    @close="closeResourcePopup" />
  <!-- 下载入口不再用右下角浮层：改成主窗口顶栏上的动态图标 + 进度条，
       即 DownloadTitleIndicator，与整合包那个并排、排在主页按钮左边；
       下载管理弹窗仍在这里渲染 -->
  <!-- 全局轻提示（所有窗口统一） -->
  <AppToast />

  <!-- 关闭被拦截：还有下载任务时先问一句再停下载、关窗 / 退出。
       Rust 侧的关闭保护见 src-tauri 的 close_guarded -->
  <BaseModal v-if="closeAsk" :title="closeAskTitle" :closable="false" @close="closeAsk = false">
    <p class="modal-text">{{ closeAskText }}</p>
    <div class="modal-actions">
      <BaseButton @click="closeAsk = false">{{ t("actions.cancel") }}</BaseButton>
      <BaseButton variant="danger" @click="stopDownloadsAndClose">{{ closeAskButton }}</BaseButton>
    </div>
  </BaseModal>
  </template>

  <!-- 账户占用确认：任何窗口里点启动都可能弹，所以挂在根上 -->
  <AccountLockModal />
  <!-- 启动后自动进入存档的确认，与上面同理 -->
  <AutoSaveModal />
</template>
