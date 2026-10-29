// 设置窗口 · Java 标签
//
// Java 列表来自 core 的全局注册表，即 `settings_get_launch` 里的 javaList，
// 增删 / 扫描 / 导入 / 扫描目录都会改这份注册表，所以每次操作后统一 `refresh()`。
// 导入进度经 `settings-java-progress` 事件推进，订阅用 useUnlisteners，卸载时一定注销。
import { computed, onMounted, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { useUnlisteners } from "../../../composables/useUnlisteners";
import { commands } from "../../../lib/bindings";
import { SettingsJavaProgress } from "../../../lib/listens";
import { listen } from "@tauri-apps/api/event";
import type { JavaImportProgressDto, JavaInfoDto } from "../../../lib/bindings";
import { isTauri, openWindow } from "../../windowManager";

/**
 * Java 标签的状态与操作
 *
 * # 返回值
 *
 * - `javaList` / `javaGroups`: 已注册的 Java 列表，以及按发行类型分好的分组
 * - `collapsedTypes` / `toggleType` / `typeOpen`: 分组的展开状态
 * - `newJavaName` / `newJavaPath` / `addingJava` / `browseJava` / `addJava` / `removeJava` / `removeAllJava` / `confirmRemoveAll`: 手动增删
 * - `scanJava` / `scanJavaDir` / `scanning` / `scanningDir` / `importJavaArchive` / `importingJava` / `importProgress` / `importPercent` / `downloadJava`: 扫描、导入与下载
 * - `inTauri` / `load` / `resetGroup`: 环境判断、加载与分组级恢复默认
 */
export function useSettingsJava() {
  const inTauri = isTauri();
  const { track: trackUnlisten } = useUnlisteners();

  const javaList = ref<JavaInfoDto[]>([]);
  /** 按类型收起的分组（默认全展开） */
  const collapsedTypes = ref<Record<string, boolean>>({});
  const scanning = ref(false);
  const scanningDir = ref(false);
  const confirmRemoveAll = ref(false);

  // 手动添加
  const newJavaName = ref("");
  const newJavaPath = ref("");
  const addingJava = ref(false);

  // 压缩包导入
  const importingJava = ref(false);
  const importProgress = ref<JavaImportProgressDto | null>(null);
  const importPercent = computed(() => {
    const p = importProgress.value;
    if (!p || p.total <= 0) return 0;
    return Math.min(100, Math.round((p.now / p.total) * 100));
  });

  /** Java 列表按发行类型（JDK / JRE）分组，保持首次出现顺序 */
  const javaGroups = computed(() => {
    const groups: { type: string; items: JavaInfoDto[] }[] = [];
    const index = new Map<string, JavaInfoDto[]>();
    for (const j of javaList.value) {
      let items = index.get(j.javaType);
      if (!items) {
        items = [];
        index.set(j.javaType, items);
        groups.push({ type: j.javaType, items });
      }
      items.push(j);
    }
    return groups;
  });

  // ---------- 加载 ----------

  /** 拉取 Java 列表；`getLaunch` 会连运行参数一起返回，这里只取 javaList */
  async function load() {
    const launch = await commands.settings.getLaunch().catch(() => null);
    if (launch) javaList.value = launch.javaList;
  }

  onMounted(() => {
    trackUnlisten(
      listen<JavaImportProgressDto>(SettingsJavaProgress, (e) => {
        importProgress.value = e.payload;
      }),
    );
  });

  // ---------- 分组展开 ----------

  function toggleType(type: string) {
    collapsedTypes.value[type] = !collapsedTypes.value[type];
  }

  function typeOpen(type: string) {
    return !collapsedTypes.value[type];
  }

  // ---------- 添加 / 删除 ----------

  /** 选 Java 可执行文件并探测名称 */
  async function browseJava() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("winSettings.javaAdd"),
      multiple: false,
      filters: [{ name: "Java", extensions: ["exe"] }],
    });
    if (typeof picked !== "string") return;
    newJavaPath.value = picked;
    // 让后端探测该 Java，自动回填识别到的名字（失败则用户手填）
    try {
      const name = await commands.settings.detectJava(picked);
      if (name) newJavaName.value = name;
    } catch {
      /* 探测失败：名字留给用户手填 */
    }
  }

  async function addJava() {
    const name = newJavaName.value.trim();
    const path = newJavaPath.value.trim();
    if (!name || !path) return;
    addingJava.value = true;
    try {
      await commands.settings.addJava(name, path);
      await load();
      newJavaName.value = "";
      newJavaPath.value = "";
      showToast(t("winSettings.javaAdded", { name }));
    } catch (e) {
      showToast(tErr(e));
    } finally {
      addingJava.value = false;
    }
  }

  async function removeJava(name: string) {
    try {
      await commands.settings.removeJava(name);
      await load();
    } catch (e) {
      showToast(tErr(e));
    }
  }

  async function removeAllJava() {
    try {
      await commands.settings.removeAllJava();
      await load();
    } catch (e) {
      showToast(tErr(e));
    } finally {
      confirmRemoveAll.value = false;
    }
  }

  // ---------- 扫描 / 导入 ----------

  async function scanJava() {
    scanning.value = true;
    try {
      javaList.value = await commands.settings.scanJava();
      showToast(t("winSettings.javaScanDone", { n: javaList.value.length }));
    } catch (e) {
      showToast(tErr(e));
    } finally {
      scanning.value = false;
    }
  }

  /** 扫描指定文件夹（含子目录） */
  async function scanJavaDir() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("winSettings.javaScanDir"),
      multiple: false,
      directory: true,
    });
    if (typeof picked !== "string") return;
    scanningDir.value = true;
    try {
      const found = await commands.settings.scanJavaDir(picked);
      await load();
      // 一个目录下可能有多套 JDK，全部导入后按数量提示
      if (found.length === 0) showToast(t("winSettings.javaScanDirNone"));
      else if (found.length === 1) showToast(t("winSettings.javaScanDirDone", { name: found[0].name }));
      else showToast(t("winSettings.javaScanDirDoneMany", { n: found.length }));
    } catch (e) {
      showToast(tErr(e));
    } finally {
      scanningDir.value = false;
    }
  }

  /** 压缩包导入：zip / 7z 等，解包后注册其中的 Java */
  async function importJavaArchive() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("winSettings.javaImport"),
      multiple: false,
      filters: [{ name: "Archive", extensions: ["zip", "7z", "gz", "tar"] }],
    });
    if (typeof picked !== "string") return;
    importingJava.value = true;
    importProgress.value = null;
    try {
      // 名字不填，由后端从压缩包内容识别
      await commands.settings.importJava(null, picked);
      await load();
    } catch (e) {
      showToast(tErr(e));
    } finally {
      importingJava.value = false;
      importProgress.value = null;
    }
  }

  /** 下载 Java：打开独立下载窗口，按发行类型 / 主版本 / 系统 / 架构筛选 */
  function downloadJava() {
    openWindow("java_download");
  }

  // Java 列表是"系统里扫出来的东西"，没有"恢复默认"可言

  /**
   * Java 页没有可恢复的默认值：列表是系统扫描出来的
   *
   * - `_id`: 分组 id，这里不用
   *
   * # 返回值
   *
   * 恒为 `false`，表示没有处理这个分组
   */
  async function resetGroup(_id: string): Promise<boolean> {
    return false;
  }

  return {
    inTauri,
    javaList,
    javaGroups,
    collapsedTypes,
    toggleType,
    typeOpen,
    scanning,
    scanningDir,
    newJavaName,
    newJavaPath,
    addingJava,
    browseJava,
    addJava,
    removeJava,
    removeAllJava,
    scanJava,
    scanJavaDir,
    importJavaArchive,
    importingJava,
    importProgress,
    importPercent,
    confirmRemoveAll,
    downloadJava,
    load,
    resetGroup,
  };
}
