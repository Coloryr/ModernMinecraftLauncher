<script setup lang="ts">
// 截图分类：网格 + 大图预览
//
// 预览是**本分类自己的视图状态**：打开哪张、删掉后要不要跟着关掉，所以留在这一层，
// 不往上塞进窗口级 composable；删除 / 清空的确认走共用的 ops。
// 放大效果用共用的 `ImagePreview`，与「下载整合包」项目详情同一套。
import { onUnmounted, ref, watch } from "vue";
import AsyncImage from "../../../components/ui/AsyncImage.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import ImagePreview from "../../../components/ui/ImagePreview.vue";
import { t } from "../../../lib/i18n";
import { clearScreenshots, deleteScreenshot } from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
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
const { busy, askConfirm, askDelete, openFolder } = props.ops;

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
</script>

<template>
  <ContentHead :data="data">
    <template #actions>
      <button v-if="shots.length" class="mini-btn danger" :disabled="busy" @click="clearAll">
        {{ t("resource.clear") }}
      </button>
    </template>
  </ContentHead>

  <div class="shot-list">
    <div v-if="shots.length" class="shot-grid">
      <div v-for="item in shots" :key="item.name" class="shot-cell">
        <AsyncImage class="shot-img" :src="shotUrl(item)" :alt="item.name" :title="item.name" @click="preview = item" />
        <button class="shot-del" v-tip="t('resource.delete')" @click="remove(item)">
          <GlyphIcon name="close" :size="13" :weight="2.4" />
        </button>
      </div>
    </div>
    <div v-else-if="!loading" class="empty-tip">{{ t("resource.empty") }}</div>
  </div>

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
