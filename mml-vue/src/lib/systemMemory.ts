// 本机内存参考值：内存设置旁边显示的"物理内存 / 当前可用"
//
// 前端拿不到物理内存，走后端命令 `main_get_system_memory`（内核 mml-sys 查询）。
// 设置窗口的「游戏启动」与实例设置的启动参数共用这一份 —— 读的是整机一份数据，
// 所以状态也放模块级：同一窗口里多个面板同时开着时只打一个定时器、读数也一致。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api } from "./api";
import { t } from "./i18n";
import type { SystemMemoryDto } from "./bindings";

/** 自动刷新间隔，单位 ms：可用内存一直在变，读一次就永远停在旧值上 */
const REFRESH_MS = 5000;

/** MiB → 可读文本：≥1 GB 用 GB，否则 MB */
function formatMemory(mib: number): string {
  return mib >= 1024 ? `${(mib / 1024).toFixed(1)} GB` : `${Math.round(mib)} MB`;
}

/** 整机内存读数，同一窗口内共用一份 */
const data = ref<SystemMemoryDto>({ total: 0, free: 0 });
const loaded = ref(false);
const error = ref("");

/** 正在用它的面板数，减到 0 就停表：没人看时不查 */
let users = 0;
let timer: number | undefined;

async function refresh() {
  try {
    data.value = await api.getSystemMemory();
    error.value = "";
  } catch (e) {
    error.value = String(e);
  } finally {
    loaded.value = true;
  }
}

/** 窗口重新可见时立刻补一次：隐藏期间不打点，恢复瞬间挂着的可能是很久以前的读数 */
function onVisible() {
  if (document.visibilityState === "visible") {
    void refresh();
  }
}

function start() {
  users += 1;
  if (timer !== undefined) {
    return;
  }
  void refresh();
  timer = window.setInterval(() => {
    // 窗口不可见（最小化 / 被隐藏）时跳过：后台没必要打点
    if (document.visibilityState === "visible") {
      void refresh();
    }
  }, REFRESH_MS);
  document.addEventListener("visibilitychange", onVisible);
}

function stop() {
  users = Math.max(0, users - 1);
  if (users > 0) {
    return;
  }
  if (timer !== undefined) {
    window.clearInterval(timer);
    timer = undefined;
  }
  document.removeEventListener("visibilitychange", onVisible);
}

/**
 * 内存参考文案
 *
 * # 返回值
 *
 * - `text`: 参考文案，随查询结果变化的响应式文本，结构固定，模板直接渲染一行
 * - `freePercent`: 当前可用占物理内存的百分比，0–100，给"可用"那条进度条用；读不到时为 0
 *
 * `text` 始终非空，取值分这几种：
 *
 * - 未取到：`读取本机内存…`
 * - 调用失败：`本机内存读取失败：<原因>`，原因多为命令不存在 = 后端没重编
 * - 后端返回 0：`本机内存读取失败`，查询本身失败
 * - 正常：`物理内存 31.9 GB · 当前可用 18.2 GB`
 */
export function useSystemMemory() {
  onMounted(start);
  onUnmounted(stop);

  const text = computed(() => {
    if (!loaded.value) return t("args.memoryReading");
    if (error.value) return t("args.memoryError", { msg: error.value });

    const { total, free } = data.value;
    if (!total) return t("args.memoryUnknown");
    if (!free) return t("args.memoryTotal", { total: formatMemory(total) });
    return t("args.memoryHint", {
      total: formatMemory(total),
      free: formatMemory(free),
    });
  });

  // 条上画的是"可用"：这个读数的用途就是让人判断还能开多大堆
  const freePercent = computed(() => {
    const { total, free } = data.value;
    if (!total || !free) return 0;
    return Math.min(100, Math.max(0, (free / total) * 100));
  });

  return { text, freePercent };
}
