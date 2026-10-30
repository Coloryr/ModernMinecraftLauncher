<script setup lang="ts">
// 材质包列表：启用 / 禁用（写 options.txt）、打开目录、删除
//
// 多选与右键菜单与其它分类同一套（见 useRowSelect / RowMenu）：勾选框加选、
// Shift + 点整段选中，右键出菜单，**行尾不排按钮**。
import { computed, ref } from "vue";
import { t } from "../../../lib/i18n";
import {
  deleteResourcepack,
  disableResourcepack,
  enableResourcepack,
} from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import VirtualList from "./VirtualList.vue";
import { RESOURCE_ROW_GAP } from "../../../composables/useVirtualRows";
import { useRowSelect } from "../composables/useRowSelect";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { PackItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，材质包列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化与确认弹窗从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { packs, instanceUuid, loading } = props.data;
const { busy, act, actLocal, askConfirm, askDelete, openFolder } = props.ops;

function remove(item: PackItemDto) {
  askDelete(item.file, () => deleteResourcepack(instanceUuid.value, item.file));
}

/**
 * 启用 / 禁用
 *
 * **不重拉列表**：启用状态只写在 options.txt 的 `resourcePacks` 里，包本身一个字节都没动。
 * 重拉一次要把每个包重新算 SHA1 / SHA256 并解一遍 `pack.mcmeta`，模组那边是重扫 jar，
 * 同一条理由。所以就地改这一行的 `enable`，走 `actLocal`：串行化 + 失败提示，不刷新。
 */
async function toggle(item: PackItemDto) {
  const next = !item.enable;
  await actLocal(async () => {
    if (next) {
      await enableResourcepack(instanceUuid.value, item.file);
    } else {
      await disableResourcepack(instanceUuid.value, item.file);
    }
    item.enable = next;
  });
}

// ---------- 多选 ----------

/** 选中键用**磁盘文件名**：启用 / 删除都按它定位 */
const {
  selected,
  anchorKey,
  count,
  isSelected,
  toggle: toggleSelect,
  onRowClick,
  clear,
} = useRowSelect<PackItemDto>({
  keyOf: (item) => item.file,
  order: () => packs.value,
});

/** 选中的材质包 */
const selectedItems = computed(() => packs.value.filter(isSelected));

/** 选中项里还有关着的 → 才给批量启用 */
const canEnable = computed(() => selectedItems.value.some((item) => !item.enable));
/** 选中项里还有开着的 → 才给批量禁用 */
const canDisable = computed(() => selectedItems.value.some((item) => item.enable));

/**
 * 批量启用 / 禁用
 *
 * 与单行同一套：走 `actLocal` **不重拉列表**，成功一条就地改一条的 `enable`；
 * 只对当前状态与目标相反的那些下手，已经是目标状态的跳过。
 *
 * - `enable`: 目标状态，`true` 启用、`false` 禁用
 */
async function batchToggle(enable: boolean) {
  const picked = selectedItems.value.filter((item) => item.enable !== enable);
  if (!picked.length) return;
  await actLocal(async () => {
    for (const item of picked) {
      if (enable) {
        await enableResourcepack(instanceUuid.value, item.file);
      } else {
        await disableResourcepack(instanceUuid.value, item.file);
      }
      item.enable = enable;
    }
  });
}

/** 批量删除；删完选中项已不在列表里，直接清空选择 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) await deleteResourcepack(instanceUuid.value, item.file);
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
  item: PackItemDto | null;
} | null>(null);

/** 单行菜单项 */
function itemMenuItems(item: PackItemDto): RowMenuItem[] {
  return [
    {
      id: "toggle",
      label: item.enable ? t("resource.disable") : t("resource.enable"),
    },
    { id: "rename", label: t("resource.rename"), separated: true },
    { id: "open-folder", label: t("resource.openFolder"), separated: true },
    { id: "remove", label: t("resource.delete"), danger: true, separated: true },
  ];
}

/** 批量菜单项：只放对这一批每一项都成立的动作 */
function batchMenuItems(): RowMenuItem[] {
  const items: RowMenuItem[] = [];
  if (canEnable.value) items.push({ id: "enable", label: t("resource.enable") });
  if (canDisable.value) items.push({ id: "disable", label: t("resource.disable") });
  items.push({ id: "remove", label: t("resource.delete"), danger: true, separated: true });
  return items;
}

/** 打开右键菜单 */
function openMenu(payload: { event: MouseEvent; item: PackItemDto }) {
  const x = payload.event.clientX;
  const y = payload.event.clientY;

  // 有选中项时，右键 = **批量动作**
  const picked = selectedItems.value;
  if (picked.length) {
    rowMenu.value = {
      x,
      y,
      title: t("resource.selectedCount", { n: picked.length }),
      items: batchMenuItems(),
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
    if (id === "enable") void batchToggle(true);
    else if (id === "disable") void batchToggle(false);
    else if (id === "remove") batchRemove();
    return;
  }

  const item = menu.item;
  switch (id) {
    case "toggle":
      void toggle(item);
      break;
    case "rename":
      props.ops.askRename("resourcepacks", item.file);
      break;
    case "open-folder":
      openFolder("resourcepacks", item.file);
      break;
    case "remove":
      remove(item);
      break;
  }
}

/** 包格式那段文案；描述要过格式码，这一句不要，所以两段分开渲染 */
function formatLabel(item: PackItemDto): string {
  return t("resource.packFormat", { format: item.packFormat });
}
</script>

<template>
  <ContentHead :data="data">
    <!-- 有选中项时顶栏换成批量操作条，与其它分类同一口径 -->
    <template #actions>
      <template v-if="count">
        <span class="row-selected-count">{{ t("resource.selectedCount", { n: count }) }}</span>
        <button v-if="canEnable" class="mini-btn" :disabled="busy" @click="batchToggle(true)">
          {{ t("resource.enable") }}
        </button>
        <button v-if="canDisable" class="mini-btn" :disabled="busy" @click="batchToggle(false)">
          {{ t("resource.disable") }}
        </button>
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
  <VirtualList v-else class="item-list" :count="packs.length" :row-gap="RESOURCE_ROW_GAP">
    <template #default="{ start, end }">
      <ResourceRow v-for="item in packs.slice(start, end)" :key="item.file" :icon="item.icon" :name="item.file" :class="{
        'row-selected': isSelected(item),
        'row-anchor': anchorKey === item.file,
        'row-selecting': selected.size > 0,
      }" @click="onRowClick({ event: $event, item })" @contextmenu.prevent="openMenu({ event: $event, item })">
        <template #select>
          <input type="checkbox" class="row-select" :checked="isSelected(item)" @pointerdown.stop
            @change.stop="toggleSelect(item)" @click.stop />
        </template>
        <template #badges>
          <span v-if="item.fail" class="badge badge-red">{{ t("resource.modFail") }}</span>
          <span v-if="item.enable" class="badge">{{ t("resource.packOn") }}</span>
        </template>
        <template #name>
          <FormattedText :text="item.file" />
        </template>
        <!--
          描述要过格式码，整合包作者常用 `§a` 这类上色；包格式那句不要 ——
          所以两段分开渲染、中间自己接分隔符，不能先拼成一个字符串再整体格式化
        -->
        <template #sub>
          <FormattedText v-if="item.description" :text="item.description" />
          <template v-if="item.description"> · </template>{{ formatLabel(item) }}
        </template>
      </ResourceRow>
      <div v-if="!packs.length" class="empty-tip">{{ t("resource.empty") }}</div>
    </template>
  </VirtualList>

  <!-- 行右键菜单：单行与批量共用，菜单项见 itemMenuItems / batchMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title" @pick="onMenuPick"
    @close="rowMenu = null" />
</template>
