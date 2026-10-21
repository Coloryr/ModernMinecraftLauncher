// 设置窗口 · 本地文件标签
//
// 只在打开本标签时统计一次：目录树可能很大，不塞进 SettingsWindow 的统一加载里。
import { computed, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { commands } from "../../../lib/bindings";
import type { LocalFileEntryDto } from "../../../lib/bindings";

/**
 * 本地文件标签的状态与操作
 *
 * # 返回值
 *
 * - `entries`: 各文件夹的路径与占用，顺序由后端给
 * - `total`: 合计字节数
 * - `loading`: 正在统计
 * - `clearing`: 正在清理哪一项的 id，空串表示没有任务
 * - `load`: 重新统计一遍
 * - `clear`: 清理某一项并重新统计
 * - `open`: 在资源管理器里打开某一项
 */
export function useSettingsLocalFiles() {
  const entries = ref<LocalFileEntryDto[]>([]);
  const loading = ref(false);
  const clearing = ref("");

  const total = computed(() => entries.value.reduce((sum, entry) => sum + entry.size, 0));

  async function load() {
    loading.value = true;
    try {
      entries.value = await commands.settings.getLocalFiles();
    } catch (e) {
      showToast(tErr(e));
    } finally {
      loading.value = false;
    }
  }

  /**
   * 清理一项：只删内容、保留文件夹，完成后重新统计
   *
   * - `id`: 文件夹 id，取自 `entries`
   */
  async function clear(id: string) {
    clearing.value = id;
    try {
      await commands.settings.clearLocalFile(id);
      showToast(t("winSettings.localFileCleared"));
    } catch (e) {
      showToast(tErr(e));
    } finally {
      clearing.value = "";
      await load();
    }
  }

  /**
   * 在资源管理器里打开某一项
   *
   * - `id`: 文件夹 id，取自 `entries`
   */
  async function open(id: string) {
    try {
      await commands.settings.openLocalFile(id);
    } catch (e) {
      showToast(tErr(e));
    }
  }

  return { entries, loading, clearing, total, load, clear, open };
}
