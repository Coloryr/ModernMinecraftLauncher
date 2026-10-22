<script setup lang="ts">
// 结构文件列表：打开目录、删除
//
// 名字有两层，别混：
// - **标题**：NBT 元数据里的 `Name`（作者自己起的名字），没有就退回文件名；
// - **副标题**：**文件名**，即磁盘上的那个，删除 / 打开目录都按它走；再加作者 · 尺寸 ·
//   方块数 · 方块种类 + 描述。
//
// 之所以两个都要：`Name` 与文件名经常不一致，作者改了名没改文件或者反过来，
// 而用户要找的是磁盘上那个文件 —— 只显示 `Name` 时根本对不上文件夹里看到的东西
// 与光影包列表同一口径，见 ShaderPane 的 sub。
//
// `author` / `description` **只有 litematic 会填**，它的 `Metadata` 里有这两个字段，
// `.schem` / `.nbt` 那两个 reader 不设，所以那两种格式这两段是空的、整段不渲染。
//
// 多选与右键菜单与其它分类同一套（见 useRowSelect / RowMenu）：勾选框加选、
// Shift + 点整段选中，右键出菜单，**行尾不排按钮**。
import { computed, ref } from "vue";
import { t } from "../../../lib/i18n";
import { deleteSchematic, getSchematicDetail } from "../../../lib/api";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
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
import type { SchematicDetailDto, SchematicItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，结构文件列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，确认弹窗与打开目录从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { schematics, instanceUuid, loading } = props.data;
const { busy, act, actLocal, askConfirm, askDelete, openFolder } = props.ops;

function remove(item: SchematicItemDto) {
  askDelete(titleOf(item), () => deleteSchematic(instanceUuid.value, item.file));
}

// ---------- 多选 ----------

/** 选中键用**磁盘文件名**：删除与打开目录都按它走，NBT 里的 `Name` 常与它不一致 */
const {
  selected,
  anchorKey,
  count,
  isSelected,
  toggle: toggleSelect,
  onRowClick,
  clear,
} = useRowSelect<SchematicItemDto>({
  keyOf: (item) => item.file,
  order: () => schematics.value,
});

/** 选中的结构文件 */
const selectedItems = computed(() => schematics.value.filter(isSelected));

/** 批量删除；删完选中项已不在列表里，直接清空选择 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) await deleteSchematic(instanceUuid.value, item.file);
      });
      clear();
    },
    // act 内部已经重拉过列表了，这里别再拉一遍
    { reload: false },
  );
}

// ---------- 详情 ----------
//
// 「查看详情」是**只读**的一次查询：后端只解这一个文件，列表不重拉（actLocal）。

/** 打开着的详情；`null` 表示没开 */
const detail = ref<SchematicDetailDto | null>(null);

/** 拉这一个结构文件的详情 */
function openDetail(item: SchematicItemDto) {
  void actLocal(async () => {
    detail.value = await getSchematicDetail(instanceUuid.value, item.file);
  });
}

// ---------- 右键菜单 ----------

/** 打开着的菜单；`item` 为 `null` 表示批量菜单 */
const rowMenu = ref<{
  x: number;
  y: number;
  title?: string;
  items: RowMenuItem[];
  item: SchematicItemDto | null;
} | null>(null);

/** 单行菜单项：结构文件的动作与这一行的内容无关，所以不看 `item` */
function itemMenuItems(): RowMenuItem[] {
  return [
    { id: "detail", label: t("resource.detail") },
    { id: "rename", label: t("resource.rename"), separated: true },
    { id: "open-folder", label: t("resource.openFolder") },
    { id: "remove", label: t("resource.delete"), danger: true, separated: true },
  ];
}

/** 打开右键菜单 */
function openMenu(payload: { event: MouseEvent; item: SchematicItemDto }) {
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

  rowMenu.value = { x, y, items: itemMenuItems(), item: payload.item };
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
    case "detail":
      openDetail(item);
      break;
    case "rename":
      props.ops.askRename("schematics", item.file);
      break;
    case "open-folder":
      openFolder("schematics", item.file);
      break;
    case "remove":
      remove(item);
      break;
  }
}

/** 标题：NBT 里的名字，没有就用文件名 */
function titleOf(item: SchematicItemDto): string {
  return item.name || item.file;
}

/**
 * 副标题前半段：文件名 · 作者 · 尺寸 · 方块数 · 方块种类
 *
 * 文件名放**最前**：`.item-sub` 是单行 + 省略号，放最后会被长作者名 / 长尺寸挤掉，
 * 那就等于没显示。标题已经就是文件名时（没 `Name`）不再重复一遍
 */
function metaLabel(item: SchematicItemDto): string {
  return [
    titleOf(item) === item.file ? "" : item.file,
    item.author,
    t("resource.dims", { w: item.width, h: item.height, l: item.length }),
    t("resource.blockCount", { count: item.blockCount }),
    t("resource.blockTypes", { count: item.blockTypes }),
  ]
    .filter(Boolean)
    .join(" · ");
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
  <VirtualList v-else class="item-list" :count="schematics.length" :row-gap="RESOURCE_ROW_GAP">
    <template #default="{ start, end }">
      <ResourceRow v-for="item in schematics.slice(start, end)" :key="item.file" :name="titleOf(item)" :class="{
        'row-selected': isSelected(item),
        'row-anchor': anchorKey === item.file,
        'row-selecting': selected.size > 0,
      }" @click="onRowClick({ event: $event, item })" @contextmenu.prevent="openMenu({ event: $event, item })">
        <template #select>
          <input type="checkbox" class="row-select" :checked="isSelected(item)" @pointerdown.stop
            @change.stop="toggleSelect(item)" @click.stop />
        </template>
        <template #badges>
          <span class="badge badge-dim">{{ item.typeName }}</span>
          <span v-if="item.fail" class="badge badge-red">{{ t("resource.modFail") }}</span>
        </template>
        <!--
          描述单独铺在后面、且走 FormattedText：litematic 的 `Metadata.Description` 是作者
          自己写的一句话，可能带 `§` 格式码，与材质包 / 数据包那两处同一套做法。
          只有 litematic 会填它，其余格式为空、整段不渲染
        -->
        <template #sub>
          <span>{{ metaLabel(item) }}</span>
          <template v-if="item.description"> · </template>
          <FormattedText v-if="item.description" :text="item.description" />
        </template>
      </ResourceRow>
      <div v-if="!schematics.length" class="empty-tip">{{ t("resource.empty") }}</div>
    </template>
  </VirtualList>

  <!-- 行右键菜单：单行与批量共用，菜单项见 itemMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title" @pick="onMenuPick"
    @close="rowMenu = null" />

  <!--
    结构详情：只读。上面几行是文件本身的信息，下面一栏是**逐种方块的数量**——
    那是列表行里放不下的部分（行里只有"多少个方块 · 多少种"两个总数）
  -->
  <BaseModal v-if="detail" :title="t('resource.detailTitle')" :closable="false" @close="detail = null">
    <p class="schem-name">
      <FormattedText :text="detail.name || detail.file" />
    </p>
    <p class="schem-file" :title="detail.file">{{ detail.file }}</p>

    <p v-if="detail.fail" class="error-text">{{ t("resource.modFail") }}</p>
    <template v-else>
      <p class="schem-meta">{{ t("resource.detailType", { name: detail.typeName }) }}</p>
      <p class="schem-meta">
        {{ t("resource.dims", { w: detail.width, h: detail.height, l: detail.length }) }}
      </p>
      <p class="schem-meta">
        {{ t("resource.blockCount", { count: detail.blockCount }) }} ·
        {{ t("resource.blockTypes", { count: detail.blockTypes }) }}
      </p>
      <p v-if="detail.author" class="schem-meta">
        {{ t("resource.detailAuthor", { name: detail.author }) }}
      </p>

      <template v-if="detail.description">
        <label class="field-label">{{ t("resource.detailDescription") }}</label>
        <p class="schem-desc">
          <FormattedText :text="detail.description" />
        </p>
      </template>

      <label class="field-label">{{ t("resource.detailBlocks") }}</label>
      <div v-if="detail.blocks.length" class="schem-blocks">
        <div v-for="block in detail.blocks" :key="block.id" class="schem-block">
          <span class="schem-block-id" :title="block.id">{{ block.id }}</span>
          <span class="schem-block-count">{{ block.count }}</span>
        </div>
      </div>
      <p v-else class="empty-tip">{{ t("resource.detailNoBlocks") }}</p>
    </template>

    <div class="modal-actions">
      <BaseButton @click="detail = null">{{ t("resource.close") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<!-- 弹窗样式走 scoped：BaseModal 是 Teleport 到 body 的，窗口级 resource.css 够不着它 -->
<style scoped>
/* 名称与文件名两行，与其它弹窗（存档编辑 / 模组备注）同一口径 */
.schem-name {
  margin: 0 0 2px;
  font-size: 13px;
  color: var(--text);
}

.schem-file {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.schem-meta {
  margin: 0 0 3px;
  font-size: 12.5px;
  color: var(--text-dim);
}

.schem-desc {
  margin: 0;
  font-size: 12.5px;
  color: var(--text);
  line-height: 1.5;
}

/* 逐种方块：整行两列，左 ID 右数量；方块种类可能上百，给个上限自己滚 */
.schem-blocks {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 260px;
  overflow-y: auto;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行的右边缘仍落在弹窗内容区里，滚动条画在这 8px 上
     （与项目列表 .res-list 同一做法，见 styles/scrollbar.css） */
  margin-right: -8px;
  padding-right: 8px;
}

.schem-block {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 12px;
}

.schem-block:hover {
  background: var(--bg-hover);
}

.schem-block-id {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

.schem-block-count {
  flex-shrink: 0;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}
</style>
