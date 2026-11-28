<script setup lang="ts">
// 模组视图之一：列表，每行是图标 + 名称/徽标 + 副标题 + 操作，与其它分类的行同一观感
//
// **内置模组（jar-in-jar）与改写目标都展开在同一列里**：有下级的行前面多一个展开箭头，
// 展开后按 22px 缩进显示下一层 —— 先是这个 jar 装的内置模组，再是这个模组用 mixin / ASM
// 改过的目标类，一行一个类、方法写在右侧。改写目标原先放在独立子页面里，摊进列表之后
// 搜索能直接搜到目标类与方法，不必先点开。
//
// **行是拍平了渲染的**（父行 + 已展开的内置行 + 已展开的目标行），外面再包一层 `.mod-rows`
// 交给 composables/useVirtualRows 做**行虚拟化**：几百个模组时只渲染视口附近那几行。
// 模组行与目标行各有固定高度，虚拟窗口按实际行高计算。
//
// 排序：**按 modid 正序**，见 types.ts 的 sortedMods —— 与表格视图共用同一份比较器，
// 两个视图的顺序必须一致，否则切换视图会看着"顺序变了"。
import { computed, ref, watch } from "vue";
import { t } from "../../../../lib/i18n";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import HighlightText from "../../../../components/ui/HighlightText.vue";
import ResourceRow from "../ResourceRow.vue";
import { LIST_ROW_GAP, LIST_ROW_H, useVirtualRows } from "../../../../composables/useVirtualRows";
import {
  modSelectKey,
  modSub,
  sortedMods,
  type ModViewEmits,
  type ModViewProps,
} from "../../types";
import { modTreeRows, type ModTreeRow } from "../../modTree";

const props = withDefaults(
  defineProps<
    ModViewProps & {
      /** 最外层的层级；本组件不再递归，只有 ModPane 直接调它，恒为 0 */
      depth?: number;
      /** 当前搜索词，已转小写；命中处加底色，空串表示没搜 */
      query?: string;
    }
  >(),
  { depth: 0, query: "" },
);
const emit = defineEmits<ModViewEmits>();

/**
 * **展开**的父条目 key（默认**收起**）
 *
 * 内置模组与改写目标都挂在父条目下面，默认铺开会把列表撑得又长又碎
 * （一个包动辄带好几个内置库，重度 mixin 的包还有几百条目标），
 * 想看的那个模组反而被挤到屏幕外。所以默认只显示父 jar，点箭头才展开。
 *
 * 存"展开的那些"而不是"收起的那些"：默认收起，空集合即初始状态。
 */
const expanded = ref<Set<string>>(new Set());

/**
 * **展开**的「改写目标」子项 key（同样默认收起）
 *
 * 目标列表单独一套状态：一个包可能带几百上千条 mixin，跟着父行一起摊开会把整屏冲掉，
 * 虚拟列表的高度与滚动位置也会一跳几千像素。所以父行展开只多出一条「改写目标」子项，
 * 点它才列具体的类。
 */
const patchesOpen = ref<Set<string>>(new Set());

/** 收起动画时长，单位 ms */
const COLLAPSE_ROW_MS = 170;

/**
 * 正在收起的那些行
 *
 * 行列表是虚拟拍的平结构，没有"容器"可以给高度过渡：收起时先把这些旧行留在列表里，
 * 逐帧把高度与不透明度压到 0（虚拟窗口那本账同步跟着，见 `rowHeights`），
 * 动画结束再从列表里拿掉。这样下面的行是滑上来的，不是一帧顶上来。
 */
const collapseAnim = ref<{ parent: string; rows: FlatRow[]; progress: number } | null>(null);

/**
 * 收起的行退场动画
 *
 * - `parentKey`: 收起的那一行（它的子项要退场）；「改写目标」子项传它自己的 key
 */
function startRowCollapse(parentKey: string) {
  const list = rows.value;
  const index = list.findIndex((row) => row.key === parentKey);
  if (index < 0) return;
  const depth = list[index].depth;
  const kids: FlatRow[] = [];
  for (let at = index + 1; at < list.length && list[at].depth > depth; at += 1) kids.push(list[at]);
  if (!kids.length) return;

  collapseAnim.value = { parent: parentKey, rows: kids, progress: 1 };
  const from = performance.now();
  const step = () => {
    const left = performance.now() - from;
    const progress = Math.max(0, 1 - left / COLLAPSE_ROW_MS);
    if (progress > 0) {
      collapseAnim.value = { parent: parentKey, rows: kids, progress };
      // 逐帧报一次布局变化：下面的分组按新高度重算行窗口
      remeasure();
      requestAnimationFrame(step);
    } else {
      collapseAnim.value = null;
    }
  };
  requestAnimationFrame(step);
}

/** 展开 / 收起这一行；收起时把目标列表一起收掉，下次展开仍是默认收起 */
function toggleExpand(key: string) {
  const next = new Set(expanded.value);
  if (next.has(key)) {
    // 先把要退场的子行记下来，再改状态（改完它们就从拍平结果里消失了）
    startRowCollapse(key);
    next.delete(key);
    const patches = new Set(patchesOpen.value);
    patches.delete(key);
    patchesOpen.value = patches;
  } else {
    // 上一次收起的退场动画还没跑完就又展开了：直接作废，免得多出几行幽灵行
    collapseAnim.value = null;
    next.add(key);
    expandPending = true;
  }
  expanded.value = next;
}

/** 这一行是不是展开的 */
function isOpen(key: string): boolean {
  return expanded.value.has(key);
}

/** 「改写目标」子项是不是展开的 */
function isPatchesOpen(key: string): boolean {
  return patchesOpen.value.has(key);
}

/** 展开 / 收起某个模组的改写目标列表 */
function togglePatches(key: string) {
  const next = new Set(patchesOpen.value);
  if (next.has(key)) {
    // 「改写目标」那一行自己的 key 带 |patches 后缀，退场的子行挂在它下面
    startRowCollapse(key + "|patches");
    next.delete(key);
  } else {
    collapseAnim.value = null;
    next.add(key);
    expandPending = true;
  }
  patchesOpen.value = next;
}

/** 刚点开了一行：下一次行列表变化算"展开"，新出现的行播进场动画 */
let expandPending = false;

/** 这次展开出来的行：挂一段进场动画；滚动时才挂上来的行不在里面，不会闪 */
const entering = ref<Set<string>>(new Set());
let enterTimer = 0;

/**
 * 给这批新出现的行挂进场标记
 *
 * - `keys`: 行的 key
 */
function markEntering(keys: string[]) {
  if (!keys.length) return;
  entering.value = new Set([...entering.value, ...keys]);
  window.clearTimeout(enterTimer);
  // 动画结束后清掉标记：之后滚动挂上来的行不该再播一次
  enterTimer = window.setTimeout(() => {
    entering.value = new Set();
  }, 420);
}

/** 列表卡片的拼框与退场动画状态。 */
type FlatRow = ModTreeRow & {
  /** 这一行在展开卡片里的位置 */
  box?: "start" | "end" | "mid" | "single";
  /** 下一行紧贴着它（吃掉行间距） */
  tight?: boolean;
  /** 正在收起：这一行是"退场"的旧行，高度按 progress 缩到 0 */
  collapse?: boolean;
  /** 收起动画里的当前高度，单位 px */
  height?: number;
  /** 收起动画里的当前不透明度 */
  alpha?: number;
};

/** 本视图要渲染的全部行，已按展开状态拍平；收起中的旧行先留在原位 */
const rows = computed(() => {
  const out: FlatRow[] = modTreeRows(props.items, props.depth, {
    query: props.query, order: sortedMods, isOpen, isPatchesOpen,
  });
  for (const [index, row] of out.entries()) {
    const childFollows = (out[index + 1]?.depth ?? props.depth) > props.depth;
    if (row.depth === props.depth && !childFollows) continue;
    row.tight = childFollows;
    row.box = row.depth === props.depth ? "start" : childFollows ? "mid" : "end";
  }

  const anim = collapseAnim.value;
  if (anim) {
    const at = out.findIndex((row) => row.key === anim.parent);
    // 父行可能已经被别的操作带走了：找不到就这场动画作废
    if (at >= 0) {
      // 父行继续当框头，退场的行接着把它那个框填满、最后一行收口
      out[at] = { ...out[at], box: "start", tight: true, collapse: false };
      const leaving = anim.rows.map((row, index) => {
        const base = row.kind === "mod" ? LIST_ROW_H : PATCH_ROW_H;
        const last = index === anim.rows.length - 1;
        return {
          ...row,
          box: last ? ("end" as const) : ("mid" as const),
          tight: !last,
          collapse: true,
          height: Math.round(base * anim.progress),
          alpha: anim.progress,
        };
      });
      out.splice(at + 1, 0, ...leaving);
    }
  }
  return out;
});

/** 行容器：虚拟化要量它在滚动容器里的位置 */
const root = ref<HTMLElement | null>(null);

/**
 * 改写目标行的高度，单位 px
 *
 * 纯文本一行，比模组行矮一半；与 resource.css 的 .mod-patch-row 同源。
 * 两种行高不同，所以下面走变高口径（heights），不能只给 rowH。
 */
const PATCH_ROW_H = 30;

/** 每行高度，顺序与 rows 一致；变高虚拟窗口按它算。用 computed 缓存，滚动时不必每帧重算 */
const rowHeights = computed(() =>
  rows.value.map((row) => {
    const base = row.kind === "mod" ? LIST_ROW_H : PATCH_ROW_H;
    // 收起中的行按逐帧高度算；紧贴着下一行的在 CSS 里用负外边距吃掉行间距，这里同步扣掉
    if (row.collapse) {
      const height = Math.max(0, row.height ?? 0);
      return row.tight ? Math.max(0, height - LIST_ROW_GAP) : height;
    }
    return row.tight ? base - LIST_ROW_GAP : base;
  }),
);

/** 只渲染视口附近那几行，其余用上下内边距撑住高度，见 useVirtualRows */
const { start, end, padTop, padBottom, remeasure } = useVirtualRows({
  root,
  count: () => rows.value.length,
  rowH: LIST_ROW_H,
  heights: () => rowHeights.value,
  gap: LIST_ROW_GAP,
});

/** 上一次渲染出来的行 key：用来找出"这次新出现的行" */
let lastKeys = new Set<string>();

// 展开 / 收起会改变本组乃至后面各组的高度：报一次布局变化（flush: post，等 DOM 更新完）。
// 顺带给"点开那一行"带出来的行挂进场标记：滚动时才挂上来的行不播，不会闪。
// **必须 immediate**：不先记一次首屏的 key，第一次展开时整屏都会被当成"新出现的行"闪一下
watch(
  rows,
  (list) => {
    const next = new Set(list.map((row) => row.key));
    if (expandPending) {
      expandPending = false;
      markEntering(list.filter((row) => !lastKeys.has(row.key)).map((row) => row.key));
    }
    lastKeys = next;
    remeasure();
  },
  { flush: "post", immediate: true },
);

/**
 * 收起中的行的内联样式：高度与不透明度跟着动画走
 *
 * - `row`: 要样式的那一行
 */
function collapseStyle(row: FlatRow) {
  return row.collapse ? { height: `${row.height ?? 0}px`, opacity: row.alpha ?? 1 } : {};
}

/** 这一回真正渲染出来的那几行 */
const visible = computed(() => rows.value.slice(start.value, end.value));
</script>
<template>
  <div ref="root" class="mod-rows" :style="{ paddingTop: padTop + 'px', paddingBottom: padBottom + 'px' }">
    <template v-for="row in visible" :key="row.key">
      <ResourceRow v-if="row.kind === 'mod'" :icon="row.item.icon"
        :name="row.item.name || row.item.file" :depth="row.depth"
        :style="collapseStyle(row)"
        :class="[row.box ? 'box-' + row.box : '', {
          'row-selected': selectedKeys.has(modSelectKey(row.item)),
          'row-anchor': modSelectKey(row.item) === anchorKey,
          'row-selecting': selectedKeys.size > 0,
          'row-enter': entering.has(row.key),
          'row-collapsing': row.collapse,
        }]" @contextmenu.prevent="row.depth === depth ? emit('item-menu', { event: $event, item: row.item }) : undefined"
        @click="row.depth === depth ? emit('row-click', { event: $event, item: row.item }) : undefined">
        <!--
          多选勾选框（最左边）。只在顶层给 —— 内置模组装在父 jar 里、没有独立文件，
          选中了也做不了批量操作，给个勾选框只会误导。
          内置行留一个等宽占位，父行与子行的名字才不会错位。
        -->
        <template #select>
          <input v-if="row.depth === depth" type="checkbox" class="row-select"
            :checked="selectedKeys.has(modSelectKey(row.item))" @pointerdown.stop @change.stop="emit('select', row.item)"
            @click.stop />
          <span v-else class="row-select-gap" />
        </template>
        <!-- 展开箭头：带内置模组或改写目标的行才有（子行自己没有下级）；默认收起 -->
        <template #lead>
          <button v-if="row.expandable" class="mod-tree-caret" :class="{ collapsed: !row.open }"
            :aria-expanded="row.open" v-tip="row.open ? t('resource.modCollapse') : t('resource.modExpand')"
            @click.stop="toggleExpand(row.key)" @pointerdown.stop>
            <GlyphIcon name="chevron-down" :size="13" :weight="2.4" />
          </button>
          <!-- 占位：没有下级的行也留出箭头的宽度，图标与名字才能对齐成一列 -->
          <span v-else class="mod-tree-caret-placeholder" />
        </template>

        <template #badges>
          <!-- 支持的加载器：一个包可能不止一个，比如同时带 fabric.mod.json 与 mods.toml；
                后端已汇总去重，这里逐个显示 -->
          <span v-for="name in row.item.loaders" :key="name"
            class="badge badge-loader shrink-0 rounded-[6px] bg-accent-soft px-[7px] py-px text-caption text-accent"
            :class="'loader-' + name">{{ t('resource.loader.' + name) }}</span>
          <!-- 内置的**库**：没有模组元数据的内置 jar，如 asm / mixinextras。
                它不是模组，标出来免得和真正的内置模组混在一起 -->
          <span v-if="row.item.library"
            class="badge badge-dim shrink-0 rounded-[6px] px-[7px] py-px text-caption">{{
              t("resource.modLibrary") }}</span>
          <span v-if="row.depth > depth && !row.item.library"
            class="badge badge-dim shrink-0 rounded-[6px] px-[7px] py-px text-caption">
            {{ t("resource.modBuiltin") }}
          </span>
          <!-- 「有更新」：这一行在更新检查里查到了新版本；下不下由更新弹窗里勾 -->
          <span v-if="updateKeys?.has(modSelectKey(row.item))"
            class="badge shrink-0 rounded-[6px] bg-accent-soft px-[7px] py-px text-caption text-accent">{{
              t("resource.updateTag") }}</span>
          <span v-if="row.item.disable"
            class="badge badge-off shrink-0 rounded-[6px] bg-accent-soft px-[7px] py-px text-caption text-accent">{{
              t("resource.modDisabled") }}</span>
          <span v-if="row.item.fail"
            class="badge badge-red shrink-0 rounded-[6px] bg-accent-soft px-[7px] py-px text-caption text-accent">{{
              t("resource.modFail") }}</span>
          <!-- 「CoreMod」：会改写别的类的字节码 —— 老式 coremod 或声明了 ASM 变压器 -->
          <span v-if="row.item.coremod"
            class="badge shrink-0 rounded-[6px] bg-accent-soft px-[7px] py-px text-caption text-accent"
            v-tip="t('resource.modCoreModTip')">{{ t("resource.modCore") }}</span>
          <!-- 「Mixin N」：注入了多少处目标，展开这一行就能看到具体的类与方法 -->
          <span v-if="row.item.mixins.length" class="badge badge-mixin shrink-0 rounded-[6px] px-[7px] py-px text-caption"
            v-tip="t('resource.modMixinTip')">{{ t("resource.modMixin", { n: row.item.mixins.length }) }}</span>
          <span v-if="row.depth === depth && row.item.jarInJar.length"
            class="badge badge-dim shrink-0 rounded-[6px] px-[7px] py-px text-caption">
            {{ t("resource.modBuiltinCount", { n: row.item.jarInJar.length }) }}
          </span>
          <!-- 硬链接：这个文件还挂在公共目录，多实例共用一份 -->
          <span v-if="row.item.hardLink"
            class="badge badge-link shrink-0 rounded-[6px] bg-accent-soft px-[7px] py-px text-caption text-accent">{{
              t("resource.modHardLink") }}</span>
        </template>

        <!--
          备注放在**名字前面**（用户要求），并且不再加「备注：」前缀 ——
          位置本身就说明了它是备注，前缀只是白占宽度。标签样式由 .mod-note 给
        -->
        <!-- 名字走插槽才过得了高亮：命中搜索词的那一段加底色，与设置搜索同一观感 -->
        <template #name>
          <HighlightText :text="row.item.name || row.item.file" :query="props.query" />
        </template>

        <template #name-lead>
          <span v-if="row.item.note" class="mod-note" :title="row.item.note">{{ row.item.note }}</span>
        </template>

        <!-- 副标题含版本 / 作者 / 文件名：搜索也匹配文件名，命中就在这儿着色 -->
        <template #sub>
          <span>
            <HighlightText :text="modSub(row.item)" :query="props.query" />
          </span>
        </template>
      </ResourceRow>

      <!-- 「改写目标」子项：默认收起。一个包可能有几百上千条 mixin，摊开会把整屏冲掉，
           所以父行展开只多出这一条，点它才列具体的类 -->
      <div v-else-if="row.kind === 'patches'"
        :class="['mod-patch-row', 'mod-patch-list', row.box ? 'box-' + row.box : '',
          { 'row-enter': entering.has(row.key), 'row-collapsing': row.collapse }]"
        :style="{ '--row-depth': row.depth, ...collapseStyle(row) }" @click="togglePatches(row.parent)">
        <span class="row-select-gap" />
        <button class="mod-tree-caret" :class="{ collapsed: !row.open }" :aria-expanded="row.open"
          @click.stop="togglePatches(row.parent)">
          <GlyphIcon name="chevron-down" :size="12" :weight="2.4" />
        </button>
        <span class="mod-patch-list-label">{{ t("resource.modPatchList") }}</span>
        <span class="mod-patch-list-count">{{ row.count }}</span>
      </div>

      <!-- 展开出来的改写目标：一行一个目标类，不可点、不可选，只用来读 -->
      <div v-else :class="['mod-patch-row', row.box ? 'box-' + row.box : '',
        { 'row-enter': entering.has(row.key), 'row-collapsing': row.collapse }]"
        :style="{ '--row-depth': row.depth, ...collapseStyle(row) }" :title="row.title">
        <!-- 与同级行落在同一列：勾选框列、箭头列都留空，标签才对齐上面的图标那一列 -->
        <span class="row-select-gap" />
        <span class="mod-tree-caret-placeholder" />
        <span class="mod-patch-tag" :class="{ 'is-asm': row.asm }">
          {{ row.asm ? t("resource.patchesAsm") : t("resource.patchesMixin") }}
        </span>
        <!-- 目标类与方法也走高亮：按 mixin 目标搜出来时，命中的是这里而不是行名 -->
        <span class="mod-patch-target">
          <HighlightText :text="row.name" :query="props.query" />
        </span>
        <span v-if="row.detail" class="mod-patch-detail">
          <HighlightText :text="row.detail" :query="props.query" />
        </span>
      </div>
    </template>
  </div>
</template>
