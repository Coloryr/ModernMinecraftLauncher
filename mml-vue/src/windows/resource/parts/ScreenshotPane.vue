<script setup lang="ts">
// 截图分类：网格 + 大图预览
//
// 预览是**本分类自己的视图状态**：打开哪张、删掉后要不要跟着关掉，所以留在这一层，
// 不往上塞进窗口级 composable；删除 / 清空的确认走共用的 ops。
// 放大效果用共用的 `ImagePreview`，与「下载整合包」项目详情同一套。
//
// 多选与右键菜单与其它分类同一套（见 useRowSelect / RowMenu），只是这一分类是自绘的
// 网格而不是列表行：勾选框浮在格子左上角，点格子在多选状态下才是加选，见 onCellClick。
// 格子上的悬停删除按钮已撤掉 —— 单行动作统一走右键菜单。
import { computed, onUnmounted, ref, watch } from "vue";
import AsyncImage from "../../../components/ui/AsyncImage.vue";
import ImagePreview from "../../../components/ui/ImagePreview.vue";
import { t } from "../../../lib/i18n";
import { clearScreenshots, deleteScreenshot } from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import { useRowSelect } from "../composables/useRowSelect";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { ScreenshotItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，截图列表、加载态与截图地址从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，确认弹窗与打开目录从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { shots, instanceUuid, loading, shotUrl } = props.data;
const { busy, act, askConfirm, askDelete, openFolder } = props.ops;

/** 当前预览的截图，`null` 表示没开大图 */
const preview = ref<ScreenshotItemDto | null>(null);

/**
 * Esc 关掉大图
 *
 * `ImagePreview` 自己不处理 Esc，上层可能还有别的 Esc 语义，见那个组件的说明；
 * 所以这里挂一份，且只在开着预览时才挂监听。
 */
function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") preview.value = null;
}

watch(preview, (val) => {
  if (val) window.addEventListener("keydown", onKey);
  else window.removeEventListener("keydown", onKey);
});

onUnmounted(() => window.removeEventListener("keydown", onKey));

function remove(item: ScreenshotItemDto) {
  askDelete(item.name, async () => {
    await deleteScreenshot(instanceUuid.value, item.name);
    // 删的正是开着的那张：顺手把大图关掉
    if (preview.value?.name === item.name) preview.value = null;
  });
}

function clearAll() {
  askConfirm(t("resource.clear"), t("resource.clearConfirm"), async () => {
    await clearScreenshots(instanceUuid.value);
    preview.value = null;
  });
}

// ---------- 多选 ----------

/** 选中键用**文件名**：删除按它定位，也是这一行唯一的身份字段 */
const {
  selected,
  anchorKey,
  count,
  isSelected,
  toggle,
  onRowClick,
  clear,
} = useRowSelect<ScreenshotItemDto>({
  keyOf: (item) => item.name,
  order: () => shots.value,
});

/** 选中的截图 */
const selectedItems = computed(() => shots.value.filter(isSelected));

/**
 * 点格子
 *
 * 多选状态下点格子 = 加选，与其它分类一致；否则维持原行为：开大图
 *
 * - `payload.event`: 原始点击事件，取 `shiftKey` 判整段选择
 * - `payload.item`: 点到的这一张
 */
function onCellClick(payload: { event: MouseEvent; item: ScreenshotItemDto }) {
  if (payload.event.shiftKey || count.value) {
    onRowClick(payload);
    return;
  }
  preview.value = payload.item;
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
        for (const item of picked) await deleteScreenshot(instanceUuid.value, item.name);
      });
      // 大图开着的那张可能就在这一批里
      if (preview.value && picked.some((item) => item.name === preview.value?.name)) {
        preview.value = null;
      }
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
  item: ScreenshotItemDto | null;
} | null>(null);

/** 单行菜单项 */
function itemMenuItems(): RowMenuItem[] {
  return [
    { id: "open-folder", label: t("resource.openFolder") },
    { id: "remove", label: t("resource.delete"), danger: true, separated: true },
  ];
}

/** 打开右键菜单 */
function openMenu(payload: { event: MouseEvent; item: ScreenshotItemDto }) {
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
    case "open-folder":
      openFolder("screenshots", item.name);
      break;
    case "remove":
      remove(item);
      break;
  }
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
      <button v-else-if="shots.length" class="mini-btn danger" :disabled="busy" @click="clearAll">
        {{ t("resource.clear") }}
      </button>
    </template>
  </ContentHead>

  <div class="shot-list">
    <div v-if="shots.length" class="shot-grid">
      <div v-for="item in shots" :key="item.name" class="shot-cell" :class="{
        'row-selected': isSelected(item),
        'row-anchor': anchorKey === item.name,
        'row-selecting': selected.size > 0,
      }" @click="onCellClick({ event: $event, item })"
        @contextmenu.prevent="openMenu({ event: $event, item })">
        <!-- 勾选框是进入多选的入口：点它不能顺带开大图，所以整块 stop -->
        <label class="shot-check" @click.stop>
          <input type="checkbox" class="row-select" :checked="isSelected(item)" @pointerdown.stop
            @change.stop="toggle(item)" @click.stop />
        </label>
        <AsyncImage class="shot-img" :src="shotUrl(item)" :alt="item.name" :title="item.name" />
      </div>
    </div>
    <div v-else-if="!loading" class="empty-tip">{{ t("resource.empty") }}</div>
  </div>

  <!-- 行右键菜单：单行与批量共用，菜单项见 itemMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title"
    @pick="onMenuPick" @close="rowMenu = null" />

  <!--
    大图预览：与「下载整合包」项目详情的截图**同一套观感**，共用 ImagePreview ——
    全屏浮层 + 滚轮缩放 + 放大后拖动平移 + 点空白关闭。
    两个操作放进浮层底部的槽里：原来的弹窗有它们，换成浮层不该弄丢
  -->
  <ImagePreview v-if="preview" :src="shotUrl(preview)" @close="preview = null">
    <button class="mini-btn" @click="openFolder('screenshots', preview.name)">
      {{ t("resource.openFolder") }}
    </button>
    <button class="mini-btn danger" @click="remove(preview)">
      {{ t("resource.delete") }}
    </button>
  </ImagePreview>
</template>

<!-- 浮层里的按钮是 Teleport 到 body 的，`.resource-layout` 前缀够不着，所以自带 scoped 样式 -->
<style scoped>
.mini-btn {
  height: 28px;
  padding: 0 12px;
  border-radius: 7px;
  border: 1px solid rgb(255 255 255 / 25%);
  background: rgb(0 0 0 / 55%);
  color: rgb(255 255 255 / 90%);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.12s;
}

.mini-btn:hover {
  border-color: #fff;
  color: #fff;
}

.mini-btn.danger:hover {
  border-color: var(--red);
  color: var(--red);
}
</style>
