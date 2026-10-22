// 构建信息：版本号与构建通道，主页 / 顶栏据此提示"测试版 / 预发布版"。
//
// 通道由后端在编译期决定，见 src-tauri/src/build_info.rs：预发布与正式发布的前端
// 产物是同一份，前端没法自己判断。这里只拉一次，之后各窗口共享同一个响应式状态。
import { computed, ref } from "vue";
import { commands, type BuildChannel } from "./bindings";

/** 构建通道；还没拉到或拉取失败时按正式发布处理，宁可不提示也不误报 */
export const buildChannel = ref<BuildChannel>("release");
/** 版本号，如 0.1.0；还没拉到时是空串 */
export const appVersion = ref("");

/** 是否需要提示：测试版与预发布版都要，正式版不提示 */
export const isTestBuild = computed(() => buildChannel.value !== "release");

/** 提示文案的键：测试版与预发布版说法不同 */
export const buildNoticeKey = computed(() =>
  buildChannel.value === "debug" ? "build.debugNotice" : "build.prereleaseNotice",
);

/** 徽章文案的键；正式版没有徽章 */
export const buildBadgeKey = computed(() =>
  buildChannel.value === "debug" ? "build.debugBadge" : "build.prereleaseBadge",
);

let loading: Promise<void> | null = null;

/**
 * 拉一次构建信息
 *
 * 多个组件同时调也只发一次请求；失败就保持正式发布状态，不影响界面。
 *
 * # 返回值
 *
 * 拉取完成的 Promise，供需要等值的调用方 await
 */
export function loadBuildInfo(): Promise<void> {
  if (loading) return loading;
  loading = (async () => {
    try {
      const info = await commands.buildInfo.info();
      buildChannel.value = info.channel;
      appVersion.value = info.version;
    } catch {
      // 浏览器预览没有 IPC：按正式版处理，不显示提示
    }
  })();
  return loading;
}
