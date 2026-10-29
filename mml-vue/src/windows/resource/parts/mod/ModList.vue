<script setup lang="ts">
// 模组视图之一：列表，每行是图标 + 名称/徽标 + 副标题 + 操作，与其它分类的行同一观感
//
// **内置模组（jar-in-jar）就展开在同一列里**：有下级的行前面多一个展开箭头，
// 展开后按 22px 缩进显示下一层（那是这个 jar 里装的模组）。原先这是独立的「树形」视图，
// 两者已合并 —— 列表本来就要显示"这个包内置了什么"，分成两个视图等于让用户在
// "看得见层级"和"看得见图标/副标题"之间二选一。
//
// **行是拍平了渲染的**（父行 + 已展开的内置行），外面再包一层 `.mod-rows` 交给
// composables/useVirtualRows 做**行虚拟化**：几百个模组时只渲染视口附近那几行。
// 不这么做的话每条都要建一整棵 DOM 子树、还要解码一张图标，内存与首帧都吃不消。
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
  modRowKey,
  modSelectKey,
  modSub,
  sortedMods,
  type ModViewEmits,
  type ModViewProps,
} from "../../types";

const props = withDefaults(
  defineProps<
    ModViewProps & {
      /** 最外层的层级；本组件不再递归，只有 ModPane 直接调它，恒为 0 */
      depth?: number;
      /** 当前搜索词，命中处加底色；空串表示没搜 */
      query?: string;
    }
  >(),
  { depth: 0, query: "" },
);
const emit = defineEmits<ModViewEmits>();

/**
 * **展开**的父条目 key（默认**收起**）
 *
 * 内置模组（jar-in-jar）装在父 jar 里、多数时候只是"顺带一提"：
 * 默认铺开会把列表撑得又长又碎（一个包动辄带好几个内置库），
 * 想看的那个模组反而被挤到屏幕外。所以默认只显示父 jar，点箭头才展开。
 *
 * 存"展开的那些"而不是"收起的那些"：默认收起，空集合即初始状态。
 */
const expanded = ref<Set<string>>(new Set());

/** 展开 / 收起这一行 */
function toggleExpand(key: string) {
  const next = new Set(expanded.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  expanded.value = next;
}

/** 这一行是不是展开的 */
function isOpen(key: string): boolean {
  return expanded.value.has(key);
}

/** 拍平后的一行 */
interface FlatRow {
  /** 这一行的模组条目 */
  item: ModViewProps["items"][number];
  /** 层级：与最外层同层是 0，每往下嵌一层 +1（缩进与"给不给操作入口"都看它） */
  depth: number;
}

/**
 * 拍平：父行 + **已展开**的内置行，每一层各自排序
 *
 * 内置的行跟着父行走，所以展开只会让后面那些行的下标往后挪，不会打乱父行的顺序。
 *
 * - `items`: 这一层要显示的模组
 * - `depth`: 这一层的层级
 * - `out`: 结果累加到这里
 */
function flatten(items: ModViewProps["items"], depth: number, out: FlatRow[]) {
  for (const item of sortedMods(items)) {
    out.push({ item, depth });
    if (item.jarInJar.length && isOpen(modRowKey(item))) {
      flatten(item.jarInJar, depth + 1, out);
    }
  }
}

/** 本视图要渲染的全部行，已按展开状态拍平 */
const rows = computed(() => {
  const out: FlatRow[] = [];
  flatten(props.items, props.depth, out);
  return out;
});

/** 行容器：虚拟化要量它在滚动容器里的位置 */
const root = ref<HTMLElement | null>(null);

/** 只渲染视口附近那几行，其余用上下内边距撑住高度，见 useVirtualRows */
const { start, end, padTop, padBottom, remeasure } = useVirtualRows({
  root,
  count: () => rows.value.length,
  rowH: LIST_ROW_H,
  gap: LIST_ROW_GAP,
});

// 展开 / 收起内置模组会改变本组乃至后面各组的高度：报一次布局变化（flush: post，等 DOM 更新完）
watch(rows, () => remeasure(), { flush: "post" });

/** 这一回真正渲染出来的那几行 */
const visible = computed(() => rows.value.slice(start.value, end.value));
</script>

<template>
  <div ref="root" class="mod-rows"
    :style="{ paddingTop: padTop + 'px', paddingBottom: padBottom + 'px' }">
    <ResourceRow v-for="row in visible" :key="modRowKey(row.item)" :icon="row.item.icon"
      :name="row.item.name || row.item.file" :depth="row.depth" :class="{
        'row-selected': selectedKeys.has(modSelectKey(row.item)),
        'row-anchor': modSelectKey(row.item) === anchorKey,
        'row-selecting': selectedKeys.size > 0,
      }" @contextmenu.prevent="row.depth === depth ? emit('item-menu', { event: $event, item: row.item }) : undefined"
      @click="row.depth === depth ? emit('row-click', { event: $event, item: row.item }) : undefined">
      <!--
        多选勾选框（最左边）。只在顶层给 —— 内置模组装在父 jar 里、没有独立文件，
        选中了也做不了批量操作，给个勾选框只会误导。
        内置行留一个等宽占位，父行与子行的名字才不会错位。
      -->
      <template #select>
        <input v-if="row.depth === depth" type="checkbox" class="row-select"
          :checked="selectedKeys.has(modSelectKey(row.item))" @pointerdown.stop
          @change.stop="emit('select', row.item)" @click.stop />
        <span v-else class="row-select-gap" />
      </template>
      <!-- 展开箭头：只有带内置模组的行才有（内置行自己没有下级）；默认收起 -->
      <template #lead>
        <button v-if="row.item.jarInJar.length" class="mod-tree-caret"
          :class="{ collapsed: !isOpen(modRowKey(row.item)) }" v-tip="isOpen(modRowKey(row.item))
            ? t('resource.modBuiltinCollapse')
            : t('resource.modBuiltinExpand')
            " @click.stop="toggleExpand(modRowKey(row.item))" @pointerdown.stop>
          <GlyphIcon name="chevron-down" :size="13" :weight="2.4" />
        </button>
        <!-- 占位：没有下级的行也留出箭头的宽度，图标与名字才能对齐成一列 -->
        <span v-else class="mod-tree-caret-placeholder" />
      </template>

      <template #badges>
        <!-- 支持的加载器：一个包可能不止一个，比如同时带 fabric.mod.json 与 mods.toml；
              后端已汇总去重，这里逐个显示 -->
        <span v-for="name in row.item.loaders" :key="name" class="badge badge-dim">{{ t(`resource.loader.${name}`) }}</span>
        <!-- 内置的**库**：没有模组元数据的内置 jar，如 asm / mixinextras。
              它不是模组，标出来免得和真正的内置模组混在一起 -->
        <span v-if="row.item.library" class="badge badge-dim">{{ t("resource.modLibrary") }}</span>
        <span v-if="row.depth > depth && !row.item.library" class="badge badge-dim">
          {{ t("resource.modBuiltin") }}
        </span>
        <!-- 「有更新」：这一行在更新检查里查到了新版本；下不下由更新弹窗里勾 -->
        <span v-if="updateKeys?.has(modSelectKey(row.item))" class="badge">{{ t("resource.updateTag") }}</span>
        <span v-if="row.item.disable" class="badge badge-off">{{ t("resource.modDisabled") }}</span>
        <span v-if="row.item.fail" class="badge badge-red">{{ t("resource.modFail") }}</span>
        <span v-if="row.item.core" class="badge">{{ t("resource.modCore") }}</span>
        <span v-if="row.depth === depth && row.item.jarInJar.length" class="badge">
          {{ t("resource.modBuiltinCount", { n: row.item.jarInJar.length }) }}
        </span>
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
        <span><HighlightText :text="modSub(row.item)" :query="props.query" /></span>
      </template>
    </ResourceRow>
  </div>
</template>
