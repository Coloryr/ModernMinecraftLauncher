<script setup lang="ts">
// 数据包子页：先选存档（下拉），再列该存档里的数据包，可启用 / 禁用 / 删除
//
// 多选与右键菜单与其它分类同一套（见 useRowSelect / RowMenu）：勾选框加选、
// Shift + 点整段选中，右键出菜单，**行尾不排按钮**。换存档时清空选中项 ——
// 选中的是"这个存档里的数据包"，换了存档原选中项已经不在这份列表里了。
import { computed, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import { stripFormatting } from "../../../lib/formatting";
import { deleteDatapack, toggleDatapack } from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import SaveTabs from "./SaveTabs.vue";
import VirtualList from "./VirtualList.vue";
import { RESOURCE_ROW_GAP } from "../../../composables/useVirtualRows";
import { useRowSelect } from "../composables/useRowSelect";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { DataPackItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，存档下拉、数据包列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化与确认弹窗从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { saves, datapacks, dpSave, instanceUuid, loading } = props.data;
const { busy, act, askConfirm, askDelete, openFolder } = props.ops;

/** 启用 / 禁用单条：后端只有 toggle，没有独立的 enable / disable */
function toggle(item: DataPackItemDto) {
  act(async () => {
    await toggleDatapack(instanceUuid.value, dpSave.value, item.name);
  });
}

function remove(item: DataPackItemDto) {
  askDelete(item.file, () => deleteDatapack(instanceUuid.value, dpSave.value, item.name));
}

// ---------- 多选 ----------

/** 选中键用**登记名**：启用与删除都按它定位，磁盘文件名可能和它不一样 */
const {
  selected,
  anchorKey,
  count,
  isSelected,
  toggle: toggleSelect,
  onRowClick,
  clear,
} = useRowSelect<DataPackItemDto>({
  keyOf: (item) => item.name,
  order: () => datapacks.value,
});

/** 选中的数据包 */
const selectedItems = computed(() => datapacks.value.filter(isSelected));

// 换存档后清空选择
watch(dpSave, () => clear());

/** 选中项里还有"禁用中"的 → 才给批量启用 */
const canEnable = computed(() => selectedItems.value.some((item) => item.enable === false));
/** 选中项里还有"启用中或没登记"的 → 才给批量禁用 */
const canDisable = computed(() => selectedItems.value.some((item) => item.enable !== false));

/**
 * 批量启用 / 禁用
 *
 * 后端只有 toggle，所以只对**当前状态与目标相反**的那些下手，判据与单行一致：
 * `enable === false` 才算禁用中，`true` 与 `null`（没登记进 level.dat）都按启用中算。
 *
 * - `enable`: 目标状态，`true` 启用、`false` 禁用
 */
async function batchToggle(enable: boolean) {
  const picked = selectedItems.value.filter((item) => (item.enable === false) === enable);
  if (!picked.length) return;
  await act(async () => {
    for (const item of picked) await toggleDatapack(instanceUuid.value, dpSave.value, item.name);
  });
}

/** 批量删除：清 level.dat 引用 + 进回收站；删完选中项已不在列表里，直接清空选择 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) await deleteDatapack(instanceUuid.value, dpSave.value, item.name);
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
  item: DataPackItemDto | null;
} | null>(null);

/** 单行菜单项 */
function itemMenuItems(item: DataPackItemDto): RowMenuItem[] {
  return [
    // 与原来的行内按钮同一口径：只有明确"禁用中"的才给「启用」，其余给「禁用」
    { id: "toggle", label: item.enable === false ? t("resource.enable") : t("resource.disable") },
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
function openMenu(payload: { event: MouseEvent; item: DataPackItemDto }) {
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
      toggle(item);
      break;
    case "rename":
      props.ops.askRename("datapacks", item.name, dpSave.value);
      break;
    case "open-folder":
      // 数据包目录是**整个存档**的数据包目录，与具体哪一条无关
      openFolder("datapacks", null, dpSave.value);
      break;
    case "remove":
      remove(item);
      break;
  }
}

/**
 * 启用状态的显示名：**只有两态**
 *
 * `null`（level.dat 的两个列表里都没有）按**开着**算：存档下次载入时 MC 会把它当启用，
 * 界面上因此不出现"未登记"这一态，只分已启用 / 未启用
 */
function state(item: DataPackItemDto): string {
  return item.enable === false ? t("resource.dpOff") : t("resource.dpOn");
}

/** 显示名：去掉 NBT 登记名里的 `file/` 前缀 */
function name(item: DataPackItemDto): string {
  return item.name.startsWith("file/") ? item.name.slice(5) : item.file || item.name;
}

/** 包格式那段文案；描述要过格式码，这一句不要，所以两段分开渲染 */
function formatLabel(item: DataPackItemDto): string {
  return t("resource.packFormat", { format: item.packFormat });
}
</script>

<template>
  <ContentHead :data="data">
    <SaveTabs :data="data" />
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

  <VirtualList class="item-list" :count="dpSave ? datapacks.length : 0" :row-gap="RESOURCE_ROW_GAP">
    <!-- 存档下拉：选完之后选中项只显示存档名，前面补一个常驻的标签说明这是什么。
         标签与下拉同一行，下拉吃满剩余宽度；固定在列表上方，不参与行虚拟化 -->
    <template #header>
      <div class="dp-save-row">
        <span class="dp-label">{{ t("resource.saveLabel") }}</span>
        <select v-model="dpSave" class="field-select dp-select">
          <option value="" disabled>{{ t("resource.selectSave") }}</option>
          <option v-for="s in saves" :key="s.dir" :value="s.dir">
            {{ stripFormatting(s.levelName || s.dir) }}
          </option>
        </select>
      </div>
      <div v-if="!dpSave" class="empty-tip">{{ t("resource.selectSave") }}</div>
    </template>

    <template #default="{ start, end }">
      <!-- 加载中用骨架行占位：与其它分类同一套；这里只换行，存档下拉要一直留着 -->
      <ListSkeleton v-if="loading" />
      <ResourceRow v-for="item in datapacks.slice(start, end)" :key="item.name" :name="name(item)" :class="{
        'row-selected': isSelected(item),
        'row-anchor': anchorKey === item.name,
        'row-selecting': selected.size > 0,
      }" @click="onRowClick({ event: $event, item })" @contextmenu.prevent="openMenu({ event: $event, item })">
        <template #select>
          <input type="checkbox" class="row-select" :checked="isSelected(item)" @pointerdown.stop
            @change.stop="toggleSelect(item)" @click.stop />
        </template>
        <template #badges>
          <span class="badge" :class="item.enable === false ? 'badge-off' : 'badge-on'">
            {{ state(item) }}
          </span>
        </template>
        <template #sub>
          <!-- 描述过格式码，数据包作者会用 `§a` 上色；包格式那句不过 -->
          <FormattedText v-if="item.description" :text="item.description" />
          <template v-if="item.description"> · </template>{{ formatLabel(item) }}
        </template>
      </ResourceRow>
      <div v-if="!datapacks.length && !loading" class="empty-tip">{{ t("resource.empty") }}</div>
    </template>
  </VirtualList>

  <!-- 行右键菜单：单行与批量共用，菜单项见 itemMenuItems / batchMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title" @pick="onMenuPick"
    @close="rowMenu = null" />
</template>
