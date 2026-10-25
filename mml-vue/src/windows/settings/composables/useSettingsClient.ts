// 设置窗口 · 客户端设置标签：服务器 / MOTD / 登录方式锁定 / 实例锁定 / 自定义主页面
//
// 这一片基本都是 `gui_config.client`（即改即存，整对象提交），加上两份"外部状态"：
// 实例列表（锁定下拉的候选，进页面时重拉）与自定义主页面的导入状态。
// 游戏标题那组虽然显示在这个标签里，但数据属于 `window`，由 useSettingsLaunch 持有。
import { computed, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { commands, type CustomHomeInfoDto, type CustomUiInfoDto, type InstanceInfoDto } from "../../../lib/bindings";
import {
  defaultConfig,
  loadGuiConfig,
  saveGuiConfig,
  type ClientConfig,
  type ExperimentalConfig,
  type LaunchCheckConfig,
} from "../../../lib/guiConfig";
import { applyWindowAlpha, applyWindowMaterial } from "../../../lib/appearance";
import { typeLabelKey } from "../../../lib/accountStore";
import { useSaveState } from "./useSaveState";
import { useInstanceChange } from "../../../composables/useInstanceChange";

/** 登录方式锁定的候选（复用账户添加类型的文案） */
const LOGIN_TYPES = ["offline", "microsoft", "littleskin", "selflittleskin", "authlib", "nide8"];

/** 需要服务器信息的类型：外置登录 / 自定义皮肤站填服务器地址，统一通行证填服务器 ID；
 *  这些类型可重复添加，条目带「登录模型名字」，添加账户时直接下拉选择 */
const SERVER_LOCK_TYPES = ["authlib", "selflittleskin", "nide8"];

/**
 * 客户端设置的共享状态与保存：客户端设置页与界面页的窗口外观都用这一份
 *
 * # 返回值
 *
 * - `client`: `gui_config.client` 的副本，即改即存
 * - `serverAddr`: 自动进服与 MOTD 共用的服务器地址，可双向绑定
 * - `loginTypeLabel` / `addLock*` / `lockServerError` / `lockServerPlaceholder`: 登录方式锁定的添加表单
 * - `lockInstances` / `refreshLockInstances` / `lockMissing` / `onLockInstanceChange`: 实例锁定
 * - `customHome` / `importingCustomHome` / `customHomeState` / `refreshCustomHome` / `importCustomHome` / `removeCustomHome`: 自定义主页面
 * - `customUi` / `customPageState` / `refreshCustomUi` / `importCustomPage` / `removeCustomPage`: 完全自定义界面的整页包
 * - `addCustomCss` / `setCustomCssEnabled` / `moveCustomCss` / `removeCustomCss` / `openCustomUiDir`: 自定义 CSS 的安装、启停、排序与删除
 * - `pickMusicFile`: 选一个本地音频文件填进"自定义音乐"
 * - `experimental` / `applyExperimental`: 实验性功能四开关（界面先行，行为未接线）
 * - `applyWindowOpacity`: 透明度写进 CSS 变量并落盘，界面页的窗口外观用
 * - `load` / `applyClient` / `saveState` / `resetGroup`: 加载、落盘与分组级恢复默认
 */
export function useSettingsClient() {
  const client = ref<ClientConfig>({ ...defaultConfig().client });
  /** 实验性功能四个开关：界面先行，行为未接线 */
  const experimental = ref<ExperimentalConfig>({ ...defaultConfig().experimental });
  /** 启动前检查开关：加载器 / 内存 */
  const launchCheck = ref<LaunchCheckConfig>({ ...defaultConfig().launchCheck });
  const { state, track } = useSaveState();

  // ---------- 加载 ----------

  async function load() {
    const cfg = await loadGuiConfig();
    if (cfg?.client) client.value = { ...cfg.client };
    if (cfg?.experimental) experimental.value = { ...cfg.experimental };
    if (cfg?.launchCheck) launchCheck.value = { ...cfg.launchCheck };
  }

  // ---------- 即改即存 ----------

  /** 整份 client 提交（与原来一致）；失败时把保存状态标红 */
  async function applyClient() {
    try {
      await track(() => saveGuiConfig({ client: { ...client.value } }));
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 设置密码并锁定客户端设置 */
  async function setClientLock(password: string) {
    try {
      await commands.windows.clientLockSet(password);
      await load();
    } catch (e) {
      showToast(tErr(e));
      throw e;
    }
  }

  /** 解除客户端设置锁定 */
  async function clearClientLock() {
    try {
      await commands.windows.clientLockClear();
      await load();
    } catch (e) {
      showToast(tErr(e));
      throw e;
    }
  }

  /** 校验解锁密码；命令失败按不通过处理 */
  async function verifyClientLock(password: string) {
    return commands.windows.clientLockVerify(password).catch(() => false);
  }

  /** 透明度改完立刻反映到本窗口，再落盘；别的窗口靠 client-config-change 自己跟上 */
  function applyWindowOpacity(v: number) {
    applyWindowAlpha(v);
    void applyClient();
  }

  /** 材质改用同一个口径：本窗口先换表面透明度，再落盘广播 */
  function applyWindowEffect() {
    applyWindowMaterial(client.value.windowEffect);
    void applyClient();
  }

  /** 整份 experimental 提交；同样失败时把保存状态标红 */
  async function applyExperimental() {
    try {
      await track(() => saveGuiConfig({ experimental: { ...experimental.value } }));
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 整份启动前检查开关提交 */
  async function applyLaunchCheck() {
    try {
      await track(() => saveGuiConfig({ launchCheck: { ...launchCheck.value } }));
    } catch (e) {
      showToast(tErr(e));
    }
  }

  // ---------- 服务器地址：自动进服与 MOTD 共用 ----------

  const serverAddr = computed({
    get: () => client.value.motdServer,
    set: (v: string) => {
      client.value.motdServer = v;
      client.value.autoJoinServer = v;
      void applyClient();
    },
  });

  // ---------- 登录方式锁定 ----------

  const addLockType = ref<string>("offline");
  const addLockName = ref("");
  const addLockServer = ref("");
  /** 非空 = 服务器输入框红框 + 提示文字 */
  const lockServerError = ref("");

  const addLockHasServer = computed(() => SERVER_LOCK_TYPES.includes(addLockType.value));

  /** 带服务器的类型始终可加（可添加不同地址）；其余类型加过就不再出现在下拉里 */
  const addLockOptions = computed(() =>
    LOGIN_TYPES.filter(
      (ty) =>
        SERVER_LOCK_TYPES.includes(ty) || !client.value.loginLock.some((e) => e.ty === ty),
    ),
  );

  /** 登录类型标签：key 走 accountStore 的标准映射，别手拼 */
  function loginTypeLabel(ty: string): string {
    return t(typeLabelKey(ty));
  }

  /** 服务器输入框的占位文案 */
  const lockServerPlaceholder = computed(() =>
    addLockType.value === "nide8"
      ? t("winSettings.lockNide8ServerHint")
      : addLockType.value === "selflittleskin"
        ? t("winSettings.lockSelfServerHint")
        : t("winSettings.lockAuthlibServerHint"),
  );

  /** 校验服务器输入：通过则清空错误，否则把原因写进 `lockServerError` */
  function validateLockServer(): boolean {
    if (!addLockHasServer.value) {
      lockServerError.value = "";
      return true;
    }
    const v = addLockServer.value.trim();
    if (!v) {
      lockServerError.value = t("winSettings.loginLockServerRequired");
      return false;
    }
    if (client.value.loginLock.some((e) => e.ty === addLockType.value && e.server === v)) {
      lockServerError.value = t("winSettings.loginLockServerDup");
      return false;
    }
    lockServerError.value = "";
    return true;
  }

  /** 添加一个锁定条目 */
  function addLock() {
    const ty = addLockType.value;
    if (!ty) return;
    if (!validateLockServer()) return;
    const name = addLockName.value.trim();
    if (addLockHasServer.value && !name) {
      lockServerError.value = t("winSettings.loginLockNameRequired");
      return;
    }
    client.value.loginLock = [
      ...client.value.loginLock,
      { ty, name, server: addLockServer.value.trim() },
    ];
    addLockName.value = "";
    addLockServer.value = "";
    lockServerError.value = "";
    void applyClient();
  }

  function removeLock(i: number) {
    client.value.loginLock.splice(i, 1);
    void applyClient();
  }

  // ---------- 实例锁定 ----------

  const lockInstances = ref<InstanceInfoDto[]>([]);

  /** 拉取实例列表；进客户端设置页时刷新，避免锁定的实例已被删掉 */
  async function refreshLockInstances() {
    lockInstances.value = await commands.main.getInstances().catch(() => []);
  }

  // 实例在别的窗口里被增删改：锁定的下拉跟着变，删掉的那条不再出现
  useInstanceChange(refreshLockInstances);

  /** 已保存的锁定 uuid 在实例列表里找不到（实例被删 / 改名）：下拉框额外补一条选中项 */
  const lockMissing = computed(
    () =>
      !!client.value.lockInstance &&
      !lockInstances.value.some((i) => i.uuid === client.value.lockInstance),
  );

  /** 切换锁定的实例（空串 = 不锁定） */
  function onLockInstanceChange(uuid: string) {
    client.value.lockInstance = uuid;
    void applyClient();
  }

  // ---------- 自定义主页面 ----------

  /** 已导入情况；未导入或后端不可用时为 null */
  const customHome = ref<CustomHomeInfoDto | null>(null);
  /** 导入进行中；导入只是校验加复制压缩包，通常瞬间完成 */
  const importingCustomHome = ref(false);

  /** 状态行：未导入 / 已保存压缩包（n 个文件）/ 启用中 */
  const customHomeState = computed(() => {
    const info = customHome.value;
    if (!info?.installed) return t("winSettings.customHomeNone");
    const parts = [t("winSettings.customHomeInstalled", { count: info.fileCount })];
    if (client.value.customHome) parts.push(t("winSettings.customHomeEnabled"));
    return parts.join(" · ");
  });

  /** 重拉状态（进页面 / 删除后调用） */
  async function refreshCustomHome() {
    customHome.value = await commands.customHome.status().catch(() => null);
  }

  /** 选一个 zip 导入；只保存压缩包，页面请求时从包里现读，不落盘解压 */
  async function importCustomHome() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("winSettings.customHomeImport"),
      multiple: false,
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (typeof picked !== "string") return;
    importingCustomHome.value = true;
    try {
      customHome.value = await commands.customHome.import(picked);
    } catch (e) {
      showToast(`${t("winSettings.customHomeImportFailed")}${t("common.detailSep")}${tErr(e)}`);
    } finally {
      importingCustomHome.value = false;
    }
  }

  /** 选一个音频文件填进"自定义音乐"；只把路径写进配置，播放由标题栏那枚按钮管 */
  async function pickMusicFile() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("winSettings.musicPick"),
      multiple: false,
      filters: [{ name: t("winSettings.musicFilter"), extensions: ["mp3", "m4a", "wav", "ogg", "flac"] }],
    });
    if (typeof picked !== "string") return;
    client.value.musicSource = picked;
    await applyClient();
  }

  /** 删除已保存的包；配置里的启用开关不动，主窗口会自动回落到内置主页 */
  async function removeCustomHome() {
    try {
      await commands.customHome.remove();
      await refreshCustomHome();
    } catch (e) {
      showToast(tErr(e));
    }
  }

  // ---------- 自定义界面 ----------
  //
  // CSS 条目与两个开关都是 gui_config.client 的字段，由这里整份提交；后端只负责
  // 它拿不到的文件读写。整页包的已导入情况要问后端，存在 customUi 里。

  /** 整页包状态；CSS 条目的名字与开关直接看 client */
  const customUi = ref<CustomUiInfoDto | null>(null);
  /** 整页包导入进行中 */
  const importingCustomPage = ref(false);

  /** 整页包状态行：已导入（n 个文件）/ 未导入 */
  const customPageState = computed(() => {
    const info = customUi.value;
    if (!info?.pageInstalled) return t("winSettings.customPageNone");
    return t("winSettings.customPageInstalled", { count: info.pageFileCount });
  });

  /** 重拉整页包状态：进页面、导入或删除之后调用 */
  async function refreshCustomUi() {
    customUi.value = await commands.customUi.status().catch(() => null);
  }

  /** 选一个 CSS 包装上；文件由后端落盘，条目追加进配置并保存 */
  async function addCustomCss() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("winSettings.customCssImport"),
      multiple: false,
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (typeof picked !== "string") return;
    try {
      const item = await commands.customUi.importCss(picked);
      client.value.customCss = [...client.value.customCss, item];
      await applyClient();
    } catch (e) {
      showToast(`${t("winSettings.customUiImportFailed")}${t("common.detailSep")}${tErr(e)}`);
    }
  }

  /** 改某条 CSS 的启用状态；停用的条目保留文件，只是不再注入 */
  function setCustomCssEnabled(id: string, enabled: boolean) {
    client.value.customCss = client.value.customCss.map((item) =>
      item.id === id ? { ...item, enabled } : item,
    );
    void applyClient();
  }

  /**
   * 调整一份 CSS 的注入顺序，列表顺序就是注入顺序
   *
   * - `id`: 条目 id
   * - `delta`: 正数往后、负数往前；越界时不动
   */
  function moveCustomCss(id: string, delta: number) {
    const list = [...client.value.customCss];
    const from = list.findIndex((item) => item.id === id);
    const to = from + delta;
    if (from < 0 || to < 0 || to >= list.length) return;
    const [moved] = list.splice(from, 1);
    list.splice(to, 0, moved);
    client.value.customCss = list;
    void applyClient();
  }

  /** 删除一条 CSS：先删文件，再从配置里去掉；文件删失败就不动配置 */
  async function removeCustomCss(id: string) {
    try {
      await commands.customUi.removeCss(id);
      client.value.customCss = client.value.customCss.filter((item) => item.id !== id);
      await applyClient();
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 导入整页包：只能有一份，再导入就是换掉旧的 */
  async function importCustomPage() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("winSettings.customPageImport"),
      multiple: false,
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (typeof picked !== "string") return;
    importingCustomPage.value = true;
    try {
      customUi.value = await commands.customUi.importPage(picked);
    } catch (e) {
      showToast(`${t("winSettings.customUiImportFailed")}${t("common.detailSep")}${tErr(e)}`);
    } finally {
      importingCustomPage.value = false;
    }
  }

  /** 删除整页包；开关不动，删掉后主窗口回内置界面 */
  async function removeCustomPage() {
    try {
      customUi.value = await commands.customUi.removePage();
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 在文件管理器里打开 custom_ui/ */
  async function openCustomUiDir() {
    try {
      await commands.customUi.openDir();
    } catch (e) {
      showToast(tErr(e));
    }
  }

  // ---------- 恢复默认 ----------

  /** 这一片全部属于 gui_config.client，默认值取前端镜像 guiConfig.defaultConfig */

  async function resetGroup(id: string): Promise<boolean> {
    const d = defaultConfig().client;
    switch (id) {
      case "servers":
        client.value.autoJoin = d.autoJoin;
        client.value.autoJoinServer = d.autoJoinServer;
        client.value.motdServer = d.motdServer;
        client.value.motdCard = d.motdCard;
        client.value.motdInterval = d.motdInterval;
        break;
      case "window-look":
        client.value.windowEffect = d.windowEffect;
        client.value.windowOpacity = d.windowOpacity;
        break;
      case "loginLock":
        client.value.loginLockOn = d.loginLockOn;
        client.value.loginLock = [];
        break;
      case "instanceLock":
        client.value.lockInstance = d.lockInstance;
        break;
      case "autoSave":
        client.value.autoSaveNotice = d.autoSaveNotice;
        break;
      case "customHome":
        // 只把开关恢复成默认；导入的压缩包是用户的东西，恢复默认不动它
        client.value.customHome = d.customHome;
        break;
      case "music":
        client.value.musicOn = d.musicOn;
        client.value.musicSource = d.musicSource;
        break;
      case "customCss":
        // 只恢复开关；已安装的样式是用户的东西，恢复默认不动它
        client.value.customCssOn = d.customCssOn;
        break;
      case "customPage":
        // 同上：整页包保留，只把开关关掉
        client.value.customPageOn = d.customPageOn;
        break;
      case "launchCheck":
        launchCheck.value = { ...defaultConfig().launchCheck };
        break;
      default:
        return false;
    }
    await applyClient();
    // 透明度是纯 CSS 变量，恢复默认后要把窗口里的实际效果一起改回来
    if (id === "window-look") {
      applyWindowAlpha(client.value.windowOpacity);
      applyWindowMaterial(client.value.windowEffect);
    }
    return true;
  }

  return {
    client,
    applyClient,
    setClientLock,
    clearClientLock,
    verifyClientLock,
    applyWindowOpacity,
    applyWindowEffect,
    experimental,
    applyExperimental,
    launchCheck,
    applyLaunchCheck,
    serverAddr,
    pickMusicFile,
    LOGIN_TYPES,
    loginTypeLabel,
    addLockType,
    addLockName,
    addLockServer,
    addLockHasServer,
    addLockOptions,
    addLock,
    removeLock,
    lockServerError,
    lockServerPlaceholder,
    lockInstances,
    refreshLockInstances,
    lockMissing,
    onLockInstanceChange,
    customHome,
    importingCustomHome,
    customHomeState,
    refreshCustomHome,
    importCustomHome,
    removeCustomHome,
    customUi,
    importingCustomPage,
    customPageState,
    refreshCustomUi,
    addCustomCss,
    setCustomCssEnabled,
    moveCustomCss,
    removeCustomCss,
    importCustomPage,
    removeCustomPage,
    openCustomUiDir,
    load,
    saveState: state,
    resetGroup,
  };
}
