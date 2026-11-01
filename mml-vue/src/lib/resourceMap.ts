// 资源映射（实验性）的运行时状态
//
// 处理存量实例是后台任务，设置页切走再切回来组件会重建：状态留在组件里的话，
// 进度条会消失、按钮也会变回可点。所以状态放模块级，组件只读它、由它发起任务。
// 事件订阅同样挂在模块里，切页期间进度照常累积。
import { ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type ResourceMapProgressDto, type ResourceMapResultDto } from "./bindings";
import { ResourceMapProgress } from "./listens";
import { t, tErr } from "./i18n";
import { showToast } from "./toast";

/** 是否正在处理 */
export const mapping = ref(false);
/** 处理中的进度；还没收到事件时为 `null` */
export const progress = ref<ResourceMapProgressDto | null>(null);
/** 上一次处理结果 */
export const result = ref<ResourceMapResultDto | null>(null);

/** 进行中的事件订阅，任务结束或组件重挂时用 */
let unlisten: UnlistenFn | null = null;

/** 挂上进度事件；重复调用只保留一份订阅 */
async function subscribe() {
  if (unlisten) return;
  unlisten = await listen<ResourceMapProgressDto>(ResourceMapProgress, (event) => {
    progress.value = event.payload;
  });
}

/** 摘掉进度事件订阅 */
function unsubscribe() {
  unlisten?.();
  unlisten = null;
}

/**
 * 处理所有实例里带在线信息的模组 / 资源包
 *
 * 已在处理中时直接返回，避免重复触发；后端另有忙碌保护
 */
export async function startMapping(): Promise<void> {
  if (mapping.value) return;
  mapping.value = true;
  result.value = null;
  progress.value = null;
  await subscribe();
  try {
    result.value = await commands.settings.mapResources();
    showToast(
      t("winSettings.resourceLinkMapDone", {
        instances: result.value.instances,
        mapped: result.value.mapped,
        failed: result.value.failed,
        skipped: result.value.skipped,
      }),
    );
  } catch (e) {
    showToast(tErr(e));
  } finally {
    unsubscribe();
    mapping.value = false;
  }
}

/**
 * 组件挂载时调用：任务还在跑就把状态接回来
 *
 * 设置窗口关掉再打开时模块状态是新的，但后端任务还在，进度事件继续发
 */
export async function attachMapping(): Promise<void> {
  if (mapping.value) return;
  const running = await commands.settings.resourceMapRunning().catch(() => false);
  if (!running) return;
  mapping.value = true;
  await subscribe();
}
