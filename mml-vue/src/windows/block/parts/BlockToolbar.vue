<script setup lang="ts">
// 方块列表 · 工具条：搜索（可清空）+ 图标尺寸档 + 添加玩家头颅 / 重新渲染 + 计数
import { computed } from "vue";
import { t } from "../../../lib/i18n";
import BaseButton from "../../../components/ui/BaseButton.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import { BLOCK_SIZE_ORDER, isBlockSize, type BlockSize } from "../types";

defineProps<{
  /** 搜索词，父级持有；清空与输入都经 `update:keyword` 回写 */
  keyword: string;
  /** 当前图标尺寸档 */
  size: BlockSize;
  /** 渲染中：重新渲染禁用 */
  running: boolean;
}>();

const emit = defineEmits<{
  /** 搜索词变化，v-model 的回写 */
  (e: "update:keyword", v: string): void;
  /** 图标尺寸档变化，v-model 的回写 */
  (e: "update:size", v: BlockSize): void;
  /** 请求打开添加玩家头颅弹窗 */
  (e: "add-skin"): void;
  /** 请求重新渲染，父级以 force 调 startRender */
  (e: "re-render"): void;
}>();

const sizeOptions = computed(() =>
  BLOCK_SIZE_ORDER.map((s) => ({ value: s as string, label: t(`blocks.size.${s}`) })),
);

/** 分段控件给出的是 string，这里收口成 BlockSize */
function onSize(v: string) {
  if (isBlockSize(v)) emit("update:size", v);
}
</script>

<template>
  <div class="block-top">
    <div class="search-wrap">
      <input :value="keyword" class="field-input block-search" type="text" :placeholder="t('blocks.search')"
        spellcheck="false" autocomplete="off" @input="emit('update:keyword', ($event.target as HTMLInputElement).value)"
        @keydown.esc="emit('update:keyword', '')" />
      <button v-if="keyword" type="button" class="search-clear" v-tip="t('blocks.searchClear')"
        @click="emit('update:keyword', '')">
        <GlyphIcon name="close" :size="12" :weight="2.4" />
      </button>
    </div>

    <!-- 图标尺寸档：小 / 中 / 大 -->
    <SegmentedTabs :options="sizeOptions" :model-value="size" @update:model-value="onSize" />

    <!-- 图标按钮：标题栏里空间紧，文字放提示里，aria-label 给读屏 -->
    <BaseButton size="sm" variant="ghost" v-tip="t('blocks.addSkin')" :aria-label="t('blocks.addSkin')"
      @click="emit('add-skin')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
        <circle cx="9" cy="7" r="4" />
        <path d="M19 8v6M22 11h-6" />
      </svg>
    </BaseButton>
    <BaseButton size="sm" variant="ghost" :disabled="running" v-tip="t('blocks.reRender')"
      :aria-label="t('blocks.reRender')" @click="emit('re-render')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 12a9 9 0 1 1-2.64-6.36" />
        <polyline points="21 3 21 9 15 9" />
      </svg>
    </BaseButton>
  </div>
</template>

<style scoped>
.block-top {
  display: flex;
  align-items: center;
  gap: 8px;
  /* 在标题栏里要能收缩：窗口最窄时优先挤搜索框，不去裁按钮 */
  flex-shrink: 1;
  min-width: 0;
}

/* 图标按钮与同行的搜索框 / 尺寸档统一高度：BaseButton 的 sm 是 28px，这里用 :deep 穿透进去。
   36px 是三个用 head-right 的窗口（账户 / 方块 / 设置）共同的约定，改这里记得同步另外两处 */
.block-top :deep(.ui-btn) {
  height: 36px;
  padding: 0 10px;
}

/* 尺寸档（SegmentedTabs）默认 35px，跟着这一排一起到 36 */
.block-top :deep(.seg-btn) {
  height: 28px;
}

.search-wrap {
  position: relative;
  /* 现在固定住在标题栏里：给个确定宽度，不再靠 flex:1 撑（标题栏里没有富余空间可分）；
     窗口很窄时允许收缩到这里的最小值 */
  width: 200px;
  min-width: 120px;
  flex: 0 1 auto;
  display: flex;
}

/* 压到与工具条按钮同高（这一排统一 36px）：
   .field-input 默认 min-height: var(--field-h) = 42px，这里显式覆盖 */
.block-top .block-search {
  flex: 1;
  min-width: 0;
  height: 36px;
  min-height: 0;
  padding: 0 30px 0 12px;
  font-size: 13px;
}

.search-clear {
  position: absolute;
  top: 50%;
  right: 6px;
  transform: translateY(-50%);
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  font-size: 11px;
  line-height: 1;
  cursor: pointer;
}

.search-clear:hover {
  background: var(--bg-hover);
  color: var(--text);
}
</style>
