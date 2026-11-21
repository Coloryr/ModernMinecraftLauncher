<script setup lang="ts">
// 实例图标：优先用 mml-image 协议加载真图；没有 / 失败时画占位图 ——
// 原版是草方块、装了加载器的是那家的标志（见 components/InstancePlaceholder.vue）
import { computed, onUnmounted, ref, watch } from "vue";
import { onInstanceChange } from "../lib/api";
import InstancePlaceholder from "./InstancePlaceholder.vue";
import {
  cacheIconBlob,
  cachedImageBase,
  iconObjectUrls,
  loadedIconUrls,
  loadedIconUuids,
  loadImageBase,
} from "../lib/instanceIconCache";

const props = withDefaults(
  defineProps<{
    /** 实例名；真图加载后作为 alt */
    name: string;
    /** 实例 uuid；真图地址由它算 */
    uuid: string;
    /** 加载器 id：占位图按它挑画哪一枚 */
    loader?: string;
    /** 图标边长，单位 px */
    size?: number;
  }>(),
  { loader: "", size: 48 },
);

/** 容器尺寸与圆角；底色交给占位图 / 真图，自己不再铺渐变 */
const iconStyle = computed(() => ({
  width: props.size + "px",
  height: props.size + "px",
  borderRadius: Math.round(props.size * 0.24) + "px",
}));

// ---------- 真实图标加载 ----------

// mml-image 协议前缀；浏览器预览无 IPC 时取不到，画占位图
const base = ref(cachedImageBase() ?? "");
const failed = ref(false);
/** 真图是否已成功加载；加载成功后占位图被 <img> 盖住即可，不用再藏 */
const loaded = ref(loadedIconUuids.has(props.uuid));
/** 缓存破坏参数（图标被更换后刷新） */
const ver = ref(0);

const url = computed(() =>
  base.value ? base.value + "/instance/" + props.uuid + "?v=" + ver.value : "",
);

/** 当前实际给 <img> 的地址：优先用缓存下来的 Blob URL */
const src = ref("");

/** 同步当前图片地址与已加载状态 */
function syncSrc() {
  const link = url.value;
  if (!link) {
    src.value = "";
    return;
  }
  src.value = iconObjectUrls.get(link) ?? link;
  // 首次取前缀后就开始存 Blob，别等到图片 load 完才准备
  cacheIconBlob(link);
  if (loadedIconUrls.has(link) || loadedIconUuids.has(props.uuid)) loaded.value = true;
}

/** 加载失败：回到占位图 */
function onError() {
  failed.value = true;
  loaded.value = false;
}

/** 真图加载完成 */
function onLoad() {
  if (failed.value) return;
  if (url.value) {
    loadedIconUrls.add(url.value);
    loadedIconUuids.add(props.uuid);
    // 首屏加载完后把图片存成 Blob URL，占满 / 收回重建时直接画它
    cacheIconBlob(url.value);
  }
  loaded.value = true;
}

// URL 变化（首次取到前缀 / 换图标）时同步实际地址；重建时命中缓存直接进已加载态
watch(url, syncSrc, { immediate: true });

// onUnmounted 必须在 setup 同步期注册，await 后再调会失去组件实例，
// 所以注销函数先存下来，由同步注册的钩子代为调用
let unlistenChange: (() => void) | null = null;
let disposed = false;
onUnmounted(() => {
  disposed = true;
  unlistenChange?.();
  unlistenChange = null;
});

(async () => {
  base.value = await loadImageBase();
  if (disposed) return;

  // 只认 icon 类型：图标换 / 清了才重拉图。
  // 别改回 edit —— 改实例设置、重命名、换分组都会发 edit，那样每动一次就白重载一次图标
  const off = await onInstanceChange((type) => {
    if (type !== "icon") return;
    ver.value = Date.now();
    failed.value = false;
    // 换图期间先回到占位图，新图到了再盖上
    loadedIconUuids.delete(props.uuid);
    loaded.value = false;
  });
  // 订阅期间组件已卸载：立刻退订，别把监听留到窗口关闭之后
  if (disposed) {
    off();
    return;
  }
  unlistenChange = off;
})();

watch(
  () => props.uuid,
  () => {
    failed.value = false;
    loaded.value = loadedIconUuids.has(props.uuid);
  },
);
</script>

<template>
  <div class="inst-icon" :style="iconStyle">
    <!-- 占位图垫在底下：真图加载成功后把它整块盖住，加载中 / 失败时它就是图标本身 -->
    <InstancePlaceholder :loader="loader" />
    <img v-if="src && !failed" class="inst-icon-img" :src="src" :alt="name" loading="eager" decoding="sync"
      data-no-fallback @load="onLoad" @error="onError" />
    <!-- 悬停浮层（如"改图标"入口）：放在图标内部才被圆角与 overflow 裁住 -->
    <slot />
  </div>
</template>

<style scoped>
.inst-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  user-select: none;
  overflow: hidden;
  position: relative;
}

.inst-icon-img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  /* 实例图标基本都是内核渲染的 256×256 方块图（mml-tex-draw 的 BLOCK_SIZE），
     与方块网格 / 账户格子同一条口径：缩放交给浏览器的平滑插值，**不要 pixelated**，
     最近邻会把方块的斜边采成锯齿。代价是自传的小尺寸像素图放大会发软 */
  image-rendering: auto;
}

/* 不再压那层白高光：占位图已经是完整的图形，再叠一层会把上沿洗淡 */
</style>
