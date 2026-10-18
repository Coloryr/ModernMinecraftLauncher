<script setup lang="ts">
// 主窗口：实例列表（分组 / 平铺 / 搜索）、实例详情与设置、启动流程与日志、启动器主页
// 子组件在 topbar / sidebar / ctxmenu/；拖拽、多选、文件拖放逻辑在 composables/
// 显示模式的切换入口不在这里，在设置窗口「界面 → 主窗口」
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import {
  api,
  getLoadState,
  onClientConfigChange,
  onCustomHomeChange,
  onGameExit,
  onGuiViewModeChange,
  onInstanceChange,
  onJavaChange,
  onLaunchError,
  onLaunchState,
  openInstanceDir,
} from "../../lib/api";
import { commands } from "../../lib/bindings";
import { loadGuiConfig, type ClientConfig } from "../../lib/guiConfig";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { stateText } from "../../lib/launchState";
import { useDownloadProgress } from "../../composables/useDownloadProgress";
import { KEYS, onStorageChange } from "../../lib/storage";
import { openWindow } from "../windowManager";
import {
  selectedInstance,
  setSelectedInstance,
  sidebarCollapsed,
  sidebarSide,
  setSidebarCollapsed,
  startupPage,
  viewMode,
} from "../../lib/settings";
import type { AccountStoreDto, CustomHomeInfoDto, GroupDto, InstanceInfoDto, JavaInfoDto, NewsItem, StartArgDto, VersionInfoDto } from "../../lib/bindings";
import InstanceIcon from "../../components/InstanceIcon.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import DropOverlay from "../../components/ui/DropOverlay.vue";
import InstanceSelect from "../../components/InstanceSelect.vue";
import InstanceMetaPanel from "../../components/InstanceMetaPanel.vue";
import LaunchArgsPanel from "../../components/LaunchArgsPanel.vue";
import CustomJsonPanel from "../../components/CustomJsonPanel.vue";
import HomePage from "../../components/HomePage.vue";
import MotdCard from "../../components/MotdCard.vue";
import CustomHomePage from "../../components/CustomHomePage.vue";
import CustomExecPanel from "../../components/CustomExecPanel.vue";
import ProxyPanel from "../../components/ProxyPanel.vue";
import { useModpackStatus } from "../../lib/modpackTasks";
import SplashScreen from "../../components/ui/SplashScreen.vue";
import MainTopbar from "./topbar/MainTopbar.vue";
import MainSidebar from "./sidebar/MainSidebar.vue";
import MainCtxMenu from "./ctxmenu/MainCtxMenu.vue";
import LaunchProgress from "../../components/LaunchProgress.vue";
import IconPickModal from "./IconPickModal.vue";
import ColorMcMigrateModal from "./ColorMcMigrateModal.vue";
import { useInstanceDrag } from "../../composables/useInstanceDrag";
import { useMultiSelect } from "../../composables/useMultiSelect";
import { useFileDrop } from "../../composables/useFileDrop";
import type { FileDropPayload } from "../../composables/useFileDrop";
import { useMotd } from "./composables/useMotd";
import { useInstanceArgs } from "./composables/useInstanceArgs";
import { useInstanceStats } from "./composables/useInstanceStats";
import type { ColorMcInfoDto, ViewMode } from "../../lib/bindings";
import type { CtxMenuState, FeatureId, GroupView, InstMenuAction } from "./types";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import WindowControls from "../../components/ui/WindowControls.vue";
import CollapsePanel from "../../components/ui/CollapsePanel.vue";
import { titleBarStyle, onTitleBarPointerDown, useWindowTitle } from "../../lib/titlebar";
import { useWindowDecoration } from "../../lib/useWindowDecoration";

// 注意：不能用顶层 await —— 会让 <script setup> 变成 async setup，
// App.vue 没有 <Suspense> 包裹，Vue 将不渲染该组件（窗口白屏）
// 系统窗口标题（任务栏 / Alt+Tab）走自定义命令 window_set_title：
// JS API setTitle 需要 core:window:allow-set-title 权限，capabilities 未放行。
// 必须用 useWindowTitle 而不是在这里直接设一次：单窗口模式下本页被 KeepAlive 缓存，
// 从子窗口切回来不重新挂载，只设一次的话任务栏上会一直挂着子窗口的标题
useWindowTitle(() => t("winTitle.main"));

// ================= 基础状态 =================

import { closeSplash, splashError, splashVisible } from "../../lib/splash";

// ================= 数据 =================

const instances = ref<InstanceInfoDto[]>([]);
const javas = ref<JavaInfoDto[]>([]);
const selected = ref<InstanceInfoDto | null>(null);

// ================= 实例搜索 =================

const searchText = ref("");
const searching = computed(() => searchText.value.trim().length > 0);

function matchInst(inst: InstanceInfoDto, q: string): boolean {
  return [
    inst.name,
    inst.version,
    inst.versionType ?? "",
    inst.loader,
    inst.loaderVersion ?? "",
    groupNameOf(groupIdOf(inst)),
  ].some((s) => s.toLowerCase().includes(q));
}

const filteredInstances = computed(() => {
  const q = searchText.value.trim().toLowerCase();
  if (!q) return instances.value;
  return instances.value.filter((i) => matchInst(i, q));
});

const filteredGroups = computed(() => {
  const q = searchText.value.trim().toLowerCase();
  if (!q) return groups.value;
  return groups.value
    .map((g) => ({
      ...g,
      items: g.name.toLowerCase().includes(q)
        ? g.items
        : g.items.filter((i) => matchInst(i, q)),
    }))
    .filter((g) => g.items.length > 0);
});

function select(inst: InstanceInfoDto) {
  // 实例锁定：只能使用锁定的那个实例；对玩家静默拦截，不暴露"被锁定"这件事
  if (lockActive.value && inst.uuid !== lockUuid.value) return;
  selected.value = inst;
  // 持久化当前选中实例到 gui_config.json；uuid 未变时内部跳过，不会重复写盘
  setSelectedInstance(inst.uuid);
  newsActive.value = false;
  // 分组模式下展开所在分组
  if (mode.value === "group") {
    const key = groupIdOf(inst);
    if (collapsedGroups.value[key]) {
      collapsedGroups.value = { ...collapsedGroups.value, [key]: false };
    }
  }
}

// ================= 游戏列表模式 =================

// 显示模式：共享状态，默认「列表」，改它的入口在设置窗口（界面 → 主窗口），
// 那边改完由后端广播 gui-view-mode-change，见 subscribeEvents
const mode = viewMode;

/**
 * 分组注册表，含空分组；顺序即显示顺序
 *
 * 分组以 **uuid 为身份**、组名只是显示数据，见 mml-game 的 `game_group`，
 * 所以界面上一律按 uuid 认组：改名、重复显示名都不会混淆，也不怕组名留白。
 */
const groupList = ref<GroupDto[]>([]);

/**
 * 默认分组的 uuid；对不上时回退到"名字为空"那个
 *
 * 默认分组由后端保证恒存在、名字留白；它**只是初始排在首位**，用户能拖到别处，
 * 所以这里**按名字找**，不能取"表里的第一个"（那会取到恰好排在最前的那一组）。
 */
const defaultGroupId = computed(
  () => groupList.value.find((g) => !g.name.trim())?.uuid ?? "",
);

/**
 * 实例所在分组的 uuid
 *
 * 归属为空说明实例在默认分组，指向一个已经不在表里的组说明界面上是过期数据，
 * 两种都一律算默认分组 —— 宁可归错组，也不能让实例从界面上消失。
 */
function groupIdOf(inst: InstanceInfoDto): string {
  const id = inst.group;
  if (id && groupList.value.some((g) => g.uuid === id)) return id;
  return defaultGroupId.value;
}

/** 分组 uuid → 显示名（默认分组按界面语言翻译） */
function groupNameOf(id: string): string {
  const g = groupList.value.find((x) => x.uuid === id);
  return g && g.name.trim() ? g.name : t("group.default");
}

const groups = computed<GroupView[]>(() => {
  const map = new Map<string, InstanceInfoDto[]>();
  for (const inst of instances.value) {
    const key = groupIdOf(inst);
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push(inst);
  }
  // 分组表拿不到时（命令失败）退回"只有一个默认分组"，别让实例从界面上消失
  const list: GroupDto[] = groupList.value.length
    ? groupList.value
    : [{ uuid: "", name: "" }];
  // 顺序就是内核表给的顺序：默认分组在前，其余按用户排定的先后
  return list.map((g) => {
    // 组内次序由后端下发，见分组表 group_save.json 的 order；
    // 同值时按名字兜底，保证顺序确定而不是随 HashMap 抖动
    const items = [...(map.get(g.uuid) ?? [])].sort(
      (a, b) => a.order - b.order || a.name.localeCompare(b.name),
    );
    return {
      id: g.uuid,
      name: g.name.trim() ? g.name : t("group.default"),
      isDefault: !g.name.trim(),
      items,
    };
  });
});

// 分组收缩状态（键是分组 uuid）
const collapsedGroups = ref<Record<string, boolean>>({});

function isCollapsed(id: string): boolean {
  return collapsedGroups.value[id] ?? false;
}

function toggleGroup(id: string) {
  collapsedGroups.value = { ...collapsedGroups.value, [id]: !isCollapsed(id) };
}

// ================= 侧栏：收起 / 展开 =================

/** 是否启用侧栏过渡：默认关闭，只在用户主动收起 / 展开的这 300ms 内打开。
    常开的话，启动时按配置恢复侧栏宽度也会被播成一次滑动动画 */
const sidebarAnim = ref(false);
let sidebarAnimTimer: number | undefined;

/** 用户主动收起 / 展开侧栏（唯一会播放动画的入口） */
function collapseSidebar(collapsed: boolean) {
  sidebarAnim.value = true;
  setSidebarCollapsed(collapsed);
  window.clearTimeout(sidebarAnimTimer);
  sidebarAnimTimer = window.setTimeout(() => {
    sidebarAnim.value = false;
  }, 300);
}

// ================= 启动器主页 =================

// 启动时先落到主页还是项目列表，由设置窗口「主窗口 → 启动页」决定；
// main.ts 的 bootstrap 在挂载前已把 gui_config 恢复进 startupPage，这里读到的就是最终值
const newsActive = ref(startupPage.value === "Home");
const newsLoading = ref(false);

/** 打开 / 关闭启动器主页（保留选中实例） */
function toggleNews() {
  newsActive.value = !newsActive.value;
}

// ---- 自定义主页面：服主导入的 zip 顶替内置主页，导入 / 删除在设置窗口「客户端设置」 ----
// 启用与否在 gui_config.client.customHome，已导入情况来自常驻压缩包，都由 status 现算

/** 自定义主页面状态；null 表示还没拉到或出错，一律回落内置主页 */
const customHome = ref<CustomHomeInfoDto | null>(null);

/** 是否用自定义页面顶替内置主页：启用 + 已导入 + 有入口 URL，任一不满足都回落，不白屏 */
const useCustomHome = computed(
  () =>
    !!customHome.value?.enabled && !!customHome.value.installed && !!customHome.value.entryUrl,
);

/** 拉取自定义主页面状态；没导入过就是 installed=false，不报错 */
async function loadCustomHome() {
  try {
    customHome.value = await commands.customHome.status();
  } catch {
    customHome.value = null;
  }
}

// 切换选中实例后自动滚动到详情顶部
//
// 监听 uuid 而不是 selected 对象本身：改实例设置会触发 instance-change → 重拉实例列表，
// 即 loadInstances，那里会把 selected 换成一个新对象，uuid 并没变。监听对象引用的话，
// 每次改设置都会把详情滚回顶部
const detailEl = ref<HTMLElement | null>(null);

watch(
  () => selected.value?.uuid,
  () => {
    nextTick(() => {
      if (detailEl.value) detailEl.value.scrollTop = 0;
    });
  },
);

/** 主页卡片的启动：选中当前实例并收起主页，再走正常启动流程 */
function quickLaunch() {
  const inst = selected.value;
  if (inst) {
    select(inst);
    launch();
  }
}

const news = ref<NewsItem[]>([]);
const newsPage = ref(1);
const newsHasMore = ref(true);
let newsLoadedPage = 0; // 已成功加载的页码（0 = 未加载）

/** 拉取指定页的新闻；页码从 1 开始，翻到空页时回退并禁用下一页 */
async function fetchNews(page: number) {
  if (newsLoading.value) return;
  newsLoading.value = true;
  try {
    const list = await api.getNews(page);
    if (list.length === 0 && page > 1) {
      newsHasMore.value = false;
      return;
    }
    newsHasMore.value = true;
    newsPage.value = page;
    news.value = list;
    newsLoadedPage = page;
  } catch (e) {
    console.warn("[news] 加载失败", e);
  } finally {
    newsLoading.value = false;
  }
}

/** 启动 / 核心加载完成后加载首页新闻（已加载则跳过） */
function loadNews() {
  if (newsLoadedPage) return;
  fetchNews(1);
}

function nextNewsPage() {
  if (!newsHasMore.value) return;
  fetchNews(newsPage.value + 1);
}

/** 打开新闻原文（系统浏览器） */
function openNews(url: string) {
  api.openUrl(url);
}

function prevNewsPage() {
  if (newsPage.value > 1) fetchNews(newsPage.value - 1);
}

// ================= 账户 =================

// 账户与当前账户来自共享存储（与账户窗口联动）
import {
  accounts as storeAccounts,
  currentAccount as storeCurrentAccount,
  setCurrentAccount,
  clearCurrentAccount,
} from "../../lib/accountStore";
import { listen } from "@tauri-apps/api/event";
import { LoadDone } from "../../lib/listens.ts";
import type { LoadState } from "../../lib/bindings";
const accounts = storeAccounts;
const currentAccount = storeCurrentAccount;

function onAccountChange(account: AccountStoreDto) {
  setCurrentAccount(account);
}

// ================= 启动状态 =================

const statusText = ref(t("launch.ready"));



// ================= 启动进度（顶部进度条） =================
// 进度百分比由后端按 LaunchState 阶段随状态事件下发；无进度语义的状态走滚动条
const launchStage = ref("");
/** 内核给的阶段百分比：按 LaunchState 等权近似，下载阶段恒为 36 */
const statePct = ref<number | null>(null);

/** 下载队列的真实进度 */
const downloadProgress = useDownloadProgress();

/** 界面上的百分比：下载阶段用真实下载进度，其它阶段用内核的阶段近似值 */
const launchPct = computed(() =>
  launchStage.value === "download" && downloadProgress.hasTasks.value
    ? Math.round(downloadProgress.percent.value)
    : statePct.value,
);

const launchStageText = computed(() =>
  launchStage.value ? stateText(launchStage.value) : statusText.value,
);

// ================= 启动参数（经 IPC 读写核心实例配置） =================

/** 列表模式下的实例设置面板，内容与分组模式下的设置区一致 */
const settingsOpen = ref(false);
const execOpen = ref(false);
const serverOpen = ref(false);
const proxyOpen = ref(false);
const jsonOpen = ref(false);

/** 打开独立日志窗口；实例日志只在那个窗口里看，主页面不展示 */
function openLogWindow() {
  if (selected.value) openWindow("log", { uuid: selected.value.uuid });
}

function toggleSettings() {
  settingsOpen.value = !settingsOpen.value;
}
// 启动参数（懒加载骨架值 / 防抖写回）与启动统计（时长 / 次数）：见 composables/ 下的组合式函数
const { argsOf, updateArgs, onServerIp, onServerJoin } = useInstanceArgs({ selected });
const { statsOf, playHoursOf, refresh: refreshStats } = useInstanceStats();

/** 列表模式图标下那行：选中实例的启动统计；未选中实例时为空串，那一行不渲染 */
const listStats = computed(() => {
  const inst = selected.value;
  if (!inst) return "";
  return t("list.launchStats", {
    count: statsOf(inst.uuid)?.count ?? 0,
    hours: playHoursOf(inst.uuid),
  });
});

// ================= 实例操作 =================

type ActionId =
  | "addResource"
  | "manageResource"
  | "export"
  | "openFolder"
  | "viewLog"
  | "editConfig"
  | "rename"
  | "delete";

const ACTION_LABELS: Record<ActionId, string> = {
  addResource: "actions.addResource",
  manageResource: "actions.manageResource",
  export: "actions.export",
  openFolder: "actions.openFolder",
  viewLog: "actions.viewLog",
  editConfig: "actions.editConfig",
  rename: "actions.rename",
  delete: "actions.delete",
};

/** 打开实例导出窗口（导出整合包） */
function openExportWindow() {
  if (!selected.value) return;
  openWindow("export", { uuid: selected.value.uuid });
}

// ================= 元信息：版本 / 加载器 / 整合包 / 语言，合并进实例设置 =================

async function onMetaUpdate(patch: Partial<InstanceInfoDto>) {
  if (!selected.value) return;
  try {
    await api.updateInstance(selected.value.uuid, patch);
    await loadInstances();
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 实例设置面板刷新版本列表（清空后端缓存重新拉取） */
function onVersionsRefreshed(list: VersionInfoDto[]) {
  versions.value = list;
}

// ================= 分组拖拽移动 =================

// ================= 自定义拖拽：useInstanceDrag，见下方多选初始化之后 =================

/** 点击分组标题，拖拽结束后抑制误触折叠 */
function onGroupTitleClick(id: string) {
  if (consumeSuppressClick()) return;
  toggleGroup(id);
}

// ================= 多选模式：逻辑见 composables/useMultiSelect =================
// 入口：右键分组 → 全选 → 进入多选；多选模式下右键实例出现操作菜单。

const {
  multiSelect,
  selectedIds,
  enterMultiSelect,
  exitMultiSelect,
  toggleSelect,
} = useMultiSelect({
  selected,
  newsActive,
  collapsedGroups,
  onExit: closeCtxMenu,
});

const {
  dragActive,
  ghost: instanceGhost,
  draggingUuid,
  onDragPointerDown,
  isInstInsert,
  isInstInsertEnd,
  isGroupInsert,
  consumeSuppressClick,
} = useInstanceDrag({
  multiSelect,
  groups,
  collapsedGroups,
  loadInstances,
  loadGroups,
});

/** 实例点击：多选模式切换勾选，普通模式单选（拖拽结束后忽略误触点击） */
function onInstClick(inst: InstanceInfoDto) {
  if (consumeSuppressClick()) return;
  if (multiSelect.value) toggleSelect(inst);
  else select(inst);
}

// ----- 自定义右键菜单 -----

const ctxMenu = ref<CtxMenuState | null>(null);
/** 目标分组选择弹窗是否打开；转移分组与移动选中实例共用 */
const showMoveGroupPick = ref(false);
/** 要转移的源分组；null 表示移动选中的实例 */
const groupMoveSource = ref<string | null>(null);
/** 弹窗下拉框里选中的目标分组 uuid */
const moveGroupTarget = ref("");

function openCtxMenu(e: MouseEvent, payload: Omit<CtxMenuState, "x" | "y">) {
  // 简单边界钳制，避免菜单超出窗口
  const x = Math.min(e.clientX, window.innerWidth - 200);
  const y = Math.min(e.clientY, window.innerHeight - 180);
  ctxMenu.value = { x, y, ...payload };
  groupMoveSource.value = null;
}

function closeCtxMenu() {
  ctxMenu.value = null;
}

/** 右键分组标题：菜单提供“全选”进入多选 */
function onGroupContext(e: MouseEvent, groupId: string) {
  openCtxMenu(e, { kind: "group", groupId });
}

/** 分组菜单“全选”：选中该分组全部实例并进入多选 */
function onGroupSelectAll(groupId?: string) {
  const g = groups.value.find((x) => x.id === groupId);
  closeCtxMenu();
  if (!g || g.items.length === 0) return;
  enterMultiSelect(g.items.map((i) => i.uuid));
  showToast(t("multi.selectAllDone", { count: g.items.length }));
}

/** 分组菜单“启动全部”：启动该分组全部实例 */
async function launchGroupAll(groupId?: string) {
  const g = groups.value.find((x) => x.id === groupId);
  closeCtxMenu();
  if (!g || g.items.length === 0) return;
  for (const inst of g.items) {
    markLaunching(inst.uuid);
    try {
      await api.launchGame(inst.uuid);
    } catch {
      // 单个失败：把它的按钮放开，别的照旧
      clearLaunching(inst.uuid);
    }
  }
  showToast(t("multi.launched", { count: g.items.length }));
}

/** 分组菜单“转移分组”：源分组就是当前右键的那个分组 */
function onMoveGroupClick() {
  // 菜单一关 ctxMenu 就清空，右键目标要在这之前取出来
  const id = ctxMenu.value?.groupId;
  if (id) openMoveGroupPicker(id);
}

/** 打开"选择目标分组"弹窗；`sourceId` 为 null 表示移动当前多选的实例 */
function openMoveGroupPicker(sourceId: string | null) {
  // 顺序有讲究：closeCtxMenu 之后才记源分组，否则会被菜单的复位清掉
  closeCtxMenu();
  groupMoveSource.value = sourceId;
  // 预设第一个候选（候选里已经排掉了源分组本身），免得"确定"下去等于没动
  moveGroupTarget.value = movePickOptions.value[0]?.id ?? "";
  showMoveGroupPick.value = true;
}

/** 候选里的具名分组（默认分组在候选里单独占一项） */
const moveTargets = computed(() => groups.value.filter((g) => !g.isDefault));

/** 有没有"别的分组"可去：只有默认分组一个组时，改分组这件事本身就无从谈起 */
const canChangeGroup = computed(() => groups.value.length > 1);

/** 默认分组的视图（候选里要显示它的实例数） */
const defaultGroupView = computed(() => groups.value.find((g) => g.isDefault));

/**
 * 下拉框里的候选：默认分组在前，其余按显示顺序
 *
 * **源分组自己不进候选**：选它等于没动，留在下拉里只会让人犹豫一下。
 * 所以"转移默认分组"时这里只剩具名分组，"转移具名分组"时默认分组也在。
 */
const movePickOptions = computed(() => {
  const list = [
    {
      id: defaultGroupId.value,
      name: t("group.default"),
      count: defaultGroupView.value?.items.length ?? 0,
    },
    ...moveTargets.value.map((g) => ({ id: g.id, name: g.name, count: g.items.length })),
  ];
  return list.filter((o) => o.id !== groupMoveSource.value);
});

/** 弹窗里那句说明：按"转移整个分组"或"移动选中实例"两种来源取文案 */
const movePickDesc = computed(() => {
  const source = groupMoveSource.value;
  if (!source) return t("group.pickMultiDesc", { count: selectedIds.value.size });
  const g = groups.value.find((x) => x.id === source);
  return t("group.pickGroupDesc", { name: g?.name ?? "", count: g?.items.length ?? 0 });
});

/** 弹窗里点"确定"：把下拉框选中的目标交出去 */
async function confirmMoveTarget() {
  const id = moveGroupTarget.value;
  if (!id) return;
  // 默认分组按后端口径用 null 表示
  await pickMoveTarget(id === defaultGroupId.value ? null : id);
}

/** 弹窗里选定目标分组 */
async function pickMoveTarget(targetId: string | null) {
  const source = groupMoveSource.value;
  showMoveGroupPick.value = false;
  groupMoveSource.value = null;
  if (source) await moveGroupTo(source, targetId);
  else await moveSelectedToGroup(targetId);
}

/** 把源分组的全部实例合并到目标分组（null = 默认分组），保留空分组 */
async function moveGroupTo(sourceId: string, targetId: string | null) {
  const g = groups.value.find((x) => x.id === sourceId);
  const target = targetId ?? defaultGroupId.value;
  if (!g || g.items.length === 0 || target === sourceId) return;
  for (const inst of g.items) {
    // 已经在目标组里的不必再发一次；一次一条 IPC，省掉无谓往返
    if (groupIdOf(inst) !== target) {
      await api.updateInstance(inst.uuid, { group: targetId });
    }
  }
  await loadInstances();
  showToast(t("multi.moved", { count: g.items.length }));
}

/** 删除分组（组内实例移至默认分组） */
const showDeleteGroup = ref(false);
const deleteGroupId = ref("");
const deleteGroupName = ref("");
const deleteGroupCount = ref(0);
const deleteGroupBusy = ref(false);

function onDeleteGroup(groupId?: string) {
  // 默认分组删不得；它的名字是空白，按 uuid 判，不看名字
  if (!groupId || groupId === defaultGroupId.value) return;
  const g = groups.value.find((x) => x.id === groupId);
  closeCtxMenu();
  if (!g) return;
  deleteGroupId.value = g.id;
  deleteGroupName.value = g.name;
  deleteGroupCount.value = g.items.length;
  showDeleteGroup.value = true;
}

async function doDeleteGroup() {
  deleteGroupBusy.value = true;
  const id = deleteGroupId.value;
  const name = deleteGroupName.value;
  for (const inst of instances.value) {
    if (groupIdOf(inst) === id) {
      await api.updateInstance(inst.uuid, { group: null });
    }
  }
  await api.removeGroup(id);
  deleteGroupBusy.value = false;
  showDeleteGroup.value = false;
  await Promise.all([loadInstances(), loadGroups()]);
  showToast(t("group.deleted", { name }));
}

/** 右键实例：多选模式打开操作菜单；普通模式弹出实例操作菜单 */
function onInstContext(e: MouseEvent, inst: InstanceInfoDto) {
  if (multiSelect.value) {
    // 右键未勾选的实例时先加入选择
    if (!selectedIds.value.has(inst.uuid)) {
      const s = new Set(selectedIds.value);
      s.add(inst.uuid);
      selectedIds.value = s;
    }
    openCtxMenu(e, { kind: "multi" });
  } else {
    // 实例锁定时选不中别的实例，菜单动作又是按选中实例执行的，这里直接不给开
    if (lockActive.value && inst.uuid !== lockUuid.value) return;
    // 普通模式：先选中该实例，再弹出实例菜单
    select(inst);
    openCtxMenu(e, { kind: "instance", instance: inst });
  }
}

/** 实例右键菜单动作（与详情面板一致） */
function onInstMenuAction(id: InstMenuAction) {
  // 菜单一关 ctxMenu 就清空，右键目标要在这之前取出来（"修改图标"要用）
  const target = ctxMenu.value?.instance ?? null;
  closeCtxMenu();
  switch (id) {
    case "launch":
      launch();
      break;
    case "changeIcon":
      if (target) void pickIconFile(target);
      break;
    case "rename":
      onAction("rename");
      break;
    case "delete":
      onAction("delete");
      break;
    default:
      onAction(id);
  }
}

/** "修改图标"的目标实例与已选图片（弹窗只在选到图之后才开） */
const iconPickInst = ref<InstanceInfoDto | null>(null);
const iconPickPath = ref("");

/**
 * 右键菜单「选择图片」：**直接开系统文件对话框**，不先弹一层自己的窗口
 *
 * 选到图才开截图弹窗 —— 裁剪范围要用图片本身，那一步需要界面；
 * 取消选择就什么都不发生。
 */
async function pickIconFile(inst: InstanceInfoDto) {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({
      title: t("actions.pickImage"),
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
    if (typeof path !== "string") return;
    iconPickPath.value = path;
    iconPickInst.value = inst;
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 顶部工具栏的“修改分组”：直接开目标分组弹窗（源 = 当前多选的实例） */
function openMoveFromBar() {
  openMoveGroupPicker(null);
}

/** 把选中的实例移动到指定分组（null = 默认分组） */
async function moveSelectedToGroup(targetId: string | null) {
  const ids = [...selectedIds.value];
  const target = targetId ?? defaultGroupId.value;
  for (const uuid of ids) {
    const inst = instances.value.find((i) => i.uuid === uuid);
    if (inst && groupIdOf(inst) !== target) {
      await api.updateInstance(uuid, { group: targetId });
    }
  }
  closeCtxMenu();
  await loadInstances();
  collapsedGroups.value = { ...collapsedGroups.value, [target]: false };
  showToast(t("multi.moved", { count: ids.length }));
}

/** 多选删除 */
const showMultiDelete = ref(false);
const multiDeleteBusy = ref(false);

// 多选删除进度：整体进度可知（已完成 / 总数），单实例内部不可知
const multiDone = ref(0);
const multiTotal = ref(0);

/** 多选删除总进度百分比（0-100） */
const multiPct = computed(() => {
  if (!multiTotal.value) return 0;
  return Math.min(100, Math.round((multiDone.value / multiTotal.value) * 100));
});

function onMultiDelete() {
  closeCtxMenu();
  showMultiDelete.value = true;
}

async function doMultiDelete() {
  multiDeleteBusy.value = true;
  const ids = [...selectedIds.value];
  multiDone.value = 0;
  multiTotal.value = ids.length;
  // 逐条删：正忙（在下载 / 游戏在跑）的那几个会被后端拦住，先记下、删完一起提示，
  // 一条被拒不该把后面几条一起带走
  let failed = 0;
  let reason = "";
  try {
    for (const uuid of ids) {
      try {
        await api.deleteInstance(uuid);
        multiDone.value += 1;
      } catch (e) {
        failed += 1;
        if (!reason) reason = tErr(e);
      }
    }
  } finally {
    multiDeleteBusy.value = false;
    multiTotal.value = 0;
  }
  showMultiDelete.value = false;
  exitMultiSelect();
  await Promise.all([loadInstances(), loadGroups()]);
  showToast(t("multi.deleted", { count: ids.length - failed }));
  if (failed) showToast(t("multi.deleteFailed", { count: failed, reason }));
}

/** 启动所有选中实例 */
async function multiLaunch() {
  const ids = [...selectedIds.value];
  closeCtxMenu();
  for (const uuid of ids) {
    markLaunching(uuid);
    try {
      await api.launchGame(uuid);
    } catch {
      // 单个失败：把它的按钮放开，别的照旧
      clearLaunching(uuid);
    }
  }
  exitMultiSelect();
  showToast(t("multi.launched", { count: ids.length }));
}

// 点击空白处关闭菜单；Esc 依次关闭菜单 / 退出多选
function onDocClick() {
  if (ctxMenu.value) closeCtxMenu();
}

function onDocKeyDown(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  if (ctxMenu.value) closeCtxMenu();
  else if (multiSelect.value) exitMultiSelect();
}

/** 刷新实例列表并选中给定实例；添加 / 整合包安装完成后调用 */
async function adoptAddedInstance(uuid: string) {
  await Promise.all([loadInstances(), loadGroups()]);
  const inst = instances.value.find((i) => i.uuid === uuid);
  if (inst) select(inst);
}

/** 添加实例窗口创建成功后（跨窗口存储通知），刷新并选中新实例 */
async function onAddedInstanceStorage(newValue: string | null) {
  if (!newValue) return;
  await adoptAddedInstance(newValue);
}

/** 上面那条通知的退订函数（挂载时订阅，卸载时摘掉） */
let unlistenAdded: (() => void) | null = null;

// 轻提示统一使用全局 showToast
const showRename = ref(false);
const renameName = ref("");
const renameBusy = ref(false);
const showDelete = ref(false);
const deleteBusy = ref(false);

function onAction(id: ActionId) {
  if (!selected.value) return;
  switch (id) {
    case "rename":
      renameName.value = selected.value.name;
      showRename.value = true;
      break;
    case "delete":
      showDelete.value = true;
      break;
    case "manageResource":
      // 跳转到资源管理窗口；它是**一个实例一个**的窗口族，实例必须带上（窗口身份由它派生），
      // 已经在时只聚焦，所以重复点不会开出第二个
      openWindow("resource", { uuid: selected.value.uuid });
      break;
    case "addResource":
      // 跳转到添加资源窗口；它也是**一个实例一个**的窗口族，实例必须带上
      openWindow("add_resource", { uuid: selected.value.uuid });
      break;
    case "viewLog":
      // 日志只在独立窗口里看（主页面不展示日志内容）
      openLogWindow();
      break;
    case "openFolder":
      // 打开的是**实例目录**（instances/<实例名>/），不是游戏目录 .minecraft
      void openInstanceDirOf(selected.value);
      break;
    case "editConfig":
      // 配置文件窗口也是**一个实例一个**的窗口族，实例必须带上
      openWindow("config", { uuid: selected.value.uuid });
      break;
    default:
      showToast(t("actions.wip", { name: t(ACTION_LABELS[id]) }));
  }
}

/** 在文件管理器里打开某个实例的目录；失败时把后端错误串提示出来 */
async function openInstanceDirOf(instance: InstanceInfoDto | null) {
  if (!instance) return;
  try {
    await openInstanceDir(instance.uuid);
  } catch (e) {
    showToast(tErr(e));
  }
}

async function doRename() {
  if (!selected.value) return;
  renameBusy.value = true;
  try {
    const ok = await api.renameInstance(selected.value.uuid, renameName.value);
    if (ok) {
      showRename.value = false;
      await loadInstances();
      showToast(t("actions.rename"));
    }
  } catch (e) {
    // 重名等核心错误直接提示
    showToast(tErr(e));
  } finally {
    renameBusy.value = false;
  }
}

async function doDelete() {
  if (!selected.value) return;
  deleteBusy.value = true;
  try {
    await api.deleteInstance(selected.value.uuid);
    showDelete.value = false;
    selected.value = null;
    await Promise.all([loadInstances(), loadGroups()]);
  } catch (e) {
    // 删除失败（如文件被占用）直接提示
    showToast(tErr(e));
  } finally {
    deleteBusy.value = false;
  }
}

// ================= 启动器功能入口（顶部栏） =================

const features: Array<{ id: FeatureId; icon: string }> = [
  { id: "settings", icon: "gear" },
  { id: "stats", icon: "chart" },
  { id: "collect", icon: "star" },
  { id: "help", icon: "book" },
];

// ================= 添加分组 弹窗 =================

const versions = ref<VersionInfoDto[]>([]);

// ================= 拖拽文件：逻辑见 composables/useFileDrop =================

/**
 * 拖入的文件 / 文件夹 / 登录卡片：认出类型后交给对应窗口
 *
 * - **网页卡片**（DOM 给的文本地址）与 `.url` / `.webloc` 快捷方式文件 → 「账户管理」预填地址；
 * - 整合包与文件夹 → 「添加实例」，那边有完整的导入链路，主窗口只负责"认类型 + 开窗"。
 *
 * 多个文件只取第一个。
 */
async function onFilesDropped(payload: FileDropPayload) {
  // ① 网页卡片：地址已经在手上（DOM 那条路），直接开账户窗口
  if (payload.url) {
    openWindow("account", { drop: { path: "", kind: "shortcut", url: payload.url } });
    return;
  }
  // ② 文件：路径由路径桥补上，照旧认类型再开窗
  const first = payload.paths[0];
  if (!first) {
    // 路径桥没把真实路径送回来（文件拖入拿不到路径）：说一声，别静默什么都不做
    showToast(t("drop.noPath"));
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
  if (probe.kind === "shortcut" && probe.url) {
    openWindow("account", { drop: { path: first, kind: "shortcut", url: probe.url } });
    return;
  }
  if (probe.kind !== "folder" && probe.kind !== "archive") {
    showToast(t("drop.notModpack"));
    return;
  }
  openWindow("add", { drop: { path: first, kind: probe.kind } });
}

const { fileDragOver, dragKind } = useFileDrop(onFilesDropped);

/**
 * 拖入遮罩的标题：按这次拖的是哪一档选，见 useFileDrop 的 DragKind
 *
 * 悬停阶段拿不到文件名，所以"整合包"只在 MIME 认得出来时才算。
 */
const DROP_TITLE_KEYS: Record<string, string> = {
  text: "drop.titleLogin",
  archive: "drop.titleArchive",
  folder: "drop.titleFolder",
  files: "drop.titleFiles",
};
const dropTitleKey = computed(() => DROP_TITLE_KEYS[dragKind.value] ?? DROP_TITLE_KEYS.files);

const showAddGroup = ref(false);
const groupName = ref("");
const groupError = ref("");
const groupAdding = ref(false);

// ================= 服务器 MOTD 卡片 =================
// 地址来源、竞态保护与定时刷新见 composables/useMotd；卡片外观在 components/MotdCard.vue，
// 本窗口的悬浮卡、实例详情卡与资源窗口服务器列表上方那张三处共用同一份

// ---- 客户端设置，存在 gui_config.client：MOTD 卡片显示与自动刷新间隔 ----
const clientConfig = ref<ClientConfig>({
  motdCard: true,
  motdInterval: 15,
  loginLockOn: false,
  loginLock: [],
  autoJoin: false,
  autoJoinServer: "",
  motdServer: "",
  lockInstance: "",
  customHome: false,
  musicOn: false,
  musicSource: "",
});

// ---- 实例锁定：只允许使用锁定的那个实例，入口在设置窗口「客户端设置 → 实例锁定」 ----
/** 锁定的实例 uuid（空 = 未锁定） */
const lockUuid = computed(() => clientConfig.value.lockInstance);
/** 锁定是否生效：uuid 非空且实例还在；实例被删掉后走 lockMissing 的错误页 */
const lockActive = computed(
  () => !!lockUuid.value && instances.value.some((i) => i.uuid === lockUuid.value),
);
/** 实例列表是否已加载过：区分「还在加载」与「确实没有这个实例」，避免启动瞬间闪错误页 */
const instancesLoaded = ref(false);
/** 锁定的实例已不存在，例如整合包作者的实例目录被删 / 改名：主窗口整块换成错误页 */
const lockMissing = computed(
  () =>
    instancesLoaded.value &&
    !!lockUuid.value &&
    !instances.value.some((i) => i.uuid === lockUuid.value),
);
/** 锁定生效时强制按列表模式渲染；只覆盖渲染，不写回 gui_config，解锁后自动回到原模式 */
const effectiveMode = computed<ViewMode>(() => (lockActive.value ? "list" : mode.value));

// 锁定生效时把选中实例压到锁定实例：切换入口已隐藏，若还停在别处（比如上次退出时选的是另一个）
// 就会启动错的实例。不走 select()——那会关掉启动器主页。
watch(
  () => (lockActive.value ? lockUuid.value : ""),
  (uuid) => {
    if (!uuid || selected.value?.uuid === uuid) return;
    const inst = instances.value.find((i) => i.uuid === uuid);
    if (!inst) return;
    selected.value = inst;
    setSelectedInstance(inst.uuid);
  },
);

// ---- 登录方式锁定：账户选择列表只显示锁定类型，当前账户被滤掉就取消选择 ----
/** 锁定的登录类型；总开关关闭或列表为空时不过滤 */
const lockedTypes = computed(
  () =>
    new Set(
      clientConfig.value.loginLockOn
        ? clientConfig.value.loginLock.map((e) => e.ty)
        : [],
    ),
);

/** 过滤后的账户列表（传给顶栏选择器） */
const visibleAccounts = computed(() => {
  const lock = lockedTypes.value;
  const list = accounts.value;
  return lock.size ? list.filter((a) => lock.has(a.authType)) : list;
});

watch([lockedTypes, currentAccount], ([lock, acc]) => {
  if (lock.size && acc && !lock.has(acc.authType)) void clearCurrentAccount();
});

// 两处 MOTD 卡片（悬浮卡 / 实例详情卡）：地址来源、竞态保护与定时刷新都在这里
const {
  motdInfo,
  motdLoading,
  instMotd,
  instMotdLoading,
  motdCardVisible,
  refreshMotd,
  restartMotdTimer,
} = useMotd({ clientConfig, selected, argsOf });

/** 设置窗口保存 client 后广播：实时更新卡片显隐与刷新间隔 */
function onClientConfig(c: ClientConfig) {
  clientConfig.value = c;
  // 没配地址就没东西可查，卡片不显示
  motdCardVisible.value = c.motdCard && !!c.motdServer.trim();
  restartMotdTimer();
  void refreshMotd();
  // 「启用自定义主页面」开关在这里；enabled 由 status 现算，顺手回拉一次
  void loadCustomHome();
}

// ================= 事件订阅 =================

const unlistens: Array<() => void> = [];

// 整合包安装任务：进度本身已交给标题栏上的 ModpackTitleIndicator 指示器，
// 这里只借它做一件事 —— 安装成功后刷新实例列表并选中刚装好的那个，即 adoptAddedInstance。
// 所以不取 status，只用 init 返回的订阅。
const { init: initModpackStatus } = useModpackStatus(false, adoptAddedInstance);

async function subscribeEvents() {
  const fns = await Promise.all([
    onLaunchState((e) => {
      if (e.uuid !== selected.value?.uuid) return;
      statusText.value = stateText(e.state);
      launchStage.value = e.state;
      statePct.value = e.progress ?? null;
    }),
    onGameExit((e) => {
      if (e.uuid !== selected.value?.uuid) return;
      statusText.value =
        e.code === 0 ? t("launch.exited") : t("launch.exitedCode", { code: e.code });
      launchStage.value = "";
      statePct.value = null;
      clearLaunching(e.uuid);
      // 状态行只在运行中显示，退出之后界面上什么都不剩：非正常退出得说出来
      if (e.code !== 0) showToast(t("launch.exitedCode", { code: e.code }));
      // 这一局的时长 / 次数刚记进内核统计，重拉一次，界面上那行不是启动前的旧值
      void refreshStats();
    }),
    onLaunchError((e) => {
      if (e.uuid && e.uuid !== selected.value?.uuid) return;
      statusText.value = t("launch.failed");
      launchStage.value = "";
      statePct.value = null;
      if (e.uuid) clearLaunching(e.uuid);
      // 同上：失败的结论不能只写在那行状态里，它随 running 一起消失
      showToast(t("launch.error", { msg: e.message }));
    }),
    onInstanceChange(() => {
      loadInstances();
      loadGroups();
    }),
    // 自定义主页面导入 / 删除：该动作不改 gui_config，client-config-change 不会发
    onCustomHomeChange(() => {
      void loadCustomHome();
    }),
    // Java 列表变更：添加 / 删除 / 配置加载完成，都重新拉取
    onJavaChange(() => {
      loadJava();
    }),
    // 显示模式在设置窗口改：多窗口模式下这里是另一个 webview，只能靠这条广播同步
    onGuiViewModeChange((v) => {
      viewMode.value = v;
    }),
  ]);
  unlistens.push(...fns);
}

// ================= 初始化 =================

// 只初始化一次：load-done 事件与下面的状态查询兜底可能先后到达
let inited = false;

/** 首次启动的 ColorMC 迁移弹窗数据（非空 = 弹窗显示中） */
const colorMcInfo = ref<ColorMcInfoDto | null>(null);
/** 弹窗"已结束"信号：答完（复制 / 移动 / 不迁移）或直接关掉时解开 */
let colorMcSettled: (() => void) | null = null;

/**
 * 问后端有没有可迁移的 ColorMC 数据，**在加载期间问，主页面之前**
 *
 * 探测只读几个目录项、不依赖内核，所以挂载时就能发起（见 onMounted），与核心加载并行；
 * 一旦要弹窗，就**等用户答完**才放行主界面 —— 加载页继续盖着，见 doInit。
 * 后端已问过（标记文件存在）或本机没有 ColorMC 时返回 null，直接放行。
 */
async function checkColorMc() {
  try {
    const info = await api.checkColorMc();
    if (!info) return;
    colorMcInfo.value = info;
    await new Promise<void>((resolve) => {
      colorMcSettled = resolve;
    });
  } catch {
    colorMcInfo.value = null;
  }
}

/** 弹窗关闭：三种选择任一或右上角直接关掉，都放行主界面 */
function onColorMcClosed() {
  colorMcInfo.value = null;
  colorMcSettled?.();
  colorMcSettled = null;
}

/**
 * 迁移完成
 *
 * 实例是"文件夹搬进来"的，运行中的核心**不会**重新扫实例目录，见 windows/colormc.rs，
 * 所以权威结果是重启后再看；这里先把列表拉一次，万一被文件监听捡到了也能立刻显示。
 */
function onColorMcDone() {
  void loadInstances();
  void loadGroups();
}

/**
 * 迁移探测任务：**建组件时就发起**（此刻还在加载页上），与核心加载并行；
 * `doInit` 里 await 它，答完（或关掉弹窗）才关启动页 —— 于是"搬运在主页面之前"。
 */
const colorMcReady = checkColorMc();

async function doInit() {
  if (inited) return;
  inited = true;
  try {
    await Promise.all([loadInstances(), loadGroups(), loadJava(), loadCustomHome()]);
    restoreSelection();
    // ColorMC 迁移在主页面**之前**：加载期间就弹了，这里等它答完再关启动页
    await colorMcReady;
    closeSplash();
    // 启动参数（命令行 / 协议）留到实例表就绪之后再执行
    void handleStartArg();
    loadVersions();
    loadNews();
  } catch (e) {
    // 初始化失败：关闭启动页并显示错误页
    closeSplash(String(e));
  }
}

/** 初始化失败反馈：打开 GitHub Issues */
function openFeedback() {
  api.openUrl("https://github.com/Coloryr/ModernColoryrMinecraftLauncher/issues");
}

// ================= 数据加载 =================

async function loadInstances() {
  instances.value = (await api.getInstances()).map((i) =>
    // 启动中的实例后端还没登记成运行中，别让它在这一刻变回可点
    launching.value.has(i.uuid) ? { ...i, running: true } : i,
  );
  instancesLoaded.value = true;
  // 保留当前选中；不自动选中实例（默认停在启动器主页）
  if (selected.value) {
    const now = instances.value.find((i) => i.uuid === selected.value?.uuid);
    if (now) selected.value = now;
    else selected.value = null;
  }
}

/** 启动时按 gui_config 恢复上次选中的实例。
 *  故意不走 select()：那会把 newsActive 置 false，而启动页设成「启动器主页」时应停在主页。 */
function restoreSelection() {
  const uuid = selectedInstance.value;
  if (!uuid) return;
  selected.value = instances.value.find((i) => i.uuid === uuid) ?? null;
}

async function loadGroups() {
  try {
    groupList.value = await api.getGroups();
  } catch {
    groupList.value = [];
  }
}

async function loadJava() {
  try {
    javas.value = await api.getJavaList();
    // 不给空的 javaName 补第一个 Java：空就是「自动」，由 core 按版本要求的
    // Java 主版本去挑（比"列表里第一个"更对；这条以前会把刚扫到的 Java 顶上去）
  } catch {
    javas.value = [];
  }
}

async function loadVersions() {
  try {
    versions.value = await api.getVersions();
  } catch {
    versions.value = [];
  }
}

// ================= 启动 =================
// 不弹窗：只有正在运行的实例的启动按钮显示加载态并禁用

/**
 * 正在启动中的实例 uuid
 *
 * 后端要到**进程真正起来**才把实例算作运行中（`is_running`），检查 / 下载那段窗口期它还是
 * false；这期间只要列表刷新一次，按钮就会变回可点，于是能重复点出两次启动。界面靠这一份顶住：
 * 启动失败、进程退出、IPC 报错时摘掉。
 */
const launching = ref<Set<string>>(new Set());

/**
 * 记下一次启动尝试，并把该实例的按钮按住
 *
 * - `uuid`: 实例 uuid
 */
function markLaunching(uuid: string) {
  // Set 要整体换掉才会触发响应式
  launching.value = new Set(launching.value).add(uuid);
  const inst = instances.value.find((i) => i.uuid === uuid);
  if (inst) inst.running = true;
}

/**
 * 结束一次启动尝试：摘掉"启动中"，按钮恢复可点
 *
 * - `uuid`: 实例 uuid
 */
function clearLaunching(uuid: string) {
  if (launching.value.has(uuid)) {
    const next = new Set(launching.value);
    next.delete(uuid);
    launching.value = next;
  }
  const inst = instances.value.find((i) => i.uuid === uuid);
  if (inst) inst.running = false;
}

/**
 * 处理启动参数（命令行 / `mml:` 协议）：启动实例，或用文件 / 文件夹打开「添加实例」
 *
 * 参数由 Rust 在进程启动时先存下，这里在实例表加载完之后取走执行 ——
 * 启动那一刻内核实例表还没加载，所以不能在后端直接动手。
 */
async function handleStartArg() {
  let arg: StartArgDto | null = null;
  try {
    arg = await api.takeStartArg();
  } catch {
    return;
  }
  if (!arg) return;

  if (arg.action === "install") {
    // 与拖入文件同一个落点：先认类型，再交给「添加实例」
    try {
      const probe = await commands.windows.probeDrop(arg.value);
      if (probe.kind === "shortcut" && probe.url) {
        openWindow("account", { drop: { path: arg.value, kind: "shortcut", url: probe.url } });
        return;
      }
      if (probe.kind !== "folder" && probe.kind !== "archive") {
        showToast(t("drop.notModpack"));
        return;
      }
      openWindow("add", { drop: { path: arg.value, kind: probe.kind } });
    } catch (e) {
      showToast(tErr(e));
    }
    return;
  }

  const inst = instances.value.find((i) => i.uuid === arg.value);
  if (!inst) {
    showToast(t("err.gameNotFound"));
    return;
  }
  select(inst);
  // 实例锁定生效时 select 会拒绝切换：这时候别顺手把锁定的那个实例启动了
  if (selected.value?.uuid !== arg.value) {
    showToast(t("err.gameNotFound"));
    return;
  }
  await launch();
}

async function launch() {
  if (!selected.value || selected.value.running) return;
  const uuid = selected.value.uuid;
  markLaunching(uuid);
  statusText.value = t("launch.launching");
  launchStage.value = "";
  statePct.value = null;
  try {
    await api.launchGame(uuid);
  } catch (e) {
    clearLaunching(uuid);
    statusText.value = t("launch.failed");
    showToast(t("launch.error", { msg: tErr(e) }));
  }
}

function onPickInstance(uuid: string) {
  const inst = instances.value.find((i) => i.uuid === uuid);
  if (inst) select(inst);
}

// ================= 添加实例（独立窗口） =================

function openAdd() {
  // 实例锁定生效时禁止新建实例；入口本就随侧边栏 / 禁用按钮不可达，这里兜底
  if (lockActive.value) return;
  openWindow("add");
}

// ================= 添加分组 =================

async function createGroup() {
  if (!groupName.value.trim()) {
    groupError.value = t("group.nameEmpty");
    return;
  }
  groupAdding.value = true;
  groupError.value = "";
  const uuid = await api.addGroup(groupName.value.trim());
  if (!uuid) {
    groupError.value = t("group.exists");
    groupAdding.value = false;
    return;
  }
  groupAdding.value = false;
  groupName.value = "";
  showAddGroup.value = false;
  await loadGroups();
  // 新组默认是展开的，别让用户以为没建上
  collapsedGroups.value = { ...collapsedGroups.value, [uuid]: false };
}

// ================= 生命周期 =================

/** 主窗口根元素：贴靠热区在里面惰性查找自己的最大化按钮 */
const rootEl = ref<HTMLElement | null>(null);

// 窗口装饰：激活插件装饰 + 交出贴靠热区，与其它窗口共用同一套逻辑。
// 主窗口的特别之处只在"启动画面"——标题栏要等核心 load 完成才渲染，
// waitForSnapTarget 会一直等到那时，所以这里无需额外处理
useWindowDecoration(rootEl);

onMounted(async () => {
  subscribeEvents();
  initModpackStatus().then((unlisten) => unlistens.push(unlisten));
  document.addEventListener("click", onDocClick);
  document.addEventListener("keydown", onDocKeyDown);
  unlistenAdded = onStorageChange(KEYS.addedInstance, (v) => void onAddedInstanceStorage(v));
  // 客户端设置：初始加载 + 设置窗口保存后实时跟随
  try {
    const cfg = await loadGuiConfig();
    if (cfg?.client) onClientConfig(cfg.client);
  } catch {
    /* 浏览器环境忽略 */
  }
  onClientConfigChange(onClientConfig).then((unlisten) => unlistens.push(unlisten));
});

onUnmounted(() => {
  unlistens.forEach((fn) => fn());
  document.removeEventListener("click", onDocClick);
  document.removeEventListener("keydown", onDocKeyDown);
  unlistenAdded?.();
});

// 后端核心加载完成事件：决定关闭启动页（进入主界面）还是显示错误页
listen<LoadState>(LoadDone, async (data) => {
  const state = data.payload;
  if (state.ok) {
    doInit();
  } else {
    closeSplash(state.error || t("init.failed"));
  }
});

// 兜底：启动很快时 load-done 可能在页面监听注册前就已发出而被错过，
// 挂载后主动查一次核心状态（已加载则直接进主界面）
onMounted(async () => {
  try {
    const state = await getLoadState();
    if (state.ok) {
      doInit();
    }
  } catch {
    /* 浏览器环境忽略，等 load-done 事件 */
  }
});
</script>

<template>
  <div class="main-window" ref="rootEl">
    <!-- 加载期间：**只画一个关闭按钮**的标题栏，不放整条顶栏。
         两颗不画的按钮用 visibility 占位，见 .boot-head 的样式 —— 自绘装饰激活要量
         "最大化按钮"的矩形，即 lib/decoration.ts 的 findMaximizeBtn，量不到插件会退回
         原生 frame、系统标题栏就又冒出来。 -->
    <header v-if="splashVisible || splashError" class="boot-head" :class="titleBarStyle"
      @pointerdown="onTitleBarPointerDown">
      <span class="spacer" />
      <WindowControls :style="titleBarStyle" />
    </header>

    <!-- 主界面顶栏：加载完成后才出现；加载期间上面那条只给关闭按钮 -->
    <MainTopbar v-else :features="features" :news-active="newsActive" :current-account="currentAccount"
      :accounts="visibleAccounts" @toggle-news="toggleNews" @feature="openWindow" @update:account="onAccountChange" />

    <!-- ===== 启动画面 / 初始化失败错误页（SplashScreen 组件） ===== -->
    <SplashScreen v-if="splashVisible || splashError" :splash-visible="splashVisible" :splash-error="splashError"
      @feedback="openFeedback" />

    <!-- ===== 主界面 ===== -->
    <template v-else>

      <!-- 整合包安装进度不再内嵌在这里：它改成了标题栏上的指示器 + 弹窗，
           即 ModpackTitleIndicator / ModpackPopup，否则这条卡片会一直占着内容区的高度、把列表顶下去。
           状态订阅在各处自己拿，标题栏指示器自包含；弹窗由 App.vue 渲染 -->

      <!-- 资源下载进度同样改成了标题栏指示器 + 弹窗，即 ResourceTitleIndicator / ResourcePopup，
           主页面顶栏那份挂在 MainTopbar 上，这里不再内嵌卡片 -->

      <!-- 主体 -->
      <main class="main" :class="{ 'side-right': sidebarSide === 'Right', 'sidebar-anim': sidebarAnim }">
        <!-- 多选模式浮动工具栏 -->
        <div v-if="multiSelect" class="multi-bar" @contextmenu.prevent @click.stop>
          <span class="multi-count">{{ t("multi.selected", { count: selectedIds.size }) }}</span>
          <button class="multi-btn" :disabled="!canChangeGroup" v-tip="canChangeGroup ? '' : t('group.pickNone')"
            @click="openMoveFromBar">{{ t("multi.moveGroup") }}</button>
          <button class="multi-btn danger" @click="onMultiDelete">{{ t("multi.delete") }}</button>
          <button class="multi-btn" @click="multiLaunch">{{ t("multi.launch") }}</button>
          <span class="multi-sep"></span>
          <button class="multi-exit" @click="exitMultiSelect">{{ t("multi.exit") }}</button>
        </div>

        <!-- 锁定的实例已不存在，例如整合包作者的实例被删 / 改名：整块换成错误页，顶部栏保留 -->
        <template v-if="lockMissing">
          <section class="lock-error">
            <div class="lock-error-badge">!</div>
            <p class="lock-error-text">{{ t("launch.instanceLockError") }}</p>
          </section>
        </template>

        <!-- 空实例：强制打开启动器主页，主页内融合空状态引导（隐藏实例分组） -->
        <template v-else-if="instances.length === 0">
          <!-- 自定义主页面：整块铺满内容区，不留四周内边距，也不给底部留白 -->
          <section v-if="useCustomHome" class="custom-home-page">
            <CustomHomePage :entry-url="customHome?.entryUrl ?? ''" />
          </section>
          <section v-else class="news-page">
            <HomePage :items="news" :loading="newsLoading" @refresh="fetchNews(newsPage)" :page="newsPage"
              :has-more="newsHasMore" @prev="prevNewsPage" @next="nextNewsPage" @open="openNews"
              :current-instance="null" :empty="true" @add-instance="openAdd" @add-account="openWindow('account')"
              @add-java="openWindow('settings')" />
          </section>
        </template>

        <!-- 列表模式：启动器主页 ↔ 实例列表。两者布局角色一致（flex:1 纵向），
             直接作为进出动画的两个子节点，内容不用包一层 -->
        <template v-else-if="effectiveMode === 'list'">
          <Transition name="view-swap" mode="out-in">
            <!-- 自定义主页面：与列表模式一样铺满内容区（自己滚动，不留内边距） -->
            <section v-if="newsActive && useCustomHome" key="custom-home" class="custom-home-page">
              <CustomHomePage :entry-url="customHome?.entryUrl ?? ''" />
            </section>

            <section v-else-if="newsActive" key="home" class="news-page">
              <HomePage :items="news" :loading="newsLoading" @refresh="fetchNews(newsPage)" :page="newsPage"
                :has-more="newsHasMore" @prev="prevNewsPage" @next="nextNewsPage" @open="openNews"
                :current-instance="selected" @select="(inst: InstanceInfoDto) => select(inst)"
                @quick-launch="quickLaunch" back-label-key="home.backToList" @back="newsActive = false" />
            </section>

            <section v-else key="list" class="list-mode" :class="{ 'has-motd': motdCardVisible }">
              <InstanceIcon :name="selected?.name ?? '—'" :uuid="selected?.uuid ?? '0'" :size="120" />

              <!-- 图标下面这行是**启动统计**（实例名在下拉框里已经有一份，不重复）；
                   一个实例都没选时整行不渲染，免得与下拉框的「选择游戏实例」说两遍 -->
              <p v-if="listStats" class="list-stats">{{ listStats }}</p>

              <!-- 实例锁定生效：只有这一个实例可用，切换入口整块隐藏 -->
              <InstanceSelect v-if="!lockActive" :instances="instances" :model-value="selected?.uuid ?? null"
                @update:model-value="onPickInstance" @add="openAdd" />

              <!-- 启动按钮一行、次级操作一行小按钮，两块宽度不同、整块居中不撑满：
                   启动按钮按自己的文案取宽，下面那行按三个按钮的文案取宽，
                   见 main-window.css .launch-actions -->
              <div class="launch-actions">
                <BaseButton variant="primary" size="lg" class="launch-main-btn"
                  :class="{ running: selected?.running }" :disabled="!selected || selected.running" @click="launch">
                  <!-- 启动流程就在按钮里：阶段文本 + 贴着底边的进度条 -->
                  <LaunchProgress v-if="selected?.running" :text="launchStageText" :percent="launchPct" />
                  <template v-else>
                    <GlyphIcon name="play" :size="15" /> {{ t("launch.play") }}
                  </template>
                </BaseButton>
                <div class="launch-sub-actions">
                  <!-- 实例设置：含启动参数（与分组模式下的设置面板一致） -->
                  <BaseButton size="sm" :disabled="!selected" @click="toggleSettings">
                    <GlyphIcon name="gear" :size="13" :weight="1.8" /> {{ t("detail.settings") }}
                  </BaseButton>
                  <!-- 实例日志：主页面不展示日志内容，直接开独立日志窗口 -->
                  <BaseButton size="sm" :disabled="!selected" @click="openLogWindow">
                    <GlyphIcon name="terminal" :size="13" :weight="1.8" /> {{ t("detail.logs") }}
                  </BaseButton>
                  <!-- 资源管理：分组 / 平铺模式的右侧详情面板里有这个入口，而列表模式没有详情面板，
                     这里不补一个就进不去资源管理窗口；添加资源在资源窗口里有自己的入口 -->
                  <BaseButton size="sm" :disabled="!selected" @click="onAction('manageResource')">
                    <GlyphIcon name="package" :size="13" :weight="1.8" /> {{ t("actions.manageResource") }}
                  </BaseButton>
                </div>
              </div>
              <!-- 设置面板整体展开/收起；宽度与居中由外层槽位承担，与原 .list-args 的占位一致 -->
              <CollapsePanel :open="settingsOpen && !!selected" class="list-settings-wrap">
                <div v-if="selected" class="list-args list-settings">
                  <InstanceMetaPanel :instance="selected" :versions="versions" :locked="lockActive"
                    @update="onMetaUpdate" @refreshed="onVersionsRefreshed" />
                  <LaunchArgsPanel :args="argsOf(selected.uuid)" :javas="javas" :locked="lockActive"
                    @update:args="updateArgs" />
                </div>
              </CollapsePanel>
            </section>
          </Transition>
        </template>

        <!-- 模式：游戏分组 / 平铺；列表模式已在上面的分支处理，这里兜底 -->
        <template v-else>
          <!-- 侧栏槽位：宽度在展开 300px / 收起 24px 之间过渡，动画期间内容被裁掉而不被压扁 -->
          <div class="sidebar-slot" :class="{ collapsed: sidebarCollapsed }">
            <MainSidebar :mode="effectiveMode" :search-text="searchText" :groups="groups"
              :filtered-groups="filteredGroups" :filtered-instances="filteredInstances" :searching="searching"
              :selected="selected" :multi-select="multiSelect" :selected-ids="selectedIds" :drag-active="dragActive"
              :dragging-uuid="draggingUuid" :ghost="instanceGhost" :is-collapsed="isCollapsed"
              :on-drag-pointer-down="onDragPointerDown"
              :on-inst-click="onInstClick" :on-inst-context="onInstContext" :on-group-context="onGroupContext"
              :on-group-title-click="onGroupTitleClick" :is-inst-insert="isInstInsert"
              :is-inst-insert-end="isInstInsertEnd" :is-group-insert="isGroupInsert"
              @update:search-text="searchText = $event" @add-instance="openAdd" @add-group="showAddGroup = true"
              @collapse="collapseSidebar(true)" />

            <!-- 展开把手：绝对定位在槽位外缘，收起时淡入（不占位，避免展开瞬间内容跳动） -->
            <button class="sidebar-expand" v-tip="t('sidebar.expand')" @click="collapseSidebar(false)">
              <GlyphIcon name="chevron-right" :size="18" />
            </button>
          </div>

          <!-- 右侧内容区：实例详情 / 启动器主页（进出都走动画） -->
          <section ref="detailEl" class="detail">
            <!-- 自定义主页面走自己的 flex 布局：.detail 带 padding 且可滚动，直接塞 iframe 时
                 height:100% 解析不出高度（父元素没有确定高度），会把页面压扁；这里单独包一层
                 去掉 padding 的撑满容器，让 iframe 铺满整个内容区 -->
            <Transition name="view-swap" mode="out-in">
              <div v-if="newsActive && useCustomHome" key="custom-home" class="custom-home-fill">
                <CustomHomePage :entry-url="customHome?.entryUrl ?? ''" />
              </div>

              <HomePage v-else-if="newsActive" key="home" :items="news" :loading="newsLoading"
                @refresh="fetchNews(newsPage)" :page="newsPage" :has-more="newsHasMore" @prev="prevNewsPage"
                @next="nextNewsPage" @open="openNews" :current-instance="selected"
                @select="(inst: InstanceInfoDto) => select(inst)" @quick-launch="quickLaunch"
                back-label-key="home.backToDetail" @back="newsActive = false" />

              <div v-else key="detail" class="detail-content">
                <!-- 按实例 uuid 做 key：切换实例时整块详情走进出动画 -->
                <Transition name="view-swap" mode="out-in">
                  <div v-if="selected" :key="selected.uuid" class="detail-instance">
                    <div class="detail-top">
                      <InstanceIcon :name="selected.name" :uuid="selected.uuid" :size="84" />
                      <div class="detail-info">
                        <h2>{{ selected.name }}</h2>
                      </div>
                      <!-- 右侧：启动游戏用大按钮，添加 / 管理资源用小按钮 -->
                      <div class="detail-side">
                        <BaseButton variant="primary" size="lg" class="play-btn"
                          :class="{ running: selected.running }" :disabled="selected.running" @click="launch">
                          <!-- 启动流程就在按钮里：阶段文本 + 贴着底边的进度条 -->
                          <LaunchProgress v-if="selected.running" :text="launchStageText" :percent="launchPct" />
                          <template v-else>
                            <GlyphIcon name="play" :size="15" /> {{ t("launch.play") }}
                          </template>
                        </BaseButton>
                        <div class="side-row">
                          <BaseButton size="sm" variant="accent" @click="onAction('addResource')">{{
                            t("actions.addResource") }}</BaseButton>
                          <BaseButton size="sm" variant="accent" @click="onAction('manageResource')">{{
                            t("actions.manageResource") }}</BaseButton>
                        </div>
                      </div>
                    </div>

                    <div class="detail-meta">
                      <span>{{ t("detail.playTime", { hours: playHoursOf(selected.uuid) }) }}</span>
                      <span class="sep">·</span>
                      <span>{{ t("detail.launchCount", { count: statsOf(selected.uuid)?.count ?? 0 }) }}</span>
                      <!-- 小图标快捷操作（单色 SVG） -->
                      <div class="meta-actions">
                        <button class="icon-btn" v-tip="t('actions.export')" @click="openExportWindow">
                          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor"
                            stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M12 15V3M8 7l4-4 4 4" />
                            <path d="M4 14v5a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-5" />
                          </svg>
                        </button>
                        <button class="icon-btn" v-tip="t('actions.openFolder')" @click="onAction('openFolder')">
                          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor"
                            stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" />
                          </svg>
                        </button>
                        <!-- 实例日志窗口：与列表模式的「日志」按钮同一个图标，两种模式里认得出来 -->
                        <button class="icon-btn" v-tip="t('actions.viewLog')" @click="onAction('viewLog')">
                          <GlyphIcon name="terminal" :size="15" :weight="1.8" />
                        </button>
                        <button class="icon-btn" v-tip="t('actions.editConfig')" @click="onAction('editConfig')">
                          <GlyphIcon name="wrench" :size="15" :weight="1.8" />
                        </button>
                        <button class="icon-btn" v-tip="t('actions.rename')" @click="onAction('rename')">
                          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor"
                            stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
                          </svg>
                        </button>
                        <button class="icon-btn danger" v-tip="t('actions.delete')" @click="onAction('delete')">
                          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor"
                            stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                            <path
                              d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6M10 11v6M14 11v6" />
                          </svg>
                        </button>
                      </div>
                    </div>

                    <!-- 实例设置：版本 / 加载器 / 整合包 / 语言 / 内存 / Java / 启动参数，全部直接展示 -->
                    <div class="inline-settings">
                      <InstanceMetaPanel :instance="selected" :versions="versions" :locked="lockActive"
                        @update="onMetaUpdate" @refreshed="onVersionsRefreshed" />
                      <LaunchArgsPanel :args="argsOf(selected.uuid)" :javas="javas" :locked="lockActive"
                        @update:args="updateArgs" />
                    </div>

                    <!-- 自定义执行 -->
                    <div class="args-section">
                      <button class="args-toggle" :class="{ merged: execOpen }" @click="execOpen = !execOpen">
                        <span>
                          <GlyphIcon name="gear" :size="14" :weight="1.8" /> {{ t("exec.title") }}
                        </span>
                        <svg class="args-chevron" :class="{ flip: execOpen }" viewBox="0 0 24 24" width="13" height="13"
                          fill="none" stroke="currentColor" stroke-width="2">
                          <path d="m6 9 6 6 6-6" />
                        </svg>
                      </button>
                      <CollapsePanel :open="execOpen">
                        <CustomExecPanel :args="argsOf(selected.uuid)" @update:args="updateArgs" />
                      </CollapsePanel>
                    </div>

                    <!-- 自定义启动配置（json/ 目录）：与 ColorMC 游戏编辑窗口 tab1 同一张表 -->
                    <div class="args-section">
                      <button class="args-toggle" :class="{ merged: jsonOpen }" @click="jsonOpen = !jsonOpen">
                        <span>
                          <GlyphIcon name="gear" :size="14" :weight="1.8" /> {{ t("customJson.title") }}
                        </span>
                        <svg class="args-chevron" :class="{ flip: jsonOpen }" viewBox="0 0 24 24" width="13"
                          height="13" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="m6 9 6 6 6-6" />
                        </svg>
                      </button>
                      <CollapsePanel :open="jsonOpen">
                        <CustomJsonPanel :uuid="selected.uuid" />
                      </CollapsePanel>
                    </div>

                    <!-- 自定义服务器（自动加入 + MOTD 展示） -->
                    <div class="args-section">
                      <button class="args-toggle" :class="{ merged: serverOpen }" @click="serverOpen = !serverOpen">
                        <span>
                          <GlyphIcon name="gear" :size="14" :weight="1.8" /> {{ t("server.title") }}
                        </span>
                        <svg class="args-chevron" :class="{ flip: serverOpen }" viewBox="0 0 24 24" width="13"
                          height="13" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="m6 9 6 6 6-6" />
                        </svg>
                      </button>
                      <CollapsePanel :open="serverOpen">
                        <div class="server-config">
                          <!-- 自动加入服务器设置：地址可带 `:端口`，不写就用 25565；另有"启动时加入"开关 -->
                          <div class="server-row">
                            <span class="server-label">{{ t("server.ip") }}</span>
                            <input class="field-input grow" :value="argsOf(selected.uuid).serverIp"
                              :placeholder="t('server.ipPlaceholder')" spellcheck="false"
                              @input="onServerIp(($event.target as HTMLInputElement).value)" />
                            <label class="chk">
                              <input type="checkbox" :checked="argsOf(selected.uuid).joinServer"
                                @change="onServerJoin(($event.target as HTMLInputElement).checked)" />
                              {{ t("server.join") }}
                            </label>
                          </div>

                          <!-- MOTD 展示：只在**填了服务器地址**时出现。
                           没填时整块不显示 —— 原来会渲染一张只有兜底文案的卡片
                           "M²L 服务器 / 欢迎来到 M²L 服务器大厅"，看着像查询成功了，其实没查。
                           卡片本身在 components/MotdCard.vue：图标 / 彩色分段 / 人数-版本-延迟，
                           与底部悬浮卡、资源窗口服务器列表上方那张是同一份 -->
                          <MotdCard v-if="argsOf(selected.uuid).serverIp.trim()" compact :motd="instMotd"
                            :loading="instMotdLoading" :name="instMotd?.ip || argsOf(selected.uuid).serverIp" />
                        </div>
                      </CollapsePanel>
                    </div>

                    <!-- 游戏内代理 -->
                    <div class="args-section">
                      <button class="args-toggle" :class="{ merged: proxyOpen }" @click="proxyOpen = !proxyOpen">
                        <span>
                          <GlyphIcon name="gear" :size="14" :weight="1.8" /> {{ t("proxy.title") }}
                        </span>
                        <svg class="args-chevron" :class="{ flip: proxyOpen }" viewBox="0 0 24 24" width="13"
                          height="13" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="m6 9 6 6 6-6" />
                        </svg>
                      </button>
                      <CollapsePanel :open="proxyOpen">
                        <ProxyPanel :args="argsOf(selected.uuid)" @update:args="updateArgs" />
                      </CollapsePanel>
                    </div>
                  </div>
                </Transition>

                <!-- 未选中实例：多选提示 / 空提示。两者都要是 .detail-content 的直接 flex 子节点，
                   靠 flex:1 垂直居中 -->
                <template v-if="!selected">
                  <template v-if="multiSelect">
                    <div class="multi-detail">
                      <div class="multi-detail-icon">
                        <GlyphIcon name="check-square" :size="32" :weight="1.8" />
                      </div>
                      <h2>{{ t("multi.detailTitle") }}</h2>
                      <p>{{ t("multi.detailDesc", { count: selectedIds.size }) }}</p>
                    </div>
                  </template>
                  <div v-else class="placeholder">{{ t("detail.selectHint") }}</div>
                </template>
              </div>
            </Transition>
          </section>
        </template>

      </main>

      <!-- 服务器 MOTD 悬浮卡片，摆在启动器下方；地址与开关在客户端设置里配置。
           `.motd-float` 只管固定摆位，卡片外观与内容在 components/MotdCard.vue —
           与实例详情里的那张、资源窗口服务器列表上方那张是同一份 -->
      <div v-if="motdCardVisible" class="motd-float">
        <MotdCard refreshable :motd="motdInfo" :loading="motdLoading" :name="motdInfo?.ip || t('server.name')"
          @refresh="refreshMotd" />
      </div>

      <!-- ===== 右键菜单（MainCtxMenu 组件） ===== -->
      <MainCtxMenu :menu="ctxMenu" :groups="groups" @select-all="onGroupSelectAll" @launch-group="launchGroupAll"
        @move-group="onMoveGroupClick" @delete-group="onDeleteGroup" @inst-action="onInstMenuAction"
        @multi-move="openMoveGroupPicker(null)" @multi-delete="onMultiDelete" @multi-launch="multiLaunch" />
    </template>

    <!-- ===== 启动界面：已移除（启动按钮显示加载态，不弹窗） ===== -->

    <!-- ===== 添加实例：独立窗口（openAdd 打开） ===== -->

    <!-- ===== 重命名实例弹窗 ===== -->
    <BaseModal v-if="showRename && selected" :title="t('actions.renameTitle')" :closable="false"
      @close="showRename = false">
      <label class="field-label">{{ t("add.name") }}</label>
      <input v-model="renameName" class="field-input" @keyup.enter="doRename" spellcheck="false" />

      <div class="modal-actions">
        <BaseButton @click="showRename = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" :disabled="renameBusy" @click="doRename">
          {{ t("actions.confirm") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- ===== 删除实例确认 ===== -->
    <BaseModal v-if="showDelete && selected" :title="t('actions.deleteTitle')" :closable="false"
      @close="!deleteBusy && (showDelete = false)">
      <p class="delete-tip">{{ t("actions.deleteConfirm", { name: selected.name }) }}</p>

      <!-- 删除进度：整目录挪回收站无法取得真实进度，只显示滚动动画条 -->
      <div v-if="deleteBusy" class="delete-progress">
        <div class="delete-progress-bar rolling"></div>
      </div>
      <p v-if="deleteBusy" class="delete-progress-text">{{ t("actions.deleting") }}</p>

      <div class="modal-actions">
        <BaseButton :disabled="deleteBusy" @click="showDelete = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="danger" :disabled="deleteBusy" @click="doDelete">
          {{ deleteBusy ? t("actions.deleting") : t("actions.delete") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- ===== 多选删除确认 ===== -->
    <BaseModal v-if="showMultiDelete" :title="t('multi.deleteTitle')" :closable="false"
      @close="!multiDeleteBusy && (showMultiDelete = false)">
      <p class="delete-tip">{{ t("multi.deleteConfirm", { count: selectedIds.size }) }}</p>

      <!-- 删除进度：真实进度，已完成实例数 / 总数 -->
      <div v-if="multiDeleteBusy" class="delete-progress">
        <div class="delete-progress-bar" :style="{ width: multiPct + '%' }"></div>
      </div>
      <p v-if="multiDeleteBusy" class="delete-progress-text">
        {{ t("multi.deleting", { done: multiDone, total: multiTotal }) }}
      </p>

      <div class="modal-actions">
        <BaseButton :disabled="multiDeleteBusy" @click="showMultiDelete = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="danger" :disabled="multiDeleteBusy" @click="doMultiDelete">
          {{ multiDeleteBusy ? t("actions.deleting") : t("multi.delete") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- ===== 修改实例图标：右键实例 → 选择图片 → 截图范围 ===== -->
    <IconPickModal v-if="iconPickInst" :uuid="iconPickInst.uuid" :name="iconPickInst.name" :path="iconPickPath"
      @close="((iconPickInst = null), (iconPickPath = ''))" />

    <!-- ===== 首次启动：把 ColorMC 的数据搬过来（见 doInit 的探测）===== -->
    <ColorMcMigrateModal v-if="colorMcInfo" :info="colorMcInfo" @close="onColorMcClosed" @done="onColorMcDone" />

    <!-- ===== 选择目标分组：转移分组与移动选中实例共用 ===== -->
    <BaseModal v-if="showMoveGroupPick" :title="t('group.pickTitle')" :closable="false"
      @close="showMoveGroupPick = false">
      <p class="delete-tip">{{ movePickDesc }}</p>

      <!-- 目标分组：候选里已排掉源分组自己，所以"确定"下去一定有实际动作 -->
      <select v-model="moveGroupTarget" class="field-select pick-select" :disabled="!movePickOptions.length">
        <option v-for="o in movePickOptions" :key="o.id" :value="o.id">
          {{ t("group.pickOption", { name: o.name, count: o.count }) }}
        </option>
      </select>
      <p v-if="!movePickOptions.length" class="modal-sub">{{ t("group.pickNone") }}</p>

      <div class="modal-actions">
        <BaseButton @click="showMoveGroupPick = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" :disabled="!movePickOptions.length" @click="confirmMoveTarget">
          {{ t("actions.confirm") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- ===== 删除分组确认 ===== -->
    <BaseModal v-if="showDeleteGroup" :title="t('group.delete')" :closable="false" @close="showDeleteGroup = false">
      <p class="delete-tip">{{ t("group.deleteConfirm", { name: deleteGroupName, count: deleteGroupCount }) }}</p>

      <div class="modal-actions">
        <BaseButton @click="showDeleteGroup = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="danger" :disabled="deleteGroupBusy" @click="doDeleteGroup">
          {{ t("group.delete") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- ===== 添加分组弹窗 ===== -->
    <BaseModal v-if="showAddGroup" :title="t('group.addGroup')" :closable="false" @close="showAddGroup = false">
      <label class="field-label">{{ t("group.namePlaceholder") }}</label>
      <input v-model="groupName" class="field-input" @keyup.enter="createGroup" spellcheck="false" />

      <p v-if="groupError" class="error-text">{{ groupError }}</p>

      <div class="modal-actions">
        <BaseButton @click="showAddGroup = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" :disabled="groupAdding" @click="createGroup">
          {{ t("group.addGroup") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- ===== 拖拽整合包文件遮罩层 ===== -->
    <!-- 拖拽文件 / 登录卡片时的遮罩；标题按拖入类型变，见 DROP_TITLE_KEYS -->
    <DropOverlay v-if="fileDragOver" :title="t(dropTitleKey)" />
  </div>
</template>

<style scoped src="./main-window.css"></style>
<style scoped src="../../styles/parts/multi-bar.css"></style>
<style scoped src="../../styles/parts/instance-check.css"></style>
