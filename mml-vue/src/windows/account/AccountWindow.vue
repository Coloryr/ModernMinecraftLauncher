<script setup lang="ts">
// 账户管理窗口：平铺 / 列表 / 详情 三种展示 + 类型筛选 + 搜索 + 添加账户
import { computed, defineAsyncComponent, onDeactivated, onMounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import SearchBox from "../../components/ui/SearchBox.vue";
import DropOverlay from "../../components/ui/DropOverlay.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { useFileDrop } from "../../composables/useFileDrop";
import { useUnlisteners } from "../../composables/useUnlisteners";
import type { FileDropPayload } from "../../composables/useFileDrop";
import { dropFromUrl, multiWindow, windowParams } from "../windowManager";
import type { WindowDropParam } from "../windowManager";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import AccountGrid from "./views/AccountGrid.vue";
import AccountList from "./views/AccountList.vue";
import AccountDetail from "./views/AccountDetail.vue";
import RowMenu, { type RowMenuItem } from "../resource/parts/RowMenu.vue";
import {
  ACCOUNT_TYPES,
  accounts,
  addAccount,
  currentAccount,
  loadAccounts,
  loginMicrosoft,
  removeAccount,
  refreshAccountToken,
  setCurrentAccount,
  typeLabelKey,
} from "../../lib/accountStore";
import { onClientConfigChange } from "../../lib/api";
import type { AccountStoreDto, ClientConfigDto } from "../../lib/bindings";
import { listen } from "@tauri-apps/api/event";
import { AccountOAuth, AccountOAuthState } from "../../lib/listens";
import { commands } from "../../lib/bindings";
import type { AccountOAuthDto, AccountOAuthStateDto } from "../../lib/bindings";
import { loadGuiConfig, type LoginLockItem } from "../../lib/guiConfig";
import { bumpImageVersion, hasNoSkin, resetImageState } from "../../lib/accountImages";
// 皮肤预览子页面按需加载：它静态引入 skinview3d，含 three.js 整块 ~560 kB。
// 账户窗口打开时绝大多数操作是看列表 / 切账户，不该为此先付掉这个体积。
const SkinPage = defineAsyncComponent(() => import("./SkinPage.vue"));

// 必须显式声明 close：不声明的话 Vue 会把父级的 @close 当 attrs 透传到根组件（WindowFrame），
// 与模板里的 @close="$emit('close')" 合并成两个处理器 —— 一次"返回"会调两遍 closeWindow()，
// 第二遍时 currentKind 已经回到 main，于是"返回"变成退出应用
/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

/** 皮肤预览子页面是否打开 */
const showSkin = ref(false);

/** 登录失败弹窗：微软账户给「重新登录」，其余类型带回添加表单重填 */
const loginError = ref<{ message: string; microsoft: boolean } | null>(null);

// 系统窗口标题由 WindowFrame 统一同步，即 watch title → window_set_title；
// 这里不再手动调 JS API setTitle，那需要 capabilities 未放行的 set-title 权限

type ViewMode = "grid" | "list" | "detail";
const view = ref<ViewMode>("grid");

const { track } = useUnlisteners();

// 进入窗口时加载账户数据与登录方式锁定，并订阅微软登录事件
onMounted(() => {
  loadAccounts();
  // 读客户端设置的登录方式锁定
  loadGuiConfig().then((cfg) => {
    if (cfg?.client) {
      // 总开关关闭时锁定列表不生效
      loginLock.value = cfg.client.loginLockOn ? cfg.client.loginLock : [];
    }
  });
  // 设置窗口保存客户端设置时实时同步锁定，增删登录模型后无需重开账户窗口
  track(
    onClientConfigChange((c: ClientConfigDto) => {
      loginLock.value = c.loginLockOn ? c.loginLock : [];
    }),
  );

  // 后端拿到设备码：弹出授权码窗口
  track(
    listen<AccountOAuthDto>(AccountOAuth, (data) => {
      showAdd.value = false;
      oauthCode.value = data.payload.code;
      oauthUrl.value = data.payload.url;
      showOauth.value = true;
    }),
  );

  // 登录阶段进度：waiting / xbox / xsts / token / profile / ok / fail
  track(
    listen<AccountOAuthStateDto>(AccountOAuthState, (data) => {
      const s = data.payload;
      if (s.state === "ok") {
        showOauth.value = false;
        showOauthRun.value = false;
        showToast(t("account.oauthOk"));
        loadAccounts();
      } else if (s.state === "fail") {
        showOauth.value = false;
        showOauthRun.value = false;
        loginError.value = {
          message: tErr(s.message || "account.oauthFail"),
          microsoft: true,
        };
      } else {
        oauthState.value = t(`account.oauthState.${s.state}`);
        // 授权码弹窗还开着（用户尚未点「打开浏览器」）时不抢焦点，只静默更新文案
        if (!showOauth.value) {
          showOauthRun.value = true;
        }
      }
    }),
  );
});

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次（账户可能在别处被加/删）
useWindowRefresh(loadAccounts);

// 主窗口拖入的登录快捷方式：多窗口模式下这次挂载就是为它来的（参数挂在 URL 上）
onMounted(() => void consumeDrop());

const VIEW_OPTIONS = computed(() => [
  { value: "grid", label: t("account.view.grid"), icon: "grid" },
  { value: "list", label: t("account.view.list"), icon: "list" },
  { value: "detail", label: t("account.view.detail"), icon: "folder" },
]);

// 类型筛选 + 搜索
const typeFilter = ref("all");
const searchText = ref("");

// 离开账户窗口时清掉搜索词（类型筛选留着：那是浏览上下文）。
// 单窗口模式下这一页被 KeepAlive 缓存，切走不销毁，不清的话再打开时输入框里
// 还压着上次搜的词，用户早忘了自己搜过什么，只会觉得界面不对劲。
onDeactivated(() => {
  searchText.value = "";
});

/** 按登录方式锁定、类型筛选与搜索词过滤后的账户列表 */
const filtered = computed(() => {
  const q = searchText.value.trim().toLowerCase();
  const lock = lockedTypes.value;
  return accounts.value.filter((a) => {
    // 登录方式锁定生效时，列表只显示锁定类型的账户
    if (lock.size && !lock.has(a.authType)) return false;
    if (typeFilter.value !== "all" && a.authType !== typeFilter.value) return false;
    if (q) {
      return [a.userName, a.uuid, a.authType].some((s) => s.toLowerCase().includes(q));
    }
    return true;
  });
});

/** 列表是不是被筛过：搜索词或类型筛选生效。空结果文案与「添加」入口都看它 */
const filtering = computed(() => searchText.value.trim() !== "" || typeFilter.value !== "all");

/** 账户库是否为空：空库时标题栏只留标题，添加入口交给空状态里的按钮 */
const hasAccounts = computed(() => accounts.value.length > 0);

/** 添加账户弹窗是否打开 */
const showAdd = ref(false);
/** 添加弹窗当前选中的账户类型 */
const addType = ref("offline");
/** 表单值，键为字段 key */
const addFields = ref<Record<string, string>>({});
/** 提交时没填的必填字段 key：就地标红，用户补填一格就撤一格 */
const addInvalid = ref<string[]>([]);

// 登录方式锁定：客户端设置里配置的条目列表，总开关打开时只允许列表中的登录方式
const loginLock = ref<LoginLockItem[]>([]);
/** 添加弹窗可选的登录类型：锁定生效时只留锁定类型 */
const addTypeOptions = computed(() =>
  loginLock.value.length
    ? ACCOUNT_TYPES.filter((x) => loginLock.value.some((e) => e.ty === x.value))
    : ACCOUNT_TYPES,
);

/** 需要服务器信息的锁定类型：外置登录 / 自定义皮肤站 / 统一通行证 */
const SERVER_LOCK_TYPES = ["authlib", "selflittleskin", "nide8"];
/** 生效中的锁定类型集合：loginLock 已按总开关过滤，非空即锁定生效 */
const lockedTypes = computed(() => new Set(loginLock.value.map((e) => e.ty)));

/** 当前添加类型的锁定服务器条目：有值时服务器输入框换成「登录模型」下拉 */
const lockServerEntries = computed(() =>
  SERVER_LOCK_TYPES.includes(addType.value)
    ? loginLock.value.filter((e) => e.ty === addType.value && e.server)
    : [],
);
/** 当前选中的登录模型，值即条目的服务器地址，类型内唯一 */
const selectedLockServer = ref("");

/** 选中登录模型：把条目服务器写进对应字段，外置登录 / 自定义皮肤站写 server，统一通行证写 serverId */
function onSelectLockServer(server: string) {
  selectedLockServer.value = server;
  if (addType.value === "nide8") {
    addFields.value.serverId = server;
  } else {
    addFields.value.server = server;
  }
}

/** 类型切换后重置表单：有锁定条目时默认选中第一个登录模型 */
function resetAddFields() {
  addFields.value = {};
  addInvalid.value = [];
  const first = lockServerEntries.value[0];
  if (first) {
    onSelectLockServer(first.server);
  } else {
    selectedLockServer.value = "";
  }
}
/** 开始补填某一格就撤掉它的标红 */
function clearInvalid(key: string) {
  addInvalid.value = addInvalid.value.filter((k) => k !== key);
}

/** 微软授权码弹窗：显示设备码与验证地址 */
const showOauth = ref(false);
/** 授权进行中弹窗：等后端轮询结果 */
const showOauthRun = ref(false);
/** 后端下发的设备码 */
const oauthCode = ref("");
/** 设备码对应的验证地址 */
const oauthUrl = ref("");
/** 当前登录阶段的文案 */
const oauthState = ref("");

/** 添加账户表单里的一个输入字段 */
interface AddField {
  key: string;
  labelKey: string;
  password?: boolean;
  /** 可选字段：如 LittleSkin 的服务器地址，留空即官方站 */
  optional?: boolean;
}

/** 各添加类型的输入字段；微软账户没有字段，走设备码流程 */
const ADD_FIELDS: Record<string, AddField[]> = {
  offline: [{ key: "name", labelKey: "account.name" }],
  microsoft: [],
  littleskin: [
    { key: "name", labelKey: "account.username" },
    { key: "pass", labelKey: "account.password", password: true },
  ],
  selflittleskin: [
    { key: "server", labelKey: "account.server" },
    { key: "name", labelKey: "account.username" },
    { key: "pass", labelKey: "account.password", password: true },
  ],
  authlib: [
    { key: "server", labelKey: "account.server" },
    { key: "name", labelKey: "account.username" },
    { key: "pass", labelKey: "account.password", password: true },
  ],
  nide8: [
    { key: "serverId", labelKey: "account.serverId" },
    { key: "name", labelKey: "account.username" },
    { key: "pass", labelKey: "account.password", password: true },
  ],
};

/** 弹窗实际渲染的字段：锁定条目存在时隐藏服务器字段（由登录模型下拉决定） */
const addFormFields = computed(() =>
  (ADD_FIELDS[addType.value] ?? []).filter(
    (f) => !(lockServerEntries.value.length && (f.key === "server" || f.key === "serverId")),
  ),
);

/** Nide8 注册页前缀与 LittleSkin 注册页；其余账户类型没有固定入口 */
const NIDE8_REGISTER_URL = "https://login.mc-user.com:233/";
const LITTLESKIN_REGISTER_URL = "https://littleskin.cn/auth/register";

/**
 * 注册页地址，空串表示这个类型没有现成入口、不显示按钮
 *
 * 统一通行证的注册页挂在**服务器 ID** 下，所以 ID 没填也没锁定条目时不给地址 ——
 * 缺一段 ID 的地址只会打开一个 404。注意它属于 Nide8 的**网页站**，
 * 与本项目做认证的 API 站 `auth.mc-user.com:233` 不是同一个主机。
 */
const registerUrl = computed(() => {
  if (addType.value === "nide8") {
    const id = (addFields.value.serverId ?? "").trim();
    return id ? `${NIDE8_REGISTER_URL}${id}/loginreg` : "";
  }
  return addType.value === "littleskin" ? LITTLESKIN_REGISTER_URL : "";
});

/** 打开注册页：交给系统浏览器，失败静默 */
function openRegister() {
  if (!registerUrl.value) return;
  commands.main.openUrl(registerUrl.value).catch(() => {});
}

/** 打开添加账户弹窗；锁定登录方式时默认选第一个锁定类型 */
function openAdd() {
  showAdd.value = true;
  // 锁定登录方式时默认第一个锁定条目的类型，并选中第一个登录模型
  addType.value = loginLock.value[0]?.ty ?? "offline";
  resetAddFields();
}

/** 切换添加类型并重置表单 */
function onAddTypeChange(value: string) {
  addType.value = value;
  resetAddFields();
}

// ================= 拖入登录快捷方式：.url / .webloc 预填外置登录地址 =================
//
// 从浏览器直接拖网页上的"卡片"是收不到的：wry 在 Windows 上只认 CF_HDROP，
// 见后端 `window_probe_drop` 的说明。卡片先拖到桌面会变成 `.url` 互联网快捷方式，
// 再把那个文件拖进来 —— 这两个落点（主窗口 / 本窗口）都走这一段。

/** 拖入的路径是否已经应用过：多窗口模式下 URL 上的参数会一直留着，只能消费一次 */
let dropApplied = false;

/** 开「添加账户」表单并预填外置登录地址 */
function applyAuthDrop(url: string) {
  addType.value = "authlib";
  // 先清上一次的残留，有登录模型锁定时它会把第一个锁定条目选上
  resetAddFields();
  showAdd.value = true;
  // 锁定生效时服务器字段由下拉决定：地址正好在锁定列表里就选中它，否则直接填进字段
  const matched = lockServerEntries.value.find((item) => item.server === url);
  if (matched) {
    onSelectLockServer(matched.server);
  } else {
    addFields.value.server = url;
  }
}

/** 消费"拖入的登录快捷方式"，参数从哪读见 windowManager 的 targetDrop */
async function consumeDrop() {
  let drop: WindowDropParam | null;
  if (multiWindow.value) {
    if (dropApplied) return;
    drop = dropFromUrl();
    if (!drop) return;
    dropApplied = true;
  } else {
    drop = windowParams.value.drop;
    if (!drop) return;
    // 单窗口：参数用完就清，同 goTo 清 windowParams 的做法
    windowParams.value = { ...windowParams.value, drop: null };
  }
  if (drop.kind === "shortcut" && drop.url) {
    applyAuthDrop(drop.url);
  }
}

/** 本窗口自己也接：网页卡片（DOM 给的文本地址）与 .url / .webloc 快捷方式文件 */
async function onFilesDropped(payload: FileDropPayload) {
  // ① 网页卡片：地址已经在手上
  if (payload.url) {
    applyAuthDrop(payload.url);
    return;
  }
  // ② 快捷方式文件：读文件里的地址
  const first = payload.paths[0];
  if (!first) {
    // 文本但不是地址，悬停阶段读不到内容、只能等松手：说明一句，别静默
    showToast(t("account.dropNotShortcut"));
    return;
  }
  if (payload.paths.length > 1) showToast(t("drop.onlyFirst"));
  let probe: { kind: string; url: string };
  try {
    probe = await commands.windows.probeDrop(first);
  } catch (e) {
    showToast(tErr(e));
    return;
  }
  if (probe.kind !== "shortcut" || !probe.url) {
    showToast(t("account.dropNotShortcut"));
    return;
  }
  applyAuthDrop(probe.url);
}

// 这里**不传 accept**：悬停阶段 DOM 读不到文本内容，`getData` 在 dragover 期间是空的，
// 判不出"这张卡片是不是地址"，一律接住、松手后再按内容分辨（不是地址就给提示）。
const { fileDragOver, dragKind } = useFileDrop(onFilesDropped);

// 单窗口：切回本页时消费一次；已经停在本页时靠参数变化接住
useWindowRefresh(() => consumeDrop());
watch(
  () => windowParams.value.drop,
  () => void consumeDrop(),
);

/** 添加弹窗是否在处理中（防止重复提交） */
const adding = ref(false);

/** 提交添加：校验必填字段后按类型走设备码流程或直接添加 */
async function confirmAdd() {
  if (adding.value) return;
  const missing = addFormFields.value.filter((f) => !f.optional && !addFields.value[f.key]?.trim());
  if (missing.length) {
    // 就地标红：toast 一闪而过，用户回头还得自己猜是哪一格没填
    addInvalid.value = missing.map((f) => f.key);
    return;
  }
  addInvalid.value = [];

  adding.value = true;
  try {
    if (addType.value === "microsoft") {
      // 触发后端设备码流程，弹窗由 account-oauth 事件接管；失败提示后弹窗保留
      await loginMicrosoft();
    } else {
      const server = (addFields.value.server ?? addFields.value.serverId)?.trim();
      await addAccount(
        addType.value,
        addFields.value.name!.trim(),
        server || undefined,
        addFields.value.pass,
      );
      showAdd.value = false;
      showToast(t("account.added"));
    }
  } catch (e) {
    const microsoft = addType.value === "microsoft";
    showAdd.value = false;
    loginError.value = { message: tErr(e), microsoft };
  } finally {
    adding.value = false;
  }
}

/** 微软登录失败后重试：错误弹窗收起，重新走设备码流程 */
function retryMicrosoftLogin() {
  loginError.value = null;
  loginMicrosoft().catch((e) => {
    loginError.value = { message: tErr(e), microsoft: true };
  });
}

/** 非微软账户登录失败：带回已填内容重新打开添加弹窗 */
function reopenAddForm() {
  loginError.value = null;
  showAdd.value = true;
}

// 微软授权弹窗：打开浏览器并切换到进度窗口
async function openBrowser() {
  await commands.account.openBrowser(oauthUrl.value).catch(() => { /* 忽略 */ });
  showOauth.value = false;
  showOauthRun.value = true;
  oauthState.value = t("account.oauthState.waiting");
}

/**
 * 复制文本到剪贴板：优先异步 Clipboard API，失败回退 execCommand
 *
 * - `text`: 要复制的文本
 *
 * # 返回值
 *
 * 是否复制成功
 */
async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    return ok;
  }
}

/** 点击登录码 / 网址复制 */
async function copyOauthValue(value: string) {
  if (!value) return;
  showToast(await copyText(value) ? t("account.copied") : t("account.copyFail"));
}

/** 双击切换当前账户 */
async function switchAccount(acc: AccountStoreDto) {
  if (await setCurrentAccount(acc)) showToast(t("account.switched", { name: acc.userName }));
}

/** 刷新账户令牌；真实状态由后端的 account-change 广播刷回 */
function refreshToken(acc: AccountStoreDto) {
  void refreshAccountToken(acc.uuid);
  showToast(t("account.refreshed"));
}

/** 刷新皮肤：后端清渲染缓存重取，前端 bump 版本号破 webview 旧图并清失败标记 */
async function refreshSkin(acc: AccountStoreDto) {
  try {
    await commands.account.refreshSkin(acc.uuid);
    resetImageState(acc);
    bumpImageVersion();
    showToast(t("account.skinRefreshed"));
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 右键账户菜单的位置与目标 */
const accountMenu = ref<{ x: number; y: number; acc: AccountStoreDto } | null>(null);

/** 右键菜单项：按账户能力置灰，删除单独隔开 */
const accountMenuItems = computed<RowMenuItem[]>(() => {
  const acc = accountMenu.value?.acc;
  if (!acc) return [];
  const items: RowMenuItem[] = [];
  // 查看皮肤需要有皮肤；刷新皮肤只有在线账户能重拉
  items.push({ id: "viewSkin", label: t("account.viewSkin"), disabled: hasNoSkin(acc) });
  items.push({
    id: "refreshSkin",
    label: t("account.refreshSkin"),
    disabled: acc.authType === "offline",
  });
  items.push({
    id: "refresh",
    label: t("account.refreshAuth"),
    disabled: !acc.canRefresh,
  });
  if (acc.canRelogin) items.push({ id: "relogin", label: t("account.relogin") });
  if (acc.canEdit) items.push({ id: "edit", label: t("account.editOffline") });
  items.push({ id: "delete", label: t("account.delete"), danger: true, separated: true });
  return items;
});

/** 打开右键菜单 */
function openAccountMenu(payload: { event: MouseEvent; acc: AccountStoreDto }) {
  accountMenu.value = {
    x: payload.event.clientX,
    y: payload.event.clientY,
    acc: payload.acc,
  };
}

/** 执行右键菜单动作 */
function onAccountMenuPick(id: string) {
  const acc = accountMenu.value?.acc;
  accountMenu.value = null;
  if (!acc) return;
  switch (id) {
    case "viewSkin":
      viewSkin(acc);
      break;
    case "refreshSkin":
      void refreshSkin(acc);
      break;
    case "refresh":
      refreshToken(acc);
      break;
    case "relogin":
      relogin(acc);
      break;
    case "edit":
      openEdit(acc);
      break;
    case "delete":
      deleteTarget.value = acc;
      break;
  }
}

/** 皮肤弹窗预览的账户；不要求它是"当前使用"的账户 */
const skinTarget = ref<AccountStoreDto | null>(null);

/** 打开皮肤弹窗，预览被点开的这一条账户 */
function viewSkin(acc: AccountStoreDto) {
  skinTarget.value = acc;
  showSkin.value = true;
}

/** 重新登录：微软账户直接走设备码流程；其余类型弹窗重输密码重新认证 */
function relogin(acc: AccountStoreDto) {
  if (acc.authType === "microsoft") {
    loginMicrosoft().catch((e) => {
      loginError.value = { message: tErr(e), microsoft: true };
    });
    return;
  }
  reloginTarget.value = acc;
  reloginFields.value = { name: acc.userName, pass: "" };
}

/** 待重新登录的账户；非空即弹窗打开 */
const reloginTarget = ref<AccountStoreDto | null>(null);
/** 重新登录只重输用户名 / 密码：登录地址由账户本身固定，不在这里改 */
const reloginFields = ref({ name: "", pass: "" });

/** 防止重复提交 */
const relogining = ref(false);

/** 提交重新登录：同 uuid + 类型会覆盖旧账户数据，等效重新认证 */
async function confirmRelogin() {
  const acc = reloginTarget.value;
  if (!acc || relogining.value) return;
  if (!reloginFields.value.name.trim() || !reloginFields.value.pass.trim()) {
    showToast(t("account.fieldsRequired"));
    return;
  }

  relogining.value = true;
  try {
    // 同 uuid + 类型会覆盖旧账户数据，等效重新认证；地址沿用账户自己的
    const server = (acc.server ?? "").trim();
    await addAccount(acc.authType, reloginFields.value.name.trim(), server || undefined, reloginFields.value.pass);
    reloginTarget.value = null;
    showToast(t("account.reloginOk"));
  } catch (e) {
    // 非微软账户重新登录失败：把这一条的类型 / 用户名 / 密码 / 服务器带回添加弹窗
    addType.value = acc.authType;
    addFields.value = { name: reloginFields.value.name, pass: reloginFields.value.pass };
    if (acc.server) {
      if (acc.authType === "nide8") addFields.value.serverId = acc.server;
      else addFields.value.server = acc.server;
    }
    selectedLockServer.value =
      lockServerEntries.value.find((item) => item.server === acc.server)?.server ?? "";
    addInvalid.value = [];
    reloginTarget.value = null;
    loginError.value = { message: tErr(e), microsoft: false };
  } finally {
    relogining.value = false;
  }
}

// 取消微软登录：通知后端终止轮询并关闭弹窗
async function cancelLogin() {
  await commands.account.cancelOAuth().catch(() => { /* 忽略 */ });
  showOauth.value = false;
  showOauthRun.value = false;
}

const deleteTarget = ref<AccountStoreDto | null>(null);

/** 删除确认：成功后收起弹窗并提示 */
async function confirmDelete() {
  if (!deleteTarget.value) return;
  if (!await removeAccount(deleteTarget.value.uuid)) return;
  showToast(t("account.removed"));
  deleteTarget.value = null;
}

// 编辑离线账户：改名 / 改 UUID（UUID 右边可一键随机）
/** 待编辑的离线账户；非空即编辑弹窗打开 */
const editTarget = ref<AccountStoreDto | null>(null);
const editFields = ref({ name: "", uuid: "" });
/** 保存中，防重复提交 */
const editing = ref(false);

function openEdit(acc: AccountStoreDto) {
  editTarget.value = acc;
  editFields.value = { name: acc.userName, uuid: acc.uuid };
}

/** 随机 UUID v4，与账户存储一致用不带连字符的写法；crypto.randomUUID 不可用时手动拼 */
function randomUuid() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    editFields.value.uuid = crypto.randomUUID().replace(/-/g, "");
    return;
  }
  const hex = (n: number) =>
    Array.from({ length: n }, () => Math.floor(Math.random() * 16).toString(16)).join("");
  editFields.value.uuid = `${hex(8)}${hex(4)}4${hex(3)}${hex(4)}${hex(12)}`;
}

/** 提交改名 / 改 UUID，成功后重拉列表 */
async function confirmEdit() {
  const acc = editTarget.value;
  if (!acc || editing.value) return;
  if (!editFields.value.name.trim()) {
    showToast(t("err.nameEmpty"));
    return;
  }
  editing.value = true;
  try {
    await commands.account.editOffline(acc.uuid, editFields.value.name, editFields.value.uuid);
    editTarget.value = null;
    showToast(t("account.edited"));
    loadAccounts();
  } catch (e) {
    showToast(tErr(e));
  } finally {
    editing.value = false;
  }
}

const TYPE_OPTIONS = computed(() => {
  // 锁定生效时筛选下拉只保留锁定的登录方式
  const lock = lockedTypes.value;
  const types = lock.size ? ACCOUNT_TYPES.filter((x) => lock.has(x.value)) : ACCOUNT_TYPES;
  return [{ value: "all", label: t("account.all") }, ...types.map((x) => ({ value: x.value, label: t(x.labelKey) }))];
});

// 锁定变化后当前筛选类型可能已不在下拉里，回退到「全部」
watch(TYPE_OPTIONS, (opts) => {
  if (!opts.some((o) => o.value === typeFilter.value)) typeFilter.value = "all";
});

/** 令牌状态转文案 */
function tokenLabel(acc: AccountStoreDto): string {
  return acc.tokenStatus === "valid" ? t("account.tokenValid") : t("account.tokenExpired");
}
</script>

<template>
  <WindowFrame :title="showSkin ? t('account.viewSkin') : t('account.manage')"
    :body-fill="view === 'detail' || showSkin || filtered.length === 0" body-gutter
    :back="showSkin ? t('winCommon.back') : undefined" @back="showSkin = false" @close="$emit('close')">
    <!-- 工具栏：类型筛选 + 搜索 + 视图切换 + 添加账户。高度必须压在 --titlebar-h 内，
         否则会撑高整条顶栏；「查看皮肤」是针对单个账户的动作，留在各条目的按钮里 -->
    <template #head-right>
      <!-- 空库时整排收起：筛选与视图切换都无从谈起，加号交给空状态；
           皮肤子页面占满窗口时同样收起 -->
      <template v-if="hasAccounts && !showSkin">
        <!-- 观感（不透明底 / 主题描边 / 共享箭头 / 页内弹层）全部来自 .field-select，
             这里只覆盖标题栏要的尺寸 -->
        <select v-model="typeFilter" class="field-select head-select">
          <option v-for="o in TYPE_OPTIONS" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
        <!-- 共享组件 SearchBox；168px 是这一排减掉筛选下拉与视图切换后剩下的宽度 -->
        <SearchBox v-model="searchText" :placeholder="t('account.search')" width="168px" />
        <SegmentedTabs class="head-tabs" :model-value="view" :options="VIEW_OPTIONS" icon-only
          @update:model-value="view = $event as ViewMode" />
        <BaseButton size="sm" variant="primary" class="icon-btn-head" v-tip="t('account.addTitle')"
          :aria-label="t('account.addTitle')" @click="openAdd">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </BaseButton>
      </template>
    </template>

    <!-- 视图：平铺 / 列表 / 详情 / 皮肤子页面（见 views/ 目录）；详情与皮肤子页面由自己管理滚动 -->
    <div class="view-area" :class="{ fill: view === 'detail' || showSkin || filtered.length === 0 }">
      <SkinPage v-if="showSkin" :account="skinTarget" />
      <AccountGrid v-else-if="view === 'grid'" :accounts="filtered" :filtering="filtering"
        :current-uuid="currentAccount?.uuid ?? ''" :query="searchText" @menu="openAccountMenu"
        :token-label="tokenLabel" @switch="switchAccount" @refresh="refreshToken" @view-skin="viewSkin"
        @refresh-skin="refreshSkin" @relogin="relogin" @edit="openEdit" @delete="deleteTarget = $event"
        @add="openAdd" />
      <AccountList v-else-if="view === 'list'" :accounts="filtered" :filtering="filtering"
        :current-uuid="currentAccount?.uuid ?? ''" :query="searchText" @menu="openAccountMenu"
        :token-label="tokenLabel" @switch="switchAccount" @refresh="refreshToken" @view-skin="viewSkin"
        @refresh-skin="refreshSkin" @relogin="relogin" @edit="openEdit" @delete="deleteTarget = $event"
        @add="openAdd" />
      <AccountDetail v-else :accounts="filtered" :filtering="filtering" :current-uuid="currentAccount?.uuid ?? ''"
        :query="searchText" @menu="openAccountMenu"
        :token-label="tokenLabel"
        @switch="switchAccount" @refresh="refreshToken" @view-skin="viewSkin" @refresh-skin="refreshSkin"
        @relogin="relogin" @edit="openEdit" @delete="deleteTarget = $event" @add="openAdd" />
    </div>

    <!-- 添加账户弹窗（按类型显示不同输入框）：不遮标题栏 -->
    <BaseModal v-if="showAdd" :title="t('account.addTitle')" :closable="false" below-titlebar
      @close="showAdd = false">
      <label class="field-label">{{ t("account.type") }}</label>
      <select v-model="addType" class="field-select"
        @change="onAddTypeChange(($event.target as HTMLSelectElement).value)">
        <option v-for="x in addTypeOptions" :key="x.value" :value="x.value">{{ t(x.labelKey) }}</option>
      </select>

      <!-- 锁定了服务器条目的类型：选登录模型，服务器由条目决定，不再显示服务器输入框 -->
      <template v-if="lockServerEntries.length">
        <label class="field-label">{{ t("account.loginModel") }}</label>
        <select class="field-select" :value="selectedLockServer"
          @change="onSelectLockServer(($event.target as HTMLSelectElement).value)">
          <option v-for="e in lockServerEntries" :key="e.server" :value="e.server">
            {{ e.name || e.server }}
          </option>
        </select>
      </template>

      <template v-for="f in addFormFields" :key="f.key">
        <label class="field-label">{{ t(f.labelKey) }}</label>
        <input v-model="addFields[f.key]" class="field-input" :class="{ 'is-invalid': addInvalid.includes(f.key) }"
          :type="f.password ? 'password' : 'text'" spellcheck="false"
          @input="clearInvalid(f.key)" @keyup.enter="confirmAdd" />
      </template>

      <p v-if="addInvalid.length" class="error-text">{{ t("account.fieldsRequired") }}</p>

      <p v-if="addType === 'microsoft'" class="hint">{{ t("account.loginHint") }}</p>

      <div class="modal-actions">
        <!-- 统一通行证 / LittleSkin 有现成的注册页：没有账户时不必关掉弹窗再去开浏览器 -->
        <BaseButton v-if="registerUrl" variant="ghost" class="reg-btn" @click="openRegister">
          {{ t("account.register") }}
        </BaseButton>
        <BaseButton @click="showAdd = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmAdd">{{ t("account.add") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 微软登录：请求码 + 地址 + 打开浏览器 / 取消 -->
    <BaseModal v-if="showOauth" :title="t('account.oauthTitle')" :closable="false" below-titlebar
      @close="cancelLogin">
      <label class="field-label">{{ t("account.oauthCode") }}</label>
      <div class="oauth-code copyable" v-tip="t('account.copy')" @click="copyOauthValue(oauthCode)">
        {{ oauthCode }}
        <svg class="copy-ico" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
          stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <rect x="9" y="9" width="12" height="12" rx="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      </div>

      <label class="field-label">{{ t("account.oauthUrl") }}</label>
      <div class="oauth-url copyable" v-tip="t('account.copy')" @click="copyOauthValue(oauthUrl)">
        <span class="oauth-url-text">{{ oauthUrl }}</span>
        <svg class="copy-ico" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
          stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <rect x="9" y="9" width="12" height="12" rx="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      </div>

      <p class="hint">{{ t("account.oauthHint") }}</p>

      <div class="modal-actions">
        <BaseButton @click="cancelLogin">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="openBrowser">{{ t("account.openBrowser") }}</BaseButton>
      </div>
    </BaseModal>

    <BaseModal v-if="showOauthRun" :title="t('account.oauthTitle')" :closable="false" below-titlebar
      @close="cancelLogin">
      <p class="hint">{{ oauthState }}</p>

      <div class="modal-actions">
        <BaseButton @click="cancelLogin">{{ t("add.cancel") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 删除确认：账户名与类型一行，UUID 单独一行，方便核对删的是哪一个 -->
    <BaseModal v-if="deleteTarget" :title="t('account.delete')" :closable="false" below-titlebar
      @close="deleteTarget = null">
      <div class="delete-tip">
        <span class="delete-name">{{ deleteTarget.userName }} - {{ t(typeLabelKey(deleteTarget.authType)) }}</span>
        <span class="delete-uuid">{{ deleteTarget.uuid }}</span>
      </div>
      <div class="modal-actions">
        <BaseButton @click="deleteTarget = null">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="confirmDelete">{{ t("actions.confirm") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 重新登录（非微软账户）：重输密码重新认证。
         登录地址不许改 —— 换地址就是换服，账户属于哪个服由账户自己带着 -->
    <BaseModal v-if="reloginTarget" :title="t('account.reloginTitle')" :closable="false" below-titlebar
      @close="reloginTarget = null">
      <template v-if="reloginTarget.server">
        <label class="field-label">
          {{ reloginTarget.authType === "nide8" ? t("account.serverId") : t("account.server") }}
        </label>
        <input class="field-input" :value="reloginTarget.server" disabled spellcheck="false" />
        <div class="field-hint">{{ t("account.reloginServerLocked") }}</div>
      </template>

      <label class="field-label">{{ t("account.username") }}</label>
      <input v-model="reloginFields.name" class="field-input" spellcheck="false" />

      <label class="field-label">{{ t("account.password") }}</label>
      <input v-model="reloginFields.pass" class="field-input" type="password" @keyup.enter="confirmRelogin" />

      <p class="hint">{{ t("account.reloginHint") }}</p>

      <div class="modal-actions">
        <BaseButton @click="reloginTarget = null">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmRelogin">{{ t("actions.confirm") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 编辑离线账户：改名 / 改 UUID -->
    <BaseModal v-if="editTarget" :title="t('account.editOffline')" :closable="false" below-titlebar
      @close="editTarget = null">
      <label class="field-label">{{ t("account.name") }}</label>
      <input v-model="editFields.name" class="field-input" spellcheck="false" />

      <label class="field-label">{{ t("account.uuid") }}</label>
      <div class="uuid-row">
        <input v-model="editFields.uuid" class="field-input grow" spellcheck="false" />
        <BaseButton size="sm" @click="randomUuid">{{ t("account.randomUuid") }}</BaseButton>
      </div>

      <div class="modal-actions">
        <BaseButton @click="editTarget = null">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmEdit">{{ t("actions.confirm") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 登录失败：微软给「重新登录」，其余类型带回已填内容重开添加表单 -->
    <BaseModal v-if="loginError" :title="t('account.loginFailTitle')" :closable="false" below-titlebar
      @close="loginError = null">
      <p class="error-text">{{ loginError.message }}</p>
      <div class="modal-actions">
        <BaseButton @click="loginError = null">{{ t("add.cancel") }}</BaseButton>
        <BaseButton v-if="loginError.microsoft" variant="primary" @click="retryMicrosoftLogin">
          {{ t("account.relogin") }}
        </BaseButton>
        <BaseButton v-else variant="primary" @click="reopenAddForm">{{ t("account.refill") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 右键账户：查看皮肤 / 刷新 / 重新登录 / 编辑 / 删除 -->
    <RowMenu v-if="accountMenu" :x="accountMenu.x" :y="accountMenu.y" :items="accountMenuItems"
      @pick="onAccountMenuPick" @close="accountMenu = null" />

    <!-- 拖入登录卡片 / 快捷方式时的遮罩（标题按拖入类型变） -->
    <DropOverlay v-if="fileDragOver" :title="t(dragKind === 'text' ? 'account.dropTitle' : 'account.dropFileTitle')" />
  </WindowFrame>
</template>

<style scoped>
/* 详情 / 皮肤子页面 / 空状态时视图区占满剩余高度，滚动交给内部容器 */
.view-area.fill {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* 删除确认：名字与类型一行，UUID 换行，长串都允许折行 */
.delete-tip {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 4px 0 16px;
}

.delete-name {
  font-size: 14px;
  font-weight: 700;
  word-break: break-all;
}

.delete-uuid {
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11.5px;
  color: var(--text-dim);
  word-break: break-all;
}

/* UUID 输入 + 随机按钮 */
.uuid-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* 注册按钮靠左：它是"还没有账户"的旁路，不跟取消 / 添加挤在右端 */
.reg-btn {
  margin-right: auto;
}

/* 这一排的通用标准（宽度 170–280、省略号、按钮不缩、chip 可压）在 styles/titlebar.css；
   这里只留账户窗口自己的例外：类型筛选比标准下拉窄一档，标题栏还要塞搜索框与分段控件 */
.head-select {
  min-width: 128px;
}

/* 标题栏搜索框走共享组件 SearchBox；它自己的外观全部来自 .field-input 与 .input-clear */

.grow {
  flex: 1;
  min-width: 140px;
}

/* 微软登录码：等宽字体 + 宽字距，方便照着抄 */
.oauth-code {
  font-size: 18px;
  font-weight: 800;
  letter-spacing: 4px;
  font-family: "Cascadia Code", Consolas, monospace;
  color: var(--accent);
  background: var(--bg-side);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 14px;
  text-align: center;
}

.oauth-url {
  font-size: 13px;
  color: var(--accent);
  background: var(--bg-side);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 14px;
  word-break: break-all;
}

/* 登录码 / 网址可点击复制 */
.copyable {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  cursor: pointer;
  user-select: text;
  transition: border-color 0.15s;
}

.copyable:hover {
  border-color: var(--accent);
}

.oauth-url-text {
  text-align: left;
}

.copy-ico {
  flex-shrink: 0;
  opacity: 0.55;
}

.copyable:hover .copy-ico {
  opacity: 1;
}

.hint {
  font-size: 12px;
  color: var(--text-dim);
  margin: 10px 0 0;
}

</style>
