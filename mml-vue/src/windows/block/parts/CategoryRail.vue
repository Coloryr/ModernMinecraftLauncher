<script setup lang="ts">
// 方块列表 · 左侧分类栏：全部 + 各分类条目数

interface CatItem {
  /** 分类 ID（"" = 全部） */
  id: string;
  label: string;
  count: number;
}

defineProps<{
  /** 分类条目，第一项是「全部」，其余按后端排序 */
  items: CatItem[];
  /** 当前选中的分类 ID，空串表示「全部」 */
  active: string;
}>();

const emit = defineEmits<{
  /** 选中一个分类，参数是分类 ID，空串表示「全部」 */
  (e: "pick", id: string): void;
}>();
</script>

<template>
  <aside class="cat-rail" role="tablist" aria-orientation="vertical">
    <button v-for="c in items" :key="c.id || '__all'" type="button" role="tab" class="cat-item"
      :class="{ on: active === c.id }" :aria-selected="active === c.id" @click="emit('pick', c.id)">
      <span class="cat-name">{{ c.label }}</span>
      <span class="cat-num">{{ c.count }}</span>
    </button>
  </aside>
</template>

<style scoped>
.cat-rail {
  width: 168px;
  min-width: 168px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  /* 上下留白跟着窗口内容区的原内边距（22px）走，且放进滚动内容里：
     空档只在滚到两端时出现，分类栏的滚动条轨道保持整条 */
  padding: 22px 2px 22px 0;
}

.cat-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 11px;
  border: none;
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.cat-item:hover {
  background: var(--bg-hover);
}

.cat-item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}

.cat-item.on {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}

.cat-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cat-num {
  flex-shrink: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.cat-item.on .cat-num {
  color: var(--accent);
}
</style>
