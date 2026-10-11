// 添加实例 · 加载器联动：某个游戏版本支持哪些加载器 + 该加载器的版本列表
//
// 数据两级缓存：支持列表按游戏版本、加载器版本按 `loader:mc`；
// 刷新按钮清缓存重查，联动（选版本 / 换加载器）只查不提示 —— 提示只属于用户主动点的刷新。
// 查询期间后端拦关闭请求（CloseRequested），这里统一维护那个开关。
import { onMounted, ref, watch, type Ref } from "vue";
import { api, onAddLoaderProgress } from "../../../lib/api";
import { showToast } from "../../../lib/toast";
import { t } from "../../../lib/i18n";
import { useUnlisteners } from "../../../composables/useUnlisteners";

/** 没有版本列表可选的加载器 ID（原版 / 自定义） */
export const NO_VERSION_LOADERS = ["normal", "custom"];

/** 支持列表查询失败时的兜底选项（原版 + 自定义） */
const FALLBACK_LOADERS = NO_VERSION_LOADERS;

/**
 * 加载器联动：某个游戏版本支持哪些加载器，以及该加载器的版本列表
 *
 * - `gameVersion`: 当前游戏版本，变化时自动重查
 * - `isLeaving`: 询问是否正在关窗；关窗后在途结果一律丢弃
 *
 * # 返回值
 *
 * - `loaders` / `loader` / `loaderVersion`: 支持的加载器列表、已选加载器、已选加载器版本
 * - `loaderVersions`: 当前加载器与游戏版本对应的版本列表
 * - `loaderLoading` / `loaderVerLoading`: 两个查询各自的进行中标志
 * - `queryStep` / `queryTotal` / `showQueryProgress`: 支持列表查询进度，给进度条用
 * - `fetchSupportLoaders` / `refreshSupportLoaders` / `refreshLoaderVersions`: 查询与刷新方法
 * - `dropPending` / `syncCloseGuard`: 丢在途查询状态、按当前状态重判关闭保护
 */
export function useLoaders(gameVersion: Ref<string>, isLeaving: () => boolean) {
  const { track } = useUnlisteners();
  /** 当前游戏版本支持的加载器 ID 列表（mml-core 独立 ID，显示名走 i18n） */
  const loaders = ref<string[]>([]);
  /** 已选加载器 ID */
  const loader = ref("normal");
  /** 已选加载器版本（原版 / 自定义为空） */
  const loaderVersion = ref("");
  /** 当前加载器 + 游戏版本对应的版本列表 */
  const loaderVersions = ref<string[]>([]);
  /** 支持列表查询中（查询期间加载器下拉禁用） */
  const loaderLoading = ref(false);
  /** 加载器版本列表拉取中 */
  const loaderVerLoading = ref(false);

  /** 支持列表查询进度：mml-core 每查完一种加载器推一步，给进度条用 */
  const queryStep = ref(0);
  const queryTotal = ref(0);
  const showQueryProgress = ref(false);

  /** 已查询过的游戏版本 → 支持的加载器列表 */
  const supportCache = new Map<string, string[]>();
  /** `loader:mc` → 加载器版本列表（切回旧加载器不重新请求） */
  const versionCache = new Map<string, string[]>();
  /** 正在查询支持列表的游戏版本（防同一版本并发重复查询） */
  let querying = "";

  /** 当前加载器不被支持时回退到原版 */
  function ensureLoaderSupported() {
    if (!loaders.value.includes(loader.value)) {
      loader.value = "normal";
    }
  }

  /**
   * 查询当前游戏版本支持的加载器
   *
   * 无选中版本时清空列表，加载器下拉随之禁用。
   *
   * # 返回值
   *
   * 是否拿到了该版本的有效列表；刷新按钮据此决定要不要提示
   */
  async function fetchSupportLoaders(): Promise<boolean> {
    const mc = gameVersion.value;
    if (!mc) {
      loaders.value = [];
      return false;
    }
    const cached = supportCache.get(mc);
    if (cached) {
      loaders.value = cached;
      ensureLoaderSupported();
      return true;
    }
    if (querying === mc) return false;
    // 查询期间清空旧列表（下拉显示「查询中」并禁用），弹出进度条
    loaders.value = [];
    querying = mc;
    loaderLoading.value = true;
    queryStep.value = 0;
    queryTotal.value = 0;
    showQueryProgress.value = true;
    try {
      const list = await api.addGetSupportLoaders(mc);
      if (isLeaving()) return false;
      supportCache.set(mc, list);
      // 请求期间版本已变化时丢弃过期结果
      if (gameVersion.value === mc) {
        loaders.value = list;
        ensureLoaderSupported();
        return true;
      }
      return false;
    } catch {
      if (isLeaving()) return false;
      // 查询失败不缓存：保留原版 + 自定义兜底，可用刷新按钮重试
      if (gameVersion.value === mc) {
        loaders.value = [...FALLBACK_LOADERS];
        // 兜底列表里没有当前加载器时回退原版，否则下拉显示与表单值会不一致
        ensureLoaderSupported();
      }
      return false;
    } finally {
      if (querying === mc) {
        querying = "";
        loaderLoading.value = false;
        showQueryProgress.value = false;
      }
    }
  }

  /** 刷新支持的加载器：清掉当前版本缓存后重查，成功才提示 */
  async function refreshSupportLoaders() {
    const mc = gameVersion.value;
    if (!mc || loaderLoading.value) return;
    supportCache.delete(mc);
    if (await fetchSupportLoaders()) showToast(t("tip.refreshed"));
  }

  /**
   * 按当前加载器 + 游戏版本拉取版本列表
   *
   * 命中缓存直接返回，不再请求。
   *
   * # 返回值
   *
   * 是否拿到了有效列表；刷新按钮据此决定要不要提示
   */
  async function fetchLoaderVersions(): Promise<boolean> {
    if (NO_VERSION_LOADERS.includes(loader.value) || !gameVersion.value) {
      loaderVersions.value = [];
      loaderVersion.value = "";
      return false;
    }
    const key = `${loader.value}:${gameVersion.value}`;
    const cached = versionCache.get(key);
    if (cached) {
      loaderVersions.value = cached;
      loaderVersion.value = cached[0] ?? "";
      return true;
    }
    loaderVerLoading.value = true;
    // 立即清掉上一个加载器的列表：下拉锁定显示「获取中」，避免新旧列表串显
    loaderVersions.value = [];
    loaderVersion.value = "";
    try {
      // 记录请求参数，返回后对比：期间选择已变化则丢弃过期结果
      const reqLoader = loader.value;
      const reqMc = gameVersion.value;
      const list = await api.addLoaderVersions(reqLoader, reqMc);
      if (isLeaving()) return false;
      versionCache.set(key, list);
      if (loader.value === reqLoader && gameVersion.value === reqMc) {
        loaderVersions.value = list;
        loaderVersion.value = list[0] ?? "";
        return true;
      }
      return false;
    } catch {
      if (isLeaving()) return false;
      // 拉取失败（如数据源不可达）时清空并提示，可用刷新按钮重试
      loaderVersions.value = [];
      loaderVersion.value = "";
      showToast(t("add.loaderVerFail"));
      return false;
    } finally {
      loaderVerLoading.value = false;
    }
  }

  /** 刷新加载器版本列表：清缓存重拉，成功才提示 */
  async function refreshLoaderVersions() {
    if (!gameVersion.value || NO_VERSION_LOADERS.includes(loader.value)) return;
    if (loaderVerLoading.value) return;
    versionCache.delete(`${loader.value}:${gameVersion.value}`);
    if (await fetchLoaderVersions()) showToast(t("tip.refreshed"));
  }

  /** 选版本 / 换加载器时自动重查；用 watch 而非模板事件，避免漏掉程序性赋值 */
  watch(gameVersion, () => {
    fetchSupportLoaders();
  });
  watch([loader, gameVersion], () => {
    fetchLoaderVersions();
  });

  // 查询数据期间开启窗口关闭保护，后端在 CloseRequested 阶段拒绝关闭
  //
  // 只在"这一页正被用户看着、且真的在查"时才拦：这两个查询是后台跑的，单窗口模式下
  // 页面被 KeepAlive 缓存、切走照样跑完；如果只看 loading 标志，用户切去安装整合包、
  // 甚至过一会儿回来关窗口，守卫还按着，就会莫名其妙弹"正在获取数据，暂时无法关闭窗口"。
  // 关窗不该被这种事拦住 —— 真拦下了也只是浪费一次点击。
  watch(
    [loaderLoading, loaderVerLoading],
    () => syncCloseGuard(),
    { immediate: true },
  );

  // 支持列表查询进度（每查完一种加载器推一步）
  onMounted(() => {
    track(
      onAddLoaderProgress((e) => {
        queryStep.value = e.step;
        queryTotal.value = e.total;
      }),
    );
  });

  /** 关窗（不继续添加）时丢掉在途查询状态：否则关闭保护会一直拦着窗口 */
  function dropPending() {
    querying = "";
    loaderLoading.value = false;
    loaderVerLoading.value = false;
    showQueryProgress.value = false;
  }

  /**
   * 按当前状态重新判定关闭保护
   *
   * 页面被切走时调用方会直接把守卫关掉（见 AddInstanceWindow 的 onDeactivated），
   * 切回来得按"这会儿还在不在查"再定一次，否则查询在途时窗口又能被随手关掉。
   */
  function syncCloseGuard() {
    api.setCloseGuard((loaderLoading.value || loaderVerLoading.value) && !isLeaving()).catch(
      () => { },
    );
  }

  return {
    loaders,
    loader,
    loaderVersion,
    loaderVersions,
    loaderLoading,
    loaderVerLoading,
    queryStep,
    queryTotal,
    showQueryProgress,
    fetchSupportLoaders,
    refreshSupportLoaders,
    refreshLoaderVersions,
    dropPending,
    syncCloseGuard,
  };
}
