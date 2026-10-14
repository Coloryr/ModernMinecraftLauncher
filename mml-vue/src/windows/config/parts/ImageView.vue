<script setup lang="ts">
// 图片视图：把配置目录里的图片读成 data URL 直接显示
//
// 与其它三个视图同一口径，只吃 uuid + 相对路径；读取走后端 config_get_image，
// 配置目录里的图片都是图标 / 缩略图量级，一次 IPC 读完即可。
// 画图交给 AsyncImage，加载失败它自己会兜底。
import { ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { readConfigImage } from "../../../lib/api";
import AsyncImage from "../../../components/ui/AsyncImage.vue";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 相对游戏目录的文件路径 */
  file: string;
}>();

const src = ref("");
const loading = ref(false);

/** 拉这张图 */
async function load() {
  loading.value = true;
  src.value = "";
  try {
    src.value = await readConfigImage(props.uuid, props.file);
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

// 换文件就重拉；`immediate` 让首次打开就有内容
watch(
  () => [props.uuid, props.file],
  () => void load(),
  { immediate: true },
);
</script>

<template>
  <div class="img-view">
    <AsyncImage v-if="src" class="img-view-img" :src="src" :alt="file" :title="file" />
    <p v-else-if="!loading" class="empty-tip">{{ t("config.imageEmpty") }}</p>
  </div>
</template>

<style scoped>
.img-view {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  min-height: 0;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
  overflow: auto;
}

/* 等比缩放塞进可视区；图片比容器小时按原尺寸居中，不放大糊掉 */
.img-view-img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}
</style>
