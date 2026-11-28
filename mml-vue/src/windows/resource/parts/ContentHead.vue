<script setup lang="ts">
// 内容区顶栏：左边放该分类自己的前置内容（存档与数据包页的子页切换），右边是操作按钮 + 刷新
//
// **这里不再写分类名**：左侧导航已经把当前分类高亮出来了，顶栏再写一遍是同一份信息
// 出现两次（用户点名）。所以顶栏只承担"操作"与"分类内的子页切换"。
//
// 刷新与「打开文件夹」都在这里统一渲染：每个分类都要有，且逻辑完全一样，
// 各分类自己的按钮（清空截图 / 添加服务器）走 actions 插槽塞在它们前面。
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { openResourceFolder } from "../../../lib/api";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import type { useResourceData } from "../composables/useResourceData";
import { dropKindOf } from "../types";

const props = withDefaults(
  defineProps<{
    /** 数据层，顶栏只用它的加载态与重拉 */
    data: ReturnType<typeof useResourceData>;
    /** 额外的禁用条件；置真时刷新也不可用，如模组在跑依赖检测，默认不禁用 */
    busy?: boolean;
    /** 还画不画这颗刷新键；模组页在搜索时整条工具条让位，见 ModPane，默认画 */
    showRefresh?: boolean;
    /**
     * 整组操作按钮是否一起禁用
     *
     * 用在"列表还不属于当前分类"的窗口（切分类那一瞬、正在重拉）：由 fieldset 一次禁掉，
     * 里面的按钮各自动挂上禁用态，不必每颗各写一遍"等数据就绪"的条件。
     */
    actionsDisabled?: boolean;
  }>(),
  { busy: false, showRefresh: true, actionsDisabled: false },
);
const { loading, reloadCurrent, category, saveTab, dpSave, instanceUuid } = props.data;

/** 当前这一档在磁盘上的目录名：光影是 shaderpacks，存档的数据包子页是 datapacks */
const folderKind = computed(() => dropKindOf(category.value, saveTab.value));

/** 数据包要带存档目录名，其它分类为 null */
const folderParent = computed(() => (folderKind.value === "datapacks" ? dpSave.value : null));

/** 能不能点：得有实例；数据包还得先选存档 */
const canOpenFolder = computed(
  () => !!instanceUuid.value && (folderKind.value !== "datapacks" || !!folderParent.value),
);

/** 在文件管理器里打开当前分类的目录；目录不存在时后端会先建出来 */
async function openCurrentFolder() {
  try {
    await openResourceFolder(instanceUuid.value, folderKind.value, null, folderParent.value);
  } catch (e) {
    showToast(tErr(e));
  }
}

const headEl = ref<HTMLDivElement | null>(null);
const leadEl = ref<HTMLDivElement | null>(null);
const actionsEl = ref<HTMLDivElement | null>(null);

/** 上一次的判定；留一段回差，窗口正好卡在临界值时不会来回闪 */
let compact = false;
/** 回差，单位 px：展开要多出这么多空间、收起要真放不下才切换 */
const COMPACT_HYSTERESIS = 8;

/**
 * 按实际可用宽度决定操作区要不要只留图标
 *
 * 顶栏里既有搜索框这类可伸缩的前置内容，又有数量会变的操作按钮，单靠一个固定断点
 * 判断不准（模组页的搜索框占掉三百多像素，窗口再宽也可能放不下文字）。所以量一次：
 * 先摘掉 `is-compact` 让文字都显示出来，读两边的自然宽度，不够就再挂回去。
 * 同一帧内读回来再写回，浏览器不会中途绘制，看不到闪动。
 *
 * 量的时候要临时改两处，否则量出来的不是"需要多宽"：
 * - 搜索框按上限钉住：它可伸缩，不钉住的话"放得下"就成了"把搜索框挤窄"换来的；
 * - 左区退出伸缩：`flex-1` 会把它撑满剩余宽度，量出来永远刚好等于顶栏内宽，
 *   于是收起之后就再也展不开了；
 * 两处都在 `is-measuring` 里，见 resource.css。
 */
function fitActions() {
  const head = headEl.value;
  const lead = leadEl.value;
  const actions = actionsEl.value;
  if (!head || !lead || !actions) return;
  // 还没布局（隐藏、切换中）：这一帧不判，保持原状态
  if (!head.clientWidth) return;

  // 先关掉按钮过渡再展开：不然量到的是过渡中途的宽度，判定会跟着摆动
  head.classList.add("is-measuring");
  actions.classList.remove("is-compact");
  const style = getComputedStyle(head);
  const gap = Number.parseFloat(style.columnGap) || 0;
  const inner = head.clientWidth - Number.parseFloat(style.paddingLeft) - Number.parseFloat(style.paddingRight);
  const needed = lead.scrollWidth + gap + actions.scrollWidth;
  head.classList.remove("is-measuring");

  // 拖动窗口时宽度是小数、这里是取整的：卡在临界值上会一帧"放得下"一帧"放不下"。
  // 所以两个方向各留一段：展开要多出回差才敢放开，收起要真的不够才收
  const fits = needed <= inner;
  compact = compact ? !(needed + COMPACT_HYSTERESIS <= inner) : !fits;
  actions.classList.toggle("is-compact", compact);
}

let observer: ResizeObserver | null = null;
onMounted(async () => {
  // 插槽内容渲染完再量：批量条 / 子页切换都会改变两侧的宽度
  await nextTick();
  fitActions();
  if (typeof ResizeObserver === "undefined") return;
  observer = new ResizeObserver(() => fitActions());
  for (const el of [headEl.value, leadEl.value, actionsEl.value]) {
    if (el) observer.observe(el);
  }
});
onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
});
</script>

<template>
  <div ref="headEl" class="content-head flex items-center justify-between gap-2.5 pr-[26px]">
    <div ref="leadEl" class="head-lead flex min-w-0 flex-1 items-center gap-2">
      <slot />
    </div>
    <!-- fieldset 承载操作区：`disabled` 会把里面所有按钮一次性变成禁用态（外观仍走 .mini-btn:disabled） -->
    <fieldset ref="actionsEl" :disabled="props.actionsDisabled"
      class="head-actions ml-auto flex shrink-0 items-center gap-1.5">
      <slot name="actions" />
      <button class="mini-btn icon-btn head-btn" :disabled="busy || loading || !canOpenFolder"
        v-tip="t('resource.openFolder')" :aria-label="t('resource.openFolder')" @click="openCurrentFolder">
        <GlyphIcon name="folder" :size="14" />
        <span class="head-btn-text">{{ t("resource.openFolder") }}</span>
      </button>
      <button v-if="props.showRefresh" class="mini-btn icon-btn head-btn" :disabled="busy || loading"
        v-tip="t('resource.refresh')" :aria-label="t('resource.refresh')" @click="reloadCurrent">
        <GlyphIcon name="refresh" :size="14" />
        <span class="head-btn-text">{{ t("resource.refresh") }}</span>
      </button>
    </fieldset>
  </div>
</template>
