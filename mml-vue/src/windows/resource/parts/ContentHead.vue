<script setup lang="ts">
// 内容区顶栏：左边放该分类自己的前置内容（存档页的子页切换），右边是操作按钮 + 刷新
//
// **这里不再写分类名**：左侧导航已经把当前分类高亮出来了，顶栏再写一遍是同一份信息
// 出现两次（用户点名）。所以顶栏只承担"操作"与"分类内的子页切换"。
//
// 刷新按钮在这里统一渲染：每个分类都要有，且逻辑完全一样，
// 各分类自己的按钮（清空截图 / 添加服务器）走 actions 插槽塞在它前面。
import { t } from "../../../lib/i18n";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import type { useResourceData } from "../composables/useResourceData";

const props = defineProps<{
  /** 数据层，顶栏只用它的加载态与重拉 */
  data: ReturnType<typeof useResourceData>;
  /** 额外的禁用条件；置真时刷新也不可用，如模组在跑依赖检测，默认不禁用 */
  busy?: boolean;
}>();
const { loading, reloadCurrent } = props.data;
</script>

<template>
  <div class="content-head">
    <div class="head-lead">
      <slot />
    </div>
    <div class="head-actions">
      <slot name="actions" />
      <button class="mini-btn icon-btn icon-only" :disabled="busy || loading" v-tip="t('resource.refresh')"
        :aria-label="t('resource.refresh')" @click="reloadCurrent">
        <GlyphIcon name="refresh" :size="14" />
      </button>
    </div>
  </div>
</template>
