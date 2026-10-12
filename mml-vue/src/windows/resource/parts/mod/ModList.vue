<script setup lang="ts">
// 模组视图之一：列表，每行是图标 + 名称/徽标 + 副标题 + 操作，与其它分类的行同一观感
//
// **内置模组（jar-in-jar）就展开在同一列里**：有下级的行前面多一个展开箭头，
// 展开后按 22px 缩进显示下一层（那是这个 jar 里装的模组）。原先这是独立的「树形」视图，
// 两者已合并 —— 列表本来就要显示"这个包内置了什么"，分成两个视图等于让用户在
// "看得见层级"和"看得见图标/副标题"之间二选一。
//
// 自引用递归：本组件按文件名引用自己渲染下一层（Vue SFC 支持），
// 所以 jar 里套 jar 也能一层层展开，不用写死层数。
//
// 内置模组装在父 jar 里，**没有独立文件**：只有展示，不给操作按钮，也不参与归组
// （归组的是父 jar）。
//
// 排序：**按 modid 正序**，见 types.ts 的 `sortedMods` —— 与表格视图共用同一份比较器，
// 两个视图的顺序必须一致，否则切换视图会看着"顺序变了"。
import { computed, ref } from "vue";
import { t } from "../../../../lib/i18n";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import ResourceRow from "../ResourceRow.vue";
import {
  modRowKey,
  modSub,
  sortedMods,
  type ModViewEmits,
  type ModViewProps,
} from "../../types";
import { modSelectKey } from "../../types";

const props = withDefaults(
  defineProps<
    ModViewProps & {
      /** 递归层级；0 是顶层，每层 +1，只用来缩进与决定给不给操作入口 */
      depth?: number;
    }
  >(),
  { depth: 0 },
);
const emit = defineEmits<ModViewEmits>();

/** 按 modid 正序排好的本层条目，递归渲染的每一层各自排 */
const rows = computed(() => sortedMods(props.items));

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

/** 展开状态用的键：顶层认 sha1，内置条目没有 sha1 时退回名字 */
function keyOf(item: ModViewProps["items"][number]): string {
  return modRowKey(item);
}
</script>

<template>
  <template v-for="item in rows" :key="keyOf(item)">
    <ResourceRow :icon="item.icon" :name="item.name || item.file" :depth="depth" :class="{
      'row-selected': selectedKeys.has(modSelectKey(item)),
      'row-anchor': modSelectKey(item) === anchorKey,
      'row-selecting': selectedKeys.size > 0,
    }" @contextmenu.prevent="depth === 0 ? emit('item-menu', { event: $event, item }) : undefined"
      @click="depth === 0 ? emit('row-click', { event: $event, item }) : undefined">
      <!--
        多选勾选框（最左边）。只在顶层给 —— 内置模组装在父 jar 里、没有独立文件，
        选中了也做不了批量操作，给个勾选框只会误导。
        内置行留一个等宽占位，父行与子行的名字才不会错位。
      -->
      <template #select>
        <input v-if="depth === 0" type="checkbox" class="row-select" :checked="selectedKeys.has(modSelectKey(item))"
          @pointerdown.stop @change.stop="emit('select', item)" @click.stop />
        <span v-else class="row-select-gap" />
      </template>
      <!-- 展开箭头：只有带内置模组的行才有（内置行自己没有下级）；默认收起 -->
      <template #lead>
        <button v-if="item.jarInJar.length" class="mod-tree-caret" :class="{ collapsed: !isOpen(keyOf(item)) }" v-tip="isOpen(keyOf(item))
            ? t('resource.modBuiltinCollapse')
            : t('resource.modBuiltinExpand')
          " @click.stop="toggleExpand(keyOf(item))" @pointerdown.stop>
          <GlyphIcon name="chevron-down" :size="13" :weight="2.4" />
        </button>
        <!-- 占位：没有下级的行也留出箭头的宽度，图标与名字才能对齐成一列 -->
        <span v-else class="mod-tree-caret-placeholder" />
      </template>

      <template #badges>
        <!-- 支持的加载器：一个包可能不止一个，比如同时带 fabric.mod.json 与 mods.toml；
             后端已汇总去重，这里逐个显示 -->
        <span v-for="name in item.loaders" :key="name" class="badge badge-dim">{{ t(`resource.loader.${name}`) }}</span>
        <!-- 内置的**库**：没有模组元数据的内置 jar，如 asm / mixinextras。
             它不是模组，标出来免得和真正的内置模组混在一起 -->
        <span v-if="item.library" class="badge badge-dim">{{ t("resource.modLibrary") }}</span>
        <span v-if="depth > 0 && !item.library" class="badge badge-dim">
          {{ t("resource.modBuiltin") }}
        </span>
        <span v-if="item.disable" class="badge badge-off">{{ t("resource.modDisabled") }}</span>
        <span v-if="item.fail" class="badge badge-red">{{ t("resource.modFail") }}</span>
        <span v-if="item.core" class="badge">{{ t("resource.modCore") }}</span>
        <span v-if="depth === 0 && item.jarInJar.length" class="badge">
          {{ t("resource.modBuiltinCount", { n: item.jarInJar.length }) }}
        </span>
      </template>

      <!--
        备注放在**名字前面**（用户要求），并且不再加「备注：」前缀 ——
        位置本身就说明了它是备注，前缀只是白占宽度。标签样式由 .mod-note 给
      -->
      <template #name-lead>
        <span v-if="item.note" class="mod-note" :title="item.note">{{ item.note }}</span>
      </template>

      <template #sub>
        <span>{{ modSub(item) }}</span>
      </template>
    </ResourceRow>

    <!-- 内置模组：递归展开下一层，默认收起，见 expanded 的说明 -->
    <ModList v-if="item.jarInJar.length && isOpen(keyOf(item))" :items="item.jarInJar" :busy="busy"
      :selected-keys="selectedKeys" :depth="depth + 1" @toggle="emit('toggle', $event)"
      @remove="emit('remove', $event)" @note="emit('note', $event)" @open-folder="emit('open-folder', $event)"
      @select="emit('select', $event)" />
  </template>
</template>
