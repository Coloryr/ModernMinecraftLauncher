<script setup lang="ts">
// 图片视图：把配置目录里的图片读成 data URL 直接显示
//
// 与其它三个视图同一口径，只吃 uuid + 来源 + 来源内相对路径；读取走后端 config_get_image，
// 配置目录里的图片都是图标 / 缩略图量级，一次 IPC 读完即可。
// 画图交给 AsyncImage，加载失败它自己会兜底。
//
// **不套卡片**：图片直接按原样居中显示，后面不垫底板、不描边 —— 底板在浅色主题下就是一圈
// 白框，看透明 PNG 时尤其碍眼。
import { ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { readConfigImage } from "../../../lib/api";
import AsyncImage from "../../../components/ui/AsyncImage.vue";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 来源，空串为实例本身，否则是存档目录名 */
  source: string;
  /** 来源内的相对文件路径 */
  file: string;
}>();

const src = ref("");
const loading = ref(false);

/** 拉这张图 */
async function load() {
  loading.value = true;
  src.value = "";
  try {
    src.value = await readConfigImage(props.uuid, props.source, props.file);
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

// 换来源或换文件就重拉；`immediate` 让首次打开就有内容
watch(
  () => [props.uuid, props.source, props.file],
  () => void load(),
  { immediate: true },
);
</script>

<template>
  <div class="img-view flex items-center justify-center h-full min-h-0 p-2.5 overflow-auto">
    <AsyncImage v-if="src" class="img-view-img max-w-full max-h-full object-contain" :src="src" :alt="file"
      :title="file" />
    <p v-else-if="!loading" class="empty-tip">{{ t("config.imageEmpty") }}</p>
  </div>
</template>

