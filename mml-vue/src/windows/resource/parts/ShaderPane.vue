<script setup lang="ts">
// 光影包列表：设为当前光影 / 取消、打开目录、删除
//
// 多选与右键菜单与其它分类同一套（见 useRowSelect / RowMenu）：勾选框加选、
// Shift + 点整段选中，右键出菜单，**行尾不排按钮**。
import { computed, ref } from "vue";
import { t } from "../../../lib/i18n";
import { deleteShaderpack, setShader } from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import VirtualList from "./VirtualList.vue";
import { RESOURCE_ROW_GAP } from "../../../composables/useVirtualRows";
import { useRowSelect } from "../composables/useRowSelect";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { ShaderItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，光影列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化与确认弹窗从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { shaders, instanceUuid, loading } = props.data;
const { busy, act, askConfirm, askDelete, openFolder } = props.ops;

/** 设为当前光影；点已选中的那个就是取消 */
function toggle(item: ShaderItemDto) {
  act(async () => {
    // 点已选中的那个 = 取消选择（后端传 null）
    await setShader(instanceUuid.value, item.selected ? null : item.file);
  });
}

function remove(item: ShaderItemDto) {
  askDelete(item.name || item.file, () => deleteShaderpack(instanceUuid.value, item.file));
}

// ---------- 多选 ----------

/** 选中键用**磁盘文件名**：设为当前与删除都按它定位，显示名可能为空或重复 */
const {
  selected,
  anchorKey,
  count,
  isSelected,
  toggle: toggleSelect,
  onRowClick,
  clear,
} = useRowSelect<ShaderItemDto>({
  keyOf: (item) => item.file,
  order: () => shaders.value,
});

/** 选中的光影包 */
const selectedItems = computed(() => shaders.value.filter(isSelected));

/**
 * 批量删除
 *
 * 批量菜单与批量条里**没有「设为当前」**：当前光影同时只能有一个，
 * 对一批下手不成立，这条只留在单行菜单里。
 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) await deleteShaderpack(instanceUuid.value, item.file);
      });
      clear();
    },
    // act 内部已经重拉过列表了，这里别再拉一遍
    { reload: false },
  );
}

// ---------- 右键菜单 ----------

/** 打开着的菜单；`item` 为 `null` 表示批量菜单 */
const rowMenu = ref<{
  x: number;
  y: number;
  title?: string;
  items: RowMenuItem[];
  item: ShaderItemDto | null;
} | null>(null);

/** 单行菜单项 */
function itemMenuItems(item: ShaderItemDto): RowMenuItem[] {
  return [
    {
      id: "toggle",
      label: item.selected ? t("resource.disable") : t("resource.enable"),
    },
    { id: "rename", label: t("resource.rename"), separated: true },
    { id: "open-folder", label: t("resource.openFolder"), separated: true },
    { id: "remove", label: t("resource.delete"), danger: true, separated: true },
  ];
}

/** 打开右键菜单 */
function openMenu(payload: { event: MouseEvent; item: ShaderItemDto }) {
  const x = payload.event.clientX;
  const y = payload.event.clientY;

  // 有选中项时，右键 = **批量动作**
  const picked = selectedItems.value;
  if (picked.length) {
    rowMenu.value = {
      x,
      y,
      title: t("resource.selectedCount", { n: picked.length }),
      items: [{ id: "remove", label: t("resource.delete"), danger: true }],
      item: null,
    };
    return;
  }

  rowMenu.value = { x, y, items: itemMenuItems(payload.item), item: payload.item };
}

/** 菜单动作分发 */
function onMenuPick(id: string) {
  const menu = rowMenu.value;
  if (!menu) return;

  if (!menu.item) {
    if (id === "remove") batchRemove();
    return;
  }

  const item = menu.item;
  switch (id) {
    case "toggle":
      toggle(item);
      break;
    case "rename":
      props.ops.askRename("shaderpacks", item.file);
      break;
    case "open-folder":
      openFolder("shaderpacks", item.file);
      break;
    case "remove":
      remove(item);
      break;
  }
}

/** 副标题：注释 · 文件名 */
function sub(item: ShaderItemDto): string {
  return [item.comment, item.file].filter(Boolean).join(" · ");
}
</script>

<template>
  <ContentHead :data="data">
    <!-- 有选中项时顶栏换成批量操作条，与其它分类同一口径 -->
    <template #actions>
      <template v-if="count">
        <span class="row-selected-count">{{ t("resource.selectedCount", { n: count }) }}</span>
        <button class="mini-btn danger" :disabled="busy" @click="batchRemove">
          {{ t("resource.delete") }}
        </button>
        <button class="mini-btn" :disabled="busy" @click="clear">
          {{ t("resource.clearSelection") }}
        </button>
      </template>
    </template>
  </ContentHead>

  <div v-if="loading" class="item-list">
    <ListSkeleton />
  </div>
  <VirtualList v-else class="item-list" :count="shaders.length" :row-gap="RESOURCE_ROW_GAP">
    <template #default="{ start, end }">
      <ResourceRow v-for="item in shaders.slice(start, end)" :key="item.file" :name="item.name || item.file" :class="{
        'row-selected': isSelected(item),
        'row-anchor': anchorKey === item.file,
        'row-selecting': selected.size > 0,
      }" @click="onRowClick({ event: $event, item })" @contextmenu.prevent="openMenu({ event: $event, item })">
        <template #select>
          <input type="checkbox" class="row-select" :checked="isSelected(item)" @pointerdown.stop
            @change.stop="toggleSelect(item)" @click.stop />
        </template>
        <template #badges>
          <span v-if="item.selected" class="badge">{{ t("resource.shaderOn") }}</span>
        </template>
        <!-- 光影包注释与文件名都可能带 `§` 格式码，统一过显示期解析 -->
        <template #name>
          <FormattedText :text="item.name || item.file" />
        </template>
        <template #sub>
          <FormattedText :text="sub(item)" />
        </template>
      </ResourceRow>
      <div v-if="!shaders.length" class="empty-tip">{{ t("resource.empty") }}</div>
    </template>
  </VirtualList>

  <!-- 行右键菜单：单行与批量共用，菜单项见 itemMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title" @pick="onMenuPick"
    @close="rowMenu = null" />
</template>
