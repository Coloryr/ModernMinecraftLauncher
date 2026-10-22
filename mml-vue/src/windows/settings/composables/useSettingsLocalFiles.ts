// 设置窗口 · 本地文件标签
//
// 只在打开本标签时统计一次：目录树可能很大，不塞进 SettingsWindow 的统一加载里。
import { computed, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { commands } from "../../../lib/bindings";
import type { LocalFileEntryDto } from "../../../lib/bindings";
import { useDownloadProgress } from "../../../composables/useDownloadProgress";

/**
 * 本地文件标签的状态与操作
 *
 * # 返回值
 *
 * - `entries`: 各文件夹的路径与占用，顺序由后端给
 * - `total`: 合计字节数
 * - `loading`: 正在统计
 * - `downloading`: 下载队列里还有没下完的任务，此时禁止清理
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

  // 清理就是把缓存目录删空：正在写文件的下载会被打断，还可能留下半截文件
  const { tasks } = useDownloadProgress();
  const downloading = computed(() =>
    tasks.value.some((task) => task.total === 0 || task.completed + task.failed < task.total),
  );

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
   * 清理一项：只删内容、保留文件夹，重新统计完再提示结果
   *
   * `clearing` 一直挂到重新统计结束：这段时间界面上那一行在转圈，
   * 数字也还是旧的，提前弹「已清理」等于抢在结果前面报喜。
   *
   * - `id`: 文件夹 id，取自 `entries`
   */
  async function clear(id: string) {
    // 下载进行中一律不删：界面上按钮已禁用，这里再兜一次
    if (downloading.value) return;

    clearing.value = id;
    let cleared = false;
    try {
      await commands.settings.clearLocalFile(id);
      cleared = true;
      // 界面数据由 WebView2 异步回收，删完不会立刻反映到目录大小上
      if (id === "webview") await new Promise((resolve) => setTimeout(resolve, 1200));
    } catch (e) {
      showToast(tErr(e));
    } finally {
      await load();
      clearing.value = "";
    }

    if (cleared) showToast(t("winSettings.localFileCleared"));
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

  return { entries, loading, downloading, clearing, total, load, clear, open };
}
