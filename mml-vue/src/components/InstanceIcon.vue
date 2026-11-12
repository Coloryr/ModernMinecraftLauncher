<script setup lang="ts">
// 实例图标：优先 mml-image 协议加载真图，缺失 / 失败时回退为
// 按 uuid 取色的渐变底 + 实例名首字母
import { computed, onUnmounted, ref, watch } from "vue";
import { onInstanceChange } from "../lib/api";
import { instanceColor, instanceGradient } from "../lib/instanceColor";
import { gradientMode } from "../lib/theme";
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
    /** 实例名；取首字母作为占位字 */
    name: string;
    /** 实例 uuid；真图地址与回退配色都由它算 */
    uuid: string;
    /** 图标边长，单位 px */
    size?: number;
  }>(),
  { size: 48 },
);

// 取色在 lib/instanceColor：统计窗口的单日饼图用同一套，颜色才能和图标对上；
// 纯色模式直接用色组首色，与饼图色块同色
const palette = computed(() =>
  gradientMode.value ? instanceGradient(props.uuid) : instanceColor(props.uuid),
);

const char = computed(() => props.name.trim().charAt(0).toUpperCase() || "M");

/**
 * 容器样式：真图加载成功后不再画占位底色，原因见 loaded
 *
 * 底色与背景图必须分开写（backgroundColor / backgroundImage），不能用 `background` 简写：
 * 简写会先把渐变写进 background-image，随后 Vue 把 backgroundImage 置空（等于移除该
 * 长写属性），渐变随之被抹掉，占位图就只剩一个透明方块 —— 没设图标的实例全变成"透明色"。
 */
const iconStyle = computed(() => {
  const style: Record<string, string> = {
    width: props.size + "px",
    height: props.size + "px",
    backgroundColor: "transparent",
    backgroundSize: "cover",
    backgroundPosition: "center",
    fontSize: Math.round(props.size * 0.42) + "px",
    borderRadius: Math.round(props.size * 0.24) + "px",
  };
  // 已加载过的图标同时铺一层背景：重建时即使 <img> 还没触发 load，
  // 也能直接从浏览器缓存画出原图，不再先露出字母占位
  if (loaded.value && src.value) {
    style.backgroundImage = `url("${src.value}")`;
  } else if (gradientMode.value) {
    style.backgroundImage = palette.value;
  } else {
    style.backgroundColor = palette.value;
  }
  return style;
});

// ---------- 真实图标加载 ----------

// mml-image 协议前缀；浏览器预览无 IPC 时取不到，走回退渐变
const base = ref(cachedImageBase() ?? "");
const failed = ref(false);
/**
 * 真图是否已成功加载
 *
 * 加载后必须把字母占位藏掉：方块图标是**透明底** PNG，实测 94%~97% 的像素 alpha=0，
 * 字母留在 DOM 里会从透明处透出来，看着就是"设了图标但占位图没消失"。
 * 加载中 / 失败时仍显示字母，不让图标区空着。
 */
const loaded = ref(loadedIconUuids.has(props.uuid));
/** 缓存破坏参数（图标被更换后刷新） */
const ver = ref(0);

const url = computed(() =>
  base.value ? `${base.value}/instance/${props.uuid}?v=${ver.value}` : "",
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

/** 加载失败：退回字母占位 */
function onError() {
  failed.value = true;
  loaded.value = false;
}

/**
 * 真图加载完成
 *
 * 失败过就忽略：`data-no-fallback` 已经让全局图片兜底不插手了，但万一有别的
 * 来源再触发一次 load（占位图被换进来之类），也不能把 failed 的状态翻回去 ——
 * 那会把字母占位藏掉、图标区变成一片空白。
 */
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
    // 换图期间先回到字母，新图到了再藏（否则旧图会一直挂着）
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
  <div class="inst-icon" :class="{ 'has-icon': loaded }" :style="iconStyle">
    <img v-if="src && !failed" class="inst-icon-img" :src="src" alt="" loading="eager" decoding="sync" data-no-fallback
      @load="onLoad" @error="onError" />
    <!-- 字母只是回退：真图加载成功后必须移除，透明处不会再透出占位 -->
    <span v-if="!loaded" class="inst-icon-char">{{ char }}</span>
    <!-- 悬停浮层（如"改图标"入口）：放在图标内部才被圆角与 overflow 裁住 -->
    <slot />
  </div>
</template>

<style scoped>
.inst-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--on-accent);
  font-weight: 700;
  flex-shrink: 0;
  box-shadow: 0 2px 6px var(--overlay-faint);
  border: 1px solid var(--on-accent-line);
  user-select: none;
  overflow: hidden;
  position: relative;
}

/* 字母占位：加载中 / 失败时才存在；真图加载成功后由 v-if 移除 */
.inst-icon-char {
  line-height: 1;
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

/* 简单的高光装饰；只有占位态才画，真图上方不该再压一层白高光 */
.inst-icon::after {
  content: "";
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, var(--on-accent-sheen), transparent 45%);
  pointer-events: none;
}

.inst-icon.has-icon::after {
  display: none;
}

/* 真图加载后连边框与阴影也去掉：底色没了但这两样还在时，整块会读成"白底 + 深色描边"的方框。
   保留 1px 透明边框而不是 border: none，盒模型尺寸不变，图标不会比占位态偏移 1px。 */
.inst-icon.has-icon {
  border-color: transparent;
  box-shadow: none;
}
</style>
