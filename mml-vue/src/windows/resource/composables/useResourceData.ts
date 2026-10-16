// 资源管理窗口的数据层：当前实例 + 七个分类的列表 + 数据包子页
//
// 分类与存档子页的**选择**也放这里，即 `category` / `saveTab`；加载是跟着它们走的
// （watch 在里面），外壳与各列表组件共用同一份，不必再往上抛事件。
// 拉过的分类留在窗口里、切回来不重读（模组那一档重扫一次要解析几百个 jar），
// 见 loadedCats；要跟磁盘对齐走 reloadCurrent。
//
// 本窗口**一个实例一个**：实例是窗口身份的一部分，打开时就锁定、窗口内不再选实例。
// 单窗口模式下从本页开出去的「配置文件」「添加资源」返回时跳转参数已被清掉，
// 那时按主窗口当前选中的实例兜底，见 sync。
//
// 拆出来的原因：原先这 250 行和模板、样式挤在一个 1100 行的 SFC 里，
// 列表组件想拿数据只能整份 prop 传下去；现在按 `windows/block` 的写法拆开 ——
// 薄窗口 + parts/ + composables/，语义边界清楚。
import { computed, ref, watch } from "vue";
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
  onModsDuplicate,
} from "../../../lib/api";
import { takeResourceListDirty } from "../../../lib/resourceTasks";
import { loadGuiConfig } from "../../../lib/guiConfig";
import { familyInstanceFromLabel, targetUuid } from "../../windowManager";
import type {
  DataPackItemDto,
  ModDuplicateDto,
  ModItemDto,
  PackItemDto,
  SaveItemDto,
  SchematicItemDto,
  ScreenshotItemDto,
  ServerItemDto,
  ShaderItemDto,
} from "../../../lib/bindings";
import type { CategoryId, SaveTab } from "../types";

/** 左侧导航上显示的当前实例（名字、游戏版本与加载器） */
export interface ResourceInstance {
  /** 实例名，来自内核的实例列表 */
  name: string;
  /** 游戏版本，如 `1.20.1` */
  version: string;
  /** 加载器标识，如 `neoforge`；原版是 `normal` */
  loader: string;
  /** 加载器版本；原版 / 没记时是 `null` */
  loaderVersion: string | null;
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
 * - `modDuplicates`: 重复安装的模组，非空时模组面板自动弹窗
 * - `mods` / `packs` / `saves` / `shots` / `servers` / `shaders` / `schematics` / `datapacks`: 各分类的列表
 * - `dpSave`: 数据包子页当前选中的存档目录
 * - `reloadCurrent`: 重拉当前显示的那一份
 * - `dismissModDuplicates`: 关掉重复安装弹窗
 * - `sync`: 读当前实例与视图偏好，必要时重读列表（换实例，或列表已被下载标记为过期）
 * - `setInstanceReadyHook`: 注册实例就绪回调，外壳用它先恢复视图偏好
 * - `shotUrl`: 拼某个截图的预览地址
 */
export function useResourceData(initialCategory: CategoryId = "saves") {
  const category = ref<CategoryId>(initialCategory);
  const saveTab = ref<SaveTab>("saves");

  const instance = ref<ResourceInstance | null>(null);
  /** 目标实例 uuid：后端每个命令都要它；没有实例时为空串，加载直接跳过 */
  const instanceUuid = ref("");

  /**
   * 本窗口锁定的实例 uuid，空串表示没有
   *
   * 多窗口模式按窗口标签，那是后端派生窗口身份时用的同一份实例；单窗口模式按本次跳转
   * 带入的 windowParams；浏览器预览没有标签，同样退回 URL / windowParams 上的参数。
   */
  const lockedUuid = computed(() => familyInstanceFromLabel("resource") ?? targetUuid() ?? "");

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

  /**
   * 已经拉过列表的分类
   *
   * 切分类**不重读**：模组那一档要给几百个 jar 解析元数据，来回切一次就重扫一遍。
   * 新鲜度靠"操作后重拉"与顶栏刷新按钮保证（两者都走 reloadCurrent，是强制重拉），
   * 换实例时整份作废，见 sync。
   */
  const loadedCats = new Set<CategoryId>();

  /** 数据包列表当前对应哪个存档；换存档、换实例或操作后都要重拉 */
  let loadedDpSave = "";

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

  /**
   * 重复安装的 modid，非空时模组面板自动弹窗
   *
   * 列表加载完成时由后端事件推来；用户关掉后清空，下次重拉列表时后端会再报一次
   */
  const modDuplicates = ref<ModDuplicateDto[]>([]);

  /** 关掉重复安装弹窗 */
  function dismissModDuplicates() {
    modDuplicates.value = [];
  }

  /** 模组扫描进度的退订函数；每次拉列表时挂上，拿到结果就摘掉 */
  let unlistenProgress: (() => void) | null = null;
  /** 重复安装提示的退订函数，与进度同一发：拉列表时挂上，拿到结果就摘掉 */
  let unlistenDuplicate: (() => void) | null = null;

  function stopModScan() {
    modProgress.value = { done: 0, total: 0 };
    unlistenProgress?.();
    unlistenProgress = null;
    unlistenDuplicate?.();
    unlistenDuplicate = null;
  }

  /** 按分类拉列表；操作后重拉、点刷新都走这里，切分类走 loadIfNeeded */
  async function load() {
    if (!instanceUuid.value) return;
    // 拉的过程中用户可能又切了分类：这一发算在**发起时**那一档上
    const target = category.value;
    const seq = ++loadSeq;
    loading.value = true;
    let ok = false;
    try {
      switch (target) {
        case "mods":
          // 扫描开始才订阅，事件由后端在扫描期间 / 结束时发出；
          // 先订阅再调命令，否则可能漏掉最开始那几条
          modProgress.value = { done: 0, total: 0 };
          modDuplicates.value = [];
          unlistenProgress?.();
          unlistenProgress = await onModScanProgress((e) => {
            // 只认最新一发：旧的那一发回来晚了会把进度写回去（与 loadSeq 同一意图）
            if (seq === loadSeq) modProgress.value = e;
          });
          unlistenDuplicate?.();
          unlistenDuplicate = await onModsDuplicate((list) => {
            if (seq === loadSeq) modDuplicates.value = list;
          });
          try {
            mods.value = await listMods(instanceUuid.value);
          } finally {
            stopModScan();
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
      ok = true;
    } catch (e) {
      if (seq === loadSeq) showToast(tErr(e));
    } finally {
      // 慢的那一发回来时可能已经有更新的一发了：别把它的加载态关掉
      if (seq === loadSeq) loading.value = false;
    }
    // 拉成了才算这一档"已加载"；失败不记，下次切回来还能再试
    if (ok) loadedCats.add(target);
  }

  /** 这一档还没有列表时才拉：切分类走这里，操作后重拉 / 点刷新走 reloadCurrent */
  async function loadIfNeeded() {
    if (loadedCats.has(category.value)) return;
    await load();
  }

  /** 数据包列表，依赖子页选中的存档 */
  async function loadDatapacks() {
    if (!instanceUuid.value || !dpSave.value) return;
    // 与 load 同一道理：这一发算在**发起时**那个存档上，中途换存档不能记到新的头上
    const save = dpSave.value;
    const seq = ++loadSeq;
    loading.value = true;
    try {
      datapacks.value = await listDatapacks(instanceUuid.value, save);
      loadedDpSave = save;
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
    // 存档列表是这一页的前提：这一档还没拉过就先拉
    if (!loadedCats.has("saves")) await load();
    if (!dpSave.value && saves.value.length) {
      // 选中变化会触发下面的 watch，由它去拉数据包
      dpSave.value = saves.value[0].dir;
      return;
    }
    // 同一个存档的数据包列表拉过就留着：切去别的分类再切回来不重读
    if (loadedDpSave !== dpSave.value) await loadDatapacks();
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
   * 恢复"上次类别"会改 `category`，那会触发下面的 `watch(category)` → `loadIfNeeded()`；
   * 而 `sync()` 结尾自己还要拉一次 —— **同一份列表被拉两遍**，
   * 模组那一档就是"解析 401 个 jar 两次"：用户看到进度跑完 401/401 又从 0 来一遍。
   *
   * 所以 sync 期间让 watch 只更新选择、不触发加载，最后由 sync 自己统一拉一次。
   */
  let syncing = false;

  /** 当前实例 + 当前分类数据；首次挂载与切回本窗口都走这里 */
  async function sync() {
    syncing = true;
    // 换没换实例要在改写之前记下来，见下面"整份作废"
    const prevUuid = instanceUuid.value;
    try {
      const list = await api.getInstances();
      // 锁定为空时退回主窗口当前选中的实例：单窗口模式下从本页开出去的「配置文件」
      // 「添加资源」返回时跳转参数已被清掉，读它就是改造前的行为。
      // 列表里**没有**这个实例时不再退到第一个实例：窗口锁到哪个实例就显示哪个，
      // 实例被删掉时该显示"未选择"，而不是偷偷换成别人的数据
      const uuid = lockedUuid.value || (await loadGuiConfig())?.mainWindow.selectedInstance || "";
      const found = list.find((i) => i.uuid === uuid) ?? null;
      instance.value = found
        ? {
            name: found.name,
            version: found.version,
            loader: found.loader,
            loaderVersion: found.loaderVersion,
          }
        : null;
      instanceUuid.value = found?.uuid ?? "";
      imgBase.value = await getImageBaseUrl();
    } catch {
      instance.value = null;
      instanceUuid.value = "";
    }
    // 换实例：上一份列表属于别的实例，整份作废。同一个实例内切窗口回来则留着，
    // 要不要重读由下面那一段判断
    const instanceChanged = instanceUuid.value !== prevUuid;
    if (instanceChanged) {
      loadedCats.clear();
      loadedDpSave = "";
      modDuplicates.value = [];
    }
    try {
      // 先把该实例的视图偏好读回来，即分类顺序 / 上次类别 / 模组视图，再决定拉什么
      if (onInstanceReady) await onInstanceReady();
      // 同一个实例内切回本窗口**不重读**：模组那一档要给几百个 jar 解析元数据，
      // 白扫一次要好几秒。只有换了实例、或期间有资源下载完成（磁盘真变了）才重拉；
      // 强制跟磁盘对齐是另一回事，走 reloadCurrent（顶栏刷新按钮与各操作之后都调它）。
      // 不重读时也要补上"当前这一档还没有数据"的情况：偏好恢复可能刚改过 category，
      // 而那一发 watch 被 syncing 压住了
      if (instanceChanged || takeResourceListDirty()) {
        await reloadCurrent();
      } else if (isDatapackTab()) {
        await enterDatapackTab();
      } else {
        await loadIfNeeded();
      }
    } finally {
      // 无论成败都要放开：卡住的话之后切分类就再也不加载了
      syncing = false;
    }
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
    // 拉过的分类留在窗口里，切回来不再重读（见 loadedCats）
    void loadIfNeeded();
  }); watch(saveTab, (tab) => {
    if (tab === "datapacks") void enterDatapackTab();
  });
  watch(dpSave, () => void loadDatapacks());

  // 单窗口模式：已经停在本页上又开了另一个实例 —— 没有页面切换，也就没有 useWindowRefresh，
  // 上一份列表属于别的实例，得在这儿整份作废重读。
  // 只认"从一个有效实例换到另一个"：切走时跳转参数会被清空，lockedUuid 先变空、切回来再变
  // 有效，这两拍都由 useWindowRefresh 负责，这里跟着跑一遍等于把模组列表重扫一次
  watch(lockedUuid, (now, before) => {
    if (!now || !before || now === before) return;
    void sync();
  });

  return {
    category,
    saveTab,
    instance,
    instanceUuid,
    loading,
    modProgress,
    modDuplicates,
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
    dismissModDuplicates,
    sync,
    setInstanceReadyHook,
    shotUrl,
  };
}
