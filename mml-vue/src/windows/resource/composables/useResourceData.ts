// 资源管理窗口的数据层：当前实例 + 七个分类的列表 + 数据包子页
//
// 分类与存档子页的**选择**也放这里，即 `category` / `saveTab`；加载是跟着它们走的
// （watch 在里面），外壳与各列表组件共用同一份，不必再往上抛事件。
//
// 拆出来的原因：原先这 250 行和模板、样式挤在一个 1100 行的 SFC 里，
// 列表组件想拿数据只能整份 prop 传下去；现在按 `windows/block` 的写法拆开 ——
// 薄窗口 + parts/ + composables/，语义边界清楚。
import { ref, watch } from "vue";
import { locale, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import {
  api,
  getImageBaseUrl,
  listDatapacks,
  listMods,
  listResourcepacks,
  listSaves,
  listScreenshots,
  listServers,
  listShaderpacks,
  listSchematics,
  onModScanProgress,
} from "../../../lib/api";
import { loadGuiConfig } from "../../../lib/guiConfig";
import type {
  DataPackItemDto,
  ModItemDto,
  PackItemDto,
  SaveItemDto,
  SchematicItemDto,
  ScreenshotItemDto,
  ServerItemDto,
  ShaderItemDto,
} from "../../../lib/bindings";
import type { CategoryId, SaveTab } from "../types";

/** 左侧导航上显示的当前实例（只需要名字与版本） */
export interface ResourceInstance {
  /** 实例名，来自内核的实例列表 */
  name: string;
  /** 游戏版本，如 `1.20.1` */
  version: string;
}

/**
 * 资源管理窗口的数据层
 *
 * - `initialCategory`: 进入时默认打开的类别，缺省 `saves`
 *
 * # 返回值
 *
 * - `category` / `saveTab`: 当前分类与存档子页，外壳与各面板共用这一份
 * - `instance` / `instanceUuid`: 当前实例的名字版本与 uuid
 * - `loading`: 有列表正在拉
 * - `modProgress`: 模组扫描进度
 * - `mods` / `packs` / `saves` / `shots` / `servers` / `shaders` / `schematics` / `datapacks`: 各分类的列表
 * - `dpSave`: 数据包子页当前选中的存档目录
 * - `reloadCurrent`: 重拉当前显示的那一份
 * - `sync`: 读当前实例与视图偏好，再拉一次列表
 * - `setInstanceReadyHook`: 注册实例就绪回调，外壳用它先恢复视图偏好
 * - `shotUrl`: 拼某个截图的预览地址
 */
export function useResourceData(initialCategory: CategoryId = "saves") {
  const category = ref<CategoryId>(initialCategory);
  const saveTab = ref<SaveTab>("saves");

  const instance = ref<ResourceInstance | null>(null);
  /** 目标实例 uuid：后端每个命令都要它；没有实例时为空串，加载直接跳过 */
  const instanceUuid = ref("");

  /** 有列表正在拉；模组那个分类要解析 jar 元数据，可能要几秒 */
  const loading = ref(false);

  const mods = ref<ModItemDto[]>([]);
  const packs = ref<PackItemDto[]>([]);
  const saves = ref<SaveItemDto[]>([]);
  const shots = ref<ScreenshotItemDto[]>([]);
  const servers = ref<ServerItemDto[]>([]);
  const shaders = ref<ShaderItemDto[]>([]);
  const schematics = ref<SchematicItemDto[]>([]);
  const datapacks = ref<DataPackItemDto[]>([]);

  /** 数据包子页当前选中的存档目录 */
  const dpSave = ref("");

  /** 截图预览协议前缀，形如 `mml-image://...` */
  const imgBase = ref("");

  /**
   * 加载代次号
   *
   * 分类可以连点，而每次加载都要等后端（模组那次尤其慢）。只有**最新一发**的结果与
   * 收尾算数：否则先发的那一发回来晚了会把 `loading` 提前清掉、把界面闪成空态。
   * 与仓库里 `sourceSeq` 修排序竞态是同一做法。
   */
  let loadSeq = 0;

  /**
   * 模组扫描进度：`done` / `total`，`total` 为 0 表示不在扫描
   *
   * 只有模组这一档需要：它要给每个 jar 解析元数据（几百个包好几秒），
   * 其它分类是读个清单就完了，用不上。
   */
  const modProgress = ref<{ done: number; total: number }>({ done: 0, total: 0 });

  /** 模组扫描进度的退订函数；每次拉列表时挂上，拿到结果就摘掉 */
  let unlistenProgress: (() => void) | null = null;

  function stopModProgress() {
    modProgress.value = { done: 0, total: 0 };
    unlistenProgress?.();
    unlistenProgress = null;
  }

  /** 按当前分类拉列表；每次切换 / 操作后都重拉，保证与磁盘一致 */
  async function load() {
    if (!instanceUuid.value) return;
    const seq = ++loadSeq;
    loading.value = true;
    try {
      switch (category.value) {
        case "mods":
          // 扫描开始才订阅进度，事件由后端在扫描期间逐文件发出；
          // 先订阅再调命令，否则可能漏掉最开始那几条
          modProgress.value = { done: 0, total: 0 };
          unlistenProgress?.();
          unlistenProgress = await onModScanProgress((e) => {
            // 只认最新一发：旧的那一发回来晚了会把进度写回去（与 loadSeq 同一意图）
            if (seq === loadSeq) modProgress.value = e;
          });
          try {
            mods.value = await listMods(instanceUuid.value);
          } finally {
            stopModProgress();
          }
          break;
        case "resourcepacks":
          // 带上界面语言：简介写成 `translate` 组件时，后端按它查资源包自带的语言表
          packs.value = await listResourcepacks(instanceUuid.value, locale.value);
          break;
        case "saves":
          saves.value = await listSaves(instanceUuid.value);
          break;
        case "screenshots":
          shots.value = await listScreenshots(instanceUuid.value);
          break;
        case "servers":
          servers.value = await listServers(instanceUuid.value);
          break;
        case "shaders":
          shaders.value = await listShaderpacks(instanceUuid.value);
          break;
        case "schematics":
          schematics.value = await listSchematics(instanceUuid.value);
          break;
      }
    } catch (e) {
      if (seq === loadSeq) showToast(tErr(e));
    } finally {
      // 慢的那一发回来时可能已经有更新的一发了：别把它的加载态关掉
      if (seq === loadSeq) loading.value = false;
    }
  }

  /** 数据包列表，依赖子页选中的存档 */
  async function loadDatapacks() {
    if (!instanceUuid.value || !dpSave.value) return;
    const seq = ++loadSeq;
    loading.value = true;
    try {
      datapacks.value = await listDatapacks(instanceUuid.value, dpSave.value);
    } catch (e) {
      if (seq === loadSeq) {
        datapacks.value = [];
        showToast(tErr(e));
      }
    } finally {
      if (seq === loadSeq) loading.value = false;
    }
  }

  /** 当前显示的是数据包子页，重拉时要挑对那一份加载函数 */
  function isDatapackTab(): boolean {
    return category.value === "saves" && saveTab.value === "datapacks";
  }

  /** 重拉**当前显示的东西**：操作成功之后调用（分类列表 / 数据包子页） */
  async function reloadCurrent() {
    if (isDatapackTab()) {
      await loadDatapacks();
    } else {
      await load();
    }
  }

  /** 进入数据包子页：先保证存档列表在、选中一个存档，再拉数据包 */
  async function enterDatapackTab() {
    if (!instanceUuid.value) return;
    if (!saves.value.length) await load();
    if (!dpSave.value && saves.value.length) {
      // 选中变化会触发下面的 watch，由它去拉数据包
      dpSave.value = saves.value[0].dir;
      return;
    }
    await loadDatapacks();
  }

  /**
   * 实例变化时的回调，由外壳挂上"读该实例的视图偏好"
   *
   * 放在这儿是因为**只有这里才知道实例什么时候变**，sync 里读 gui_config。
   * 不用 `watch(instanceUuid)`：偏好要在 `reloadCurrent()` **之前**就位，
   * 否则第一次会按旧顺序/旧类别拉一遍列表，看着像闪了一下。
   */
  let onInstanceReady: (() => Promise<void>) | null = null;

  /** 注册实例就绪回调（见上） */
  function setInstanceReadyHook(hook: () => Promise<void>) {
    onInstanceReady = hook;
  }

  /**
   * 正在 sync 中（实例刚就绪、偏好刚恢复）
   *
   * 恢复"上次类别"会改 `category`，那会触发下面的 `watch(category)` → `load()`；
   * 而 `sync()` 结尾还要 `reloadCurrent()` 一次 —— **同一份列表被拉两遍**，
   * 模组那一档就是"解析 401 个 jar 两次"：用户看到进度跑完 401/401 又从 0 来一遍。
   *
   * 所以 sync 期间让 watch 只更新选择、不触发加载，最后由 sync 自己统一拉一次。
   */
  let syncing = false;

  /** 当前实例 + 当前分类数据；首次挂载与切回本窗口都走这里 */
  async function sync() {
    syncing = true;
    try {
      // 当前实例取自 gui_config.json（主窗口选中时写入），本窗口是独立 webview，需自己读一次
      const [list, cfg] = await Promise.all([api.getInstances(), loadGuiConfig()]);
      const uuid = cfg?.mainWindow.selectedInstance ?? "";
      const found = list.find((i) => i.uuid === uuid) ?? list[0] ?? null;
      instance.value = found ? { name: found.name, version: found.version } : null;
      instanceUuid.value = found?.uuid ?? "";
      imgBase.value = await getImageBaseUrl();
    } catch {
      instance.value = null;
      instanceUuid.value = "";
    }
    // 先把该实例的视图偏好读回来，即分类顺序 / 上次类别 / 模组视图，再拉列表
    if (onInstanceReady) await onInstanceReady();
    syncing = false;
    // 重拉**当前显示的那一份**；停在数据包子页时不该去拉存档列表
    await reloadCurrent();
  }

  /** 截图地址，走 `mml-image` 协议按实例 + 文件名取 */
  function shotUrl(item: ScreenshotItemDto): string {
    return `${imgBase.value}/screenshot/${instanceUuid.value}/${item.name}`;
  }

  watch(category, () => {
    // sync 期间只更新选择，不触发加载，否则同一份列表会被拉两遍；
    // 那一遍由 sync 结尾的 reloadCurrent 统一做，见上面 `syncing` 的说明
    if (syncing) return;
    // 停在数据包子页时切分类再切回来：要按子页那套重新进入，别把存档列表当数据包拉
    if (isDatapackTab()) {
      void enterDatapackTab();
      return;
    }
    void load();
  }); watch(saveTab, (tab) => {
    if (tab === "datapacks") void enterDatapackTab();
  });
  watch(dpSave, () => void loadDatapacks());

  return {
    category,
    saveTab,
    instance,
    instanceUuid,
    loading,
    modProgress,
    mods,
    packs,
    saves,
    shots,
    servers,
    shaders,
    schematics,
    datapacks,
    dpSave,
    reloadCurrent,
    sync,
    setInstanceReadyHook,
    shotUrl,
  };
}
