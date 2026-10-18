// 设置窗口 · 客户端设置标签：服务器 / MOTD / 登录方式锁定 / 实例锁定 / 自定义主页面
//
// 这一片基本都是 `gui_config.client`（即改即存，整对象提交），加上两份"外部状态"：
// 实例列表（锁定下拉的候选，进页面时重拉）与自定义主页面的导入状态。
// 游戏标题那组虽然显示在这个标签里，但数据属于 `window`，由 useSettingsLaunch 持有。
import { computed, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { commands, type CustomHomeInfoDto, type InstanceInfoDto } from "../../../lib/bindings";
import {
  defaultConfig,
  loadGuiConfig,
  saveGuiConfig,
  type ClientConfig,
  type ExperimentalConfig,
} from "../../../lib/guiConfig";
import { typeLabelKey } from "../../../lib/accountStore";
import { useSaveState } from "./useSaveState";

/** 登录方式锁定的候选（复用账户添加类型的文案） */
const LOGIN_TYPES = ["offline", "microsoft", "littleskin", "selflittleskin", "authlib", "nide8"];

/** 需要服务器信息的类型：外置登录 / 自定义皮肤站填服务器地址，统一通行证填服务器 ID；
 *  这些类型可重复添加，条目带「登录模型名字」，添加账户时直接下拉选择 */
const SERVER_LOCK_TYPES = ["authlib", "selflittleskin", "nide8"];

/**
 * 客户端设置标签的状态与保存
 *
 * # 返回值
 *
 * - `client`: `gui_config.client` 的副本，即改即存
 * - `serverAddr`: 自动进服与 MOTD 共用的服务器地址，可双向绑定
 * - `loginTypeLabel` / `addLock*` / `lockServerError` / `lockServerPlaceholder`: 登录方式锁定的添加表单
 * - `lockInstances` / `refreshLockInstances` / `lockMissing` / `onLockInstanceChange`: 实例锁定
 * - `customHome` / `importingCustomHome` / `customHomeState` / `refreshCustomHome` / `importCustomHome` / `openCustomHomeDir` / `removeCustomHome`: 自定义主页面
 * - `pickMusicFile`: 选一个本地音频文件填进"自定义音乐"
 * - `experimental` / `applyExperimental`: 实验性功能四开关（界面先行，行为未接线）
 * - `load` / `applyClient` / `saveState` / `resetGroup`: 加载、落盘与分组级恢复默认
 */
export function useSettingsClient() {
  const client = ref<ClientConfig>({ ...defaultConfig().client });
  /** 实验性功能四个开关：界面先行，行为未接线 */
  const experimental = ref<ExperimentalConfig>({ ...defaultConfig().experimental });
  const { state, track } = useSaveState();

  // ---------- 加载 ----------

  async function load() {
    const cfg = await loadGuiConfig();
    if (cfg?.client) client.value = { ...cfg.client };
    if (cfg?.experimental) experimental.value = { ...cfg.experimental };
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

  /** 整份 experimental 提交；同样失败时把保存状态标红 */
  async function applyExperimental() {
    try {
      await track(() => saveGuiConfig({ experimental: { ...experimental.value } }));
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
      showToast(`${t("winSettings.customHomeImportFailed")}：${tErr(e)}`);
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

  /** 在文件管理器里定位压缩包（还没导入时打开运行根目录） */
  async function openCustomHomeDir() {
    try {
      await commands.customHome.openDir();
    } catch (e) {
      showToast(tErr(e));
    }
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

  // ---------- 恢复默认 ----------

  /** 这一片全部属于 gui_config.client，默认值取前端镜像 guiConfig.defaultConfig */

  async function resetGroup(id: string): Promise<boolean> {
    const d = defaultConfig().client;
    switch (id) {
      case "servers":
        client.value.autoJoin = d.autoJoin;
        client.value.autoJoinServer = d.autoJoinServer;
  client.value.windowEffect = d.windowEffect;
  client.value.windowOpacity = d.windowOpacity;
        client.value.motdServer = d.motdServer;
        client.value.motdCard = d.motdCard;
        client.value.motdInterval = d.motdInterval;
        break;
      case "loginLock":
        client.value.loginLockOn = d.loginLockOn;
        client.value.loginLock = [];
        break;
      case "instanceLock":
        client.value.lockInstance = d.lockInstance;
        break;
      case "customHome":
        // 只把开关恢复成默认；导入的压缩包是用户的东西，恢复默认不动它
        client.value.customHome = d.customHome;
        break;
      case "music":
        client.value.musicOn = d.musicOn;
        client.value.musicSource = d.musicSource;
        break;
      default:
        return false;
    }
    await applyClient();
    return true;
  }

  return {
    client,
    applyClient,
    experimental,
    applyExperimental,
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
    openCustomHomeDir,
    removeCustomHome,
    load,
    saveState: state,
    resetGroup,
  };
}
