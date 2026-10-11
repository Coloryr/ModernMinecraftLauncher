// 启动画面控制器
//
// 主窗口在初始化完成后调用 closeSplash() 关闭启动页；失败时传入错误信息，
// 启动页关闭后转而显示错误页：splashError 非空时 SplashScreen 渲染错误页；
// 错误页重试时调用 openSplash() 重新显示启动页。
import { ref } from "vue";

/** 启动画面是否显示（主窗口读取） */
export const splashVisible = ref(true);

/** 初始化失败信息：非空时启动页关闭后显示错误页 */
export const splashError = ref("");

/**
 * 关闭启动画面，初始化完成后调用
 *
 * - `error`: 初始化失败信息，传入后启动页关闭时改显示错误页
 */
export function closeSplash(error?: string) {
  splashError.value = error ?? "";
  splashVisible.value = false;
}

/** 重新显示启动画面，错误页重试时调用，同时清空错误信息 */
export function openSplash() {
  splashError.value = "";
  splashVisible.value = true;
}
