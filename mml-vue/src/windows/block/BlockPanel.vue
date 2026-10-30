<script setup lang="ts">
// 方块列表：左侧分类栏带数量，右侧网格带搜索 / 分类过滤 / 图标尺寸档
// 三态视图：未渲染 → 提示卡；渲染中 → 顶部一行进度（下方列表照常可用）；已渲染 → 分类栏 + 网格
//
// 状态在 BlockWindow：工具条在标题栏，两边要共用同一份状态，这里只做排版与局部交互；
// 数据、事件订阅与操作在 composables/useBlockList，局部 UI 在 parts/。
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { t } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { SKIN_CAT } from "./types";
import { BLOCK_SIZE_ORDER, type BlockSize } from "./types";
import CategoryRail from "../../components/CategoryRail.vue";
import BlockGrid from "./parts/BlockGrid.vue";
import BlockDetailModal from "./parts/BlockDetailModal.vue";
import InstancePickModal from "./parts/InstancePickModal.vue";
import SkinAddModal from "./parts/SkinAddModal.vue";
import SearchBox from "../../components/ui/SearchBox.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import type { BlockItemDto, InstanceInfoDto } from "../../lib/bindings";
import type { useBlockList } from "./composables/useBlockList";
import type { useBlockGroups } from "./composables/useBlockGroups";

const props = defineProps<{
  /** 方块列表状态，在 BlockWindow 里创建：标题栏的工具条用的是同一份状态 */
  settings: ReturnType<typeof useBlockList>;
  /** 自建分组 / 收藏 / 左栏顺序，同为窗口级状态 */
  groups: ReturnType<typeof useBlockGroups>;
  /** 主窗口当前选中的实例；没有选中实例时为 null */
  currentInstance: InstanceInfoDto | null;
  /** 多选模式：标题栏按钮切换，退出时清空勾选 */
  multiSelect: boolean;
}>();

const emit = defineEmits<{
  /** 退出多选：多选条的退出按钮发这个 */
  (e: "update:multi-select", on: boolean): void;
}>();

const {
  status,
  blocks,
  rendered,
  running,
  preparing,
  percent,
  cancelling,
  startRender,
  cancelRender,
  keyword,
  cat,
  size,
  cats,
  catCounts,
  catLabel,
  isFiltered,
  filtered,
  clearFilters,
  detail,
  openDetail,
  closeDetail,
  removeSkin,
  setIcon,
  addSkin,
} = props.settings;

const {
  orderedKeys,
  setOrder,
  groups: groupList,
  addGroup,
  renameGroup,
  removeGroup,
  groupOf,
  isCustomGroup,
  setGroup,
  countOf,
  isFavorite,
  toggleFavorite,
} = props.groups;

/**
 * 分类栏条目：游戏分类与自建分组混排，顺序按用户拖出来的（没记过的追加在末尾）
 *
 * 键统一是 uuid：自带分类用后端给的固定 uuid，自建分组用自己的 uuid。
 * 文案与计数在这里算好，侧栏只管渲染；「全部」由侧栏自己那份计数画，不参与排序。
 */
const catItems = computed(() =>
  orderedKeys([...cats.value.map((c) => c.uuid), ...groupList.value.map((g) => g.uuid)]).map(
    (key) => {
      const group = groupList.value.find((each) => each.uuid === key);
      if (group) {
        return { id: key, label: group.name, count: countOf(key), custom: true };
      }
      return { id: key, label: catLabel(key), count: catCounts.value.get(key) ?? 0 };
    },
  ),
);

// ---------- 自建分组的右键菜单与新建 / 改名 / 删除 ----------

/** 打开着的分组菜单；null = 没开 */
const groupMenu = ref<{ x: number; y: number; id: string; label: string } | null>(null);

/** 新建 / 改名弹窗；uuid 为空表示新建 */
const groupForm = ref<{ uuid: string; name: string } | null>(null);

/** 待确认删除的分组 */
const groupRemove = ref<{ uuid: string; label: string } | null>(null);

/**
 * 打开自建分组的右键菜单
 *
 * - `payload.event`: 右键事件，用来取弹出坐标
 * - `payload.id`: 分类键；不是自建分组时不弹菜单
 * - `payload.label`: 菜单里用的分组名
 */
function openGroupMenu(payload: { event: MouseEvent; id: string; label: string }) {
  // 自带分类不能改名 / 删除，右键不给菜单
  if (!isCustomGroup(payload.id)) return;
  // 贴边时往回收，菜单别被窗口裁掉
  groupMenu.value = {
    x: Math.min(payload.event.clientX, window.innerWidth - 160),
    y: Math.min(payload.event.clientY, window.innerHeight - 80),
    id: payload.id,
    label: payload.label,
  };
}

/**
 * 分组菜单分发：改名或删除
 *
 * - `kind`: 菜单项标识，取 rename / remove
 */
function onGroupPick(kind: string) {
  const menu = groupMenu.value;
  groupMenu.value = null;
  if (!menu) return;
  const uuid = menu.id;
  if (kind === "rename") {
    groupForm.value = { uuid, name: menu.label };
  } else if (kind === "remove") {
    groupRemove.value = { uuid, label: menu.label };
  }
}

/** 打开新建分组弹窗；带上方块 id 时，建成后顺手把这些方块放进去 */
function openNewGroup(targets: string[] = []) {
  pendingGroupTargets.value = targets;
  groupForm.value = { uuid: "", name: "" };
}

/** 保存新建 / 改名；名为空时不动，新建时把待分配的方块一起放进去 */
function saveGroupForm() {
  const form = groupForm.value;
  if (!form) return;
  const name = form.name.trim();
  if (!name) return;
  if (form.uuid) {
    renameGroup(form.uuid, name);
  } else {
    const uuid = addGroup(name);
    for (const id of pendingGroupTargets.value) setGroup(id, uuid);
  }
  pendingGroupTargets.value = [];
  groupForm.value = null;
}

// ---------- 方块右键：设置实例与设置分组 ----------

/** 当前分类是不是自建分组：只有在这里右键才有「移出分组」，自带分类不展示 */
const inCustomGroupView = computed(() => isCustomGroup(cat.value));

/** 主菜单；null = 没开 */
const blockMenu = ref<{ x: number; y: number; item: BlockItemDto } | null>(null);

/** 设置分组弹窗里的方块 id；空数组表示没开（多选时一次设置多个） */
const groupTargets = ref<string[]>([]);

/** 从弹窗新建分组时，建成后要把这些方块放进去 */
const pendingGroupTargets = ref<string[]>([]);

/** 弹窗是否打开 */
const groupPickOpen = computed(() => groupTargets.value.length > 0);

/** 弹窗里显示的方块名；多选时显示选了几个 */
const groupPickName = computed(() => {
  if (groupTargets.value.length === 1) {
    const id = groupTargets.value[0];
    return blocks.value.find((b) => b.id === id)?.name ?? id;
  }
  return t("multi.selected", { count: groupTargets.value.length });
});

/** 多选的方块分组不一致；下拉单独显示「多个分组」占位，不冒充「未分组」 */
const groupPickMixed = computed(() => {
  const ids = groupTargets.value;
  if (ids.length < 2) return false;
  const first = groupOf(ids[0]) ?? "";
  return !ids.every((id) => (groupOf(id) ?? "") === first);
});

/** 多选时共同的分组；不一致时由占位项接管 */
const groupPickValue = computed(() => {
  const ids = groupTargets.value;
  if (!ids.length || groupPickMixed.value) return "";
  return groupOf(ids[0]) ?? "";
});

/**
 * 打开方块右键菜单
 *
 * - `payload.event`: 右键事件，用来取弹出坐标
 * - `payload.item`: 右键的那个方块
 */
function openBlockMenu(payload: { event: MouseEvent; item: BlockItemDto }) {
  blockMenu.value = {
    x: Math.min(payload.event.clientX, window.innerWidth - 200),
    y: Math.min(payload.event.clientY, window.innerHeight - 160),
    item: payload.item,
  };
}

/** 设置实例：走详情那套选实例弹窗，目标是右键的这个方块 */
function menuSetIcon() {
  const menu = blockMenu.value;
  if (!menu) return;
  blockMenu.value = null;
  openIconPick(menu.item);
}

/** 设置分组：主菜单收掉，弹窗里从下拉选一个分组 */
function menuSetGroup() {
  const menu = blockMenu.value;
  if (!menu) return;
  blockMenu.value = null;
  groupTargets.value = [menu.item.id];
}

/** 弹窗下拉框换分组：空串表示未分组，一次应用到所有目标方块 */
function onGroupPickChange(uuid: string) {
  if (uuid === "__mixed__") return;
  for (const id of groupTargets.value) setGroup(id, uuid || null);
}

// ---------- 多选：勾选与批量设置分组 ----------

/** 多选模式里勾选的方块 id */
const selectedIds = ref<Set<string>>(new Set());

/** Shift 连选的锚点：上一次点中的下标 */
let anchorIndex: number | null = null;

// 退出多选时清空勾选，免得下次进来还留着上一次的选择
watch(
  () => props.multiSelect,
  (on) => {
    if (!on) {
      selectedIds.value = new Set();
      anchorIndex = null;
    }
  },
);

/**
 * 多选模式下点方块：普通点击切换，Ctrl 点单独切换，Shift 点从锚点连选
 *
 * - `b`: 被点的方块
 * - `event`: 点击事件，用来读 Ctrl / Shift
 */
function toggleSelect(b: BlockItemDto, event?: MouseEvent) {
  const list = filtered.value;
  const index = list.findIndex((item) => item.id === b.id);
  const next = new Set(selectedIds.value);
  if (event?.shiftKey && anchorIndex !== null && anchorIndex >= 0 && index >= 0) {
    const from = Math.min(anchorIndex, index);
    const to = Math.max(anchorIndex, index);
    for (let i = from; i <= to; i += 1) next.add(list[i].id);
  } else if (next.has(b.id)) {
    next.delete(b.id);
  } else {
    next.add(b.id);
  }
  selectedIds.value = next;
  anchorIndex = index;
}

/** 全选当前筛选结果；与收藏窗口一样取并集，不清掉别的分类里已勾的 */
function selectAllVisible() {
  const next = new Set(selectedIds.value);
  for (const item of filtered.value) next.add(item.id);
  selectedIds.value = next;
  // 只有一块内容时也提示一句，否则看不出 Ctrl+A 生效了
  showToast(t("multi.selectAllDone", { count: next.size }));
}

/** 多选条上的「设置分组」：把勾选的方块一起交给分组弹窗 */
function openMultiGroup() {
  if (selectedIds.value.size) groupTargets.value = [...selectedIds.value];
}

/** 退出多选 */
function exitMultiSelect() {
  emit("update:multi-select", false);
}

/** 方块右键菜单的「多选」：进入多选并把这一项勾上 */
function enterMultiSelectFromMenu() {
  const menu = blockMenu.value;
  if (!menu) return;
  blockMenu.value = null;
  emit("update:multi-select", true);
  toggleSelect(menu.item);
}

/** Ctrl+A / Cmd+A：全选当前列表（输入框里不拦，照常全选文字） */
function onDocKey(e: KeyboardEvent) {
  if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== "a") return;
  const el = e.target as HTMLElement | null;
  if (el?.closest("input, textarea, select, [contenteditable='true']")) return;
  if (!rendered.value || filtered.value.length === 0) return;
  e.preventDefault();
  emit("update:multi-select", true);
  selectAllVisible();
}

onMounted(() => document.addEventListener("keydown", onDocKey));
onUnmounted(() => document.removeEventListener("keydown", onDocKey));

// ---------- 内容区工具栏：搜索 / 玩家头颅 / 尺寸滑杆 ----------

/** 当前是不是玩家头颅分类：添加按钮只在这里出现 */
const inSkinCat = computed(() => cats.value.find((c) => c.uuid === cat.value)?.id === SKIN_CAT);

/** 添加玩家头颅弹窗是否开着 */
const skinOpen = ref(false);
/** 提交中：禁用按钮防连点 */
const skinBusy = ref(false);

/**
 * 提交要添加的用户名或 UUID
 *
 * - `input`: 弹窗里输入的内容，交给 useBlockList 的 addSkin 校验与落盘
 */
async function onSkinSubmit(input: string) {
  if (skinBusy.value) return;
  skinBusy.value = true;
  try {
    if (await addSkin(input)) skinOpen.value = false;
  } finally {
    skinBusy.value = false;
  }
}

/** 尺寸滑杆的档位下标，小 → 中 → 大 */
const sizeIndex = computed(() => Math.max(0, BLOCK_SIZE_ORDER.indexOf(size.value)));

/** 滑杆拖动：下标换算回尺寸档 */
function onSizeSlider(event: Event) {
  const next = BLOCK_SIZE_ORDER[Number((event.target as HTMLInputElement).value)];
  if (next) size.value = next as BlockSize;
}

/** 右键菜单里删除分组：只断开这个方块的归属，分组本身保留 */
function menuRemoveGroup() {
  const menu = blockMenu.value;
  if (!menu) return;
  blockMenu.value = null;
  setGroup(menu.item.id, null);
}

/** 删除分组：分组没了，方块本身不动；删的正好是当前分类时回到「全部」 */
function confirmRemoveGroup() {
  const target = groupRemove.value;
  groupRemove.value = null;
  if (!target) return;
  removeGroup(target.uuid);
  if (cat.value === target.uuid) cat.value = "";
}

/** 详情里收藏 / 取消收藏 */
function onDetailFavorite() {
  if (detail.value) toggleFavorite(detail.value.id);
}

/** 详情里换分组：空表示未分组 */
function onDetailSetGroup(uuid: string | null) {
  if (detail.value) setGroup(detail.value.id, uuid);
}

/**
 * 提示卡的正文
 *
 * 只在"有话说"时才出来：渲染失败给原因，渲染中给下载说明；**未渲染这一档不写正文**，
 * 只留标题、下面那行下载提示与按钮
 */
const descText = computed(() =>
  status.value?.error ?? (running.value ? t("blocks.renderFirstHint") : ""),
);

// ---------- 设为实例图标 ----------

/** 正在选实例图标的方块；null = 没开 */
const iconTarget = ref<BlockItemDto | null>(null);
const iconBusy = ref(false);

/**
 * 打开选实例弹窗
 *
 * - `block`: 要设为图标的那个方块
 */
function openIconPick(block: BlockItemDto) {
  iconTarget.value = block;
}

/** 详情里点「设为实例图标」 */
function openDetailIconPick() {
  if (detail.value) openIconPick(detail.value);
}

/**
 * 确认把方块设为该实例的图标；成功后关掉弹窗与详情
 *
 * - `inst`: 选中的实例
 */
async function onIconPick(inst: InstanceInfoDto) {
  const b = iconTarget.value;
  if (!b || iconBusy.value) return;
  iconBusy.value = true;
  try {
    if (await setIcon(inst.uuid, b.id, inst.name)) {
      iconTarget.value = null;
      closeDetail();
    }
  } finally {
    iconBusy.value = false;
  }
}

/** 详情弹窗里删除玩家头颅；详情会随 removeSkin 一起关掉 */
async function onDetailRemove() {
  const b = detail.value;
  if (b) await removeSkin(b);
}
</script>

<template>
  <div class="block-panel">
    <!-- 渲染中：顶部一行进度；不挤掉下方内容，取消后立即消失 -->
    <div v-if="running" class="block-progress">
      <span class="block-progress-text">
        {{ status?.text || (preparing ? t("blocks.downloadingCore") : t("blocks.rendering")) }}
      </span>
      <div class="block-progress-bar">
        <div class="block-progress-fill" :style="{ width: percent + '%' }"></div>
      </div>
      <span class="block-progress-percent">{{ preparing ? "—" : percent.toFixed(1) + "%" }}</span>
      <span v-if="!preparing" class="block-progress-num">
        {{ status?.now ?? 0 }} / {{ status?.total ?? 0 }}
      </span>
      <BaseButton size="sm" :disabled="cancelling" @click="cancelRender">
        {{ cancelling ? t("blocks.cancelling") : t("blocks.cancelRender") }}
      </BaseButton>
    </div>

    <!-- 内容区工具条：搜索与玩家头颅入口都放这里，不占标题栏 -->
    <div v-if="rendered" class="block-filters">
      <SearchBox v-model="keyword" :placeholder="t('blocks.search')" :clear-label="t('blocks.searchClear')"
        width="240px" />
      <BaseButton v-if="inSkinCat" size="sm" variant="accent" @click="skinOpen = true">
        {{ t("blocks.addSkin") }}
      </BaseButton>
    </div>

    <!-- 多选工具条：与主窗口多选条同一套外观，浮在标题栏下方 -->
    <div v-if="multiSelect" class="multi-bar">
      <span class="multi-count">{{ t("multi.selected", { count: selectedIds.size }) }}</span>
      <button class="multi-btn" @click="selectAllVisible">{{ t("multi.selectAll") }}</button>
      <button class="multi-btn" :disabled="!selectedIds.size" @click="openMultiGroup">
        {{ t("blocks.menuSetGroup") }}
      </button>
      <span class="multi-sep" />
      <button class="multi-exit" @click="exitMultiSelect">{{ t("multi.exit") }}</button>
    </div>

    <!-- 已渲染：分类栏 + 网格 -->
    <div v-if="rendered" class="block-main">
      <CategoryRail :all-label="t('blocks.catAll')" :all-count="blocks.length" :items="catItems" :active="cat"
        :sortable="true" :new-label="t('blocks.groupAdd')" @pick="cat = $event" @reorder="setOrder"
        @group-menu="openGroupMenu" @new-group="openNewGroup()" />
      <BlockGrid :items="filtered" :keyword="keyword" :size="size" :active-id="detail?.id ?? null"
        :skin-cat="SKIN_CAT" :filtered="isFiltered" :is-favorite="isFavorite" :toggle-favorite="toggleFavorite"
        :select-mode="multiSelect" :is-selected="(id) => selectedIds.has(id)"
        @open="openDetail" @remove-skin="removeSkin" @clear-filters="clearFilters"
        @item-menu="openBlockMenu" @toggle-select="(p) => toggleSelect(p.item, p.event)" />
    </div>

    <!-- 尺寸滑杆：右下角，小 → 大连续调图标大小 -->
    <div v-if="rendered" class="size-dock">
      <span class="size-edge">{{ t("blocks.size.sm") }}</span>
      <input class="size-range" type="range" min="0" max="2" step="1" :value="sizeIndex"
        :aria-label="t('blocks.size.title')" :title="t('blocks.size.title')" @input="onSizeSlider" />
      <span class="size-edge">{{ t("blocks.size.lg") }}</span>
    </div>

    <!-- 未渲染：提示卡；渲染中显示进度说明，不再给按钮 -->
    <div v-if="!rendered" class="block-prompt">
      <div class="block-prompt-icon">
        <svg viewBox="0 0 24 24" width="40" height="40" fill="none" stroke="currentColor" stroke-width="1.6"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3 4.5 7.5v9L12 21l7.5-4.5v-9L12 3z" />
          <path d="M4.5 7.5 12 12l7.5-4.5" />
          <path d="M12 12v9" />
        </svg>
      </div>
      <h2 class="block-prompt-title">
        {{ status?.error ? t("blocks.renderFailed") : running ? t("blocks.rendering") : t("blocks.renderPromptTitle") }}
      </h2>
      <p v-if="descText" class="block-prompt-desc">{{ descText }}</p>
      <BaseButton v-if="!running" variant="primary" @click="startRender(false)">
        {{ status?.error ? t("blocks.retry") : t("blocks.renderNow") }}
      </BaseButton>
    </div>

    <!-- 设为实例图标：选哪个实例（当前实例高亮，点条目即设） -->
    <InstancePickModal v-if="iconTarget" :current-uuid="currentInstance?.uuid ?? null" :busy="iconBusy"
      @pick="onIconPick" @close="iconTarget = null" />

    <!-- 添加玩家头颅（用户名或 UUID）：入口在玩家头颅分类里 -->
    <SkinAddModal v-if="skinOpen" :busy="skinBusy" @submit="onSkinSubmit" @close="skinOpen = false" />

    <!-- 方块详情：打开选实例弹窗时先让位，避免两层遮罩叠着 -->
    <BlockDetailModal v-if="detail && !iconTarget" :block="detail" :category="catLabel(detail.catUuid)" :keyword="keyword"
      :removable="detail.catId === SKIN_CAT" :busy="iconBusy" :favorite="isFavorite(detail.id)"
      :groups="groupList" :group="groupOf(detail.id)" @close="closeDetail" @remove="onDetailRemove"
      @set-icon="openDetailIconPick" @toggle-favorite="onDetailFavorite" @set-group="onDetailSetGroup" />

    <!-- 分组右键菜单：改名 / 删除；点空白处关掉 -->
    <div v-if="groupMenu" class="menu-backdrop" @pointerdown="groupMenu = null" @contextmenu.prevent />
    <div v-if="groupMenu" class="group-menu"
      :style="{ left: groupMenu.x + 'px', top: groupMenu.y + 'px' }" @pointerdown.stop @contextmenu.prevent>
      <button class="group-menu-item" @click="onGroupPick('rename')">{{ t("blocks.groupRename") }}</button>
      <button class="group-menu-item danger" @click="onGroupPick('remove')">{{ t("blocks.groupRemove") }}</button>
    </div>

    <!-- 方块右键菜单：设置实例 / 设置分组；自建分组视图里多一个「移出分组」 -->
    <div v-if="blockMenu" class="menu-backdrop" @pointerdown="blockMenu = null" @contextmenu.prevent />
    <div v-if="blockMenu" class="group-menu"
      :style="{ left: blockMenu.x + 'px', top: blockMenu.y + 'px' }" @pointerdown.stop @contextmenu.prevent>
      <button v-if="!multiSelect" class="group-menu-item" @click="enterMultiSelectFromMenu">
        {{ t("blocks.multiSelect") }}
      </button>
      <button v-if="!multiSelect" class="group-menu-item" @click="menuSetIcon">{{ t("blocks.menuSetIcon") }}</button>
      <button v-if="!multiSelect" class="group-menu-item" @click="menuSetGroup">{{ t("blocks.menuSetGroup") }}</button>
      <button v-if="inCustomGroupView && groupOf(blockMenu.item.id)" class="group-menu-item danger"
        @click="menuRemoveGroup">
        {{ t("blocks.groupDeassign") }}
      </button>
    </div>

    <!-- 设置分组：下拉选一个分组，空串表示未分组；多选时一次应用到所有勾选的方块 -->
    <BaseModal v-if="groupPickOpen" :title="t('blocks.menuSetGroup')" :width="380" :closable="false"
      @close="groupTargets = []">
      <p class="group-pick-name">{{ groupPickName }}</p>
      <label class="field-label">{{ t("blocks.detailGroups") }}</label>
      <select class="field-select" :value="groupPickMixed ? '__mixed__' : groupPickValue"
        @change="onGroupPickChange(($event.target as HTMLSelectElement).value)">
        <option v-if="groupPickMixed" value="__mixed__" disabled>{{ t("blocks.groupMixed") }}</option>
        <option value="">{{ t("blocks.groupNone") }}</option>
        <option v-for="g in groupList" :key="g.uuid" :value="g.uuid">{{ g.name }}</option>
      </select>
      <p v-if="!groupList.length" class="group-pick-empty">{{ t("blocks.groupEmpty") }}</p>
      <div class="modal-actions">
        <BaseButton size="sm" class="group-pick-new" @click="openNewGroup(groupTargets)">
          {{ t("blocks.groupAdd") }}
        </BaseButton>
        <BaseButton @click="groupTargets = []">{{ t("blocks.close") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 新建 / 改名分组 -->
    <BaseModal v-if="groupForm" :closable="false"
      :title="groupForm.uuid ? t('blocks.groupRename') : t('blocks.groupAdd')" @close="groupForm = null">
      <input v-model="groupForm.name" class="field-input" :placeholder="t('blocks.groupName')" spellcheck="false"
        @keydown.enter="saveGroupForm" />
      <div class="modal-actions">
        <BaseButton @click="groupForm = null">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="primary" :disabled="!groupForm.name.trim()" @click="saveGroupForm">
          {{ t("blocks.groupSave") }}
        </BaseButton>
      </div>
    </BaseModal>

    <!-- 删除分组确认 -->
    <BaseModal v-if="groupRemove" :title="t('blocks.groupRemove')" :closable="false" @close="groupRemove = null">
      <p class="group-tip">{{ t("blocks.groupRemoveConfirm", { name: groupRemove.label }) }}</p>
      <div class="modal-actions">
        <BaseButton @click="groupRemove = null">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="confirmRemoveGroup">{{ t("blocks.groupRemove") }}</BaseButton>
      </div>
    </BaseModal>
  </div>
</template>

<style scoped src="../../styles/parts/multi-bar.css"></style>

<style scoped>
/* 多选工具条：外观与主窗口同一套，浮在标题栏下方居中 */
.multi-bar {
  position: fixed;
  top: calc(var(--titlebar-h) + 10px);
  left: 50%;
  transform: translateX(-50%);
  z-index: 40;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--bg-card);
  border: 1px solid var(--accent-border);
  border-radius: 12px;
  box-shadow: var(--shadow-md);
}

.block-panel {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-height: 0;
  /* 上 / 右 / 下 三边抵消 WindowFrame .frame-body 的 22/26 内边距：
     网格与它的滚动条顶到窗口右边缘、上下也一直延伸到窗口边（滚动条是整条）。
     左内边距保持原样；上下的呼吸改由各自滚动内容自己的留白给 ——
     网格见 BlockGrid 的 padding-top 与 BOTTOM_SPACE，分类栏见 .cat-rail 的 padding。
     那样空档只在滚到两端时出现，而不是把滚动区缩掉 */
  margin-right: -26px;
  margin-bottom: -22px;
  margin-top: -22px;
  padding-right: 0;
}

/* ---------- 内容区工具条 ---------- */

.block-filters {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 22px;
  margin-right: 26px;
}

/* 上面已经有进度行时不再补一份顶部留白 */
.block-progress + .block-filters {
  margin-top: 0;
}

/* ---------- 尺寸滑杆（右下角） ---------- */

.size-dock {
  position: absolute;
  right: 34px;
  bottom: 30px;
  z-index: 20;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-card);
  box-shadow: var(--shadow-md);
}

.size-edge {
  font-size: 11px;
  color: var(--text-dim);
}

.size-range {
  width: 120px;
  margin: 0;
  accent-color: var(--accent);
  cursor: pointer;
}

/* ---------- 分组右键菜单 ---------- */

/* 右键菜单挂在窗口最上层：遮罩与菜单同为 250 档，菜单在 DOM 里靠后，自然压在遮罩上 */
.menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 250;
}

.group-menu {
  position: fixed;
  z-index: 250;
  min-width: 140px;
  /* 分组多时在菜单内滚动，不把菜单撑出窗口 */
  max-height: 70vh;
  overflow-y: auto;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-modal);
  box-shadow: var(--shadow-lg);
}

/* ---------- 设置分组弹窗 ---------- */

/* 方块名跟在标题下面，与分组列表拉开一点 */
.group-pick-name {
  margin: 0 0 10px;
  font-size: 12.5px;
  color: var(--text-dim);
  word-break: break-all;
}

/* 还没有分组时，下拉框下面补一句怎么建 */
.group-pick-empty {
  margin: 6px 0 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

/* 新建分组靠左，其余操作留在右侧 */
.group-pick-new {
  margin-right: auto;
}

.group-menu-item {
  display: block;
  width: 100%;
  padding: 6px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
}

.group-menu-item:hover {
  background: var(--bg-hover);
}

.group-menu-item.danger:hover {
  color: var(--red);
}

.group-tip {
  margin: 0;
  font-size: 13px;
  color: var(--text-dim);
}

/* 只有网格 / 分类栏区域是"通栏"的，其余卡片保持和网格一致的留白：
   进度行在第一行时要补回 22px（它在面板顶部）；提示卡只在自己是第一行、或作为最底元素时补。
   右边距取 26 而不是网格那 16：网格的缩进 16 之外还有约 9px 的滚动条槽，
   卡片没有那条槽，按 16 补会让卡片右边比左边多空出去 16px，看着左右不一样宽 */
.block-progress,
.block-prompt {
  margin-right: 26px;
}

.block-progress,
.block-panel>.block-prompt:first-child {
  margin-top: 22px;
}

.block-prompt {
  margin-bottom: 22px;
}

/* ---------- 提示卡（未渲染 / 失败） ---------- */
.block-prompt {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 32px 20px 24px;
  border: 1px dashed var(--accent-border);
  border-radius: 12px;
  background: var(--bg-card);
}

.block-prompt-icon {
  width: 84px;
  height: 84px;
  border-radius: 12px;
  background: var(--accent-soft);
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 8px;
}

.block-prompt-title {
  font-size: 18px;
  font-weight: 700;
}

.block-prompt-desc {
  font-size: 13px;
  color: var(--text-dim);
  text-align: center;
  max-width: 420px;
}

/* ---------- 进度（渲染中，顶部一行） ---------- */
.block-progress {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
  font-size: 12.5px;
}

.block-progress-text {
  flex-shrink: 0;
  max-width: 45%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

/* 进度条占满中段，两端文字按内容宽度 */
.block-progress-bar {
  flex: 1;
  min-width: 60px;
  height: 6px;
  border-radius: 999px;
  background: var(--bg-side);
  overflow: hidden;
}

.block-progress-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.3s;
}

.block-progress-percent {
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  color: var(--accent);
}

.block-progress-num {
  flex-shrink: 0;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* ---------- 主体：分类栏 + 网格 ---------- */
.block-main {
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 16px;
}
</style>
