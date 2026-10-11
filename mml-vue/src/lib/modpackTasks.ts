// 整合包安装任务状态，下载整合包窗口与主窗口共用
//
// 任务在后端全局表维护，与窗口生命周期解耦；本模块只负责把
// add-modpack-status 事件 / 初始查询落到本地 ref，并处理终态副作用：
// - 安装成功 → 写本地存储通知主窗口切换选中实例，见 MainWindow 的存储监听
// - 完成 / 失败 → toast；只由当前"负责显示"的窗口弹
//
// "谁负责"两种窗口模式下不一样，见 shouldToast：
// 多窗口模式下下载整合包是一个真实窗口，Rust 的 `windowOpen` 说得清楚；
// 单窗口模式下它只是应用内的一页、而且会被 KeepAlive 缓存，`windowOpen` 恒为 false，
// 只能按"当前页"判 —— 否则整合包那一页和主窗口会各弹一份（安装完成时就是两条重复提示）。
import { ref } from "vue";
import { api, onAddModpackStatus } from "./api";
import { t, tErr } from "./i18n";
import { showToast } from "./toast";
import { KEYS, emitChange, writeRaw } from "./storage";
import { currentKind, multiWindow } from "../windows/windowManager";
import type { ModPackStatusDto } from "./bindings";

/** 单窗口模式下"下载整合包"这一页是否正显示在前台 */
function modpackPageActive(): boolean {
  return !multiWindow.value && currentKind.value === "add_modpack";
}

/**
 * 本次终态提示该不该由本窗口弹
 *
 * - `own=true`（下载整合包窗口 / 那一页）：多窗口模式恒弹；单窗口模式只在它显示在前台时弹
 *   KeepAlive 缓存着的页面照样收事件、照样能弹 toast，不挡就会和别处各弹一份
 * - `own=false`（主窗口 / 根组件那两份订阅）：只负责"整合包那页不在前台"时的兜底。
 *   单窗口模式下根组件那份**任何页面都在场**，所以它不与主窗口互斥 —— 兜底只由它做，
 *   主窗口那份自己也订阅（为了接管新实例），但不再弹提示。
 */
function shouldToast(own: boolean, windowOpen: boolean): boolean {
  const pageActive = modpackPageActive();
  if (own) return multiWindow.value || pageActive;
  // 多窗口模式：整合包窗口开着时由它弹，主窗口不弹
  if (multiWindow.value) return !windowOpen;
  // 单窗口模式：那一页在前台时由它弹；否则兜底只由根组件弹，见 App.vue 的 fallbackToast
  return !pageActive && fallbackToast;
}

/** 单窗口模式下，"整合包那一页不在前台"时的兜底提示是否由这份订阅负责（只给根组件开） */
let fallbackToast = false;

/**
 * 标记本份订阅为兜底提示的唯一负责人，根组件调用，见 App.vue
 *
 * - `enabled`: 是否由本份订阅负责兜底
 */
export function setModpackFallbackToast(enabled: boolean) {
  fallbackToast = enabled;
}

/**
 * 订阅整合包安装任务状态
 *
 * - `own`: 是否为下载整合包窗口本体。true = 多窗口模式下始终负责 toast，
 *   false = 仅在整合包窗口关闭时负责
 * - `onAddedInstance`: 安装成功时回调，主窗口用它刷新并选中新实例
 *
 * # 返回值
 *
 * - `status`: 最近一次任务快照
 * - `init`: 挂载时调用，先同步一次快照再订阅事件，返回解绑函数
 */
export function useModpackStatus(
  own: boolean,
  onAddedInstance?: (uuid: string) => void,
) {
  const status = ref<ModPackStatusDto | null>(null);
  /** 已处理过终态的任务 uuid（避免重复 toast） */
  const seenDone = new Set<string>();

  function apply(e: ModPackStatusDto, silent: boolean) {
    for (const task of e.tasks) {
      if (!(task.done || task.failed || task.cancelled) || seenDone.has(task.uuid)) {
        continue;
      }
      seenDone.add(task.uuid);
      if (task.instanceUuid) {
        writeRaw(KEYS.addedInstance, task.instanceUuid);
        // 通知其它窗口：原生的 storage 事件只跨文档触发，这里显式广播一次
        emitChange(KEYS.addedInstance, task.instanceUuid);
      }
      // 初次同步只记录：窗口打开前就结束的任务不再提示，也不把选中实例改过去
      if (silent) {
        continue;
      }

      if (task.done) {
        // 接管新实例与弹提示分开判：多窗口模式下整合包窗口开着时由它写本地存储、
        // 主窗口收跨窗口的存储通知接管，这里就不重复通知；单窗口模式没有那个事件
        // 同一个文档里写不触发跨窗口通知，主窗口只能靠这个回调，
        // 所以它不能跟着下面 toast 的门控一起被跳过
        if (task.instanceUuid && !(multiWindow.value && e.windowOpen)) {
          onAddedInstance?.(task.instanceUuid);
        }
        // 重复的那一条由 showToast 自己兜住，见 lib/toast.ts 的 DEDUP_MS：
        // 这里有多份订阅，各自判定稍有偏差就会弹两条。去重键带任务 uuid，
        // 同时装完两个同名整合包是两件事，不会被误吞
        if (shouldToast(own, e.windowOpen)) {
          showToast(
            t("modpack.installDone", { name: task.name }),
            2200,
            `modpack-done:${task.uuid}`,
          );
        }
      } else if (task.failed) {
        if (shouldToast(own, e.windowOpen)) {
          showToast(
            t("add.createFail", { msg: task.error ? tErr(task.error) : "" }),
            2200,
            `modpack-fail:${task.uuid}`,
          );
        }
      }
    }
    status.value = e;
  }

  /** 挂载时调用：先同步一次快照（已有的终态任务不提示），再订阅事件 */
  async function init(): Promise<() => void> {
    try {
      apply(await api.getModpackStatus(), true);
    } catch {
      status.value = null;
    }
    return onAddModpackStatus((e) => apply(e, false));
  }

  return { status, init };
}
