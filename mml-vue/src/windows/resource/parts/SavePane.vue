<script setup lang="ts">
// 存档列表：备份、还原、打开目录、删除；数据包是同一分类下的子页，见 DatapackPane
//
// 支持多选（与模组列表同一套口径）：行左端勾选框加选、Shift + 点整段选中，
// 有选中项时顶栏换成批量操作条，右键则换成批量菜单 —— 两处**都只有「删除」**（用户口径）。
// **行尾不排按钮**，单行动作全在右键菜单里，单行与批量共用一个菜单组件，见 RowMenu。
// 批量备份与「还原」都不进批量：备份要一个一个来、还原得逐个挑备份，见 openRestore。
import { computed, ref } from "vue";
import { t } from "../../../lib/i18n";
import { stripFormatting } from "../../../lib/formatting";
import { showToast } from "../../../lib/toast";
import {
  backupSave,
  deleteSave,
  listSaveBackups,
  restoreSaveBackup,
} from "../../../lib/api";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import ContentHead from "./ContentHead.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import SaveTabs from "./SaveTabs.vue";
import { useRowSelect } from "../composables/useRowSelect";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { SaveBackupDto, SaveItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，存档列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化、确认弹窗与打开目录从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { saves, instanceUuid, loading } = props.data;
const { busy, act, actLocal, askConfirm, askDelete, openFolder } = props.ops;

function remove(item: SaveItemDto) {
  // 确认框里用**去掉格式码**的名字：那里是纯文本，`§a` 只会显示成乱码
  askDelete(stripFormatting(item.levelName || item.dir), () =>
    deleteSave(instanceUuid.value, item.dir),
  );
}

/** 备份存档：后端返回备份出来的文件名 */
function backup(item: SaveItemDto) {
  act(async () => {
    const name = await backupSave(instanceUuid.value, item.dir);
    showToast(t("resource.backupOk", { name }));
  });
}

// ---------- 多选（顶栏批量操作） ----------
//
// 与模组列表同一套口径，选中逻辑见 useRowSelect。存档只有列表视图，没有分组也没有排序，
// 所以整段选择的顺序就是后端给的顺序。

/** 选中键用**目录名**：它就是后端认的存档身份，备份 / 删除前后都不变 */
const { selected, anchorKey, count, isSelected, toggle, onRowClick, clear } = useRowSelect<SaveItemDto>({
  keyOf: (item) => item.dir,
  order: () => saves.value,
});

/** 选中的那些存档 */
const selectedItems = computed(() => saves.value.filter(isSelected));

/** 批量删除（进回收站）：删完选中项已经不在列表里了，直接清空选择 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteSaveConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) await deleteSave(instanceUuid.value, item.dir);
      });
      clear();
    },
    // act 内部已经重拉过列表了，这里别再拉一遍
    { reload: false },
  );
}

/**
 * 要还原的那个存档 + 它的备份列表，`null` 表示弹窗没开
 *
 * 列表在**点开弹窗时**才去后端取：备份数已经随存档列表给过来了，即 `item.backups`，
 * 所以"有没有备份"不用额外请求；具体文件列表只在真要还原时才需要。
 */
const restorePick = ref<{ item: SaveItemDto; list: SaveBackupDto[] } | null>(null);

// ---------- 右键菜单 ----------
//
// 单行与批量共用 RowMenu：没有选中项时是单行菜单，动作只作用在右键那一行；
// 有选中项时是批量菜单，**只有「删除」**（用户口径）。

/** 打开着的菜单；`item` 为 `null` 表示批量菜单 */
const rowMenu = ref<{
  x: number;
  y: number;
  title?: string;
  items: RowMenuItem[];
  item: SaveItemDto | null;
} | null>(null);

/** 单行菜单项 */
function itemMenuItems(item: SaveItemDto): RowMenuItem[] {
  const items: RowMenuItem[] = [{ id: "backup", label: t("resource.backup") }];
  // 有备份才给还原入口：没有备份的存档点还原只会弹一句"还没有备份"
  if (item.backups) items.push({ id: "restore", label: t("resource.restore") });
  items.push({ id: "open-folder", label: t("resource.openFolder"), separated: true });
  items.push({ id: "remove", label: t("resource.delete"), danger: true, separated: true });
  return items;
}

/** 打开右键菜单 */
function openMenu(payload: { event: MouseEvent; item: SaveItemDto }) {
  const x = payload.event.clientX;
  const y = payload.event.clientY;

  // 有选中项时，右键 = **批量动作**：只给「删除」，「备份」留在顶栏那条批量条上
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
    case "backup":
      backup(item);
      break;
    case "restore":
      openRestore(item);
      break;
    case "open-folder":
      openFolder("saves", item.dir);
      break;
    case "remove":
      remove(item);
      break;
  }
}

function openRestore(item: SaveItemDto) {
  actLocal(async () => {
    const list = await listSaveBackups(instanceUuid.value, item.dir);
    if (!list.length) {
      showToast(t("resource.backupNone"));
      return;
    }
    restorePick.value = { item, list };
  });
}

/** 选中某个备份：先确认（**破坏性**），再还原并重拉列表 */
function pickRestore(backupFile: string) {
  const pick = restorePick.value;
  if (!pick) return;
  restorePick.value = null;
  const name = stripFormatting(pick.item.levelName || pick.item.dir);
  askConfirm(
    t("resource.restore"),
    t("resource.restoreConfirm", { name }),
    async () => {
      await act(async () => {
        await restoreSaveBackup(instanceUuid.value, pick.item.dir, backupFile);
      });
    },
    // act 里已经重拉过列表了
    { reload: false },
  );
}

/** 上次游玩时间；入参是 Unix 毫秒，未知时显示 -- */
function formatTime(ms: number): string {
  if (!ms) return "--";
  return new Date(ms).toLocaleString();
}

/** 备份体积，输入字节，输出人类可读文本 */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <ContentHead :data="data">
    <SaveTabs :data="data" />
    <!-- 有选中项时顶栏换成批量操作条，与模组列表同一口径。
         这一条与右键的批量菜单**动作一致**，都只有「删除」：批量备份用户不要 -->
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
  <div v-else class="item-list">
    <ResourceRow v-for="item in saves" :key="item.dir" :icon="item.icon" :name="item.levelName || item.dir" :class="{
      'row-selected': isSelected(item),
      'row-anchor': anchorKey === item.dir,
      'row-selecting': selected.size > 0,
    }" @click="onRowClick({ event: $event, item })" @contextmenu.prevent="openMenu({ event: $event, item })">
      <!-- 多选勾选框，与模组列表同一套，见 useRowSelect -->
      <template #select>
        <input type="checkbox" class="row-select" :checked="isSelected(item)" @pointerdown.stop
          @change.stop="toggle(item)" @click.stop />
      </template>
      <!-- 存档名可以带 `§` 格式码（地图作者常用来上色），过一遍显示期解析 -->
      <template #name>
        <FormattedText :text="item.levelName || item.dir" />
      </template>
      <template #badges>
        <span v-if="item.broken" class="badge badge-red">{{ t("resource.broken") }}</span>
        <span v-if="item.hardCore" class="badge badge-red">Hardcore</span>
        <!-- 有备份才给还原入口：没有备份的存档点还原只会弹一句"还没有备份" -->
        <span v-if="item.backups" class="badge">
          {{ t("resource.backupCount", { n: item.backups }) }}
        </span>
      </template>
      <template #sub>
        {{ item.dir }} · {{ t("resource.lastPlayed", { time: formatTime(item.lastPlayed) }) }}
      </template>
    </ResourceRow>
    <div v-if="!saves.length" class="empty-tip">{{ t("resource.empty") }}</div>
  </div>

  <!-- 行右键菜单：单行与批量共用，菜单项见 itemMenuItems / batchMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title"
    @pick="onMenuPick" @close="rowMenu = null" />

  <!-- 选一个备份来还原（破坏性，选中后还会再确认一次） -->
  <BaseModal v-if="restorePick" :title="t('resource.restorePickTitle')" :closable="false" @close="restorePick = null">
    <div class="backup-pick-list">
      <button v-for="b in restorePick.list" :key="b.file" class="backup-pick-row" :disabled="busy"
        @click="pickRestore(b.file)">
        <span class="backup-pick-name" :title="b.file">{{ b.file }}</span>
        <span class="backup-pick-meta">{{ formatTime(b.time) }} · {{ formatSize(b.size) }}</span>
      </button>
    </div>
    <div class="modal-actions">
      <BaseButton @click="restorePick = null">{{ t("resource.cancel") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<!-- 备份弹窗的样式走 scoped：BaseModal 是 Teleport 到 body 的，
     窗口级 resource.css 那套 .resource-layout 前缀够不着它（备注弹窗同理） -->
<style scoped>
.backup-pick-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  /* 备份可能很多：给个上限自己滚，别把弹窗撑出屏幕 */
  max-height: 320px;
  overflow-y: auto;
  /* 这里**不预留**滚动条槽位：预留的话行会整体缩进 9px，左端就与标题、
     右端就与下面的按钮错开。槽位由 BaseModal 的 `.modal` 统一在两侧留，
     所以行正好落在弹窗的内容区里，与标题 / 按钮对齐 */
  /* 见 styles/scrollbar.css */
}

/* 一行一个备份：整行可点，点下去弹确认框。
   底色取 `--bg-hover` 而不是 `--bg-raised` —— 亮色主题下后者与弹窗底 `--bg-card`
   都是纯白，行就只剩一圈浅边框，"看不出是个能点的项目" */
.backup-pick-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 11px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-hover);
  color: var(--text);
  font-size: 12.5px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: all 0.12s;
}

.backup-pick-row:hover:not(:disabled) {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
}

.backup-pick-row:disabled {
  opacity: 0.5;
  cursor: default;
}

/* 文件名可被挤压，时间 / 体积不缩 */
.backup-pick-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.backup-pick-meta {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
