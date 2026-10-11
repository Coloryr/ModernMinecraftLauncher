// 全局轻提示（所有窗口统一使用）
// 用法：import { showToast } from "./toast"; showToast("xxx");
import { ref } from "vue";

export interface ToastItem {
  id: number;
  msg: string;
}

export const toasts = ref<ToastItem[]>([]);

let nextId = 1;

/**
 * 去重窗口（毫秒）
 *
 * 同一件事在这个时间内重复弹出会被丢弃。装完一个整合包时，**有多份订阅**都在监听
 * 安装状态事件：根组件负责右下角入口与弹窗、主窗口负责接管新实例、下载整合包那一页
 * 自己也要显示进度。各份的"该不该由我提示"判定是各自求值的，稍有偏差就会弹两条。
 * 与其维护多份判定之间的互斥，不如在这里兜住：用户看到的就是一条。
 *
 * 去重键用调用方给的事件标识（如任务 uuid），不用文案 —— 同时装完两个**同名**整合包
 * 是两件事，各自都该提示。
 */
const DEDUP_MS = 1000;

/** 最近弹出过的事件键 → 时间戳 */
const recent = new Map<string, number>();

/**
 * 弹出轻提示，duration 毫秒后自动消失（默认 2200）
 *
 * - `msg`: 提示文案，调用方已 `t()` 好的成品
 * - `duration`: 显示时长，单位毫秒，缺省 2200
 * - `key`: 去重键，同一件事的多次调用给同一个值。不传时退回用文案去重，
 *   给"同一句话被多处同时弹出"兜底
 */
export function showToast(msg: string, duration = 2200, key?: string) {
  const dedupKey = key ?? msg;
  const now = Date.now();
  const last = recent.get(dedupKey);
  if (last !== undefined && now - last < DEDUP_MS) {
    return;
  }
  recent.set(dedupKey, now);
  // 顺手清掉过期的，免得这张表随着事件越攒越多
  for (const [k, time] of recent) {
    if (now - time >= DEDUP_MS) recent.delete(k);
  }

  const id = nextId++;
  toasts.value.push({ id, msg });
  setTimeout(() => dismissToast(id), duration);
}

/**
 * 手动关闭某条提示
 *
 * - `id`: 提示 id，由 `showToast` 分配
 */
export function dismissToast(id: number) {
  toasts.value = toasts.value.filter((t) => t.id !== id);
}
