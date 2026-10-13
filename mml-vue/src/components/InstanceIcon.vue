<script setup lang="ts">
// 实例图标：优先 mml-image 协议加载真图，缺失 / 失败时回退为
// 按 uuid 取色的渐变底 + 实例名首字母
import { computed, onUnmounted, ref, watch } from "vue";
import { getImageBaseUrl, onInstanceChange } from "../lib/api";

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

// 根据 uuid 稳定取色，保证同一实例图标一致；图标缺失 / 加载失败时就用它回退
const gradients = [
  "linear-gradient(135deg, #3f8cff, #5f6cff)",
  "linear-gradient(135deg, #34d399, #22d3ee)",
  "linear-gradient(135deg, #f59e0b, #ef4444)",
  "linear-gradient(135deg, #a855f7, #ec4899)",
  "linear-gradient(135deg, #14b8a6, #3b82f6)",
  "linear-gradient(135deg, #f97316, #f43f5e)",
  "linear-gradient(135deg, #84cc16, #22c55e)",
  "linear-gradient(135deg, #06b6d4, #6366f1)",
];

const palette = computed(() => {
  let h = 0;
  for (const c of props.uuid) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return gradients[h % gradients.length];
});

const char = computed(() => props.name.trim().charAt(0).toUpperCase() || "M");

/**
 * 容器样式
 *
 * 真图加载成功后**不再画占位底色**：方块图标是透明底 PNG，实测 94%~97% 的像素
 * alpha=0，留着渐变会从透明处透出来，看着就是"占位图没隐藏、方块图叠在绿色底上"。
 * 加载中 / 失败时仍是渐变底 + 字母。
 */
const iconStyle = computed(() => ({
  width: props.size + "px",
  height: props.size + "px",
  background: loaded.value ? "transparent" : palette.value,
  fontSize: Math.round(props.size * 0.42) + "px",
  borderRadius: Math.round(props.size * 0.24) + "px",
}));

// ---------- 真实图标加载 ----------

// mml-image 协议前缀；浏览器预览无 IPC 时取不到，走回退渐变
const base = ref("");
const failed = ref(false);
/**
 * 真图是否已成功加载
 *
 * 加载后必须把字母占位藏掉：方块图标是**透明底** PNG，实测 94%~97% 的像素 alpha=0，
 * 字母留在 DOM 里会从透明处透出来，看着就是"设了图标但占位图没消失"。
 * 加载中 / 失败时仍显示字母，不让图标区空着。
 */
const loaded = ref(false);
/** 缓存破坏参数（图标被更换后刷新） */
const ver = ref(0);

const url = computed(() =>
  base.value ? `${base.value}/instance/${props.uuid}?v=${ver.value}` : "",
);

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
  loaded.value = true;
}

// onUnmounted 必须在 setup 同步期注册，await 后再调会失去组件实例，
// 所以注销函数先存下来，由同步注册的钩子代为调用
let unlistenChange: (() => void) | null = null;
onUnmounted(() => unlistenChange?.());

(async () => {
  try {
    base.value = await getImageBaseUrl();
  } catch {
    base.value = "";
  }

  // 实例图标被更换（edit）后刷新；事件无 uuid，低频操作统一刷新即可
  unlistenChange = await onInstanceChange((type) => {
    if (type !== "edit") return;
    ver.value = Date.now();
    failed.value = false;
    // 换图期间先回到字母，新图到了再藏（否则旧图会一直挂着）
    loaded.value = false;
  });
})();

watch(
  () => props.uuid,
  () => {
    failed.value = false;
    loaded.value = false;
  },
);
</script>

<template>
  <div class="inst-icon" :class="{ 'has-icon': loaded }" :style="iconStyle">
    <img v-if="url && !failed" class="inst-icon-img" :src="url" alt="" loading="lazy" decoding="async" data-no-fallback
      @load="onLoad" @error="onError" />
    <!-- 字母只是回退：真图加载成功后必须移除，透明处不会再透出占位 -->
    <span v-if="!loaded" class="inst-icon-char">{{ char }}</span>
  </div>
</template>

<style scoped>
.inst-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 700;
  flex-shrink: 0;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
  border: 1px solid rgba(255, 255, 255, 0.14);
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
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.22), rgba(255, 255, 255, 0) 45%);
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
