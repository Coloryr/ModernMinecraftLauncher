<script setup lang="ts">
// 作者：头像 + 名字的小胶囊，两个下载窗口的列表卡片与项目详情页共用
//
// 一枚胶囊一个作者；超过 `max` 个用「+N」收尾，别让一排作者把标题挤没。
// 头像地址可能为空（Modrinth 的搜索结果只给作者名，不给头像），这时退回
// "名字首字母"的圆形占位，别让这一格空着看着像没加载出来。
import { computed } from "vue";
import AsyncImage from "./AsyncImage.vue";
import type { PicDto } from "../../lib/bindings";

const props = withDefaults(
  defineProps<{
    /** 作者列表；空数组时什么都不画，由调用方决定要不要整块隐藏 */
    authors: PicDto[];
    /** 最多画几个作者，超出的用「+N」收掉 */
    max?: number;
  }>(),
  { max: 2 },
);

/** 真正画出来的那些 */
const shown = computed(() => props.authors.slice(0, props.max));

/** 被「+N」收掉的数量 */
const more = computed(() => Math.max(0, props.authors.length - props.max));
</script>

<template>
  <span v-for="a in shown" :key="a.name" class="author-tag" v-tip="a.name">
    <AsyncImage v-if="a.logo" class="author-avatar" :src="a.logo" alt="" />
    <span v-else class="author-avatar author-avatar-fallback">{{ a.name.slice(0, 1).toUpperCase() }}</span>
    <span class="author-name">{{ a.name }}</span>
  </span>
  <span v-if="more" class="author-tag author-more">+{{ more }}</span>
</template>

<style scoped>
/* 一枚作者胶囊：底色比卡片浅一档 + 描边，与标题明确分开；
   纯灰字太小太淡，用户报过"有作者但不明显" */
.author-tag {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 140px;
  padding: 1px 8px 1px 2px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-hover);
  color: var(--text);
  font-size: 11px;
  font-weight: 500;
  line-height: 16px;
  vertical-align: middle;
}

/* 头像：圆形。尺寸定在这里，AsyncImage 只负责把图铺满 */
.author-avatar {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  flex-shrink: 0;
}

/* 没有头像时用名字首字母占位 */
.author-avatar-fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 9px;
  font-weight: 700;
}

.author-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.author-more {
  padding: 1px 8px;
  color: var(--text-dim);
}
</style>
