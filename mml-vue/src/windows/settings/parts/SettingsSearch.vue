<script setup lang="ts">
// 设置窗口 · 设置项搜索
//
// 索引在 ../search.ts（静态表，覆盖 6 个标签）。这里只负责输入与结果列表：
// ↑/↓ 选择、Enter 跳转、点击跳转；Esc 有内容时先清空（不关窗口）。
// 输入框与清空键的观感走共享组件 SearchBox，结果列表由它的 `drop` 插槽挂出来。
import { computed, onDeactivated, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import SearchBox from "../../../components/ui/SearchBox.vue";
import HighlightText from "../../../components/ui/HighlightText.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { searchSettings, type SettingsHit } from "../search";

const emit = defineEmits<{
  /** 选中一条结果，参数是命中项 */
  (e: "select", hit: SettingsHit): void;
}>();

const query = ref("");
const open = ref(false);
const active = ref(0);

const hits = computed(() => searchSettings(query.value));

watch(hits, () => {
  active.value = 0;
});

// 一打字就展开结果（清空时不用展开，列表本来就空）
watch(query, (q) => {
  if (q.trim()) open.value = true;
});

/**
 * 离开设置窗口时清掉搜索状态
 *
 * 单窗口模式下 App.vue 用 `<KeepAlive>` 缓存窗口组件：切走只是 deactivate，
 * 组件不销毁，`query` 会一直留着 —— 再打开设置时输入框里还压着上次搜的词，
 * 用户早忘了自己搜过什么，只会觉得界面"不对劲"。回来就该是干净的一页。
 * 标签页 / 滚动位置是另一回事，那些是用户主动留在那儿的，不清。
 */
onDeactivated(() => {
  query.value = "";
  open.value = false;
  active.value = 0;
});

function choose(i: number) {
  const hit = hits.value[i];
  if (!hit) return;
  // 选完清空：跳到目标分组后不该还压着一层下拉
  query.value = "";
  open.value = false;
  emit("select", hit);
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    if (!query.value) return; // 没内容：交给窗口的 Esc（关窗）
    query.value = "";
    e.stopPropagation(); // 有内容：只清搜索，别把窗口关了
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    active.value = Math.min(hits.value.length - 1, active.value + 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    active.value = Math.max(0, active.value - 1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    choose(active.value);
  }
}
</script>

<template>
  <SearchBox v-model="query" :placeholder="t('winSettings.searchPlaceholder')" width="280px" @focus="open = true"
    @blur="open = false" @keydown="onKey">
    <!-- 结果列表：位置由 SearchBox 的定位参照决定，样式在 settings.css 的 .set-search-drop -->
    <template #drop>
      <div v-if="open && query.trim()" class="set-search-drop">
        <button v-for="(h, i) in hits" :key="`${h.tab}/${h.group}/${h.label}`" type="button" class="set-search-item"
          :class="{ on: i === active }" @mousedown.prevent @click="choose(i)">
          <span class="set-search-label">
            <HighlightText :text="h.label" :query="query" />
          </span>
          <span class="set-search-path">
            {{ h.tabLabel }}
            <GlyphIcon name="chevron-right" :size="10" :weight="2.6" />
            <HighlightText :text="h.groupLabel" :query="query" />
          </span>
        </button>
        <div v-if="!hits.length" class="empty-tip">{{ t("winSettings.searchNone") }}</div>
      </div>
    </template>
  </SearchBox>
</template>
