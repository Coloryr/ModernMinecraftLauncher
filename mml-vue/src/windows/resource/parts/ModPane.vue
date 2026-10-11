<script setup lang="ts">
// 模组分类：列表 / 表格两种展示方式 + 自定义分组
//
// - 顶栏（ContentHead）：标题 + 视图切换 + 「分组」开关 + 新建分组 + 刷新
// - 分组打开：每个自建分组一块，末尾跟一块「未分组」；把模组行拖到某块上即归到该组
// - 分组关闭：选中的视图直接铺平，拖拽也随之关闭 —— 没有落点可言
//
// 数据：模组列表在 useResourceData，分组在 useModGroups（自己的 JSON），
// 拖拽在 useModDrag，两种视图共用同一套落点判定。
import { computed, onMounted, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import ContentHead from "./ContentHead.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ModList from "./mod/ModList.vue";
import ModTable from "./mod/ModTable.vue";
import ModGroupMenu, { type GroupMenuAction } from "./mod/ModGroupMenu.vue";
import ModItemMenu, { type ItemMenuAction } from "./mod/ModItemMenu.vue";
import ModBatchMenu, { type BatchMenuAction } from "./mod/ModBatchMenu.vue";
import { deleteMod, disableMod, enableMod, setModNote } from "../../../lib/api";
import { useModDrag, useReorderDrag, type ModDropTarget, type ModStateGroup } from "../composables/useModDrag";
import {
  useModGroups,
  STATE_GROUP_ID_OFF,
  STATE_GROUP_ID_ON,
  STATE_GROUP_ID_FAIL,
} from "../composables/useModGroups";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { useResourceView } from "../composables/useResourceView";
import type { ModItemDto } from "../../../lib/bindings";
import { modSelectKey, sortedMods } from "../types";
import { orderedMods } from "../modColumns";
import type { ModView } from "../types";

const props = defineProps<{
  /** 数据层，模组列表、加载态与扫描进度从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化、确认弹窗与打开目录从这里取 */
  ops: ReturnType<typeof useResourceOps>;
  /** 视图偏好，模组展示方式、列宽与表头排序从这里取 */
  view: ReturnType<typeof useResourceView>;
}>();

const { mods, instanceUuid, loading, modProgress } = props.data;
const { busy, act, actLocal, askDelete, askConfirm, openFolder } = props.ops;
const { modView, setModView, colWidths, setColWidth, sort, setSort } = props.view;

const groups = useModGroups(props.data);

// ---------- 拖拽归组 / 改状态 ----------

const { draggingKey, dropTarget, onRowPointerDown, consumeSuppressClick } = useModDrag({
  onDrop: (target, keys) => void applyDrop(target, keys),
});

/**
 * 投放：自建分组 = 归组；状态分组 = 改启用状态
 *
 * 状态分组只装"不在任何自建分组里"的模组（见 [`sections`]）。所以拖过去光改状态是看不见的
 * —— 模组还留在原来那个自建分组里，那一块不会出现在状态分组下。要**先摘出分组**，
 * 投放才算真的生效。
 *
 * 反向的"状态分组 → 自建分组"由 [`afterGroupDrop`] 收尾（把模组改回启用），见那里的说明。
 *
 * 「识别失败」不是"可拖成的状态"：坏 jar 拖不动，状态也没法靠拖拽造出来，拖过去当没发生。
 */
async function applyDrop(target: ModDropTarget, keys: string[]) {
  if (target.kind === "group") {
    await groups.setGroup(target.group, keys);
    await afterGroupDrop(keys);
    return;
  }
  if (target.kind === "ungrouped") {
    await groups.setGroup(null, keys);
    return;
  }
  if (target.state === "fail") return;

  const wantDisable = target.state === "off";
  // 拖拽只带 SHA1（内容哈希），要调启用 / 禁用得换回那一条的 uuid
  const picked = pickByState(keys, wantDisable);

  // 先摘出分组：它不属于列表数据，单独提交，走 reload: false 那条路
  await groups.setGroup(null, keys);
  await setState(picked, wantDisable);
}

/**
 * 拖进自建分组之后收尾：**把模组改成启用**
 *
 * 用户定的口径是"拖动模组修改分组 = 禁用和启用"，所以从「已禁用」拖到某个分组就是启用它，
 * 与拖到「已启用」等价。已经启用的不用碰 —— 启用 / 禁用会改文件名、uuid 跟着变，
 * 能不动就不动，也让"启动器里拖一下、外面只多了个分组"这种常见操作不产生额外写盘。
 */
async function afterGroupDrop(keys: string[]) {
  await setState(pickByState(keys, false), false);
}

/**
 * 把若干模组改成目标启用状态
 *
 * `wantDisable = true` 禁用、`false` 启用；`picked` 里只放"当前状态与目标不同"的那些。
 *
 * **不重拉列表**：启用 / 禁用只是给文件名加减后缀，jar 里的元数据一个字都没变 ——
 * 重扫要把每个包重新解析一遍（几百个包好几秒），纯属白干。
 * 后端会回**改名后的新身份**，即 uuid / 文件名 / 路径，就地改那一行即可；
 * `disable` 一改，`sections` 会把它挪到「已禁用 / 已启用」那一块去。
 */
async function setState(picked: ModItemDto[], wantDisable: boolean) {
  if (!picked.length) return;
  await actLocal(async () => {
    for (const item of picked) {
      const renamed = wantDisable
        ? await disableMod(instanceUuid.value, item.uuid)
        : await enableMod(instanceUuid.value, item.uuid);
      // uuid 必须换：它是**文件路径的 v5**，文件名一变就变；
      // 不换的话再点一次"启用"会拿旧 uuid 去找文件，后端直接报找不到
      item.uuid = renamed.uuid;
      item.file = renamed.file;
      item.path = renamed.path;
      item.disable = wantDisable;
    }
  });
}

/** 从拖拽带过来的 SHA1 里挑出"当前状态与目标不同"的那些条目 */
function pickByState(keys: string[], wantDisable: boolean): ModItemDto[] {
  return keys
    .map((key) => bySha1.value.get(key))
    .filter((item): item is ModItemDto => !!item)
    .filter((item) => item.disable !== wantDisable);
}

/** 行按下：坏 jar 没有 SHA1，不可归组 / 改状态 */
function onDragStart(payload: { event: PointerEvent; item: ModItemDto }) {
  if (!payload.item.sha1) return;
  onRowPointerDown(payload.event, payload.item.sha1);
}

// ---------- 右键多选（顶栏批量操作） ----------

/**
 * 已选中的模组（**SHA1** 集合）
 *
 * 用 SHA1 而不是 uuid：启用 / 禁用只是给文件名加减后缀，uuid（文件路径的 v5）会跟着变，
 * 而 SHA1 是内容哈希、不变 —— 与拖拽归组同一套键。所以"批量启用"之后选中状态仍然有效，
 * 可以接着"移到分组"。
 *
 * 坏 jar 没有 SHA1（读不出内容哈希），不参与多选 —— 它们也做不了启用 / 归组。
 */
const selected = ref<Set<string>>(new Set());

/** 选中的那些条目；批量操作要 uuid 去调命令，SHA1 只是选中身份 */
const selectedItems = computed<ModItemDto[]>(() =>
  mods.value.filter((item) => selected.value.has(modSelectKey(item))),
);

/** 把这一行加进 / 移出选择 */
function onSelect(item: ModItemDto) {
  const key = modSelectKey(item);
  const next = new Set(selected.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  selected.value = next;
  // 记住"上次点（或勾选）的那一项"：Shift + 点的整段选择从这里算起
  lastClickedKey.value = modSelectKey(item);
}

/** Shift 整段选择的锚点：上次点（或勾选）的那一项的 SHA1 */
const lastClickedKey = ref<string | null>(null);

/** 清空选择与锚点 */
function clearSelection() {
  selected.value = new Set();
  // 锚点也一起清：取消选择后不该还留着"整段从这里开始"的标记
  lastClickedKey.value = null;
}

/**
 * 左键点整行：多选模式下加选 / 反选；**按住 Shift 则整段选中**（用户口径）
 *
 * - Shift + 点：把"上次点的那一项"到这一项之间（按显示顺序）整段选中，
 *   与资源管理器同一口径 —— **不需要先进入多选模式**，Shift 本身就表示"要选一段"；
 * - 没有选中项、也没按 Shift：什么都不做 —— 保持原来的"按下即拖拽归组"；
 * - 刚才那一下是拖拽（拖过阈值）：用抑制标记吞掉，别把归组的拖动当成点击。
 */
function onRowClick(payload: { event: MouseEvent; item: ModItemDto }) {
  if (consumeSuppressClick()) return;
  if (payload.event.shiftKey) {
    selectRange(payload.item);
    return;
  }
  // 普通点击也**记下锚点**：这样"先点一行、再 Shift + 点另一行"就能选出这一段，
  // 即使第一次点击本身没有改变选择（不在多选模式时它什么都不做）
  lastClickedKey.value = modSelectKey(payload.item);
  if (!selected.value.size) return;
  onSelect(payload.item);
}

/**
 * Shift + 点：整段选中，**替换**当前选择
 *
 * 顺序取**当前显示顺序**：
 * - 表格视图套上"按列排序"的口径，即 `orderedMods`，与 ModTable 共用同一份实现；
 *   不共用的话，按列排序后整段选中的范围和看到的对不上，用户报过"整段选中 78 项"；
 * - 列表视图套上 `sortedMods`，**列表视图自己就按 modid 正序排**，见 ModList 的 `rows`；
 *   这里不排的话，整段选择会和屏幕顺序对不上，用户报过"选中 159 项"；
 * - **收起的分组看不见，不算进整段** —— 选中的就是看得见的那一段。
 *
 * 没有锚点时，也就是第一次就是 Shift + 点，只选这一项。
 */
function selectRange(item: ModItemDto) {
  const order = sections.value
    .filter((sec) => isOpen(sec.id))
    .flatMap((sec) => (modView.value === "table" ? orderedMods(sec.items, sort.value) : sortedMods(sec.items)));

  const to = order.findIndex((x) => modSelectKey(x) === modSelectKey(item));
  if (to < 0) return;

  // 锚点也是"混合键"，SHA1 优先、uuid 兜底，比较要用同一个函数
  const from = order.findIndex((x) => modSelectKey(x) === lastClickedKey.value);
  if (from < 0) {
    selected.value = new Set([modSelectKey(item)]);
    lastClickedKey.value = modSelectKey(item);
    return;
  }

  const [lo, hi] = from <= to ? [from, to] : [to, from];
  const keys = order
    .slice(lo, hi + 1)
    .map((x) => modSelectKey(x))
    .filter((key): key is string => !!key);
  selected.value = new Set(keys);
  // **锚点不动**：它始终是"最初点的那一项"，反复 Shift + 点都从同一个起点重新划范围
  // 资源管理器同一口径；锚点行上那道竖条就是"起点在这里"的标记
}

/** 批量改启用状态：`wantDisable = true` 禁用、`false` 启用 */
async function batchSetState(wantDisable: boolean) {
  const picked = selectedItems.value.filter((item) => !item.fail && item.disable !== wantDisable);
  await setState(picked, wantDisable);
}

/** 批量删除（进回收站）：删完选中项已经不在列表里了，直接清空选择 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) {
          await deleteMod(instanceUuid.value, item.uuid);
        }
      });
      clearSelection();
    },
    // act 内部已经重拉过列表了，这里别再拉一遍；重拉模组要重解析所有 jar，很慢
    { reload: false },
  );
}

/**
 * 批量移到某个分组
 *
 * 复用拖拽归组那条路：`groups.setGroup` 本来就收一串 SHA1。
 * 与拖进自建分组同一口径，**顺带启用**，见 [`afterGroupDrop`]。
 */
async function batchMove(group: string, keys: string[] = [...selected.value]) {
  // 分组的键**必须是 SHA1**，内核的 `Mod.Groups` 按内容哈希记：
  // 选中键现在可能是 uuid（没有 SHA1 的条目），这里换成 SHA1 并把换不出来的过滤掉，
  // 别把 uuid 混进分组表里
  const sha1s = keys
    .map((key) => mods.value.find((item) => modSelectKey(item) === key))
    .map((item) => item?.sha1)
    .filter((sha1): sha1 is string => !!sha1);
  if (!sha1s.length) return;
  await groups.setGroup(group, sha1s);
  await afterGroupDrop(sha1s);
}

/**
 * 批量"移到分组"的弹窗是否打开
 *
 * 用弹窗而不是顶栏下拉：下拉一次只看得见一项，而且"移到分组"这个占位项会以
 * **选项**的形式混在分组名里，看着像能选的东西（用户点名不要这样）。
 * 弹窗里把分组一次列全，点哪个就移过去。
 */
const groupPickOpen = ref(false);

/** 弹窗里选中某个分组：点完即关，然后移过去 */
function pickGroup(group: string) {
  groupPickOpen.value = false;
  const source = moveGroupSource.value;
  moveGroupSource.value = null;
  if (!source) {
    // 顶栏"移到分组"：源是选中项
    void batchMove(group);
    return;
  }
  // 分组右键菜单的"转移内容"：源是那一整组
  const keys = groupItems(source.id)
    .map((item) => item.sha1)
    .filter((key): key is string => !!key);
  void batchMove(group, keys);
}

/**
 * 弹窗里点「新建分组」：先关掉这个弹窗，再开新建分组表单
 *
 * 不叠着开两个弹窗（互相压、关闭顺序也乱）。**选中项不会丢** ——
 * 建完再点一次「移到分组」就能选到刚建的那个。
 */
function createGroupFromPick() {
  groupPickOpen.value = false;
  openGroupForm();
}

// ---------- 分组右键菜单 ----------
//
// 菜单项全部作用在**整组**上：启用所有 / 禁用所有 / 转移内容 / 删除所有模组，
// 自建分组再多一项删除分组；状态分组是内核按启用状态分的，删不掉。
// 菜单本身是 `mod/ModGroupMenu.vue`（只负责画与定位），动作在这里执行。

/** 打开右键菜单的那个分组（null = 没开） */
const groupMenu = ref<{
  id: string;
  label: string;
  custom: boolean;
  count: number;
  /** 组里还有禁用的模组 → 菜单给"启用所有" */
  canEnable: boolean;
  /** 组里还有启用的模组 → 菜单给"禁用所有" */
  canDisable: boolean;
  x: number;
  y: number;
} | null>(null);

/** 分组里的模组；按 id 从当前 sections 里取那一批 */
function groupItems(id: string): ModItemDto[] {
  return sections.value.find((sec) => sec.id === id)?.items ?? [];
}

/** 打开分组右键菜单；表格视图由 ModTable 抛上来，列表视图直接绑在分组头上 */
function openGroupMenu(payload: {
  event: MouseEvent;
  id: string;
  label: string;
  custom: boolean;
  count: number;
}) {
  groupMenu.value = {
    id: payload.id,
    label: payload.label,
    custom: payload.custom,
    count: payload.count,
    // 启用 / 禁用两项**按组内实际状态**给，用户口径是已启用分组只显示"禁用所有"：
    // 识别失败的模组改不了启用状态（勾选框也是禁用的），所以不计入判断
    canEnable: groupItems(payload.id).some((item) => !item.fail && item.disable),
    canDisable: groupItems(payload.id).some((item) => !item.fail && !item.disable),
    x: payload.event.clientX,
    y: payload.event.clientY,
  };
}

/** 右键菜单：整组启用 / 禁用；只挑状态不同的，和顶栏批量一个口径 */
async function groupSetState(id: string, wantDisable: boolean) {
  const picked = groupItems(id).filter((item) => item.disable !== wantDisable);
  if (picked.length) await setState(picked, wantDisable);
}

/** 右键菜单：整组删除（进回收站，先确认） */
function groupRemoveMods(id: string, label: string) {
  const picked = groupItems(id);
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.deleteGroupModsConfirm", { name: label, n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) {
          await deleteMod(instanceUuid.value, item.uuid);
        }
      });
    },
    // act 内部已经重拉过列表，这里别再拉一遍；重拉要重解析所有 jar，很慢
    { reload: false },
  );
}

/**
 * "转移内容"的源分组；`null` 表示这次转移是顶栏那条，源是选中项
 *
 * 复用同一个"移到分组"弹窗：只是把源从选中项换成整组。弹窗里会把这组名显示出来。
 */
const moveGroupSource = ref<{ id: string; label: string } | null>(null);

/** 右键菜单：整组转移 */
function groupMoveAll(id: string, label: string) {
  moveGroupSource.value = { id, label };
  groupPickOpen.value = true;
}

/** 弹窗里的候选目标：排除源分组自己（不然就是"移到自己"） */
const groupPickTargets = computed(() =>
  groups.groups.value.filter((g) => g.uuid !== moveGroupSource.value?.id),
);

/** 关掉"移到分组"弹窗：顺手清掉整组转移的源，免得下次顶栏那条被当成整组转移 */
function closeGroupPick() {
  groupPickOpen.value = false;
  moveGroupSource.value = null;
}

/** 菜单动作分发 */
function onGroupMenuAction(kind: GroupMenuAction) {
  const menu = groupMenu.value;
  if (!menu) return;
  switch (kind) {
    case "rename-group":
      openGroupForm(menu.id, menu.label);
      break;
    case "delete-group":
      removeGroup(menu.id, menu.label);
      break;
    case "enable-all":
      void groupSetState(menu.id, false);
      break;
    case "disable-all":
      void groupSetState(menu.id, true);
      break;
    case "move-all":
      groupMoveAll(menu.id, menu.label);
      break;
    case "delete-mods":
      groupRemoveMods(menu.id, menu.label);
      break;
  }
}

// ---------- 模组行右键菜单 ----------
//
// 四个动作都复用已有的**单行**操作，即 toggle / openNote / openFolder / remove，
// 所以这里只做分发。原来右键是"加选 / 取消多选"，改成菜单后多选交给左侧勾选框。

/** 打开项目菜单的那一行，`null` 表示没开 */
const itemMenu = ref<{ item: ModItemDto; x: number; y: number } | null>(null);

/** 打开模组行右键菜单；列表视图与表格视图都抛这个事件上来 */
function openItemMenu(payload: { event: MouseEvent; item: ModItemDto }) {
  const x = payload.event.clientX;
  const y = payload.event.clientY;

  // 有选中项时，右键 = **批量动作**；用户口径是多选后菜单也要跟着变
  const picked = selectedItems.value;
  if (picked.length) {
    batchMenu.value = {
      count: picked.length,
      // 判据与分组菜单同一口径：识别失败的模组改不了启用状态，不计入
      canEnable: picked.some((item) => !item.fail && item.disable),
      canDisable: picked.some((item) => !item.fail && !item.disable),
      x,
      y,
    };
    return;
  }

  itemMenu.value = { item: payload.item, x, y };
}

/** 多选状态下右键弹出的批量菜单（null = 没开） */
const batchMenu = ref<{
  count: number;
  canEnable: boolean;
  canDisable: boolean;
  x: number;
  y: number;
} | null>(null);

/** 批量菜单动作分发：全部复用顶栏那几条批量操作 */
function onBatchMenuAction(kind: BatchMenuAction) {
  switch (kind) {
    case "enable":
      void batchSetState(false);
      break;
    case "disable":
      void batchSetState(true);
      break;
    case "move":
      // 源就是当前选中项：`moveGroupSource` 清空即走"顶栏那条"分支
      moveGroupSource.value = null;
      groupPickOpen.value = true;
      break;
    case "remove":
      batchRemove();
      break;
  }
}

/** 项目菜单动作分发 */
function onItemMenuAction(kind: ItemMenuAction) {
  const menu = itemMenu.value;
  if (!menu) return;
  switch (kind) {
    case "enable":
      void setState([menu.item], false);
      break;
    case "disable":
      void setState([menu.item], true);
      break;
    case "note":
      openNote(menu.item);
      break;
    case "open-folder":
      openFolder("mods", menu.item.file);
      break;
    case "remove":
      remove(menu.item);
      break;
  }
}

// ---------- 搜索 ----------

/** 搜索词；**不持久化**，这是一次性的筛选而不是视图偏好，重开窗口该是干净的 */
const keyword = ref("");
const query = computed(() => keyword.value.trim().toLowerCase());

/** 命中判定：名字 / modid / 文件名 / 作者 / 版本 / 简介，任一包含即命中 */
function hit(item: ModItemDto, q: string): boolean {
  return [item.name, item.modId, item.file, item.author, item.version, item.description].some(
    (value) => value.toLowerCase().includes(q),
  );
}

/** 当前要显示的模组（搜索词为空时就是全部） */
const matched = computed(() => {
  const q = query.value;
  return q ? mods.value.filter((item) => hit(item, q)) : mods.value;
});

/** 搜了但一个都没命中 */
const noMatch = computed(() => query.value !== "" && matched.value.length === 0);

// ---------- 分组块 ----------

/**
 * 状态分组 → 固定 uuid / 文案键
 *
 * 状态分组与自建分组**共用一套 uuid 键**，即 useModGroups 的 `STATE_GROUP_ID_*`：
 * 渲染身份、排序、折叠状态、投放判定全用它，不必再维护"两套键互相映射"。
 *
 * 它们不是用户分组：不可改名 / 删除。归属由"启用 / 禁用"决定：
 * - 拖**到**「已启用 / 已禁用」= 改状态 + 摘出自建分组，见 [`applyDrop`]；
 * - 从状态分组里拖**到自建分组** = 只归组 + 启用，见 [`afterGroupDrop`]。
 */
const STATE_KEYS: Record<ModStateGroup, { id: string; labelKey: string }> = {
  on: { id: STATE_GROUP_ID_ON, labelKey: "resource.groupOn" },
  off: { id: STATE_GROUP_ID_OFF, labelKey: "resource.groupOff" },
  fail: { id: STATE_GROUP_ID_FAIL, labelKey: "resource.groupFail" },
};

/** 分组视图里的一块：自建分组 或 状态分组 */
interface ModSection {
  /** 分组 uuid（自建与状态分组同一套键空间） */
  id: string;
  /** 分组名；自建分组是用户起的名字，状态分组是 `t()` 好的文案 */
  label: string;
  /** 这一块里的模组，顺序就是显示顺序 */
  items: ModItemDto[];
  /** 自建分组：可改名 / 删除，落点 = 归组 */
  custom: boolean;
  /** 状态分组的类型；自建分组没有 */
  state?: ModStateGroup;
}

/**
 * 分组视图的块：**按用户拖出来的顺序**，顺序表见 useModGroups 的 `order`
 *
 * - 自建分组：空的也留着（是个可以往里拖的落点）
 * - 「已启用 / 已禁用」：**空也显示**，这是用户点名的要求 —— 它们是"改状态"的落点，空着更是个提示
 * - 「识别失败」：空就收起来（没有坏包时它只是个噪音），它也不接受投放
 *
 * 顺序表里可能有已删分组的旧 uuid，所以以实到的为准（找不到的跳过）。
 */
const sections = computed<ModSection[]>(() => {
  const of = groups.groupOfKey.value;
  // 只在自建分组里的模组：剩下的才按状态分桶（同一个模组不会在两处出现）
  const rest = matched.value.filter((item) => !item.sha1 || !of.has(item.sha1));

  const byId = new Map<string, ModSection>();
  for (const group of groups.groups.value) {
    byId.set(group.uuid, {
      id: group.uuid,
      label: group.name,
      items: matched.value.filter((item) => !!item.sha1 && of.get(item.sha1) === group.uuid),
      custom: true,
    });
  }
  for (const state of ["on", "off", "fail"] as ModStateGroup[]) {
    const { id, labelKey } = STATE_KEYS[state];
    byId.set(id, {
      id,
      label: t(labelKey),
      items: rest.filter((item) => stateHit(item, state)),
      custom: false,
      state,
    });
  }

  const list: ModSection[] = [];
  for (const key of groups.order.value) {
    const sec = byId.get(key);
    if (!sec || !visible(sec)) continue;
    list.push(sec);
  }
  // 兜底：分组是刚建的、顺序表还没回来时，别把它漏掉（后端也会在下次下发时补上）
  for (const [key, sec] of byId) {
    if (!groups.order.value.includes(key) && visible(sec)) list.push(sec);
  }
  return list;
});

/** 一个模组是否属于某个状态分组 */
function stateHit(item: ModItemDto, state: ModStateGroup): boolean {
  if (state === "fail") return item.fail;
  if (item.fail) return false;
  return state === "off" ? item.disable : !item.disable;
}

/**
 * 这一块现在要不要显示
 *
 * 「识别失败」空着就收起来；自建分组与「已启用 / 已禁用」始终显示。
 * **搜索时例外**：搜索是对"看得见的东西"做过滤，此时空分组（含状态分组）一律收起 ——
 * 否则一搜索满屏都是空的「已启用 / 已禁用」，命中的那几条反而要找。
 */
function visible(sec: ModSection): boolean {
  if (query.value) return sec.items.length > 0;
  if (sec.custom) return true;
  return sec.state !== "fail" || sec.items.length > 0;
}

/**
 * 这一块当前是不是拖拽落点（拖着东西悬在它上面）
 *
 * 「识别失败」不接受投放 —— 不给高亮，松手也不做事，免得看着像"能放"。
 */
function isDropTarget(sec: ModSection): boolean {
  const target = dropTarget.value;
  if (!target) return false;
  if (target.kind === "state") {
    return !sec.custom && sec.state === target.state && target.state !== "fail";
  }
  return sec.custom && target.kind === "group" && target.group === sec.id;
}

/** SHA1 → 列表里那一条；拖拽只带内容哈希，改状态要用 uuid 定位 */
const bySha1 = computed(() => {
  const map = new Map<string, ModItemDto>();
  for (const item of mods.value) {
    if (item.sha1) map.set(item.sha1, item);
  }
  return map;
});

/**
 * 分组头上的计数
 *
 * 自建分组两种状态都有，所以报"启用 / 禁用"；**状态分组只报项数** ——
 * 在「已启用」里再写一行"禁用 0"、在「已禁用」里写"启用 0"都是纯噪音（用户点名）。
 */
function counts(sec: ModSection): string {
  if (sec.items.length === 0) return "";
  if (!sec.custom) return t("resource.groupCountItems", { n: sec.items.length });
  const off = sec.items.filter((item) => item.disable).length;
  return t("resource.groupCounts", { n: sec.items.length, on: sec.items.length - off, off });
}

// ---------- 分组折叠 ----------

/**
 * 分组块的收起 / 展开
 *
 * 状态存在**实例的 `gui_setting.json`**，即 `Mod.GroupCollapsed`，见 useModGroups：
 * 用户明确要求记住（"已启用默认不展开"），而且换实例各记各的。
 * 键就是分组 uuid —— 与顺序表同一套，不必再转换。
 */
function isOpen(id: string): boolean {
  return !groups.isCollapsed(id);
}

/** 收起 / 展开这一块并落盘 */
function toggleCollapse(id: string) {
  groups.toggleCollapsed(id);
}

// 拖到收起的块上自动展开：收起后整块只剩一条分组头，落点太小、不好瞄
watch(dropTarget, (target) => {
  const id =
    target?.kind === "group"
      ? target.group
      : target?.kind === "state"
        ? STATE_KEYS[target.state].id
        : "";
  if (id && !isOpen(id)) toggleCollapse(id);
});

// ---------- 分组头拖拽排序 ----------

/**
 * 块的元素表，量位置算插入点用
 *
 * 用 `Map` 而不是 `querySelectorAll`：块的渲染顺序就是 `sections` 的顺序，
 * 按 uuid 直接结对更稳，uuid 也不适合写进选择器。
 */
const sectionEls = new Map<string, HTMLElement>();

function bindSection(id: string, el: Element | null) {
  if (el) sectionEls.set(id, el as HTMLElement);
  else sectionEls.delete(id);
}

/** 当前渲染的块 id；拖拽排序的坐标轴，与 DOM 里的顺序一致 */
const sectionIds = computed(() => sections.value.map((sec) => sec.id));

const {
  dragKey: draggingSection,
  insertAt: insertSectionAt,
  onPointerDown: onSectionPointerDown,
  consumeSuppressClick: consumeSectionClick,
} = useReorderDrag({
  keys: () => sectionIds.value,
  rectOf: (key) => sectionEls.get(key)?.getBoundingClientRect() ?? null,
  onDrop: (from, insertAt) => void moveSection(from, insertAt),
});

/**
 * 把某一块拖到第 `insertAt` 块之前
 *
 * 与左侧分类栏同一算法：摘掉被拖项后，目标下标在被拖项之后要减一。
 * 落盘的就是**分组 uuid**；状态分组与自建分组同一套键空间，不用转换。
 */
async function moveSection(from: string, insertAt: number) {
  const ids = sectionIds.value;
  const index = ids.indexOf(from);
  if (index < 0) return;
  const next = [...ids];
  next.splice(index, 1);
  next.splice(Math.max(0, Math.min(next.length, insertAt > index ? insertAt - 1 : insertAt)), 0, from);

  // 只把"看得见的块"排了序，隐藏的块（空的识别失败等）不在里面 ——
  // 它们在顺序表里的位置由后端保留原样，这里按原顺序补回去
  const visible = new Set(ids);
  const hidden = groups.order.value.filter((id) => !visible.has(id));
  await groups.setOrder([...next, ...hidden]);
}

/** 插入线画不画在第 index 块之前 */
function showSectionLine(index: number): boolean {
  return draggingSection.value !== null && insertSectionAt.value === index;
}

/**
 * 点分组头（箭头 / 名字那一带）→ 折叠或展开
 *
 * 名字那一带同时是**拖拽把手**，所以要先问一句"刚才那一下是不是拖完松手的"：
 * 拖过就把这次 click 吞掉，否则排完序会顺手把分组折叠了。
 * `consumeSuppressClick` 只在真的拖过时才返回 true，见 useDragGesture 的 `dragged`。
 */
function onSectionToggle(id: string) {
  if (consumeSectionClick()) return;
  toggleCollapse(id);
}

// ---------- 分组增删改 ----------

/**
 * 分组名弹窗
 *
 * `id` 为**空串**表示"新建"；否则是"重命名某个分组"（带 uuid）。
 * 名字只在这一个字段里，所以弹窗不再需要同时记"原名"。
 */
const groupForm = ref<{ id: string; name: string } | null>(null);
const groupBusy = ref(false);

/** 打开弹窗：不传 id 是新建，传分组 uuid 与当前名字是重命名 */
function openGroupForm(id = "", name = "") {
  groupForm.value = { id, name };
}

/** 提交分组名弹窗：有 id 就重命名，否则新建 */
async function saveGroupForm() {
  const form = groupForm.value;
  if (!form || groupBusy.value) return;
  const name = form.name.trim();
  if (!name) return;
  groupBusy.value = true;
  try {
    const ok = form.id ? await groups.rename(form.id, name) : await groups.add(name);
    if (ok) groupForm.value = null;
  } finally {
    groupBusy.value = false;
  }
}

/** 删除分组；传 uuid，确认框里显示的是分组名 */
function removeGroup(id: string, name: string) {
  askConfirm(
    t("resource.groupDelete"),
    t("resource.groupDeleteConfirm", { name }),
    () => groups.remove(id),
    // 分组数据不在资源列表里：删完不用重拉列表；重拉模组要重解析 jar，很慢
    { reload: false },
  );
}

// ---------- 模组备注 ----------

/**
 * 备注弹窗的草稿；`item` 是列表里那一条的引用，存完直接改它
 *
 * 备注存在 `gui_setting.json` 的 `Mod.ModName`（与 ColorMC 互通），
 * **不在模组列表里**，所以存完**不能重拉列表** —— 重拉一次要重新解析每个 jar
 * 的元数据（数秒），用户只是写一句备注而已。改本地那一份就够。
 */
const noteForm = ref<{ item: ModItemDto; text: string } | null>(null);
const noteBusy = ref(false);

/** 打开备注弹窗，草稿带上当前备注 */
function openNote(item: ModItemDto) {
  noteForm.value = { item, text: item.note };
}

/** 保存；`text` 传空串 = 清掉备注 */
async function saveNote(text: string) {
  const form = noteForm.value;
  if (!form || noteBusy.value) return;
  const value = text.trim();
  noteBusy.value = true;
  try {
    await setModNote(instanceUuid.value, form.item.file, value);
    form.item.note = value;
    noteForm.value = null;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    noteBusy.value = false;
  }
}

/**
 * 表格里双击备注**就地编辑**：与备注弹窗**同一个命令**（`setModNote`），
 * 同样只改本地那一份、不重拉列表（理由见上面那段注释）。
 */
async function saveNoteInline(item: ModItemDto, text: string) {
  if (noteBusy.value) return;
  const value = text.trim();
  noteBusy.value = true;
  try {
    await setModNote(instanceUuid.value, item.file, value);
    item.note = value;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    noteBusy.value = false;
  }
}

// ---------- 模组操作（两种视图共用） ----------

/** 单行启用 / 禁用；拖拽刚结束的那一下 click 不算 */
function toggle(item: ModItemDto) {
  if (consumeSuppressClick()) return;
  // 单个就是"批量传一个"：走同一条路，同样不重扫、就地更新那一行
  void setState([item], !item.disable);
}

function remove(item: ModItemDto) {
  askDelete(item.name || item.file, () => deleteMod(instanceUuid.value, item.uuid));
}

onMounted(() => void groups.load());
</script>

<template>
  <ContentHead :data="data">
    <template #actions>
      <!--
        有选中项时顶栏换成批量操作条，用户选的口径是"顶栏出现批量按钮"。
        整条替换而不是并列：搜索框 + 视图切换 + 新建分组本来就占满一行，
        再挤进 4 个批量按钮会互相压。
      -->
      <template v-if="selected.size">
        <span class="mod-selected-count">{{ t("resource.selectedCount", { n: selected.size }) }}</span>
        <button class="mini-btn" :disabled="busy" @click="batchSetState(false)">
          {{ t("resource.enable") }}
        </button>
        <button class="mini-btn" :disabled="busy" @click="batchSetState(true)">
          {{ t("resource.disable") }}
        </button>
        <!-- 归组：开弹窗选目标，不用下拉 —— 下拉一次只看得见一项，
             而且"移到分组"会以占位项的形式混在选项里，看着像能选的东西。
             按钮文字不带省略号（用户要求），与弹窗标题共用同一条文案 -->
        <button class="mini-btn icon-btn" :disabled="busy" @click="groupPickOpen = true">
          <GlyphIcon name="folder" :size="14" />
          {{ t("resource.batchMoveTo") }}
        </button>
        <button class="mini-btn danger" :disabled="busy" @click="batchRemove">
          {{ t("resource.delete") }}
        </button>
        <button class="mini-btn" :disabled="busy" @click="clearSelection">
          {{ t("resource.clearSelection") }}
        </button>
      </template>

      <template v-else>
        <input v-model="keyword" class="mod-search" :placeholder="t('resource.searchPlaceholder')" spellcheck="false"
          autocomplete="off" />
        <SegmentedTabs :model-value="modView" :options="[
          { value: 'list', label: t('resource.viewList') },
          { value: 'table', label: t('resource.viewTable') },
        ]" @update:model-value="setModView($event as ModView)" />
        <button class="mini-btn icon-btn" :disabled="busy" @click="openGroupForm()">
          <GlyphIcon name="folder-plus" :size="14" />
          {{ t("resource.groupAdd") }}
        </button>
      </template>
    </template>
  </ContentHead>

  <!--
    加载中：与「下载整合包」窗口同一套骨架行，呼吸动画见 `styles/skeleton.css`

    模组这一档是真慢 —— 每个 jar 都要解析元数据，几百个包要好几秒，
    所以给骨架 + **扫描进度 `x/x`**，进度事件由后端逐文件发出，见 useResourceData
  -->
  <div v-if="loading" class="item-list">
    <ListSkeleton :done="modProgress.done" :total="modProgress.total" />
  </div>
  <div v-else-if="noMatch" class="empty-tip">{{ t("resource.searchEmpty") }}</div>

  <!--
    表格视图：**一张总表**（分组名就是第一列），不是每块一张小表
    —— 这样"哪个模组在哪个组"一眼看得到，也不必在每块上重复 15 列表头。

    每个分组仍是拖拽落点，data-* 属性挂在 <section> 上，落点判定与列表视图共用一套，
    只是这一块内部不再画分组头与模组行，而是把整批交给 ModTable 自己渲染。
  -->
  <div v-else-if="modView === 'table'" class="item-list mod-groups mod-groups-table">
    <!--
      整张表共用一块白底：行不再各自一块白底。分两层是为了**滚到最右/最底不留空白**：
      - `.mod-table-panel`：白底卡片 + 内边距（白底与内容的呼吸位），**自己不滚**；
      - `.mod-table-scroll`：缩在卡片内边距里的滚动容器 —— 内边距不在它身上，
        所以它的可滚动区域正好等于内容，滚到头就是内容边缘，不会多出一块空白。
      滚动条因此落在白色底里面（用户口径）。
    -->
    <div class="mod-table-panel">
      <div class="mod-table-scroll">
        <template v-for="(sec, idx) in sections" :key="sec.id">
          <span v-if="showSectionLine(idx)" class="mod-group-insert" />

          <section :ref="(el) => bindSection(sec.id, el as Element | null)" class="mod-group mod-group-table"
            :data-mod-group="sec.custom ? sec.id : undefined" :data-state-group="sec.custom ? undefined : sec.state" :class="{
              'drop-target': isDropTarget(sec),
              'mod-group-dragging': draggingSection === sec.id,
            }">
            <ModTable :items="sec.items" :busy="busy" :dragging-key="draggingKey" :selected-keys="selected"
              :group-label="sec.label" :group-key="sec.id" :group-open="isOpen(sec.id)" :custom="sec.custom"
              :dragging-group="draggingSection" :hide-header="idx > 0" :col-widths="colWidths" :sort="sort"
              @col-resize="setColWidth" @sort-change="setSort" @group-menu="openGroupMenu" @toggle="toggle"
              @remove="remove"
              @toggle-group="onSectionToggle(sec.id)" @drag-group="onSectionPointerDown($event, sec.id)"
              @note-save="saveNoteInline" @open-folder="openFolder('mods', $event.file)" @drag-start="onDragStart"
              @select="onSelect" @item-menu="openItemMenu" @row-click="onRowClick" :anchor-key="lastClickedKey" />
          </section>
        </template>
      </div>
    </div>
  </div>

  <!-- 列表视图：按块渲染，自建分组可拖入归组，另加三个状态分组 -->
  <div v-else class="item-list mod-groups">
    <!-- 按块渲染，不用 section 上的 v-for：插入线要画在"两块之间"，得有个兄弟节点 -->
    <template v-for="(sec, idx) in sections" :key="sec.id">
      <!-- 拖分组头时的插入线：松手后这一块会落到这里 -->
      <span v-if="showSectionLine(idx)" class="mod-group-insert" />

      <section :ref="(el) => bindSection(sec.id, el as Element | null)" class="mod-group"
        :data-mod-group="sec.custom ? sec.id : undefined" :data-state-group="sec.custom ? undefined : sec.state" :class="{
          'drop-target': isDropTarget(sec),
          'mod-group-dragging': draggingSection === sec.id,
          'mod-group-empty': !sec.items.length,
        }">
        <div class="mod-group-head" @contextmenu.prevent="openGroupMenu({
          event: $event, id: sec.id, label: sec.label, custom: sec.custom, count: sec.items.length,
        })">
          <!--
            分组头分两块，**两块都能点开 / 收起**：
            - 箭头：只管折叠，不参与拖拽，`@pointerdown.stop` 挡住把手的按下事件；
            - 名字那一带：既是折叠的点击区，也是拖拽把手（按住拖动即调整分组顺序）。
              拖动结束时那一下 click 由 onSectionToggle 里的抑制标记吞掉。
          -->
          <button class="mod-group-toggle" :aria-label="t('resource.groupCollapse')" @pointerdown.stop
            @click.stop="toggleCollapse(sec.id)">
            <GlyphIcon class="mod-group-caret" :class="{ collapsed: !isOpen(sec.id) }" name="chevron-down" :size="13"
              :weight="2.4" />
          </button>
          <span class="mod-group-handle" v-tip="t('resource.groupDragTip')"
            @pointerdown="onSectionPointerDown($event, sec.id)" @click="onSectionToggle(sec.id)">
            <span class="mod-group-name">{{ sec.label }}</span>
          </span>
          <span class="mod-group-count">{{ counts(sec) }}</span>
          <!-- 分组的重命名 / 删除 / 整组批量操作都在**右键菜单**里，用户口径是撤掉原来那排
               悬停按钮，与表格视图同一套入口 -->
        </div>

        <div v-show="isOpen(sec.id)" class="mod-group-body">
          <p v-if="!sec.items.length" class="empty-tip">
            {{ t("resource.groupEmptyHint") }}
          </p>
          <ModList :items="sec.items" :busy="busy" :dragging-key="draggingKey" :selected-keys="selected"
            @toggle="toggle" @remove="remove" @note="openNote" @open-folder="openFolder('mods', $event.file)"
            @drag-start="onDragStart" @select="onSelect" @item-menu="openItemMenu" @row-click="onRowClick"
            :anchor-key="lastClickedKey" />
        </div>
      </section>

      <!-- 尾部插入线：拖到最末一块之后 -->
      <span v-if="idx === sections.length - 1 && showSectionLine(sections.length)" class="mod-group-insert" />
    </template>
  </div>

  <!-- 新建 / 重命名分组 -->
  <BaseModal v-if="groupForm" :title="groupForm.id ? t('resource.groupRenameTitle') : t('resource.groupAddTitle')"
    :closable="false" @close="groupForm = null">
    <label class="field-label">{{ t("resource.groupName") }}</label>
    <input v-model="groupForm.name" class="field-input" :placeholder="t('resource.groupPlaceholder')" spellcheck="false"
      @keydown.enter="saveGroupForm" />
    <div class="modal-actions">
      <BaseButton :disabled="groupBusy" @click="groupForm = null">
        {{ t("resource.cancel") }}
      </BaseButton>
      <BaseButton variant="primary" :disabled="groupBusy" @click="saveGroupForm">
        {{ t("resource.save") }}
      </BaseButton>
    </div>
  </BaseModal>

  <!-- 模组备注：文件名只读展示；用户认的是名字，落盘的键是文件名 -->
  <BaseModal v-if="noteForm" :title="t('resource.modNoteTitle')" :closable="false" @close="noteForm = null">
    <label class="field-label">{{ noteForm.item.name || noteForm.item.file }}</label>
    <p class="mod-note-file" :title="noteForm.item.file">{{ noteForm.item.file }}</p>
    <textarea v-model="noteForm.text" class="field-input mod-note-input" :placeholder="t('resource.modNotePlaceholder')"
      spellcheck="false" @keydown.ctrl.enter="saveNote(noteForm.text)" />
    <div class="modal-actions">
      <BaseButton v-if="noteForm.item.note" :disabled="noteBusy" @click="saveNote('')">
        {{ t("resource.modNoteClear") }}
      </BaseButton>
      <BaseButton :disabled="noteBusy" @click="noteForm = null">
        {{ t("resource.cancel") }}
      </BaseButton>
      <BaseButton variant="primary" :disabled="noteBusy" @click="saveNote(noteForm.text)">
        {{ t("resource.save") }}
      </BaseButton>
    </div>
  </BaseModal>

  <!--
    批量「移到分组」：把自建分组一次列全，点哪一行就移到哪一组（点完即关）。
    比下拉快一步 —— 下拉要先展开、再选、再触发 change。

    **没有「移出分组」那一行**（用户要求）：那是"撤销归组"，把模组拖到状态分组上
    就能做；混在这里会让"移到分组"多出一个反向选项。
    一个分组都没有时列表位置写「无分组」，下面用「新建分组」建一个
  -->
  <BaseModal v-if="groupPickOpen" :closable="false" @close="closeGroupPick"
    :title="moveGroupSource
      ? t('resource.moveGroupTitle', { name: moveGroupSource.label })
      : t('resource.batchMoveTo')">
    <div class="group-pick-list">
      <button v-for="g in groupPickTargets" :key="g.uuid" class="group-pick-row" :disabled="busy"
        @click="pickGroup(g.uuid)">
        <GlyphIcon name="folder" :size="14" />
        <span class="group-pick-name">{{ g.name }}</span>
        <span class="group-pick-count">
          {{ t("resource.groupCountItems", { n: g.mods.length }) }}
        </span>
      </button>

      <!-- 一个可去的自建分组都没有：写明白"无分组"，别留一块空地看着像没加载出来。
           下面那个「新建分组」就是这时该做的事（用户要求） -->
      <p v-if="!groupPickTargets.length" class="empty-tip">
        {{ t("resource.groupNoGroups") }}
      </p>
    </div>

    <div class="modal-actions">
      <BaseButton :disabled="busy" @click="createGroupFromPick">
        {{ t("resource.groupAdd") }}
      </BaseButton>
      <BaseButton @click="closeGroupPick">{{ t("resource.cancel") }}</BaseButton>
    </div>
  </BaseModal>

  <!-- 分组右键菜单：只负责画与定位，动作全部抛回这里执行，见 onGroupMenuAction -->
  <ModGroupMenu v-if="groupMenu" :x="groupMenu.x" :y="groupMenu.y" :label="groupMenu.label"
    :custom="groupMenu.custom" :count="groupMenu.count" :can-enable="groupMenu.canEnable"
    :can-disable="groupMenu.canDisable" @action="onGroupMenuAction" @close="groupMenu = null" />

  <!-- 模组行右键菜单：同一套外观，动作见 onItemMenuAction -->
  <ModItemMenu v-if="itemMenu" :x="itemMenu.x" :y="itemMenu.y" :item="itemMenu.item"
    @action="onItemMenuAction" @close="itemMenu = null" />

  <!-- 多选状态下右键：换成作用于选中项的批量菜单 -->
  <ModBatchMenu v-if="batchMenu" :x="batchMenu.x" :y="batchMenu.y" :count="batchMenu.count"
    :can-enable="batchMenu.canEnable" :can-disable="batchMenu.canDisable" @action="onBatchMenuAction"
    @close="batchMenu = null" />
</template>

<!-- 备注弹窗的样式走 scoped：BaseModal 是 `Teleport to="body"` 的，弹窗不在
     `.resource-layout` 里，窗口级 resource.css 那套前缀规则够不着它。
     同窗口的 ServerFormModal 也是这么处理的 -->
<style scoped>
/* 文件名（落盘的键）压暗一行 */
.mod-note-file {
  margin: 0 0 8px;
  font-size: 11.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mod-note-input {
  min-height: 96px;
  resize: vertical;
  line-height: 1.5;
}

/* ---------- 批量「移到分组」弹窗 ---------- */

/* 分组列表：整行可点，一行一组（比下拉一次只看得见一项强） */
.group-pick-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  /* 分组可能很多：给个上限自己滚，别把弹窗撑出屏幕 */
  max-height: 320px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
}

.group-pick-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 9px 11px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-raised);
  color: var(--text);
  font-size: 12.5px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: all 0.12s;
}

.group-pick-row:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.group-pick-row:disabled {
  opacity: 0.5;
  cursor: default;
}

/* 分组名可被挤压，数量不缩 */
.group-pick-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-pick-count {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
