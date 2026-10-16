<script setup lang="ts">
// 方块列表：左侧分类栏带数量，右侧网格带搜索 / 分类过滤 / 图标尺寸档
// 三态视图：未渲染 → 提示卡；渲染中 → 顶部一行进度（下方列表照常可用）；已渲染 → 分类栏 + 网格
//
// 状态在 BlockWindow：工具条在标题栏，两边要共用同一份状态，这里只做排版与局部交互；
// 数据、事件订阅与操作在 composables/useBlockList，局部 UI 在 parts/。
import { computed, ref } from "vue";
import { t } from "../../lib/i18n";
import { GROUP_PREFIX, SKIN_CAT } from "./types";
import CategoryRail from "./parts/CategoryRail.vue";
import BlockGrid from "./parts/BlockGrid.vue";
import BlockDetailModal from "./parts/BlockDetailModal.vue";
import InstancePickModal from "./parts/InstancePickModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import type { BlockItemDto, InstanceInfoDto } from "../../lib/bindings";
import type { useBlockList } from "./composables/useBlockList";
import { groupKey, isGroupKey, type useBlockGroups } from "./composables/useBlockGroups";

const props = defineProps<{
  /** 方块列表状态，在 BlockWindow 里创建：标题栏的工具条用的是同一份状态 */
  settings: ReturnType<typeof useBlockList>;
  /** 自建分组 / 收藏 / 左栏顺序，同为窗口级状态 */
  groups: ReturnType<typeof useBlockGroups>;
  /** 主窗口当前选中的实例；没有选中实例时为 null */
  currentInstance: InstanceInfoDto | null;
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
} = props.settings;

const {
  orderedKeys,
  setOrder,
  groups: groupList,
  addGroup,
  renameGroup,
  removeGroup,
  inGroup,
  groupsOf,
  setMember,
  countOf,
  isFavorite,
  toggleFavorite,
} = props.groups;

/**
 * 分类栏条目：游戏分类与自建分组混排，顺序按用户拖出来的（没记过的追加在末尾）
 *
 * 文案与计数在这里算好，侧栏只管渲染；「全部」由侧栏自己那份计数画，不参与排序。
 */
const catItems = computed(() =>
  orderedKeys([...cats.value, ...groupList.value.map((g) => groupKey(g.uuid))]).map((key) => {
    if (isGroupKey(key)) {
      const group = groupList.value.find((each) => groupKey(each.uuid) === key);
      return {
        id: key,
        label: group?.name ?? key,
        count: group ? countOf(group.uuid) : 0,
        custom: true,
      };
    }
    return { id: key, label: catLabel(key), count: catCounts.value.get(key) ?? 0 };
  }),
);

// ---------- 自建分组的右键菜单与新建 / 改名 / 删除 ----------

/** 打开着的分组菜单；null = 没开 */
const groupMenu = ref<{ x: number; y: number; id: string; label: string } | null>(null);

/** 新建 / 改名弹窗；uuid 为空表示新建 */
const groupForm = ref<{ uuid: string; name: string } | null>(null);

/** 待确认删除的分组 */
const groupRemove = ref<{ uuid: string; label: string } | null>(null);

function openGroupMenu(payload: { event: MouseEvent; id: string; label: string }) {
  // 贴边时往回收，菜单别被窗口裁掉
  groupMenu.value = {
    x: Math.min(payload.event.clientX, window.innerWidth - 160),
    y: Math.min(payload.event.clientY, window.innerHeight - 80),
    id: payload.id,
    label: payload.label,
  };
}

/** 分组菜单分发 */
function onGroupPick(kind: string) {
  const menu = groupMenu.value;
  groupMenu.value = null;
  if (!menu) return;
  const uuid = menu.id.slice(GROUP_PREFIX.length);
  if (kind === "rename") {
    groupForm.value = { uuid, name: menu.label };
  } else if (kind === "remove") {
    groupRemove.value = { uuid, label: menu.label };
  }
}

/** 打开新建分组弹窗；从方块菜单进来时记下那个方块，建成后顺手放进去 */
function openNewGroup(blockId: string | null = null) {
  pendingGroupBlock.value = blockId;
  groupForm.value = { uuid: "", name: "" };
}

/** 保存新建 / 改名：名字为空不动 */
function saveGroupForm() {
  const form = groupForm.value;
  if (!form) return;
  const name = form.name.trim();
  if (!name) return;
  if (form.uuid) {
    renameGroup(form.uuid, name);
  } else {
    const uuid = addGroup(name);
    if (pendingGroupBlock.value) setMember(uuid, pendingGroupBlock.value, true);
  }
  pendingGroupBlock.value = null;
  groupForm.value = null;
}

// ---------- 方块右键：收藏与分组 ----------

/** 打开着的方块菜单；null = 没开 */
const blockMenu = ref<{ x: number; y: number; item: BlockItemDto } | null>(null);

/** 从方块菜单新建分组时，建成后要把这个方块放进去 */
const pendingGroupBlock = ref<string | null>(null);

function openBlockMenu(payload: { event: MouseEvent; item: BlockItemDto }) {
  blockMenu.value = {
    x: Math.min(payload.event.clientX, window.innerWidth - 200),
    y: Math.min(payload.event.clientY, window.innerHeight - 220),
    item: payload.item,
  };
}

/** 菜单里收藏 / 取消收藏 */
function menuToggleFavorite() {
  const menu = blockMenu.value;
  if (menu) toggleFavorite(menu.item.id);
}

/** 菜单里勾选分组：可以同时属于多个，点一下就是切换 */
function menuToggleGroup(uuid: string) {
  const menu = blockMenu.value;
  if (menu) setMember(uuid, menu.item.id, !inGroup(uuid, menu.item.id));
}

/** 菜单里新建分组：建成后把当前方块放进去 */
function menuNewGroup() {
  const menu = blockMenu.value;
  blockMenu.value = null;
  openNewGroup(menu?.item.id ?? null);
}

/** 删除分组：分组没了，方块本身不动；删的正好是当前分类时回到「全部」 */
function confirmRemoveGroup() {
  const target = groupRemove.value;
  groupRemove.value = null;
  if (!target) return;
  removeGroup(target.uuid);
  if (cat.value === groupKey(target.uuid)) cat.value = "";
}

/** 详情里收藏 / 取消收藏 */
function onDetailFavorite() {
  if (detail.value) toggleFavorite(detail.value.id);
}

/** 详情里勾选分组：可同时属于多个 */
function onDetailToggleGroup(uuid: string, on: boolean) {
  if (detail.value) setMember(uuid, detail.value.id, on);
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

const iconPick = ref(false);
const iconBusy = ref(false);

function openIconPick() {
  if (detail.value) iconPick.value = true;
}

async function onIconPick(inst: InstanceInfoDto) {
  const b = detail.value;
  if (!b || iconBusy.value) return;
  iconBusy.value = true;
  try {
    if (await setIcon(inst.uuid, b.id, inst.name)) {
      iconPick.value = false;
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

    <!-- 已渲染：分类栏 + 网格 -->
    <div v-if="rendered" class="block-main">
      <CategoryRail :all-count="blocks.length" :items="catItems" :active="cat" @pick="cat = $event"
        @reorder="setOrder" @group-menu="openGroupMenu" @new-group="openNewGroup()" />
      <BlockGrid :items="filtered" :keyword="keyword" :size="size" :active-id="detail?.id ?? null" :skin-cat="SKIN_CAT"
        :filtered="isFiltered" :is-favorite="isFavorite" @open="openDetail" @remove-skin="removeSkin"
        @clear-filters="clearFilters" @item-menu="openBlockMenu" />
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
    <InstancePickModal v-if="iconPick" :current-uuid="currentInstance?.uuid ?? null" :busy="iconBusy" @pick="onIconPick"
      @close="iconPick = false" />

    <!-- 方块详情：打开选实例弹窗时先让位，避免两层遮罩叠着 -->
    <BlockDetailModal v-if="detail && !iconPick" :block="detail" :category="catLabel(detail.cat)" :keyword="keyword"
      :removable="detail.cat === SKIN_CAT" :busy="iconBusy" :favorite="isFavorite(detail.id)"
      :groups="groupList" :member-of="groupsOf(detail.id)" @close="closeDetail" @remove="onDetailRemove"
      @set-icon="openIconPick" @toggle-favorite="onDetailFavorite" @toggle-group="onDetailToggleGroup" />

    <!-- 分组右键菜单：改名 / 删除；点空白处关掉 -->
    <div v-if="groupMenu" class="menu-backdrop" @pointerdown="groupMenu = null" @contextmenu.prevent />
    <div v-if="groupMenu" class="group-menu"
      :style="{ left: groupMenu.x + 'px', top: groupMenu.y + 'px' }" @pointerdown.stop @contextmenu.prevent>
      <button class="group-menu-item" @click="onGroupPick('rename')">{{ t("blocks.groupRename") }}</button>
      <button class="group-menu-item danger" @click="onGroupPick('remove')">{{ t("blocks.groupRemove") }}</button>
    </div>

    <!-- 方块右键菜单：收藏 + 逐个分组勾选 + 新建分组 -->
    <div v-if="blockMenu" class="menu-backdrop" @pointerdown="blockMenu = null" @contextmenu.prevent />
    <div v-if="blockMenu" class="group-menu"
      :style="{ left: blockMenu.x + 'px', top: blockMenu.y + 'px' }" @pointerdown.stop @contextmenu.prevent>
      <button class="group-menu-item" @click="menuToggleFavorite">
        {{ isFavorite(blockMenu.item.id) ? t("blocks.unfavorite") : t("blocks.favorite") }}
      </button>
      <!-- 分组那一段：一个分组都没有时不画，否则与下面那条分隔线会连成两条 -->
      <template v-if="groupList.length">
        <span class="group-menu-sep" />
        <button v-for="g in groupList" :key="g.uuid" class="group-menu-item" @click="menuToggleGroup(g.uuid)">
          <span class="group-menu-check">{{ inGroup(g.uuid, blockMenu.item.id) ? "✓" : "" }}</span>
          {{ g.name }}
        </button>
      </template>
      <span class="group-menu-sep" />
      <button class="group-menu-item" @click="menuNewGroup">{{ t("blocks.groupAdd") }}</button>
    </div>

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

<style scoped>
.block-panel {
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

/* ---------- 分组右键菜单 ---------- */

.menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 400;
}

.group-menu {
  position: fixed;
  z-index: 401;
  min-width: 140px;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-modal);
  box-shadow: var(--shadow-lg);
}

/* 菜单里的分隔线 */
.group-menu-sep {
  display: block;
  height: 1px;
  margin: 4px 6px;
  background: var(--border);
}

/* 分组项前面的勾位：没勾也占宽，名字才对得齐 */
.group-menu-check {
  display: inline-block;
  width: 14px;
  color: var(--accent);
}

.group-menu-item {
  display: block;
  width: 100%;
  padding: 7px 10px;
  border: none;
  border-radius: 7px;
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
  color: var(--danger, #e5484d);
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
  padding: 30px 20px 26px;
  border: 1px dashed var(--accent-border);
  border-radius: 16px;
  background: var(--bg-card);
}

.block-prompt-icon {
  width: 84px;
  height: 84px;
  border-radius: 24px;
  background: var(--accent-soft);
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 8px;
}

.block-prompt-title {
  font-size: 17px;
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
  transition: width 0.2s ease;
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
