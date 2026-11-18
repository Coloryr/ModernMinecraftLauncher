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
// 目标行与模组行同为 56px 高，虚拟窗口按同一行高算，不会错位。
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
  patchKindLabel,
  sortedMods,
  type ModViewEmits,
  type ModViewProps,
} from "../../types";
import type { ModItemDto, ModMixinTargetDto } from "../../../../lib/bindings";

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

/** 展开后的一行：模组行，或这个模组改写的一个目标类 */
type FlatRow = ModFlatRow | PatchFlatRow;

/** 模组行 */
interface ModFlatRow {
  /** 行类别 */
  kind: "mod";
  /** v-for 的 key */
  key: string;
  /** 这一行的模组条目 */
  item: ModItemDto;
  /** 层级：与最外层同层是 0，每往下嵌一层 +1（缩进与"给不给操作入口"都看它） */
  depth: number;
}

/** 改写目标行：一个目标类一行 */
interface PatchFlatRow {
  /** 行类别 */
  kind: "patch";
  /** v-for 的 key，带父条目前缀，避免不同模组的目标类重名 */
  key: string;
  /** 层级，规则同模组行 */
  depth: number;
  /** 是 ASM 变压器，而不是 mixin 注入 */
  asm: boolean;
  /** 目标类名；ASM 没声明目标时是变压器名 */
  name: string;
  /** 方法 / 注入方式摘要，一行放不下就省略 */
  detail: string;
  /** 悬停提示：完整的目标与方法 */
  title: string;
}

/**
 * 把一个模组的改写目标整理成行
 *
 * 同一目标类的多条注入合成一行：重度包有几百条，按类摊开还能读，方法写在右侧；
 * 搜索命中方法名时同样保留这一行，所以搜 "renderItem" 也能找到那个类。
 *
 * - `item`: 模组条目
 * - `q`: 已转小写的搜索词，空串表示没在搜
 *
 * # 返回值
 *
 * 目标行；没命中搜索词的那些已经滤掉
 */
function patchRowsOf(item: ModItemDto, q: string): PatchFlatRow[] {
  const groups = new Map<string, ModMixinTargetDto[]>();
  for (const target of item.mixins) {
    const bucket = groups.get(target.target);
    if (bucket) bucket.push(target);
    else groups.set(target.target, [target]);
  }

  const rows: PatchFlatRow[] = [];
  for (const [name, list] of groups) {
    const detail = list
      .map((target) => target.method + " · " + patchKindLabel(target.kind))
      .join("，");
    // 悬停才给"谁改的"：列表里一行放不下 mixin 类名，但排查冲突时要看得到
    const sources = [...new Set(list.map((target) => target.mixin))].join("、");
    const hit = (
      name + " " + list.map((target) => target.method + " " + target.mixin).join(" ")
    ).toLowerCase();
    if (q && !hit.includes(q)) continue;
    rows.push({
      kind: "patch",
      key: "mixin|" + name,
      depth: 0,
      asm: false,
      name,
      detail,
      title: name + "：" + detail + (sources ? "；来自 " + sources : ""),
    });
  }

  for (const target of item.asm) {
    const name = target.target || target.transformer;
    const detail = [target.method, target.source].filter(Boolean).join(" · ");
    const hit = (
      target.source +
      " " +
      target.transformer +
      " " +
      target.target +
      " " +
      target.method
    ).toLowerCase();
    if (q && !hit.includes(q)) continue;
    rows.push({
      kind: "patch",
      key: "asm|" + target.source + "|" + target.transformer + "|" + name,
      depth: 0,
      asm: true,
      name,
      detail,
      title: target.transformer + " · " + detail,
    });
  }

  return rows;
}

/**
 * 拍平：父行 + **已展开**的内置行与目标行，每一层各自排序
 *
 * 内置的行跟着父行走，所以展开只会让后面那些行的下标往后挪，不会打乱父行的顺序。
 * 搜索时命中的目标行直接摊开：用户搜的就是那些类名，还要再点一次箭头才看得见太绕。
 *
 * - `items`: 这一层要显示的模组
 * - `depth`: 这一层的层级
 * - `out`: 结果累加到这里
 */
function flatten(items: ModItemDto[], depth: number, out: FlatRow[]) {
  for (const item of sortedMods(items)) {
    const key = modRowKey(item);
    out.push({ kind: "mod", key, item, depth });

    // 收起且没在搜索时不用算改写目标，省掉几百个模组的目标分组
    const open = isOpen(key);
    if (!open && !props.query) continue;

    const patches = patchRowsOf(item, props.query);
    if (!open && !patches.length) continue;

    if (item.jarInJar.length) {
      flatten(item.jarInJar, depth + 1, out);
    }
    for (const patch of patches) {
      out.push({ ...patch, key: key + "|" + patch.key, depth: depth + 1 });
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

/**
 * 改写目标行的高度，单位 px
 *
 * 纯文本一行，比模组行矮一半；与 resource.css 的 .mod-patch-row 同源。
 * 两种行高不同，所以下面走变高口径（heights），不能只给 rowH。
 */
const PATCH_ROW_H = 30;

/** 每行高度，顺序与 rows 一致；变高虚拟窗口按它算。用 computed 缓存，滚动时不必每帧重算 */
const rowHeights = computed(() =>
  rows.value.map((row) => (row.kind === "patch" ? PATCH_ROW_H : LIST_ROW_H)),
);

/** 只渲染视口附近那几行，其余用上下内边距撑住高度，见 useVirtualRows */
const { start, end, padTop, padBottom, remeasure } = useVirtualRows({
  root,
  count: () => rows.value.length,
  rowH: LIST_ROW_H,
  heights: () => rowHeights.value,
  gap: LIST_ROW_GAP,
});

// 展开 / 收起会改变本组乃至后面各组的高度：报一次布局变化（flush: post，等 DOM 更新完）
watch(rows, () => remeasure(), { flush: "post" });

/** 这一回真正渲染出来的那几行 */
const visible = computed(() => rows.value.slice(start.value, end.value));
</script>
<template>
  <div ref="root" class="mod-rows" :style="{ paddingTop: padTop + 'px', paddingBottom: padBottom + 'px' }">
    <template v-for="row in visible" :key="row.key">
      <ResourceRow v-if="row.kind === 'mod'" :icon="row.item.icon"
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
            :checked="selectedKeys.has(modSelectKey(row.item))" @pointerdown.stop @change.stop="emit('select', row.item)"
            @click.stop />
          <span v-else class="row-select-gap" />
        </template>
        <!-- 展开箭头：带内置模组或改写目标的行才有（子行自己没有下级）；默认收起 -->
        <template #lead>
          <button v-if="row.item.jarInJar.length || row.item.mixins.length || row.item.asm.length"
            class="mod-tree-caret" :class="{ collapsed: !isOpen(modRowKey(row.item)) }"
            v-tip="isOpen(modRowKey(row.item)) ? t('resource.modCollapse') : t('resource.modExpand')"
            @click.stop="toggleExpand(modRowKey(row.item))" @pointerdown.stop>
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

      <!-- 展开出来的改写目标：一行一个目标类，不可点、不可选，只用来读 -->
      <div v-else class="mod-patch-row" :style="{ '--row-depth': row.depth }" :title="row.title">
        <span class="mod-patch-tag" :class="{ 'is-asm': row.asm }">
          {{ row.asm ? t("resource.patchesAsm") : t("resource.patchesMixin") }}
        </span>
        <span class="mod-patch-target">{{ row.name }}</span>
        <span v-if="row.detail" class="mod-patch-detail">{{ row.detail }}</span>
      </div>
    </template>
  </div>
</template>