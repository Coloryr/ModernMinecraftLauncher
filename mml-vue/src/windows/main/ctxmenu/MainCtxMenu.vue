<script setup lang="ts">
// 主窗口右键菜单：分组 / 实例 / 多选 三种视图
//
// 外观与交互直接复用资源管理的 RowMenu（紧凑样式、贴边内收、点外面 / Esc 收起），
// 这里只把三种视图翻译成菜单项，再把动作抛给主窗口执行。
//
// 目标分组的选择**不在这里**：那是个弹窗，见 MainWindow 的"选择目标分组"。
import { computed } from "vue";
import { t } from "../../../lib/i18n";
import { isSourceBuild } from "../../../lib/buildInfo";
import RowMenu, { type RowMenuItem } from "../../../components/ui/RowMenu.vue";
import type { CtxMenuState, GroupView, InstMenuAction } from "../types";

const props = defineProps<{
  /** 当前菜单状态；null 表示菜单没开 */
  menu: CtxMenuState | null;
  /** 全部分组视图，用来算置灰与"转移分组"的候选 */
  groups: GroupView[];
}>();

/** 当前右键的分组；空分组时"全选 / 启动全部"置灰 */
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
 * 组里有实例可转移，而且除它自己以外还有别的组可去
 */
const canMoveGroup = computed(() => {
  const source = currentGroup.value;
  if (!source || source.items.length === 0) return false;
  return props.groups.some((g) => g.id !== source.id);
});

/** 改分组（多选菜单与实例菜单共用）：表里只有默认分组一个组时无处可去 */
const canChangeGroup = computed(() => props.groups.length > 1);

/** 菜单项：三种视图各一份，动作 id 见 onPick */
const items = computed<RowMenuItem[]>(() => {
  const menu = props.menu;
  if (!menu) return [];
  if (menu.kind === "group") {
    return [
      { id: "select-all", label: t("multi.selectAll"), disabled: groupEmpty.value },
      { id: "launch-group", label: t("multi.launch"), disabled: groupEmpty.value },
      { id: "move-group", label: t("group.moveTo"), disabled: !canMoveGroup.value },
      { id: "delete-group", label: t("group.delete"), danger: true, disabled: !canDeleteGroup.value, separated: true },
    ];
  }
  if (menu.kind === "instance") {
    const list: RowMenuItem[] = [
      { id: "launch", label: t("launch.play") },
      { id: "multiSelect", label: t("multi.enter") },
      { id: "viewMods", label: t("actions.viewMods"), separated: true },
      { id: "manageResource", label: t("actions.manageResource") },
      { id: "viewLog", label: t("actions.viewLog") },
      { id: "openFolder", label: t("actions.openFolder") },
      { id: "changeIcon", label: t("actions.changeIcon"), separated: true },
      { id: "rename", label: t("actions.rename") },
      { id: "moveGroup", label: t("multi.moveGroup"), disabled: !canChangeGroup.value },
      { id: "export", label: t("actions.export"), separated: true },
      {
        id: "serverInstance",
        label: t("actions.serverInstance"),
        disabled: props.menu?.instance?.modpackType === "serverpack",
      },
      { id: "shortcut", label: t("actions.shortcut") },
      { id: "copy", label: t("actions.copyInstance") },
    ];
    // 云存档同步：功能还没做，只在源码启动（测试版）时露出来，点了提示「制作中」
    if (isSourceBuild) list.push({ id: "cloudSave", label: t("actions.cloudSync") });
    list.push({ id: "delete", label: t("actions.delete"), danger: true, separated: true });
    return list;
  }
  return [
    { id: "multi-move", label: t("multi.moveGroup"), disabled: !canChangeGroup.value },
    { id: "multi-delete", label: t("multi.delete"), danger: true },
    { id: "multi-launch", label: t("multi.launch") },
  ];
});

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
  /** 菜单要收起（点了外面或按了 Esc） */
  (e: "close"): void;
}>();

/**
 * 选中一项：按 id 分发到对应的 emit
 *
 * - `id`: 菜单项 id，见 items
 */
function onPick(id: string) {
  const menu = props.menu;
  switch (id) {
    case "select-all":
      emit("select-all", menu?.groupId);
      break;
    case "launch-group":
      emit("launch-group", menu?.groupId);
      break;
    case "move-group":
      emit("move-group");
      break;
    case "delete-group":
      emit("delete-group", menu?.groupId);
      break;
    case "multi-move":
      emit("multi-move");
      break;
    case "multi-delete":
      emit("multi-delete");
      break;
    case "multi-launch":
      emit("multi-launch");
      break;
    default:
      emit("inst-action", id as InstMenuAction);
  }
}
</script>

<template>
  <RowMenu v-if="menu" :x="menu.x" :y="menu.y" :items="items" @pick="onPick" @close="emit('close')" />
</template>
