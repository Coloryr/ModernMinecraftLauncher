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
import type { FileDropPayload } from "../../composables/useFileDrop";
import { dropFromUrl, multiWindow, windowParams } from "../windowManager";
import type { WindowDropParam } from "../windowManager";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import AccountGrid from "./views/AccountGrid.vue";
import AccountList from "./views/AccountList.vue";
import AccountDetail from "./views/AccountDetail.vue";
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
} from "../../lib/accountStore";
import { onClientConfigChange } from "../../lib/api";
import type { AccountStoreDto, ClientConfigDto } from "../../lib/bindings";
import { listen } from "@tauri-apps/api/event";
import { AccountOAuth, AccountOAuthState } from "../../lib/listens";
import { commands } from "../../lib/bindings";
import type { AccountOAuthDto, AccountOAuthStateDto } from "../../lib/bindings";
import { loadGuiConfig, type LoginLockItem } from "../../lib/guiConfig";
import { bumpImageVersion, resetImageState } from "../../lib/accountImages";
// 皮肤预览弹窗按需加载：它静态引入 skinview3d，含 three.js 整块 ~560 kB。
// 账户窗口打开时绝大多数操作是看列表 / 切账户，不该为此先付掉这个体积。
const SkinModal = defineAsyncComponent(() => import("./SkinModal.vue"));

// 必须显式声明 close：不声明的话 Vue 会把父级的 @close 当 attrs 透传到根组件（WindowFrame），
// 与模板里的 @close="$emit('close')" 合并成两个处理器 —— 一次"返回"会调两遍 closeWindow()，
// 第二遍时 currentKind 已经回到 main，于是"返回"变成退出应用
/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

/** 皮肤预览弹窗（原独立皮肤窗口已并入） */
const showSkin = ref(false);

// 系统窗口标题由 WindowFrame 统一同步，即 watch title → window_set_title；
// 这里不再手动调 JS API setTitle，那需要 capabilities 未放行的 set-title 权限

type ViewMode = "grid" | "list" | "detail";
const view = ref<ViewMode>("grid");

// 进入窗口时从 Rust 加载账户数据，并注册微软登录事件
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
  onClientConfigChange((c: ClientConfigDto) => {
    loginLock.value = c.loginLockOn ? c.loginLock : [];
  }).catch(() => { /* 忽略 */ });

  // 后端拿到设备码：弹出授权码窗口
  listen<AccountOAuthDto>(AccountOAuth, (data) => {
    showAdd.value = false;
    oauthCode.value = data.payload.code;
    oauthUrl.value = data.payload.url;
    showOauth.value = true;
  });

  // 登录阶段进度：waiting / xbox / xsts / token / profile / ok / fail
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
      showToast(s.message || t("account.oauthFail"));
    } else {
      oauthState.value = t(`account.oauthState.${s.state}`);
      // 授权码弹窗还开着（用户尚未点「打开浏览器」）时不抢焦点，只静默更新文案
      if (!showOauth.value) {
        showOauthRun.value = true;
      }
    }
  });
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

// 添加账户弹窗（按类型显示不同输入框）
const showAdd = ref(false);
const addType = ref("offline");
const addFields = ref<Record<string, string>>({});

// 登录方式锁定：客户端设置里配置的条目列表，总开关打开时只允许列表中的登录方式
const loginLock = ref<LoginLockItem[]>([]);
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
/** 当前选中的登录模型：值为条目的服务器地址，类型内唯一 */
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
  const first = lockServerEntries.value[0];
  if (first) {
    onSelectLockServer(first.server);
  } else {
    selectedLockServer.value = "";
  }
}
const showOauth = ref(false);
const showOauthRun = ref(false);
const oauthCode = ref("");
const oauthUrl = ref("");
const oauthState = ref("");

interface AddField {
  key: string;
  labelKey: string;
  password?: boolean;
  /** 可选字段：如 LittleSkin 的服务器地址，留空即官方站 */
  optional?: boolean;
}

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

function openAdd() {
  showAdd.value = true;
  // 锁定登录方式时默认第一个锁定条目的类型，并选中第一个登录模型
  addType.value = loginLock.value[0]?.ty ?? "offline";
  resetAddFields();
}

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
  const fields = ADD_FIELDS[addType.value] ?? [];
  if (fields.some((f) => !f.optional && !addFields.value[f.key]?.trim())) {
    showToast(t("account.fieldsRequired"));
    return;
  }

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
    showToast(String(e));
  } finally {
    adding.value = false;
  }
}

// 微软授权弹窗：打开浏览器并切换到进度窗口
async function openBrowser() {
  await commands.account.openBrowser(oauthUrl.value).catch(() => { /* 忽略 */ });
  showOauth.value = false;
  showOauthRun.value = true;
  oauthState.value = t("account.oauthState.waiting");
}

/** 复制到剪贴板：优先异步 Clipboard API，失败回退 execCommand */
/** 复制文本：异步 Clipboard API 失败时回退 execCommand */
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
function switchAccount(acc: AccountStoreDto) {
  setCurrentAccount(acc);
  showToast(t("account.switched", { name: acc.userName }));
}

// 操作
function refreshToken(acc: AccountStoreDto) {
  refreshAccountToken(acc.uuid);
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

/** 查看皮肤：预览被点击的那一条账户，不要求它同时是"当前使用"的账户 */
const skinTarget = ref<AccountStoreDto | null>(null);

function viewSkin(acc: AccountStoreDto) {
  skinTarget.value = acc;
  showSkin.value = true;
}

/** 重新登录：微软账户直接走设备码流程；其余类型弹窗重输密码重新认证 */
function relogin(acc: AccountStoreDto) {
  if (acc.authType === "microsoft") {
    loginMicrosoft().catch((e) => showToast(String(e)));
    return;
  }
  reloginTarget.value = acc;
  reloginFields.value = { server: acc.server ?? "", name: acc.userName, pass: "" };
}

const reloginTarget = ref<AccountStoreDto | null>(null);
const reloginFields = ref({ server: "", name: "", pass: "" });

/** 防止重复提交 */
const relogining = ref(false);

async function confirmRelogin() {
  const acc = reloginTarget.value;
  if (!acc || relogining.value) return;
  if (!reloginFields.value.name.trim() || !reloginFields.value.pass.trim()) {
    showToast(t("account.fieldsRequired"));
    return;
  }

  relogining.value = true;
  try {
    // 同 uuid + 类型会覆盖旧账户数据，等效重新认证
    const server = reloginFields.value.server.trim();
    await addAccount(acc.authType, reloginFields.value.name.trim(), server || undefined, reloginFields.value.pass);
    reloginTarget.value = null;
    showToast(t("account.reloginOk"));
  } catch (e) {
    showToast(String(e));
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

function confirmDelete() {
  if (!deleteTarget.value) return;
  removeAccount(deleteTarget.value.uuid);
  showToast(t("account.removed"));
  deleteTarget.value = null;
}

// 编辑离线账户：改名 / 改 UUID（UUID 右边可一键随机）
const editTarget = ref<AccountStoreDto | null>(null);
const editFields = ref({ name: "", uuid: "" });
const editing = ref(false);

function openEdit(acc: AccountStoreDto) {
  editTarget.value = acc;
  editFields.value = { name: acc.userName, uuid: acc.uuid };
}

/** 随机 UUID v4，crypto.randomUUID 不可用时手动拼 */
function randomUuid() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    editFields.value.uuid = crypto.randomUUID();
    return;
  }
  const hex = (n: number) =>
    Array.from({ length: n }, () => Math.floor(Math.random() * 16).toString(16)).join("");
  editFields.value.uuid = `${hex(8)}-${hex(4)}-4${hex(3)}-${hex(4)}-${hex(12)}`;
}

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
  <WindowFrame :title="t('account.manage')" :body-fill="view === 'detail'" body-gutter @close="$emit('close')">
    <!-- 工具栏整体搬进标题栏右侧：类型筛选 + 搜索 + 视图切换 + 添加账户。
         「查看皮肤」不在这里 —— 它是针对某个账户的动作，放在各账户条目的操作按钮里。
         标题栏高度是固定的 --titlebar-h，这一排必须比它矮，否则会把整条顶栏撑高。
         标题本身不删 —— 「账户管理」是这一页的身份，删掉窗口标题就只剩一排控件，
         用户分不清自己在哪一页 -->
    <template #head-right>
      <!-- 同时挂 .field-select：尺寸以外的观感（不透明底 / 主题描边 / 共享箭头 /
           以及 Chromium 135+ 的 base-select 页内弹层）全部由它统一提供，
           这里只覆盖标题栏要的尺寸。之前单独写一套，弹层还是系统原生那个 -->
      <select v-model="typeFilter" class="field-select head-select">
        <option v-for="o in TYPE_OPTIONS" :key="o.value" :value="o.value">{{ o.label }}</option>
      </select>
      <!-- 搜索框与设置窗口那是同一份（components/ui/SearchBox），
           宽度按这里的需要给：这一排还有筛选下拉与视图切换，168px 是余下的空间 -->
      <SearchBox v-model="searchText" :placeholder="t('account.search')" width="168px" />
      <SegmentedTabs class="head-tabs" :model-value="view" :options="VIEW_OPTIONS"
        @update:model-value="view = $event as ViewMode" />
      <!-- 添加账户已移到账户列表里（作为列表的第一项，三种视图都是），
           标题栏只留筛选 / 搜索 / 视图切换这些"看"的控件 -->
    </template>

    <!-- 视图：平铺 / 列表 / 详情（见 views/ 目录）；详情模式下视图区自己管理滚动 -->
    <div class="view-area" :class="{ fill: view === 'detail' }">
      <AccountGrid v-if="view === 'grid'" :accounts="filtered" :filtering="filtering"
        :current-uuid="currentAccount?.uuid ?? ''" @add="openAdd"
        @switch="switchAccount" @refresh="refreshToken" @view-skin="viewSkin" @refresh-skin="refreshSkin"
        @relogin="relogin" @edit="openEdit" @delete="deleteTarget = $event" />
      <AccountList v-else-if="view === 'list'" :accounts="filtered" :filtering="filtering"
        :current-uuid="currentAccount?.uuid ?? ''"
        :token-label="tokenLabel" @add="openAdd" @switch="switchAccount" @refresh="refreshToken" @view-skin="viewSkin"
        @refresh-skin="refreshSkin" @relogin="relogin" @edit="openEdit" @delete="deleteTarget = $event" />
      <AccountDetail v-else :accounts="filtered" :filtering="filtering" :current-uuid="currentAccount?.uuid ?? ''"
        :token-label="tokenLabel"
        @add="openAdd" @switch="switchAccount" @refresh="refreshToken" @view-skin="viewSkin" @refresh-skin="refreshSkin"
        @relogin="relogin" @edit="openEdit" @delete="deleteTarget = $event" />
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
        <input v-model="addFields[f.key]" class="field-input" :type="f.password ? 'password' : 'text'"
          spellcheck="false" @keyup.enter="confirmAdd" />
      </template>

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

    <!-- 删除确认 -->
    <BaseModal v-if="deleteTarget" :title="t('account.delete')" :closable="false" below-titlebar
      @close="deleteTarget = null">
      <p class="delete-tip">{{ t("account.deleteConfirm", { name: deleteTarget.userName }) }}</p>
      <div class="modal-actions">
        <BaseButton @click="deleteTarget = null">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="confirmDelete">{{ t("actions.confirm") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 重新登录（非微软账户）：重输密码重新认证，服务器地址预填 -->
    <BaseModal v-if="reloginTarget" :title="t('account.reloginTitle')" :closable="false" below-titlebar
      @close="reloginTarget = null">
      <template v-if="reloginTarget.authType === 'nide8'">
        <label class="field-label">{{ t("account.serverId") }}</label>
        <input v-model="reloginFields.server" class="field-input" spellcheck="false" />
      </template>
      <template v-else>
        <label class="field-label">{{ t("account.server") }}</label>
        <input v-model="reloginFields.server" class="field-input" spellcheck="false" />
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

    <!-- 皮肤 / 披风预览弹窗（2D / 3D 预览 + 纹理列表 + 正版装备）。
         目标账户由点开它的那一条决定，不是"当前使用"的账户 -->
    <SkinModal v-if="showSkin" :account="skinTarget" @close="showSkin = false" />

    <!-- 拖入登录卡片 / 快捷方式时的遮罩（标题按拖入类型变） -->
    <DropOverlay v-if="fileDragOver" :title="t(dragKind === 'text' ? 'account.dropTitle' : 'account.dropFileTitle')" />
  </WindowFrame>
</template>

<style scoped>
/* 详情模式下视图区占满剩余高度，滚动交给表格容器 */
.view-area.fill {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
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

/* ===== 标题栏右侧控件（head-right 插槽）=====
   全 app 控件统一 32px（controls.css 的 --ctl-h），这一排不再各自定高 */
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

/* 平铺 */
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

.mono {
  font-family: "Cascadia Code", Consolas, monospace;
}
</style>
