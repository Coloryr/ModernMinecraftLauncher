<script setup lang="ts">
// 资源收藏窗口
//
// 左栏是分类（与方块窗口同一个 CategoryRail）：全部 + 按资源类型自动分的默认分组 +
// 用户自建分组，选中哪一组就只显示哪一组。
// 安装走 InstallModal：按目标实例的加载器 / 游戏版本挑兼容文件。
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import CategoryRail from "../../components/CategoryRail.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import AsyncImage from "../../components/ui/AsyncImage.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import SearchBox from "../../components/ui/SearchBox.vue";
import HighlightText from "../../components/ui/HighlightText.vue";
import DragGhost from "../../components/ui/DragGhost.vue";
import InstallModal from "./parts/InstallModal.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { useInstanceChange } from "../../composables/useInstanceChange";
import { useCollectDrag } from "../../composables/useCollectDrag";
import { api, onCollectChange } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import { FIXED_GROUP_TYPES, isDefaultGroupName, typeGroupName } from "../../lib/collectGroups";
import { formatMinute } from "../../lib/format";
import { showToast } from "../../lib/toast";
import type { CollectItemDto, InstanceInfoDto } from "../../lib/bindings";
import { openWindow } from "../windowManager";

defineEmits<{
  /** 请求关闭本窗口，由标题栏关闭按钮与 Esc 触发 */
  (e: "close"): void;
}>();

/** 未归组那一批收藏的键：拖拽与删除时用来判断"是否在具名分组里"，见 useCollectDrag */
const DEFAULT_GROUP = "";

const items = ref<CollectItemDto[]>([]);
const groups = ref<Record<string, string[]>>({});
/** 勾选的收藏项 uuid */
const checked = ref<Set<string>>(new Set());
/** 多选模式：默认关。由卡片右键菜单的「多选」进入，退出见多选条 */
const multiSelect = ref(false);
/** 本次操作的目标 uuid。点菜单 / 多选条时**快照**下来 ——
 *  等确认弹窗弹出来时，右键菜单早就关了，不能再从 menu 里现取 */
const actionUuids = ref<string[]>([]);
/** 左栏选中的分组；空串表示「全部」 */
const activeGroup = ref("");

/** 收藏数据变更订阅的退订函数（挂载后赋值，卸载时统一摘掉） */
let unlistenCollect: (() => void) | null = null;

const groupNames = computed(() => Object.keys(groups.value).sort());

/** uuid → 它所在的具名分组名（未归组的没有条目） */
const groupOfUuid = computed(() => {
  const map = new Map<string, string>();
  for (const [name, list] of Object.entries(groups.value)) {
    for (const uuid of list) {
      map.set(uuid, name);
    }
  }
  return map;
});


/** 一个收藏都没有；这时只给一条引导文案，不铺一排空分组 */
const isEmpty = computed(() => items.value.length === 0);

/** 搜索词：按收藏项名字过滤，不持久化 */
const keyword = ref("");

/**
 * 当前搜索词下要显示的收藏
 *
 * 类型已经由左栏的默认分组负责，这里只按名字过滤。
 */
const shownItems = computed(() => {
  const kw = keyword.value.trim().toLowerCase();
  return items.value.filter((item) => !kw || item.name.toLowerCase().includes(kw));
});

/** 「共 N 项」：当前搜索词下所有分组合起来 */
const shownCount = computed(() => shownItems.value.length);

/** 某一组在当前搜索下看得见的项 */
function itemsOfGroup(name: string): CollectItemDto[] {
  return shownItems.value.filter((i) => groupOfUuid.value.get(i.uuid) === name);
}

/**
 * 左栏的分组条目
 *
 * 前四项是**默认分组**（整合包 / 模组 / 资源包 / 光影包）：按资源类型自动归组，
 * 没有收藏时也要留在栏里（0 项），否则用户会以为那一类收藏丢了。
 * 数据里其余的默认分组（存档 / 数据包）接在后面，最后是用户自建分组（按名字排序）。
 */
const railItems = computed<Array<{ id: string; label: string; count: number; custom: boolean }>>(() => {
  const fixed = FIXED_GROUP_TYPES.map((type) => typeGroupName(type)).filter((n): n is string => !!n);
  const restDefaults = groupNames.value
    .filter((name) => isDefaultGroupName(name) && !fixed.includes(name))
    .sort((a, b) => a.localeCompare(b));
  const custom = groupNames.value
    .filter((name) => !isDefaultGroupName(name))
    .sort((a, b) => a.localeCompare(b));

  return [...fixed, ...restDefaults, ...custom].map((name) => ({
    id: name,
    label: name,
    count: itemsOfGroup(name).length,
    custom: !isDefaultGroupName(name),
  }));
});

/** 当前选中分组里的项；「全部」就是整份搜索结果 */
const activeItems = computed(() =>
  activeGroup.value === "" ? shownItems.value : itemsOfGroup(activeGroup.value),
);

// 选中的分组被删掉（或在别的窗口里被改名）时回落到「全部」，
// 否则正文会一直空着、左栏里也没有任何一项是选中的
watch(railItems, (list) => {
  if (activeGroup.value && !list.some((item) => item.id === activeGroup.value)) {
    activeGroup.value = "";
  }
});

// ---------------- 加载 ----------------

async function reload() {
  const data = await api.collectGetData();
  items.value = data.items;
  groups.value = data.groups;
  void backfillGroups();
  // 丢掉已不存在的勾选
  const alive = new Set(data.items.map((i) => i.uuid));
  checked.value = new Set([...checked.value].filter((u) => alive.has(u)));
  void fillIcons();
}

/** 可用的图片地址：由条目存的原始网址登记而来，或回源取到（键 = `源:项目ID`） */
const extraIcons = ref<Record<string, string>>({});
/** 重试过的条目：给 src 加 `?v=` 破掉 webview 缓存，让同一地址重新请求 */
const iconRetry = ref<Record<string, number>>({});
/** 正在解析中的 key（同一项不并发重复请求） */
const iconPending = new Set<string>();

function iconKey(item: CollectItemDto): string {
  return `${item.source}:${item.pid}`;
}

/** 存的是**原始网址**，收藏时落盘的就是它；老条目存的是本地协议地址 */
function isRemoteIcon(url: string): boolean {
  return !!url && !url.startsWith("http://mml-image.localhost") && !url.startsWith("mml-image://");
}

/**
 * 解析条目图标
 *
 * - 存的是原始网址 → 交给后端 `collect_image_url` 登记，拿回可直接用的地址；
 *   图片因此走统一的磁盘缓存与 ETag 过期校验
 * - 完全没有图标 → 按 下载源 + 项目 ID 回平台 API 要一次
 * - 老条目存的本地协议地址 → 直接就能用；协议那层查不到登记时会先查磁盘缓存
 */
async function resolveIcon(item: CollectItemDto) {
  const key = iconKey(item);
  if (extraIcons.value[key] || iconPending.has(key)) return;
  const stored = item.icon ?? "";
  if (!isRemoteIcon(stored) && stored) return;
  if (!stored && !item.pid) return;

  iconPending.add(key);
  try {
    if (isRemoteIcon(stored)) {
      const local = await api.collectImageUrl(stored);
      if (local) extraIcons.value = { ...extraIcons.value, [key]: local };
    } else {
      const url = await api.collectProjectIcon(item.source, item.pid);
      if (url) extraIcons.value = { ...extraIcons.value, [key]: url };
    }
  } catch {
    // 取不到就算了，卡片继续显示占位图
  } finally {
    iconPending.delete(key);
  }
}

/** 列表加载完 / 切回窗口后把图标解析一遍；一次一个，免得把平台 API 打爆 */
async function fillIcons() {
  for (const item of items.value) {
    await resolveIcon(item);
  }
}

/**
 * 图片加载失败：多半是本地协议地址的登记丢了，重启后 URL_IMAGE 里没有它、
 * 磁盘也没有缓存 → 重新解析一次，再给 src 加 `?v=` 让 webview 重新请求。
 * 协议那层会忽略查询串，主窗口的 `instance/…?v=0` 就是这个用法。
 */
async function onIconError(item: CollectItemDto) {
  const key = iconKey(item);
  if ((iconRetry.value[key] ?? 0) >= 1) return;
  const stored = item.icon ?? "";
  try {
    // 原始网址：重新登记即可；本地协议地址：登记已丢，只能按 源+项目ID 回源
    const local = isRemoteIcon(stored)
      ? await api.collectImageUrl(stored)
      : await api.collectProjectIcon(item.source, item.pid);
    if (local) extraIcons.value = { ...extraIcons.value, [key]: local };
  } catch {
    // 重试也拿不到就继续用占位图
  }
  iconRetry.value = { ...iconRetry.value, [key]: 1 };
}

/** 卡片的图标地址：原始网址要先登记过才用，extraIcons 里存的就是登记后的地址 */
function iconOf(item: CollectItemDto): string {
  const key = iconKey(item);
  const stored = item.icon ?? "";
  const base = extraIcons.value[key] || (isRemoteIcon(stored) ? "" : stored);
  if (!base) return "";
  const n = iconRetry.value[key] ?? 0;
  return n > 0 ? `${base}${base.includes("?") ? "&" : "?"}v=${n}` : base;
}

onMounted(async () => {
  void loadInstances();

  // 赋值与注册分开：onUnmounted 只能在 setup 的同步阶段注册，
  // 写在 await 之后就注册不上了（监听会一直挂着，卸载时摘不掉）
  unlistenCollect = await onCollectChange(() => {
    reload().catch(() => { });
  });

  reload().catch((e) => showToast(tErr(e)));
});

onUnmounted(() => {
  unlistenCollect?.();
  document.removeEventListener("keydown", onDocKeyDown);
});

/** 重拉收藏列表与实例列表；首次挂载与切回本窗口都走这里 */
function refresh() {
  // 收藏数据可能被别的窗口（整合包 / 资源窗口的星标）改过，实例也可能刚增删过
  reload().catch((e) => showToast(tErr(e)));
  void loadInstances();
}

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次
useWindowRefresh(refresh);

// 实例在别的窗口里被增删改：「安装到」下拉跟着变，删掉的那个不再列出来
useInstanceChange(() => {
  void loadInstances();
});

// ---------------- 窗口级键盘 ----------------

/** Esc：先收右键菜单，再逐层关掉打开的弹窗（与其它窗口的逐级关闭一致） */
function onDocKeyDown(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  if (groupMenu.value) {
    groupMenu.value = null;
    return;
  }
  if (installTargets.value.length) {
    installTargets.value = [];
    return;
  }
  if (menu.value) {
    closeMenu();
    return;
  }
  if (showAddGroup.value) {
    showAddGroup.value = false;
    return;
  }
  if (deleteGroupTarget.value) {
    deleteGroupTarget.value = "";
    return;
  }
  if (clearOpen.value) {
    clearOpen.value = false;
    return;
  }
  if (removeOpen.value) {
    removeOpen.value = false;
    return;
  }
  if (addToGroupTarget.value !== "") {
    addToGroupTarget.value = "";
  }
}

document.addEventListener("keydown", onDocKeyDown);

// ---------------- 勾选 ----------------

function toggleCheck(uuid: string, value: boolean) {
  const next = new Set(checked.value);
  if (value) {
    next.add(uuid);
  } else {
    next.delete(uuid);
  }
  checked.value = next;
}

/** 进入多选模式并勾上刚右键的那一项；之后的增删都在多选条里 */
function enterMultiSelect(item: CollectItemDto) {
  closeMenu();
  multiSelect.value = true;
  toggleCheck(item.uuid, true);
}

/** 开关多选模式；退出时清空勾选（卡片上的勾选框也跟着消失） */
function toggleMultiSelect() {
  multiSelect.value = !multiSelect.value;
  if (!multiSelect.value) {
    checked.value = new Set();
  }
}

/** 多选条上的「全选」：勾上当前搜索下看得见的所有项 */
function selectAllVisible() {
  checked.value = new Set(shownItems.value.map((i) => i.uuid));
  showToast(t("multi.selectAllDone", { count: checked.value.size }));
}

/** 多选条上的「安装」：把勾选的项一起丢进安装弹窗 */
function installChecked() {
  const list = items.value.filter((item) => checked.value.has(item.uuid));
  if (list.length) {
    openInstall(list);
  }
}

/** 当前操作的目标：多选模式取勾选集，否则取右键点中的那一项 */
function takeTargets(): string[] {
  if (multiSelect.value) {
    return [...checked.value];
  }
  return menu.value ? [menu.value.item.uuid] : [];
}

// ---------------- 分组 ----------------

const showAddGroup = ref(false);
const newGroupName = ref("");
const deleteGroupTarget = ref("");
/** 清空确认弹窗是否打开 */
const clearOpen = ref(false);
/** 删除确认弹窗是否打开 */
const removeOpen = ref(false);
/** 清空目标：分组名，空串表示没有目标 */
const clearGroup = ref("");
const addToGroupTarget = ref("");

async function confirmAddGroup() {
  const name = newGroupName.value.trim();
  showAddGroup.value = false;
  if (!name) {
    return;
  }
  try {
    await api.collectAddGroup(name);
    newGroupName.value = "";
  } catch (e) {
    showToast(tErr(e));
  }
}

async function confirmDeleteGroup() {
  const name = deleteGroupTarget.value;
  deleteGroupTarget.value = "";
  try {
    await api.collectRemoveGroup(name);
    showToast(t("tip.deleted"));
  } catch (e) {
    showToast(tErr(e));
  }
}

async function confirmClear() {
  const target = clearGroup.value;
  clearOpen.value = false;
  if (!target) {
    return;
  }
  // 只清这一组里当前看得见的那些（与菜单项的可用条件一致）
  const uuids = itemsOfGroup(target).map((i) => i.uuid);
  if (uuids.length === 0) {
    return;
  }
  try {
    // 用户自建分组：只把它们移出该分组，收藏本身保留；
    // 默认分组：移出会被补组再归回来，所以这里就是删掉这些收藏
    await api.collectRemoveItems(uuids, isDefaultGroupName(target) ? null : target);
    checked.value = new Set();
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 某个分组上的「清空」：只清空该组的归属，收藏本身保留 */
function askClear(groupKey: string) {
  groupMenu.value = null;
  clearGroup.value = groupKey;
  clearOpen.value = true;
}

/** 清空确认里那句"N 项"的 N：当前清空目标那一组里看得见的项数 */
const clearCount = computed(() => itemsOfGroup(clearGroup.value).length);

/** 「移动到分组」：先记下目标再开弹窗；没有具名分组时菜单项已置灰，这里兜底 */
function askAddToGroup() {
  actionUuids.value = takeTargets();
  closeMenu();
  if (groupNames.value.length === 0 || actionUuids.value.length === 0) return;
  addToGroupTarget.value = groupNames.value[0];
}

async function confirmAddToGroup() {
  const name = addToGroupTarget.value;
  const uuids = actionUuids.value;
  addToGroupTarget.value = "";
  actionUuids.value = [];
  if (!name || uuids.length === 0) {
    return;
  }
  try {
    await api.collectSetGroupItems(name, uuids);
    // 移走的项已经不在原来的组里了，勾选跟着清掉
    checked.value = new Set();
  } catch (e) {
    showToast(tErr(e));
  }
}

// ---------------- 条目操作 ----------------

/**
 * 这一项是否落在「按类型自动分的默认分组」里
 *
 * 那种分组不是用户建的（整合包 / 模组 / 资源包 / 光影包），从里面"移出分组"没有意义 ——
 * 下次打开窗口补组时又会被归回去。所以只有自建分组里的条目才给「移出分组」。
 *
 * - `item`: 收藏项
 */
function inTypeGroup(item: CollectItemDto): boolean {
  const group = groupOfUuid.value.get(item.uuid);
  const auto = typeGroupName(item.fileType);
  return !!auto && group === auto;
}

/**
 * 这一项是否在用户自建分组里
 *
 * - `item`: 收藏项
 */
function inCustomGroup(item: CollectItemDto): boolean {
  const group = groupOfUuid.value.get(item.uuid);
  return !!group && !inTypeGroup(item);
}

/** 菜单要不要给「移出分组」：目标里至少有一个在自建分组里 */
function canLeaveGroup(): boolean {
  return takeTargets().some((uuid) => {
    const item = items.value.find((i) => i.uuid === uuid);
    return !!item && inCustomGroup(item);
  });
}

/** 「移出分组」：只断开分组归属，收藏本身保留 */
async function leaveGroup() {
  const uuids = takeTargets().filter((uuid) => {
    const item = items.value.find((i) => i.uuid === uuid);
    return !!item && inCustomGroup(item);
  });
  closeMenu();
  if (uuids.length === 0) {
    return;
  }
  const byGroup = new Map<string, string[]>();
  for (const uuid of uuids) {
    const name = groupOfUuid.value.get(uuid);
    if (!name) continue;
    const list = byGroup.get(name) ?? [];
    list.push(uuid);
    byGroup.set(name, list);
  }
  try {
    for (const [name, list] of byGroup) {
      await api.collectRemoveItems(list, name);
    }
    checked.value = new Set();
    showToast(t("collect.leftGroup"));
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 「删除收藏」：先记下目标再开确认弹窗 */
function askRemove() {
  actionUuids.value = takeTargets();
  closeMenu();
  if (actionUuids.value.length === 0) {
    return;
  }
  removeOpen.value = true;
}

/** 删除目标项：收藏本身从库里删掉，不管原来在哪个分组 */
async function confirmRemove() {
  const uuids = actionUuids.value;
  removeOpen.value = false;
  actionUuids.value = [];
  if (uuids.length === 0) {
    return;
  }
  try {
    await api.collectRemoveItems(uuids, null);
    checked.value = new Set();
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 打开项目页面（网址） */
function openUrl(item: CollectItemDto) {
  if (item.url) {
    api.openUrl(item.url).catch(() => { });
  }
}

/** 点卡片 = 按"下载"处理，见 [`startDownload`]；多选模式下改为切换勾选 */
function toggleCard(item: CollectItemDto) {
  if (multiSelect.value) {
    toggleCheck(item.uuid, !checked.value.has(item.uuid));
    return;
  }
  startDownload(item);
}

// ---------------- 拖拽改分组 ----------------

/**
 * 拖卡片到分组上 = 移到那个分组，逻辑见 composables/useCollectDrag
 *
 * 与拖动游戏实例同一套做法：指针拖拽、落点用 `data-group` 反查、多选模式下拖已勾选的
 * 卡片整批一起走。收藏的分组没有组内次序，数据层是集合，所以只做归属、不做排序。
 */
const { draggingUuids, dropGroup, ghost: collectGhost, onCardPointerDown, consumeSuppressClick } = useCollectDrag({
  checked,
  multiSelect,
  // 传取值函数而不是 computed 本身：这里只在松手那一刻读一次，不需要是 Ref
  groupOf: () => groupOfUuid.value,
  defaultGroup: DEFAULT_GROUP,
});

/** 卡片点击：刚拖完的那一下不算点击，否则一松手就顺手打开了下载窗口 */
function onCardClick(item: CollectItemDto, ev?: MouseEvent) {
  // 右键菜单原本靠 .collect-body 那一下收；这里挡住了冒泡，得自己收
  closeMenu();
  // 别让这次点击冒到 .collect-body：那里负责"点外面收起菜单"
  ev?.stopPropagation();
  if (consumeSuppressClick()) return;
  toggleCard(item);
}

/** 补组进行中：避免 reload → 补组 → reload 转圈 */
let backfilling = false;

/**
 * 把没有归组的收藏按类型补进默认分组
 *
 * 没有这一栏的收藏在界面上根本不会出现（分组列表只列具名分组），所以这次补组属于
 * "不能省"：老收藏、以及用户删掉分组后留下的条目，都在这里归位。
 */
async function backfillGroups() {
  if (backfilling) return;
  const grouped = new Set(Object.values(groups.value).flat());
  const byGroup = new Map<string, string[]>();
  for (const item of items.value) {
    if (grouped.has(item.uuid)) continue;
    const name = typeGroupName(item.fileType);
    if (!name) continue;
    const list = byGroup.get(name) ?? [];
    list.push(item.uuid);
    byGroup.set(name, list);
  }
  if (byGroup.size === 0) return;

  backfilling = true;
  try {
    for (const [name, uuids] of byGroup) {
      if (!groups.value[name]) await api.collectAddGroup(name);
      await api.collectSetGroupItems(name, uuids);
    }
  } catch (e) {
    showToast(tErr(e));
  } finally {
    backfilling = false;
  }
  await reload();
}
/** 跟手浮起副本的内容：手里抓着的第一项（按下那张排最前） */
const collectDragItem = computed(() => {
  const uuid = collectGhost.value?.item.uuids[0];
  return uuid ? (items.value.find((i) => i.uuid === uuid) ?? null) : null;
});

// ---------------- 下载 ----------------

/**
 * 下载这一项：打开「下载整合包」或「下载资源」窗口，并把项目带过去
 *
 * 两个窗口都是"按实例取数"的，`add_resource_list(game, …)` 必须有实例 uuid，
 * 所以这里只把项目参数送过去，由目标窗口在实例就绪后自己打开版本列表。
 *
 * `uuid` 也要送：非整合包那条路已经让用户选好实例了，
 * 目标窗口会优先用它，见 AddResourceWindow 的 onMounted。
 */
function openDownloadWindow(item: CollectItemDto, uuid?: string) {
  openWindow(item.fileType === "modpack" ? "add_modpack" : "add_resource", {
    uuid,
    project: {
      source: item.source,
      pid: item.pid,
      fileType: item.fileType,
      name: item.name,
      icon: item.icon ?? null,
      url: item.url,
    },
  });
}

/**
 * 点卡片后的分流
 *
 * - **整合包**：目标窗口自己就是"下载并新建实例"，不需要先有实例，直接跳详情；
 * - **其余资源**：模组 / 资源包 / 光影包要装进某个已有实例，交给 InstallModal 选实例。
 *
 * 在下载资源窗口里选不了实例：那边只有一行只读的实例名、没有选择器，
 * 所以 uuid 必须从这里带过去，见 openDownloadWindow。
 *
 * - `item`: 要装的收藏项
 */
function startDownload(item: CollectItemDto) {
  if (item.fileType === "modpack") {
    openDownloadWindow(item);
    return;
  }
  openInstall([item]);
}

/** 安装弹窗里待装的项；空数组 = 弹窗没开 */
const installTargets = ref<CollectItemDto[]>([]);

/**
 * 能走安装弹窗的类型：整合包要新建实例，存档 / 数据包要先有目标，都不从这里装
 *
 * - `item`: 收藏项
 */
function canInstall(item: CollectItemDto): boolean {
  return item.fileType !== "modpack" && item.fileType !== "datapacks" && item.fileType !== "save";
}

/**
 * 打开安装弹窗
 *
 * 整合包不走这里（它要新建实例，见 startDownload）；数据包 / 存档要先选目标，剔掉并说一声。
 *
 * - `list`: 要装的收藏项
 */
function openInstall(list: CollectItemDto[]) {
  if (instancesLoading.value) {
    showToast(t("resource.loading"));
    return;
  }
  if (!instances.value.length) {
    showToast(t("collect.pickInstanceNone"));
    return;
  }
  const installable = list.filter(canInstall);
  if (list.some((item) => item.fileType === "modpack")) {
    showToast(t("collect.installModpack"));
  }
  if (list.some((item) => item.fileType === "datapacks")) {
    showToast(t("collect.installDatapack"));
  }
  if (list.some((item) => item.fileType === "save")) {
    showToast(t("collect.installSave"));
  }
  if (!installable.length) return;
  installTargets.value = installable;
}

/**
 * 执行安装：按目标实例的游戏版本与加载器挑每个项目最新的兼容文件，逐个交给下载器
 *
 * 进度在下载窗口看；没有兼容文件的项目跳过，最后给一条汇总提示。
 *
 * - `uuid`: 目标实例 uuid
 */
async function doInstall(uuid: string) {
  const instance = instances.value.find((item) => item.uuid === uuid);
  const targets = installTargets.value;
  if (!instance || !targets.length) return;
  installTargets.value = [];

  let done = 0;
  let skipped = 0;
  for (const item of targets) {
    try {
      const page = await api.getResourceFiles(
        uuid,
        item.source,
        item.pid,
        item.fileType,
        0,
        instance.version,
        instance.loader,
      );
      const file = page.list[0];
      if (!file?.source.fid) {
        skipped += 1;
        continue;
      }
      await api.downloadResource(uuid, item.source, item.pid, file.source.fid, item.fileType, null);
      done += 1;
    } catch {
      skipped += 1;
    }
  }

  if (done) showToast(t("collect.installDone", { count: done }));
  if (skipped) showToast(t("collect.installSkipped", { count: skipped }));
}

/**
 * 打开「下载资源」窗口
 *
 * 不带实例：那是一个"还没选实例"的浏览窗口，实例在它标题栏的下拉里选。
 * 窗口族的窗口才要实例 uuid 才能开，这个静态窗口不需要。
 */
function openAddResource() {
  openWindow("add_resource");
}
/** 可选的目标实例，即「安装到」下拉的选项 */
const instances = ref<InstanceInfoDto[]>([]);
/** 实例列表拉取中：这段时间点卡片先别弹下拉，否则只有一行"还没有实例" */
const instancesLoading = ref(true);

/** 拉实例列表；挂载与每次切回窗口都走一次，实例可能刚增删过 */
async function loadInstances() {
  instancesLoading.value = true;
  try {
    instances.value = await api.getInstances();
  } catch {
    instances.value = [];
  } finally {
    instancesLoading.value = false;
  }
}

// ---------------- 右键菜单 ----------------

const menu = ref<{ x: number; y: number; item: CollectItemDto } | null>(null);

/** 菜单尺寸估值（钳边用；最多六项 + 两条分隔 + 内边距） */
const MENU_W = 170;
const MENU_H = 240;

function openMenu(e: MouseEvent, item: CollectItemDto) {
  // 钳进窗口：贴着右/下边缘右键时，菜单会整个露在可视区之外
  menu.value = {
    x: Math.max(8, Math.min(e.clientX, window.innerWidth - MENU_W - 8)),
    y: Math.max(8, Math.min(e.clientY, window.innerHeight - MENU_H - 8)),
    item,
  };
  // 多选模式下右键顺带把该项勾上；默认模式不动勾选，菜单只作用于点中的这一项
  if (multiSelect.value) {
    toggleCheck(item.uuid, true);
  }
}

function closeMenu() {
  menu.value = null;
  groupMenu.value = null;
}

/** 卡片菜单的「安装」：多选模式下装勾选的，否则只装点中的这一项 */
function menuInstall() {
  const item = menu.value?.item;
  closeMenu();
  if (multiSelect.value) {
    installChecked();
  } else if (item && canInstall(item)) {
    openInstall([item]);
  }
}

// ---------------- 分组右键菜单 ----------------

/** 右键分组标题弹出的菜单；null = 没开 */
const groupMenu = ref<{ x: number; y: number; group: string } | null>(null);
/** 分组右键菜单尺寸估值（两项 + 内边距） */
const GROUP_MENU_W = 150;
const GROUP_MENU_H = 92;

/**
 * 右键左栏的分组：弹出该组的菜单
 *
 * - `payload`: 事件（定位用）与分组键
 */
function openGroupMenu(payload: { event: MouseEvent; id: string; label: string }) {
  menu.value = null;
  groupMenu.value = {
    x: Math.max(8, Math.min(payload.event.clientX, window.innerWidth - GROUP_MENU_W - 8)),
    y: Math.max(8, Math.min(payload.event.clientY, window.innerHeight - GROUP_MENU_H - 8)),
    group: payload.id,
  };
}
</script>

<template>
  <WindowFrame :title="t('winTitle.collect')" body-fill @close="$emit('close')">
    <!-- 工具栏整体进标题栏，与账户 / 方块 / 设置三个窗口同一套做法：
         只放总计数，正文左边是分组栏 -->
    <template #head-right>
      <div class="collect-toolbar">
        <span class="count">{{ t("collect.count", { n: shownCount }) }}</span>
      </div>
    </template>

    <div class="collect-body" @click="closeMenu()">
      <!-- 多选条：由卡片右键菜单的「多选」进入 -->
      <div v-if="multiSelect" class="multi-bar">
        <span class="multi-count">{{ t("collect.installCount", { count: checked.size }) }}</span>
        <button class="multi-btn" @click="selectAllVisible">{{ t("multi.selectAll") }}</button>
        <button class="multi-btn" :disabled="checked.size === 0 || groupNames.length === 0"
          v-tip="checked.size === 0 ? t('collect.multiNoneTip') : groupNames.length === 0 ? t('collect.addToGroupNone') : ''"
          @click="askAddToGroup">
          {{ t("collect.addToGroup") }}
        </button>
        <button class="multi-btn" :disabled="checked.size === 0"
          v-tip="checked.size === 0 ? t('collect.multiNoneTip') : ''" @click="installChecked">
          {{ t("collect.installDo") }}
        </button>
        <button class="multi-btn danger" :disabled="checked.size === 0"
          v-tip="checked.size === 0 ? t('collect.multiNoneTip') : ''" @click="askRemove">
          {{ t("collect.deleteFav") }}
        </button>
        <span class="multi-sep"></span>
        <button class="multi-exit" @click="toggleMultiSelect">{{ t("multi.exit") }}</button>
      </div>

      <!-- 左栏：全部 + 默认分组 + 自建分组，与方块窗口同一个分类栏 -->
      <div class="collect-main">
        <CategoryRail flush :all-label="t('collect.all')" :all-count="shownCount" :items="railItems"
          :active="activeGroup" :drop-key="dropGroup" :new-label="t('collect.addGroup')"
          @pick="activeGroup = $event" @group-menu="openGroupMenu" @new-group="showAddGroup = true" />

        <div class="collect-content">
          <!-- 搜索：按收藏项名字过滤，分组计数与「共 N 项」都跟着变 -->
          <SearchBox v-if="!isEmpty" v-model="keyword" :placeholder="t('collect.search')"
            :clear-label="t('collect.searchClear')" width="100%" />

          <div class="collect-scroll">
            <!-- 一个收藏都没有：只给一条引导 -->
            <p v-if="isEmpty" class="empty-tip collect-empty">{{ t("collect.empty") }}</p>
            <!-- 当前分组是空的：搜索没命中与"这一组确实没收藏"分开说 -->
            <p v-else-if="!activeItems.length" class="empty-tip collect-empty">
              {{ keyword.trim() ? t("collect.searchEmpty") : t("collect.emptyGroup") }}
            </p>

            <div v-else class="collect-grid">
              <div v-for="item in activeItems" :key="item.uuid" class="collect-card"
                :class="{ active: checked.has(item.uuid), dragging: draggingUuids.has(item.uuid) }"
                @pointerdown="onCardPointerDown($event, item)" @click="onCardClick(item, $event)"
                @contextmenu.prevent="openMenu($event, item)">
                <span v-if="multiSelect" class="card-box" @click.stop @pointerdown.stop>
                  <input type="checkbox" class="card-check" :checked="checked.has(item.uuid)" @click.stop
                    @change="toggleCheck(item.uuid, ($event.target as HTMLInputElement).checked)" />
                  <span class="check-mark">
                    <GlyphIcon name="check" :size="11" :weight="3" />
                  </span>
                </span>
                <div class="card-icon">
                  <!-- 用全仓通用的 AsyncImage：加载中是流光占位、失败是灰底占位，
                       与下载整合包窗口的项目图标同一套观感 -->
                  <AsyncImage v-if="iconOf(item)" :src="iconOf(item)" @error="onIconError(item)" />
                  <GlyphIcon v-else class="card-icon-none" name="image" :size="22" :weight="1.6" />
                </div>
                <div class="card-text">
                  <!-- 搜索命中的那截给底色：看不出"为什么这条被搜出来"最难用 -->
                  <div class="card-name" v-tip="item.name">
                    <HighlightText :text="item.name" :query="keyword" />
                  </div>
                  <div class="card-meta">{{ item.source }} · {{ item.fileType }}</div>
                  <!-- 项目 ID 与收藏时间：ID 用来回源对项目，时间是"多早收的"的线索 -->
                  <div class="card-sub">
                    <span class="card-pid" v-tip="item.pid">{{ item.pid }}</span>
                    <span v-if="item.favTime" class="card-time">
                      {{ t("collect.favTime", { time: formatMinute(item.favTime) }) }}
                    </span>
                  </div>
                </div>
                <span class="card-acts" @pointerdown.stop>
                  <button class="card-act" v-tip="t('collect.openUrl')" @click.stop="openUrl(item)">
                    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
                      stroke-linecap="round" stroke-linejoin="round">
                      <path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.5 1.5" />
                      <path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7L12 19" />
                    </svg>
                  </button>
                </span>
              </div>
            </div>

            <!-- 去挑资源收藏：跟在卡片后面，一个收藏都没有时也要能从这里去挑 -->
            <div class="collect-acts-row">
              <button class="add-row-btn add-group-row" @click="openWindow('add_modpack')">
                {{ t("collect.addModpack") }}
              </button>
              <button class="add-row-btn add-group-row" @click.stop="openAddResource">
                {{ t("collect.addResource") }}
              </button>
            </div>
          </div>

          <!-- 跟手浮起副本：手里抓着的那一项，跟着指针走 -->
          <DragGhost v-if="collectGhost && collectDragItem" :x="collectGhost.x" :y="collectGhost.y">
            <div class="card-icon">
              <AsyncImage v-if="iconOf(collectDragItem)" :src="iconOf(collectDragItem)" />
              <GlyphIcon v-else class="card-icon-none" name="image" :size="22" :weight="1.6" />
            </div>
            <span class="card-name">{{ collectDragItem.name }}</span>
          </DragGhost>
        </div>
      </div>

      <!-- 安装弹窗：按目标实例的加载器 / 游戏版本挑兼容文件 -->
      <InstallModal v-if="installTargets.length" :items="installTargets" :instances="instances"
        @close="installTargets = []" @install="doInstall" />

      <!-- 分组右键菜单：清空 / 删除（删除只对自建分组给） -->
      <div v-if="groupMenu" class="ctx-menu" :style="{ left: groupMenu.x + 'px', top: groupMenu.y + 'px' }"
        @click.stop>
        <button class="ctx-item" :disabled="!itemsOfGroup(groupMenu.group).length"
          @click="askClear(groupMenu.group)">
          {{ t("collect.clear") }}
        </button>
        <button v-if="!isDefaultGroupName(groupMenu.group)" class="ctx-item danger"
          @click="deleteGroupTarget = groupMenu.group; groupMenu = null">
          {{ t("collect.deleteGroup") }}
        </button>
      </div>

      <!-- 右键菜单 -->
      <div v-if="menu" class="ctx-menu" :style="{ left: menu.x + 'px', top: menu.y + 'px' }" @click.stop>
        <button v-if="!multiSelect" class="ctx-item" @click="enterMultiSelect(menu.item)">
          {{ t("collect.multiSelect") }}
        </button>
        <button v-if="multiSelect || canInstall(menu.item)" class="ctx-item" @click="menuInstall">
          {{ t("collect.installDo") }}
        </button>
        <button class="ctx-item" @click="openUrl(menu.item); closeMenu()">
          {{ t("collect.openUrl") }}
        </button>
        <span class="ctx-sep"></span>
        <button v-if="canLeaveGroup()" class="ctx-item" @click="leaveGroup">
          {{ t("collect.removeFromGroup") }}
        </button>
        <button class="ctx-item" :disabled="groupNames.length === 0"
          v-tip="groupNames.length === 0 ? t('collect.addToGroupNone') : ''" @click="askAddToGroup">
          {{ t("collect.addToGroup") }}
        </button>
        <span class="ctx-sep"></span>
        <button class="ctx-item danger" @click="askRemove">
          {{ t("collect.deleteFav") }}
        </button>
      </div>
    </div>

    <!-- 添加分组 -->
    <BaseModal v-if="showAddGroup" :title="t('collect.addGroupTitle')" :closable="false" @close="showAddGroup = false">
      <!-- 纯文本输入：用 .field-input。别挂 .field-select —— 那个类除了尺寸还带
           右侧下拉箭头，即 padding-right: 28px 加 --select-arrow 背景图，输入框上会
           凭空多出一个"能点开下拉"的箭头；主窗口同款弹窗用的就是 .field-input -->
      <input v-model="newGroupName" class="field-input" :placeholder="t('collect.groupPlaceholder')" spellcheck="false"
        @keydown.enter="confirmAddGroup" />
      <div class="modal-actions">
        <BaseButton @click="showAddGroup = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmAddGroup">{{ t("add.yes") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 删除分组确认 -->
    <BaseModal v-if="deleteGroupTarget" :title="t('collect.deleteGroup')" :closable="false"
      @close="deleteGroupTarget = ''">
      <p class="delete-tip">{{ t("collect.deleteGroupConfirm", { name: deleteGroupTarget }) }}</p>
      <div class="modal-actions">
        <BaseButton @click="deleteGroupTarget = ''">{{ t("add.no") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmDeleteGroup">{{ t("add.yes") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 删除收藏确认：卡片菜单打开，只问一句"删几项" -->
    <BaseModal v-if="removeOpen" :title="t('collect.deleteFav')" :closable="false" @close="removeOpen = false">
      <p class="delete-tip">{{ t("collect.removeConfirm", { count: actionUuids.length }) }}</p>
      <div class="modal-actions">
        <BaseButton @click="removeOpen = false">{{ t("add.no") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmRemove">{{ t("add.yes") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 清空确认 -->
    <BaseModal v-if="clearOpen" :title="t('collect.clear')" :closable="false" @close="clearOpen = false">
      <p class="delete-tip">
        {{
          isDefaultGroupName(clearGroup)
            ? t("collect.removeConfirm", { count: clearCount })
            : t("collect.clearGroupConfirm", { name: clearGroup })
        }}
      </p>
      <div class="modal-actions">
        <BaseButton @click="clearOpen = false">{{ t("add.no") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmClear">{{ t("add.yes") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 添加到分组 -->
    <BaseModal v-if="addToGroupTarget !== ''" :title="t('collect.addToGroupTitle')" @close="addToGroupTarget = ''"
      :closable="false">
      <select v-model="addToGroupTarget" class="field-select">
        <option v-for="g in groupNames" :key="g" :value="g">{{ g }}</option>
      </select>
      <div class="modal-actions">
        <BaseButton @click="addToGroupTarget = ''">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="confirmAddToGroup">{{ t("add.yes") }}</BaseButton>
      </div>
    </BaseModal>

  </WindowFrame>
</template>

<style scoped src="./collect-window.css"></style>
<style scoped src="../../styles/parts/multi-bar.css"></style>
