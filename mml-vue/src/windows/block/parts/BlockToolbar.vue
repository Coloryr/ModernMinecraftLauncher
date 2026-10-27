<script setup lang="ts">
// 方块列表 · 工具条：搜索（可清空）+ 图标尺寸档 + 添加玩家头颅 / 重新渲染 + 计数
import { computed } from "vue";
import { t } from "../../../lib/i18n";
import BaseButton from "../../../components/ui/BaseButton.vue";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { BLOCK_SIZE_ORDER, isBlockSize, type BlockSize } from "../types";

defineProps<{
  /** 搜索词，父级持有；清空与输入都经 `update:keyword` 回写 */
  keyword: string;
  /** 当前图标尺寸档 */
  size: BlockSize;
  /** 渲染中：重新渲染禁用 */
  running: boolean;
  /** 多选模式开着时按钮高亮 */
  multiSelect: boolean;
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
  /** 切换多选模式 */
  (e: "toggle-multi"): void;
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
      <button v-if="keyword" type="button" class="input-clear search-clear" v-tip="t('blocks.searchClear')"
        @click="emit('update:keyword', '')">
        <GlyphIcon name="close" :size="12" :weight="2.4" />
      </button>
    </div>

    <!-- 图标尺寸档：小 / 中 / 大 -->
    <SegmentedTabs :options="sizeOptions" :model-value="size" @update:model-value="onSize" />

    <!-- 图标按钮：标题栏里空间紧，文字放提示里，aria-label 给读屏 -->
    <BaseButton size="sm" :variant="multiSelect ? 'accent' : 'ghost'" class="icon-btn-head"
      v-tip="t('blocks.multiSelect')" :aria-label="t('blocks.multiSelect')"
      @click="emit('toggle-multi')">
      <GlyphIcon name="check-square" :size="15" :weight="1.8" />
    </BaseButton>
    <BaseButton size="sm" variant="ghost" class="icon-btn-head" v-tip="t('blocks.addSkin')"
      :aria-label="t('blocks.addSkin')"
      @click="emit('add-skin')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
        <circle cx="9" cy="7" r="4" />
        <path d="M19 8v6M22 11h-6" />
      </svg>
    </BaseButton>
    <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="running" v-tip="t('blocks.reRender')"
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

/* 横向留白交给标题栏标准里的 .icon-btn-head（styles/titlebar.css） */

.search-wrap {
  position: relative;
  /* 现在固定住在标题栏里：给个确定宽度，不再靠 flex:1 撑（标题栏里没有富余空间可分）；
     窗口很窄时允许收缩到这里的最小值 */
  width: 200px;
  min-width: 120px;
  flex: 0 1 auto;
  display: flex;
}

/* 搜索框：高度走 --ctl-h，这里只把右侧留白加大给清空角标让位 */
.block-top .block-search {
  flex: 1;
  min-width: 0;
  padding: 3px 30px 3px 12px;
  font-size: 13px;
}

/* 外观走共享的 .input-clear（styles/controls.css），这里只负责摆位：压在输入框右端 */
.search-clear {
  position: absolute;
  top: 50%;
  right: 6px;
  transform: translateY(-50%);
}
</style>
