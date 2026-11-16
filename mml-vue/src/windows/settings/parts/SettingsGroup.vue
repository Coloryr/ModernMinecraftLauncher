<script setup lang="ts">
// 设置分组：标题（带强调色竖条）+ 搜索锚点 + 命中高亮
//
// 「恢复默认」不在这里：整页共用一个按钮，见 SettingsWindow 内容区右上角，
// 免得分组级与整页级两个入口并存、用户分不清该点哪个。
import { t } from "../../../lib/i18n";

defineProps<{
  /** 分组 id，与搜索索引、恢复默认的派发键一致 */
  id: string;
  /** 标题文案键；不传则不渲染标题，用于整页只有一个隐式分组的标签 */
  titleKey?: string;
  /** 高亮脉冲（设置项搜索命中后跳过来时用） */
  flash?: boolean;
}>();
</script>

<template>
  <section :id="`set-${id}`" class="setting-group" :class="{
    flash,
    'rounded-panel px-3 py-2.5 -mx-3 -my-2.5': flash,
  }">
    <h3 v-if="titleKey" class="group-title flex items-center gap-2 text-[13.5px] font-bold mt-6 mb-3 pb-[9px] border-b border-border">{{ t(titleKey) }}</h3>
    <slot />
  </section>
</template>
