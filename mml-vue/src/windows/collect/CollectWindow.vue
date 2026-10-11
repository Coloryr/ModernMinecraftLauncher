<script setup lang="ts">
// 资源收藏窗口
//
// 布局与交互对应旧启动器的 CollectControl：顶部分组下拉，第 0 项「默认分组」即不过滤
// + 添加 / 删除分组 / 清空，下面一行类型过滤，主体是多选卡片网格，右键出菜单。
// 「安装选中」本轮未实现。
import { computed, onMounted, onUnmounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import AsyncImage from "../../components/ui/AsyncImage.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import InstanceIcon from "../../components/InstanceIcon.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { useCollectDrag } from "../../composables/useCollectDrag";
import { api, onCollectChange } from "../../lib/api";
import { loadGuiConfig, saveGuiConfig, type CollectConfig } from "../../lib/guiConfig";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import type { CollectItemDto, InstanceInfoDto } from "../../lib/bindings";
import { openWindow } from "../windowManager";

defineEmits<{
  /** 请求关闭本窗口，由标题栏关闭按钮与 Esc 触发 */
  (e: "close"): void;
}>();

/** 「默认分组」的键：不在任何具名分组里的那批收藏，见 groupSections */
const DEFAULT_GROUP = "";

const items = ref<CollectItemDto[]>([]);
const groups = ref<Record<string, string[]>>({});
/** 勾选的收藏项 uuid */
const checked = ref<Set<string>>(new Set());
/** 多选模式：默认关。只有开着的时候卡片上才出现勾选框、点卡片才会勾选 */
const multiSelect = ref(false);
/** 本次操作的目标 uuid。点菜单 / 多选条时**快照**下来 ——
 *  等确认弹窗弹出来时，右键菜单早就关了，不能再从 menu 里现取 */
const actionUuids = ref<string[]>([]);
/** 折叠起来的分组，键是分组的 key；默认展开 */
const collapsedGroups = ref<Record<string, boolean>>({});

/** 收藏数据变更订阅的退订函数（挂载后赋值，卸载时统一摘掉） */
let unlistenCollect: (() => void) | null = null;

/** 类型过滤，持久化在 gui_config 的 collect 里 */
const filters = ref<CollectConfig>({
  modpack: true,
  showMod: true,
  resourcePack: true,
  shaderpack: true,
});

/** 勾选框 → 资源类型线串，取值同 `FileType::to_string()`；存档 / 数据包等不在收藏范围内 */
const FILTER_TYPES: Array<{ key: keyof CollectConfig; types: string[]; label: () => string }> = [
  { key: "modpack", types: ["modpack"], label: () => t("collect.filterModpack") },
  { key: "showMod", types: ["mod"], label: () => t("resource.mods") },
  { key: "resourcePack", types: ["resourcepack"], label: () => t("resource.resourcepacks") },
  { key: "shaderpack", types: ["shaderpack"], label: () => t("resource.shaders") },
];

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

/** 已经归入具名分组的收藏 uuid（默认分组 = 这批之外的） */
const groupedUuids = computed(() => new Set(groupOfUuid.value.keys()));

/** 一个收藏都没有；这时只给一条引导文案，不铺一排空分组 */
const isEmpty = computed(() => items.value.length === 0);

/** 当前类型过滤下要显示的收藏；全不勾就是空列表，与旧启动器一致，没有「全选」兜底 */
const shownItems = computed(() => {
  const on = new Set(
    FILTER_TYPES.filter((f) => filters.value[f.key]).flatMap((f) => f.types),
  );
  return items.value.filter((item) => on.has(item.fileType));
});

/** 「共 N 项」：当前过滤下所有分组合起来 */
const shownCount = computed(() => shownItems.value.length);

/**
 * 折叠分组列表：默认分组在前，其余按名字排序
 *
 * 「默认分组」是**真正的一个组**：还没归入任何具名分组的收藏 —— 收藏时（星标）没有
 * 分组参数，所以新收藏天然落在这一组里。每组只带当前类型过滤下属于自己的项。
 */
const groupSections = computed(() => [
  {
    key: DEFAULT_GROUP,
    label: t("collect.defaultGroup"),
    isDefault: true,
    items: shownItems.value.filter((i) => !groupedUuids.value.has(i.uuid)),
  },
  ...groupNames.value.map((name) => ({
    key: name,
    label: name,
    isDefault: false,
    items: shownItems.value.filter((i) => groupOfUuid.value.get(i.uuid) === name),
  })),
]);

/** 分组的实例列表是否展开；存的是"折叠"状态，所以默认展开 */
function isGroupOpen(key: string): boolean {
  return !collapsedGroups.value[key];
}

function toggleGroupOpen(key: string) {
  collapsedGroups.value = { ...collapsedGroups.value, [key]: isGroupOpen(key) };
}

// ---------------- 加载 ----------------

async function reload() {
  const data = await api.collectGetData();
  items.value = data.items;
  groups.value = data.groups;
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
  const cfg = await loadGuiConfig();
  if (cfg) {
    filters.value = cfg.collect;
  }
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

/** 重拉收藏列表；首次挂载与切回本窗口都走这里 */
function refresh() {
  // 收藏数据可能被别的窗口（整合包 / 资源窗口的星标）改过
  reload().catch((e) => showToast(tErr(e)));
}

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次
useWindowRefresh(refresh);

// ---------------- 窗口级键盘 ----------------

/** Esc：先收右键菜单，再逐层关掉打开的弹窗（与其它窗口的逐级关闭一致） */
function onDocKeyDown(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
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

// ---------------- 类型过滤 ----------------

async function toggleFilter(key: keyof CollectConfig, value: boolean) {
  filters.value = { ...filters.value, [key]: value };
  await saveGuiConfig({ collect: { [key]: value } });
}

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

// ---------------- 分组 ----------------

const showAddGroup = ref(false);
const newGroupName = ref("");
const deleteGroupTarget = ref("");
/** 清空确认弹窗是否打开 */
const clearOpen = ref(false);
/** 删除确认弹窗是否打开 */
const removeOpen = ref(false);
/** 清空目标：null = 全部，否则为分组名 */
const clearGroup = ref<string | null>(null);
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
  // 只清这一组里当前看得见的那些（与按钮的可用条件一致）
  const section = groupSections.value.find((s) => s.key === (target ?? DEFAULT_GROUP));
  const uuids = (section?.items ?? []).map((i) => i.uuid);
  if (uuids.length === 0) {
    return;
  }
  try {
    // 默认分组（null）：这些是未归组的收藏 → 从收藏里删除；
    // 具名分组：只把它们移出该分组，收藏本身保留（之后回到默认分组）
    await api.collectRemoveItems(uuids, target);
    checked.value = new Set();
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 某个分组标题上的「清空」：默认分组删掉未归组的收藏，具名分组只清空该组 */
function askClear(groupKey: string) {
  clearGroup.value = groupKey === DEFAULT_GROUP ? null : groupKey;
  clearOpen.value = true;
}

/** 清空确认里那句"清掉 N 项"的 N：当前清空目标那一组里看得见的项数 */
const clearCount = computed(
  () =>
    groupSections.value.find((s) => s.key === (clearGroup.value ?? DEFAULT_GROUP))?.items
      .length ?? 0,
);

/** 「移动到分组」：先记下目标再开弹窗；没有具名分组时按钮已置灰，这里兜底 */
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

/** 「删除」：先记下目标再开确认弹窗 */
function askRemove() {
  actionUuids.value = takeTargets();
  closeMenu();
  if (actionUuids.value.length === 0) {
    return;
  }
  removeOpen.value = true;
}

/** 删除目标项：已归组的只从该组移除（收藏保留），未归组的从收藏里删除 */
async function confirmRemove() {
  const uuids = actionUuids.value;
  removeOpen.value = false;
  actionUuids.value = [];
  if (uuids.length === 0) {
    return;
  }
  // 勾选可以跨分组（所有分组同时可见），所以逐项按归属分流
  const byGroup = new Map<string, string[]>();
  const ungrouped: string[] = [];
  for (const uuid of uuids) {
    const name = groupOfUuid.value.get(uuid);
    if (name === undefined) {
      ungrouped.push(uuid);
    } else {
      const list = byGroup.get(name) ?? [];
      list.push(uuid);
      byGroup.set(name, list);
    }
  }
  try {
    for (const [name, list] of byGroup) {
      await api.collectRemoveItems(list, name);
    }
    if (ungrouped.length > 0) {
      await api.collectRemoveItems(ungrouped, null);
    }
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

/** 本次操作的目标项（多选条上的操作会用到） */

/**
 * 点卡片：多选模式下切换勾选；默认模式下按"下载"处理
 *
 * 下载分两条路，见 [`startDownload`]：整合包不需要实例，其余资源要先选实例。
 */
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
const { draggingUuids, dropGroup, onCardPointerDown, consumeSuppressClick } = useCollectDrag({
  checked,
  multiSelect,
  // 传取值函数而不是 computed 本身：这里只在松手那一刻读一次，不需要是 Ref
  groupOf: () => groupOfUuid.value,
  defaultGroup: DEFAULT_GROUP,
});

/** 卡片点击：刚拖完的那一下不算点击，否则一松手就顺手打开了下载窗口 */
function onCardClick(item: CollectItemDto) {
  if (consumeSuppressClick()) return;
  toggleCard(item);
}

/**
 * 拖拽幽灵卡片的内容：与实例拖拽的 `.drop-ghost-row` 同一观感，图标 + 名字
 *
 * 分组没有组内次序，所以幽灵只落在**落点分组的末尾**，实例那边才要算"插到第几行"。
 * 已经在目标组里的项不会被移动，一件都不动时不给幽灵 —— 否则看着像"松手会变"，
 * 实际是空操作，见 useCollectDrag 的 commitDrag。
 */
const dragGhostCard = computed(() => {
  const group = dropGroup.value;
  if (group === null) return null;
  const moved = [...draggingUuids.value].filter(
    (uuid) => (groupOfUuid.value.get(uuid) ?? DEFAULT_GROUP) !== group,
  );
  if (!moved.length) return null;
  const item = items.value.find((i) => i.uuid === moved[0]);
  if (!item) return null;
  return {
    group,
    name: item.name,
    icon: iconOf(item),
    // 多选整批拖动时说清是几项；单项就照常显示源与类型
    meta:
      moved.length > 1
        ? t("collect.count", { n: moved.length })
        : `${item.source} · ${item.fileType}`,
  };
});

/** 幽灵卡片是否画在这一组；空分组也要渲染网格时同样用它判断 */
function isGhostGroup(key: string): boolean {
  return dragGhostCard.value?.group === key;
}

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
 * - **其余资源**：模组 / 资源包 / 光影包要装进某个已有实例，所以**先让用户选实例**
 *   再跳。在那边选不了 —— 下载资源窗口只有一行只读的实例名，没有选择器。
 *   选定后把 uuid 一并带过去，目标窗口就不会再退回主窗口选中的那个。
 */
function startDownload(item: CollectItemDto) {
  if (item.fileType === "modpack") {
    openDownloadWindow(item);
    return;
  }
  pickItem.value = item;
  void loadInstances();
}

/** 正在选实例的那一项（null = 没开选实例弹窗） */
const pickItem = ref<CollectItemDto | null>(null);
const pickInstances = ref<InstanceInfoDto[]>([]);
/** 实例列表拉取中：先别显示「还没有游戏实例」，否则会闪一下错提示 */
const pickLoading = ref(true);
/** 主窗口当前选中的实例（列表里高亮它） */
const currentUuid = ref("");

async function loadInstances() {
  pickLoading.value = true;
  try {
    const [list, cfg] = await Promise.all([api.getInstances(), loadGuiConfig()]);
    pickInstances.value = list;
    currentUuid.value = cfg?.mainWindow.selectedInstance ?? "";
  } catch {
    pickInstances.value = [];
  } finally {
    pickLoading.value = false;
  }
}

/** 选定实例：带着它跳过去，下载资源窗口会优先用这个 uuid */
function pickInstance(inst: InstanceInfoDto) {
  const item = pickItem.value;
  pickItem.value = null;
  if (item) openDownloadWindow(item, inst.uuid);
}

/** 开关多选模式；退出时清空勾选（卡片上的勾选框也跟着消失） */
function toggleMultiSelect() {
  multiSelect.value = !multiSelect.value;
  if (!multiSelect.value) {
    checked.value = new Set();
  }
}

/** 多选条上的「全选」：勾上当前类型过滤下看得见的所有项 */
function selectAllVisible() {
  checked.value = new Set(shownItems.value.map((i) => i.uuid));
  showToast(t("multi.selectAllDone", { count: checked.value.size }));
}

/** 当前操作的目标：多选模式取勾选集，否则取右键点中的那一项 */
function takeTargets(): string[] {
  if (multiSelect.value) {
    return [...checked.value];
  }
  return menu.value ? [menu.value.item.uuid] : [];
}

// ---------------- 右键菜单 ----------------

const menu = ref<{ x: number; y: number; item: CollectItemDto } | null>(null);
/** 右键点中、当前要操作的条目（卡片按钮也用它） */
const focus = ref<CollectItemDto | null>(null);

/** 菜单尺寸估值（钳边用；三项 + 内边距） */
const MENU_W = 170;
const MENU_H = 130;

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
}
</script>

<template>
  <WindowFrame :title="t('winTitle.collect')" body-gutter @close="$emit('close')">
    <!-- 工具栏整体进标题栏，与账户 / 方块 / 设置三个窗口同一套做法：
         类型开关 + 总计数 + 多选入口，正文直接从折叠分组开始 -->
    <template #head-right>
      <div class="collect-toolbar">
        <div class="collect-filters">
          <label v-for="f in FILTER_TYPES" :key="f.key" class="filter-item">
            <span class="filter-box">
              <input type="checkbox" class="filter-check" :checked="filters[f.key]"
                @change="toggleFilter(f.key, ($event.target as HTMLInputElement).checked)" />
              <span class="check-mark">
                <GlyphIcon name="check" :size="11" :weight="3" />
              </span>
            </span>
            {{ f.label() }}
          </label>
        </div>
        <span class="count">{{ t("collect.count", { n: shownCount }) }}</span>
        <!-- 多选模式的入口：默认不进行多选，点这里才进（进去后由多选条负责退出） -->
        <button v-if="!multiSelect" class="multi-toggle" @click="toggleMultiSelect">
          {{ t("collect.multiSelect") }}
        </button>
      </div>
    </template>

    <div class="collect-body" @click="closeMenu">
      <!-- 多选条：只有多选模式下才出现 -->
      <div v-if="multiSelect" class="multi-bar">
        <span class="multi-count">{{ t("collect.multiSelected", { count: checked.size }) }}</span>
        <button class="multi-btn" @click="selectAllVisible">{{ t("multi.selectAll") }}</button>
        <button class="multi-btn" :disabled="checked.size === 0 || groupNames.length === 0"
          v-tip="checked.size === 0 ? t('collect.multiNoneTip') : groupNames.length === 0 ? t('collect.addToGroupNone') : ''"
          @click="askAddToGroup">
          {{ t("collect.multiMoveGroup") }}
        </button>
        <button class="multi-btn danger" :disabled="checked.size === 0"
          v-tip="checked.size === 0 ? t('collect.multiNoneTip') : ''" @click="askRemove">
          {{ t("collect.multiDelete") }}
        </button>
        <span class="multi-sep"></span>
        <button class="multi-exit" @click="toggleMultiSelect">{{ t("multi.exit") }}</button>
      </div>

      <!-- 一个收藏都没有：只给一条引导，不铺一排空分组 -->
      <p v-if="isEmpty" class="empty-tip collect-empty">{{ t("collect.empty") }}</p>

      <!-- 折叠分组列表：默认分组在前，最后一项是「添加分组」 -->
      <div v-else class="collect-groups">
        <section v-for="g in groupSections" :key="g.key" class="group-block"
          :class="{ 'drop-target': dropGroup === g.key }" :data-group="g.key">
          <div class="group-head">
            <button class="group-title" @click="toggleGroupOpen(g.key)">
              <GlyphIcon class="group-chevron" :class="{ collapsed: !isGroupOpen(g.key) }" name="chevron-down"
                :size="13" :weight="2.4" />
              <span class="group-name">{{ g.label }}</span>
              <span class="group-count">{{ t("collect.count", { n: g.items.length }) }}</span>
            </button>
            <!-- 分组操作：悬停该行才浮出来，免得一排按钮一直在眼前 -->
            <span class="group-acts">
              <button class="group-act" :disabled="!g.items.length" @click.stop="askClear(g.key)">
                {{ t("collect.clear") }}
              </button>
              <button v-if="!g.isDefault" class="group-act danger" @click.stop="deleteGroupTarget = g.key">
                {{ t("collect.deleteGroup") }}
              </button>
            </span>
          </div>

          <div v-show="isGroupOpen(g.key)" class="group-items">
            <!-- 空分组：只有"没有收藏、也没有拖过来的幽灵卡片"时才只给提示 -->
            <p v-if="!g.items.length && !isGhostGroup(g.key)" class="empty-tip">
              {{ g.isDefault ? t("collect.emptyDefault") : t("collect.emptyGroup") }}
            </p>
            <div v-if="g.items.length || isGhostGroup(g.key)" class="collect-grid">
              <div v-for="item in g.items" :key="item.uuid" class="collect-card"
                :class="{ active: checked.has(item.uuid), dragging: draggingUuids.has(item.uuid) }"
                @pointerdown="onCardPointerDown($event, item)" @click="onCardClick(item)" @mouseenter="focus = item"
                @mouseleave="focus = null" @contextmenu.prevent="openMenu($event, item)">
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
                  <div class="card-name" v-tip="item.name">{{ item.name }}</div>
                  <div class="card-meta">{{ item.source }} · {{ item.fileType }}</div>
                </div>
                <span v-if="focus?.uuid === item.uuid || checked.has(item.uuid)" class="card-acts" @pointerdown.stop>
                  <button class="card-act" v-tip="t('collect.openUrl')" @click.stop="openUrl(item)">
                    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
                      stroke-linecap="round" stroke-linejoin="round">
                      <path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.5 1.5" />
                      <path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7L12 19" />
                    </svg>
                  </button>
                </span>
              </div>

              <!-- 拖拽幽灵卡片：与实例拖拽的 .drop-ghost-row 同一观感，虚框 + 淡强调底 +
                   淡化，摆在落点分组的末尾。分组没有组内次序，所以不跟着鼠标插行 -->
              <div v-if="isGhostGroup(g.key)" class="collect-card ghost">
                <div class="card-icon">
                  <AsyncImage v-if="dragGhostCard?.icon" :src="dragGhostCard.icon" />
                  <GlyphIcon v-else class="card-icon-none" name="image" :size="22" :weight="1.6" />
                </div>
                <div class="card-text">
                  <div class="card-name">{{ dragGhostCard?.name }}</div>
                  <div class="card-meta">{{ dragGhostCard?.meta }}</div>
                </div>
              </div>
            </div>
          </div>
        </section>

        <!-- 最后一项：添加分组 -->
        <button class="add-group-row" @click="showAddGroup = true">
          <GlyphIcon name="plus" :size="14" :weight="2.2" />
          {{ t("collect.addGroup") }}
        </button>
      </div>

      <!-- 右键菜单 -->
      <div v-if="menu" class="ctx-menu" :style="{ left: menu.x + 'px', top: menu.y + 'px' }" @click.stop>
        <button class="ctx-item" @click="openUrl(menu.item); closeMenu()">
          {{ t("collect.openUrl") }}
        </button>
        <button class="ctx-item danger" @click="askRemove">
          {{ groupOfUuid.has(menu.item.uuid) ? t("collect.removeFromGroup") : t("collect.deleteFav") }}
        </button>
        <button class="ctx-item" :disabled="groupNames.length === 0"
          v-tip="groupNames.length === 0 ? t('collect.addToGroupNone') : ''" @click="askAddToGroup">
          {{ t("collect.addToGroup") }}
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

    <!-- 删除收藏确认：多选条与卡片菜单共用，多选条上会更正标题里的数量 -->
    <BaseModal v-if="removeOpen" :title="t('collect.multiDelete')" :closable="false" @close="removeOpen = false">
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
          clearGroup === null
            ? t("collect.clearDefaultConfirm", { n: clearCount })
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

    <!--
      选实例再下载，非整合包专用，见 startDownload

      模组 / 资源包 / 光影包都得装进某个已有实例，而"下载资源"窗口里只有一行只读的实例名、
      没有选择器，所以实例只能在这儿先选好，再连同 uuid 一起带过去。
    -->
    <BaseModal v-if="pickItem" :title="t('collect.pickInstanceTitle')" :width="430" :closable="false"
      @close="pickItem = null">
      <p class="delete-tip">{{ t("collect.pickInstanceDesc", { name: pickItem.name }) }}</p>

      <div v-if="pickInstances.length" class="pick-list">
        <button v-for="inst in pickInstances" :key="inst.uuid" type="button" class="pick-item"
          :class="{ on: inst.uuid === currentUuid }" @click="pickInstance(inst)">
          <InstanceIcon :name="inst.name" :uuid="inst.uuid" :size="30" />
          <span class="pick-text">
            <span class="pick-name">{{ inst.name }}</span>
            <span class="pick-sub">{{ inst.version }}</span>
          </span>
          <span v-if="inst.uuid === currentUuid" class="pick-cur">
            {{ t("collect.pickInstanceCurrent") }}
          </span>
        </button>
      </div>
      <div v-else-if="pickLoading" class="empty-tip">{{ t("resource.loading") }}</div>
      <div v-else class="empty-tip">{{ t("collect.pickInstanceNone") }}</div>

      <div class="modal-actions">
        <BaseButton @click="pickItem = null">{{ t("add.cancel") }}</BaseButton>
      </div>
    </BaseModal>
  </WindowFrame>
</template>

<style scoped src="./collect-window.css"></style>
<style scoped src="../../styles/parts/multi-bar.css"></style>
