<script setup lang="ts">
// 模组页顶栏：搜索 / 搜索字段 / 视图切换 / 批量操作 / 新建分组 / 更多。
// 只负责画与抛事件，搜索词、选中项、菜单状态仍在 ModPane。
import { computed } from "vue";
import ContentHead from "../ContentHead.vue";
import SearchBox from "../../../../components/ui/SearchBox.vue";
import SegmentedTabs from "../../../../components/ui/SegmentedTabs.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import { t } from "../../../../lib/i18n";
import { SEARCH_FIELDS, type ModSearchField, type ModView } from "../../types";
import type { useResourceData } from "../../composables/useResourceData";

const props = defineProps<{
  /** 数据层，顶栏只用它的加载态与重拉 */
  data: ReturnType<typeof useResourceData>;
  /** 依赖检测 / 更新检查在跑 */
  busy: boolean;
  /** 模组列表正在加载 */
  loading: boolean;
  /** 搜索词 */
  keyword: string;
  /** 搜索字段 */
  searchField: ModSearchField;
  /** 已选模组数 */
  selectedCount: number;
  /** 选中项还能不能移到别的分组 */
  canMove: boolean;
  /** 当前展示方式 */
  modView: ModView;
  /** 「更多」菜单是否打开 */
  moreOpen: boolean;
}>();

const emit = defineEmits<{
  /** 搜索词变化 */
  (e: "update:keyword", value: string): void;
  /** 搜索字段变化 */
  (e: "update:searchField", value: ModSearchField): void;
  /** 展示方式变化 */
  (e: "update:modView", value: ModView): void;
  /** 批量启用 / 禁用，参数为 true 时禁用 */
  (e: "batchState", wantDisable: boolean): void;
  /** 移到分组 */
  (e: "move"): void;
  /** 删除选中项 */
  (e: "remove"): void;
  /** 导出选中项 */
  (e: "export"): void;
  /** 清空选择 */
  (e: "clear"): void;
  /** 新建分组 */
  (e: "addGroup"): void;
  /** 打开「更多」菜单 */
  (e: "more", event: MouseEvent): void;
}>();

/** 搜索词非空时收起批量图标、显示搜索字段 */
const searchActive = computed(() => props.keyword.trim().length > 0);
</script>

<template>
  <ContentHead :data="data" :busy="busy" :show-refresh="!searchActive">
    <!-- 搜索框与搜索字段同一组：字段只服务于搜索，不放到右侧按钮旁边 -->
    <template v-if="!selectedCount">
      <div class="head-search-wrap">
        <SearchBox :model-value="keyword" :placeholder="t('resource.searchPlaceholder')"
          :clear-label="t('resource.searchClear')" width="100%"
          @update:model-value="emit('update:keyword', $event)" />
      </div>
      <div class="bar-slot field-slot" :class="{ open: searchActive }" :inert="!searchActive">
        <div class="bar-slot-inner">
          <select class="field-select sel-search-field" :value="searchField" :disabled="loading"
            :aria-label="t('resource.searchField')"
            @change="emit('update:searchField', ($event.target as HTMLSelectElement).value as ModSearchField)">
            <option v-for="field in SEARCH_FIELDS" :key="field.value" :value="field.value">
              {{ t(field.labelKey) }}
            </option>
          </select>
        </div>
      </div>
    </template>

    <template #actions>
      <template v-if="selectedCount">
        <span class="row-selected-count">{{ t("resource.selectedCount", { n: selectedCount }) }}</span>
        <button class="mini-btn icon-btn icon-only" :disabled="busy" v-tip="t('resource.enable')"
          :aria-label="t('resource.enable')" @click="emit('batchState', false)">
          <GlyphIcon name="check" :size="14" />
        </button>
        <button class="mini-btn icon-btn icon-only" :disabled="busy" v-tip="t('resource.disable')"
          :aria-label="t('resource.disable')" @click="emit('batchState', true)">
          <GlyphIcon name="ban" :size="14" />
        </button>
        <button class="mini-btn icon-btn icon-only" :disabled="busy || !canMove"
          v-tip="t('resource.batchMoveTo')" :aria-label="t('resource.batchMoveTo')" @click="emit('move')">
          <GlyphIcon name="folder" :size="14" />
        </button>
        <button class="mini-btn icon-btn icon-only danger" :disabled="busy" v-tip="t('resource.delete')"
          :aria-label="t('resource.delete')" @click="emit('remove')">
          <GlyphIcon name="trash" :size="14" />
        </button>
        <button class="mini-btn icon-btn icon-only" :disabled="busy || loading" v-tip="t('modExport.title')"
          :aria-label="t('modExport.title')" @click="emit('export')">
          <GlyphIcon name="export-list" :size="14" />
        </button>
        <button class="mini-btn icon-btn icon-only" :disabled="busy" v-tip="t('resource.clearSelection')"
          :aria-label="t('resource.clearSelection')" @click="emit('clear')">
          <GlyphIcon name="close" :size="14" />
        </button>
      </template>

      <template v-else>
        <SegmentedTabs :model-value="modView" :options="[
          { value: 'list', label: t('resource.viewList') },
          { value: 'table', label: t('resource.viewTable') },
        ]" @update:model-value="emit('update:modView', $event as ModView)" />
        <div class="bar-slot" :class="{ open: !searchActive }" :inert="searchActive">
          <div class="bar-slot-inner">
            <button class="mini-btn icon-btn head-btn" :disabled="busy" v-tip="t('resource.groupAdd')"
              :aria-label="t('resource.groupAdd')" @click="emit('addGroup')">
              <GlyphIcon name="folder-plus" :size="14" />
              <span class="head-btn-text">{{ t("resource.groupAdd") }}</span>
            </button>
            <button class="mini-btn icon-btn head-btn" v-tip="t('resource.moreActions')" aria-haspopup="menu"
              :aria-expanded="moreOpen" :aria-label="t('resource.moreActions')" @click="emit('more', $event)">
              <GlyphIcon name="more" :size="14" />
              <span class="head-btn-text">{{ t("resource.moreActions") }}</span>
            </button>
          </div>
        </div>
      </template>
    </template>
  </ContentHead>
</template>
