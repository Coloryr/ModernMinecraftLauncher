// 本机内存参考值：内存设置旁边显示的"物理内存 / 当前可用"
//
// 前端拿不到物理内存，走后端命令 `main_get_system_memory`（内核 mml-sys 查询）。
// 设置窗口的「游戏启动」与实例设置的启动参数共用这一份逻辑，避免两处各写一套文案。
import { computed, onMounted, ref } from "vue";
import { api } from "./api";
import { t } from "./i18n";
import type { SystemMemoryDto } from "./bindings";

/** MiB → 可读文本（≥1 GB 用 GB，否则 MB） */
function formatMemory(mib: number): string {
  return mib >= 1024 ? `${(mib / 1024).toFixed(1)} GB` : `${Math.round(mib)} MB`;
}

/**
 * 内存参考文案
 *
 * # 返回值
 *
 * - `text`: 参考文案，随查询结果变化的响应式文本，结构固定，模板直接渲染一行
 *
 * `text` 始终非空，取值分这几种：
 *
 * - 未取到：`读取本机内存…`
 * - 调用失败：`本机内存读取失败：<原因>`，原因多为命令不存在 = 后端没重编
 * - 后端返回 0：`本机内存读取失败`，查询本身失败
 * - 正常：`物理内存 31.9 GB · 当前可用 18.2 GB`
 */
export function useSystemMemory() {
  const data = ref<SystemMemoryDto>({ total: 0, free: 0 });
  const loaded = ref(false);
  const error = ref("");

  onMounted(async () => {
    try {
      data.value = await api.getSystemMemory();
    } catch (e) {
      error.value = String(e);
    } finally {
      loaded.value = true;
    }
  });

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

  return { text };
}
