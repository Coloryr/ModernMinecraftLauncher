<script setup lang="ts">
// 整合包 / 添加资源两个窗口共用的项目卡片：图标、名称、作者、安装状态、标签与收藏星标
//
// 两个窗口原先各抄一份模板和一套 .pack-* 样式，改一处要改两处；收成组件后
// 标签折叠、数量与日期格式化也只在这里算一次。
import { computed } from "vue";
import AsyncImage from "./ui/AsyncImage.vue";
import AuthorTag from "./ui/AuthorTag.vue";
import HighlightText from "./ui/HighlightText.vue";
import { formatCount, t } from "../lib/i18n";
import { formatDate } from "../lib/format";
import { svgIcon } from "../lib/modrinthIcon";
import { cardTags, hiddenTagTip } from "../lib/projectTags";
import type { ProjectItemDto } from "../lib/bindings";

const props = defineProps<{
  /** 项目本体 */
  item: ProjectItemDto;
  /** 搜索词，传给标题高亮；空串表示没在搜索 */
  filter: string;
  /** 项目是否已安装 */
  installed?: boolean;
  /** 项目是否有文件正在下载 */
  running?: boolean;
}>();

const emit = defineEmits<{
  /** 点收藏星标 */
  (e: "star", item: ProjectItemDto): void;
  /** 点卡片主体打开详情 */
  (e: "open", item: ProjectItemDto): void;
}>();

/** 卡片上显示的标签与折叠数；同一份结果给模板多处用，别每次渲染都重算 */
const tags = computed(() => cardTags(props.item.tag));
</script>

<template>
  <div class="pack-item">
    <!-- 收藏星标（右上角）：点击收藏 / 取消收藏 -->
    <button class="pack-star" :class="{ on: item.isStar }"
      v-tip="item.isStar ? t('modpack.unstar') : t('modpack.star')" @click.stop="emit('star', item)">
      <svg viewBox="0 0 24 24" width="17" height="17" stroke-width="2" stroke-linejoin="round">
        <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
      </svg>
    </button>
    <div class="pack-main" @click="emit('open', item)">
      <AsyncImage v-if="item.image" class="pack-icon" :src="item.image" alt="" />
      <div v-else class="pack-icon pack-icon-fallback">{{ item.name.slice(0, 1).toUpperCase() }}</div>
      <div class="pack-info">
        <div class="pack-name">
          <HighlightText :text="item.name" :query="filter" />
          <!-- 作者：头像 + 名字的胶囊，与标题明确分开 -->
          <AuthorTag v-if="item.authors.length" :authors="item.authors" />
          <span v-if="installed" class="pack-badge">{{ t("modpack.installed") }}</span>
          <span v-else-if="running" class="pack-badge busy">{{ t("modpack.downloading") }}</span>
        </div>
        <div class="pack-desc">{{ item.summary }}</div>
        <div v-if="item.tag.length" class="pack-tags">
          <span v-for="tag in tags.shown" :key="tag.name" class="pack-tag">
            <span v-if="tag.svg" class="tag-svg" v-html="svgIcon(tag.svg)"></span>
            <img v-else-if="tag.logo" class="pack-tag-icon" :src="tag.logo" loading="lazy" alt="" />
            {{ tag.name }}
          </span>
          <!-- 超过上限的标签用省略号顶上，别把最后一个切掉一半 -->
          <span v-if="tags.more" class="pack-tag pack-tag-more" v-tip="hiddenTagTip(tags.hidden)">…</span>
        </div>
        <div class="pack-meta">
          {{ t("modpack.downloads", { n: formatCount(item.downloadCount) }) }}
          <template v-if="item.date"> · {{ formatDate(item.date) }}</template>
        </div>
      </div>
    </div>
  </div>
</template>

