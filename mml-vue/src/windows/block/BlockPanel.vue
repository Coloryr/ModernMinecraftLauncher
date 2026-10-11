<script setup lang="ts">
// 方块列表：左侧分类栏带数量，右侧网格带搜索 / 分类过滤 / 图标尺寸档
// 三态视图：未渲染 → 提示卡；渲染中 → 顶部一行进度（下方列表照常可用）；已渲染 → 分类栏 + 网格
//
// 状态在 BlockWindow：工具条在标题栏，两边要共用同一份状态，这里只做排版与局部交互；
// 数据、事件订阅与操作在 composables/useBlockList，局部 UI 在 parts/。
import { computed, ref } from "vue";
import { t } from "../../lib/i18n";
import { SKIN_CAT } from "./types";
import CategoryRail from "./parts/CategoryRail.vue";
import BlockGrid from "./parts/BlockGrid.vue";
import BlockDetailModal from "./parts/BlockDetailModal.vue";
import InstancePickModal from "./parts/InstancePickModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import type { InstanceInfoDto } from "../../lib/bindings";
import type { useBlockList } from "./composables/useBlockList";

const props = defineProps<{
  /** 方块列表状态，在 BlockWindow 里创建：标题栏的工具条用的是同一份状态 */
  settings: ReturnType<typeof useBlockList>;
  /** 主窗口当前选中的实例；没有选中实例时为 null */
  currentInstance: InstanceInfoDto | null;
}>();

const {
  status,
  blocks,
  rendered,
  running,
  preparing,
  percent,
  cancelling,
  startRender,
  cancelRender,
  keyword,
  cat,
  size,
  cats,
  catCounts,
  catLabel,
  isFiltered,
  filtered,
  clearFilters,
  detail,
  openDetail,
  closeDetail,
  removeSkin,
  setIcon,
} = props.settings;

/** 分类栏条目：全部 + 各分类；文案与计数在这里算好，侧栏只管渲染 */
const catItems = computed(() => [
  { id: "", label: t("blocks.catAll"), count: blocks.value.length },
  ...cats.value.map((c) => ({ id: c, label: catLabel(c), count: catCounts.value.get(c) ?? 0 })),
]);

// ---------- 设为实例图标 ----------

const iconPick = ref(false);
const iconBusy = ref(false);

function openIconPick() {
  if (detail.value) iconPick.value = true;
}

async function onIconPick(inst: InstanceInfoDto) {
  const b = detail.value;
  if (!b || iconBusy.value) return;
  iconBusy.value = true;
  try {
    if (await setIcon(inst.uuid, b.id, inst.name)) {
      iconPick.value = false;
      closeDetail();
    }
  } finally {
    iconBusy.value = false;
  }
}

/** 详情弹窗里删除玩家头颅；详情会随 removeSkin 一起关掉 */
async function onDetailRemove() {
  const b = detail.value;
  if (b) await removeSkin(b);
}
</script>

<template>
  <div class="block-panel">
    <!-- 渲染中：顶部一行进度；不挤掉下方内容，取消后立即消失 -->
    <div v-if="running" class="block-progress">
      <span class="block-progress-text">
        {{ status?.text || (preparing ? t("blocks.downloadingCore") : t("blocks.rendering")) }}
      </span>
      <div class="block-progress-bar">
        <div class="block-progress-fill" :style="{ width: percent + '%' }"></div>
      </div>
      <span class="block-progress-percent">{{ preparing ? "—" : percent.toFixed(1) + "%" }}</span>
      <span v-if="!preparing" class="block-progress-num">
        {{ status?.now ?? 0 }} / {{ status?.total ?? 0 }}
      </span>
      <BaseButton size="sm" :disabled="cancelling" @click="cancelRender">
        {{ cancelling ? t("blocks.cancelling") : t("blocks.cancelRender") }}
      </BaseButton>
    </div>

    <!-- 已渲染：分类栏 + 网格 -->
    <div v-if="rendered" class="block-main">
      <CategoryRail :items="catItems" :active="cat" @pick="cat = $event" />
      <BlockGrid :items="filtered" :keyword="keyword" :size="size" :active-id="detail?.id ?? null" :skin-cat="SKIN_CAT"
        :filtered="isFiltered" @open="openDetail" @remove-skin="removeSkin" @clear-filters="clearFilters" />
    </div>

    <!-- 未渲染：提示卡；渲染中显示进度说明，不再给按钮 -->
    <div v-if="!rendered" class="block-prompt">
      <div class="block-prompt-icon">
        <svg viewBox="0 0 24 24" width="40" height="40" fill="none" stroke="currentColor" stroke-width="1.6"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3 4.5 7.5v9L12 21l7.5-4.5v-9L12 3z" />
          <path d="M4.5 7.5 12 12l7.5-4.5" />
          <path d="M12 12v9" />
        </svg>
      </div>
      <h2 class="block-prompt-title">
        {{ status?.error ? t("blocks.renderFailed") : running ? t("blocks.rendering") : t("blocks.renderPromptTitle") }}
      </h2>
      <p class="block-prompt-desc">
        {{ status?.error ?? (running ? t("blocks.renderFirstHint") : t("blocks.renderPromptDesc")) }}
      </p>
      <p v-if="!status?.error && !running" class="block-prompt-hint">{{ t("blocks.renderSizeHint") }}</p>
      <BaseButton v-if="!running" variant="primary" @click="startRender(false)">
        {{ status?.error ? t("blocks.retry") : t("blocks.renderNow") }}
      </BaseButton>
    </div>

    <!-- 设为实例图标：选哪个实例（当前实例高亮，点条目即设） -->
    <InstancePickModal v-if="iconPick" :current-uuid="currentInstance?.uuid ?? null" :busy="iconBusy" @pick="onIconPick"
      @close="iconPick = false" />

    <!-- 方块详情：打开选实例弹窗时先让位，避免两层遮罩叠着 -->
    <BlockDetailModal v-if="detail && !iconPick" :block="detail" :category="catLabel(detail.cat)" :keyword="keyword"
      :removable="detail.cat === SKIN_CAT" :busy="iconBusy" @close="closeDetail" @remove="onDetailRemove"
      @set-icon="openIconPick" />
  </div>
</template>

<style scoped>
.block-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-height: 0;
  /* 上 / 右 / 下 三边抵消 WindowFrame .frame-body 的 22/26 内边距：
     网格与它的滚动条顶到窗口右边缘、上下也一直延伸到窗口边（滚动条是整条）。
     左内边距保持原样；上下的呼吸改由各自滚动内容自己的留白给 ——
     网格见 BlockGrid 的 padding-top 与 BOTTOM_SPACE，分类栏见 .cat-rail 的 padding。
     那样空档只在滚到两端时出现，而不是把滚动区缩掉 */
  margin-right: -26px;
  margin-bottom: -22px;
  margin-top: -22px;
  padding-right: 0;
}

/* 只有网格 / 分类栏区域是"通栏"的，其余卡片保持和网格一致的留白：
   进度行在第一行时要补回 22px（它在面板顶部）；提示卡只在自己是第一行、或作为最底元素时补 */
.block-progress,
.block-prompt {
  margin-right: 16px;
}

.block-progress,
.block-panel>.block-prompt:first-child {
  margin-top: 22px;
}

.block-prompt {
  margin-bottom: 22px;
}

/* ---------- 提示卡（未渲染 / 失败） ---------- */
.block-prompt {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 30px 20px 26px;
  border: 1px dashed var(--accent-border);
  border-radius: 16px;
  background: var(--bg-card);
}

.block-prompt-icon {
  width: 84px;
  height: 84px;
  border-radius: 24px;
  background: var(--accent-soft);
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 8px;
}

.block-prompt-title {
  font-size: 17px;
  font-weight: 700;
}

.block-prompt-desc {
  font-size: 13px;
  color: var(--text-dim);
  text-align: center;
  max-width: 420px;
}

.block-prompt-hint {
  font-size: 12px;
  color: var(--text-dim);
  opacity: 0.75;
  text-align: center;
  max-width: 420px;
  margin-bottom: 8px;
}

/* ---------- 进度（渲染中，顶部一行） ---------- */
.block-progress {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
  font-size: 12.5px;
}

.block-progress-text {
  flex-shrink: 0;
  max-width: 45%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

/* 进度条占满中段，两端文字按内容宽度 */
.block-progress-bar {
  flex: 1;
  min-width: 60px;
  height: 6px;
  border-radius: 999px;
  background: var(--bg-side);
  overflow: hidden;
}

.block-progress-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.2s ease;
}

.block-progress-percent {
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  color: var(--accent);
}

.block-progress-num {
  flex-shrink: 0;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* ---------- 主体：分类栏 + 网格 ---------- */
.block-main {
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 16px;
}
</style>
