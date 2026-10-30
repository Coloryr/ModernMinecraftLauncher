<script setup lang="ts">
// 主窗口右键菜单：分组 / 实例 / 多选 三种视图
// 所有动作通过事件抛给父组件处理
//
// 目标分组的选择**不在这里**：那是个弹窗，见 MainWindow 的"选择目标分组"。
// 菜单视图只由 `menu.kind` 决定，别再加第二级视图 —— 父级 kind 不变时永远渲染不到。
//
// 定位：`position: fixed` 钉在点击处，但**渲染后要量一次并收进窗口**——
// 否则靠近窗口下/右边缘点开时，菜单会超出可视区被裁掉 —— 实例菜单十来项，比分组菜单高一截。
import { computed, nextTick, ref, watchEffect } from "vue";
import { t } from "../../../lib/i18n";
import type { CtxMenuState, GroupView, InstMenuAction } from "../types";

const props = defineProps<{
  /** 当前菜单状态；null 表示菜单没开 */
  menu: CtxMenuState | null;
  /** 全部分组视图，用来算置灰与"转移分组"的候选 */
  groups: GroupView[];
}>();

/** 当前右键的分组；空分组时"全选 / 启动全部"置灰，"转移分组"另见 canMoveGroup */
const currentGroup = computed(() =>
  props.menu?.kind === "group"
    ? props.groups.find((x) => x.id === props.menu?.groupId)
    : undefined,
);
const groupEmpty = computed(() => !currentGroup.value || currentGroup.value.items.length === 0);

/** 默认分组不能删；它的名字留白，用 isDefault 判，不看名字 */
const canDeleteGroup = computed(() => !!currentGroup.value && !currentGroup.value.isDefault);

/**
 * 分组菜单"转移分组"能不能点
 *
 * 两个前提缺一不可：组里有实例可转移，**而且除它自己以外还有别的组可去**。
 * 后者以前没判——只有一个默认分组时点开弹窗才发现下拉是空的，
 * 所以现在直接置灰，工具提示见 MainWindow 的工具栏按钮。
 */
const canMoveGroup = computed(() => {
  const source = currentGroup.value;
  if (!source || source.items.length === 0) return false;
  return props.groups.some((g) => g.id !== source.id);
});

/** 改分组（多选菜单与实例菜单共用）：表里只有默认分组一个组时无处可去 */
const canChangeGroup = computed(() => props.groups.length > 1);

/** 菜单动作，一律抛给父组件执行 */
const emit = defineEmits<{
  /** 分组菜单"全选"：选中该分组的全部实例 */
  (e: "select-all", groupId?: string): void;
  /** 分组菜单"启动全部"：启动该分组的全部实例 */
  (e: "launch-group", groupId?: string): void;
  /** 分组菜单"转移分组" */
  (e: "move-group"): void;
  /** 分组菜单"删除分组" */
  (e: "delete-group", groupId?: string): void;
  /** 实例菜单的动作 */
  (e: "inst-action", id: InstMenuAction): void;
  /** 多选菜单"修改分组" */
  (e: "multi-move"): void;
  /** 多选菜单"删除" */
  (e: "multi-delete"): void;
  /** 多选菜单"启动全部" */
  (e: "multi-launch"): void;
}>();

/** 菜单根元素与最终落点（先按点击处放，量完再修正） */
const el = ref<HTMLElement | null>(null);
const pos = ref({ x: 0, y: 0 });
/** 与窗口边缘至少留这么宽，单位 px */
const MARGIN = 8;

let positionSeq = 0;

watchEffect(async () => {
  // 菜单高度随视图（分组 / 实例 / 多选）变化，读一下 menu 就能在切换时重新定位
  const menu = props.menu;
  const seq = ++positionSeq;
  if (!menu) return;

  pos.value = { x: menu.x, y: menu.y };
  // 等菜单渲染出来才有尺寸可量；内容由 v-if 分支决定，量不到就保持点击处
  await nextTick();
  // 快速换菜单时旧的一次定位可能晚到：作废，别拿旧坐标盖新菜单
  if (seq !== positionSeq) return;
  const rect = el.value?.getBoundingClientRect();
  if (!rect) return;

  const overX = rect.right - (window.innerWidth - MARGIN);
  const overY = rect.bottom - (window.innerHeight - MARGIN);
  pos.value = {
    x: overX > 0 ? Math.max(MARGIN, menu.x - overX) : menu.x,
    y: overY > 0 ? Math.max(MARGIN, menu.y - overY) : menu.y,
  };
});
</script>

<template>
  <div v-if="menu" ref="el" class="ctx-menu" :style="{ left: pos.x + 'px', top: pos.y + 'px' }" @contextmenu.prevent
    @click.stop>
    <!-- 分组菜单：全选 / 启动全部 / 转移分组 / 删除分组
         空分组时"全选 / 启动全部"没有实例可操作，置灰而不是让用户点了没反应；
         "转移分组"还要多一条：除本组外得有别的组可去（见 canMoveGroup）；
         "删除分组"对默认分组永远置灰 -->
    <template v-if="menu.kind === 'group'">
      <button class="ctx-item" :disabled="groupEmpty" @click="emit('select-all', menu.groupId)">
        {{ t("multi.selectAll") }}
      </button>
      <button class="ctx-item" :disabled="groupEmpty" @click="emit('launch-group', menu.groupId)">
        {{ t("multi.launch") }}
      </button>
      <button class="ctx-item" :disabled="!canMoveGroup" @click="emit('move-group')">
        {{ t("group.moveTo") }}
      </button>
      <div class="ctx-sep"></div>
      <button class="ctx-item danger" :disabled="!canDeleteGroup" @click="emit('delete-group', menu.groupId)">
        {{ t("group.delete") }}
      </button>
    </template>

    <!-- 实例菜单：按功能归类 —— 启动 / 多选 / 管理 / 组织 / 分享 / 删除 -->
    <template v-else-if="menu.kind === 'instance'">
      <button class="ctx-item" @click="emit('inst-action', 'launch')">
        {{ t("launch.play") }}
      </button>
      <!-- 多选：以这一项为起点进入多选，之后勾别的实例、用顶部工具栏批量操作 -->
      <button class="ctx-item" @click="emit('inst-action', 'multiSelect')">
        {{ t("multi.enter") }}
      </button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" @click="emit('inst-action', 'viewMods')">
        {{ t("actions.viewMods") }}
      </button>
      <button class="ctx-item" @click="emit('inst-action', 'manageResource')">
        {{ t("actions.manageResource") }}
      </button>
      <button class="ctx-item" @click="emit('inst-action', 'viewLog')">
        {{ t("actions.viewLog") }}
      </button>
      <button class="ctx-item" @click="emit('inst-action', 'openFolder')">
        {{ t("actions.openFolder") }}
      </button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" @click="emit('inst-action', 'changeIcon')">
        {{ t("actions.changeIcon") }}
      </button>
      <button class="ctx-item" @click="emit('inst-action', 'rename')">
        {{ t("actions.rename") }}
      </button>
      <button class="ctx-item" :disabled="!canChangeGroup" @click="emit('inst-action', 'moveGroup')">
        {{ t("multi.moveGroup") }}
      </button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" @click="emit('inst-action', 'export')">
        {{ t("actions.export") }}
      </button>
      <button class="ctx-item" @click="emit('inst-action', 'serverInstance')">
        {{ t("actions.serverInstance") }}
      </button>
      <button class="ctx-item" @click="emit('inst-action', 'shortcut')">
        {{ t("actions.shortcut") }}
      </button>
      <button class="ctx-item" @click="emit('inst-action', 'copy')">
        {{ t("actions.copyInstance") }}
      </button>
      <!-- 云存档同步：功能还没做，点了只提示「制作中」 -->
      <button class="ctx-item" @click="emit('inst-action', 'cloudSave')">
        {{ t("actions.cloudSync") }}
      </button>
      <div class="ctx-sep"></div>
      <button class="ctx-item danger" @click="emit('inst-action', 'delete')">
        {{ t("actions.delete") }}
      </button>
    </template>

    <!-- 多选实例操作菜单 -->
    <template v-else>
      <button class="ctx-item" :disabled="!canChangeGroup" @click="emit('multi-move')">
        {{ t("multi.moveGroup") }}
      </button>
      <button class="ctx-item danger" @click="emit('multi-delete')">
        {{ t("multi.delete") }}
      </button>
      <button class="ctx-item" @click="emit('multi-launch')">
        {{ t("multi.launch") }}
      </button>
    </template>
  </div>
</template>

<style scoped>
.ctx-menu {
  position: fixed;
  z-index: 120;
  min-width: 180px;
  /* 屏幕很矮、收进窗口后仍放不下时才内部滚动；正常窗口下列表项全部可见 */
  max-height: calc(100vh - 16px);
  overflow-y: auto;
  padding: 6px;
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-lg);
  animation: ctx-in 0.12s ease;
}

@keyframes ctx-in {
  from {
    opacity: 0;
    transform: translateY(-4px) scale(0.98);
  }

  to {
    opacity: 1;
    transform: none;
  }
}

.ctx-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 9px 12px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.1s;
}

.ctx-item:hover {
  background: var(--bg-hover);
}

.ctx-item.danger {
  color: var(--red);
}

.ctx-item.danger:hover {
  background: color-mix(in srgb, var(--red) 12%, transparent);
}

.ctx-item:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  background: transparent;
}

.ctx-sep {
  height: 1px;
  background: var(--border);
  margin: 4px 6px;
}
</style>
