<script setup lang="ts">
// 模组视图之二：表格（树状：分组名是一级节点）
//
// 两棵树叠在一起：
//   - **一级节点 = 分组行**（折叠箭头 + 组名 + 数量）：收起时整组的模组行都不渲染；
//   - **二级 = 模组行**；模组自带的内置模组（jar-in-jar）再往下缩进一级。
//
// 左起三列就是"树"：**缩进 / 折叠箭头**（都在最左，用户要求）｜启用勾选框｜名字。
// 箭头一律放在最左边那一列，层级靠 16px 一级的缩进表达：
//
//     > 分组名
//       > [√] 模组名
//         > 内置模组名
//             内置的内置模组名
//
// 列宽与排序：
//   - 列宽**由 JS 给**，`MOD_COLS` 一处定义 + 用户拖出来的宽度，所以整表是**固定宽**：
//     总宽超出容器时由外层 `.item-list` 横向滚动，不再把列挤成省略号；
//   - **启用右边的每一列**都能拖右边缘改宽度、点表头在「正序 → 倒序 → 不排序」之间切换，
//     这是用户要求；不排序时回到与列表视图一致的 modid 正序。
//
// 列序（14 列 + 多选）：
//   分组名 | 启用 | 名字 | 备注 | modid | 版本 | 加载器类型 | 加载侧
//          | 下载源 | 项目编号 | 文件编号 | 路径 | 作者 | 网页链接
//   - 备注紧挨名字右边；没有操作列，删除 / 打开文件夹在列表视图里，备注双击就地编辑。
import { computed, nextTick, ref } from "vue";
import { t } from "../../../../lib/i18n";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import type { ModItemDto } from "../../../../lib/bindings";
import {
  loaderNames,
  MIN_COL_WIDTH,
  MOD_COLS,
  orderedMods,
  sideName,
  sourceName,
  type ModCol,
  type ModSort,
} from "../../modColumns";
import { modRowKey, modSelectKey, type ModViewEmits, type ModViewProps } from "../../types";

const props = withDefaults(
  defineProps<
    ModViewProps & {
      /** 一级节点的名字（分组名） */
      groupLabel?: string;
      /** 该分组当前的折叠状态；false 表示收起，不渲染模组行 */
      groupOpen?: boolean;
      /** 是不是自建分组；只有自建分组能改名 / 删 */
      custom?: boolean;
      /** 分组键：自建分组是组名，状态分组是带前缀的内置键 */
      groupKey?: string;
      /** 正在被拖动排序的分组键，整块淡出 */
      draggingGroup?: string | null;
      /** 递归层级；1 是模组行，>1 是内置模组行 */
      depth?: number;
      /** 不画表头；只有第一块画，其余块关掉，否则一屏里重复十几列 */
      hideHeader?: boolean;
      /** 列宽，列 key → 像素宽；状态在外层 `useResourceView`，所有分组共用一份 */
      colWidths: Record<string, number>;
      /** 表头排序状态，`null` 表示不排序；同上，所有分组共用 */
      sort: ModSort | null;
    }
  >(),
  {
    groupLabel: "",
    groupOpen: true,
    custom: false,
    groupKey: "",
    draggingGroup: null,
    depth: 0,
    hideHeader: false,
  },
);

const emit = defineEmits<
  ModViewEmits & {
    /** 备注就地编辑保存：条目 + 新文本；落盘由 ModPane 负责，与备注弹窗同一个命令 */
    (e: "note-save", item: ModItemDto, text: string): void;
    /** 拖列宽：列 key + 新像素宽；宽度由 useResourceView 持有并落盘，见 modColumns.ts */
    (e: "col-resize", key: string, width: number): void;
    /** 表头排序变化：正序 / 倒序 / 不排序 */
    (e: "sort-change", next: ModSort | null): void;
    /** 右键分组行：交给 ModPane 弹分组菜单 */
    (e: "group-menu", payload: {
      /** 右键事件，菜单按它的坐标定位 */
      event: MouseEvent;
      /** 分组键，与 `groupKey` 同一口径 */
      id: string;
      /** 分组名，菜单标题与确认弹窗里显示 */
      label: string;
      /** 是不是自建分组 */
      custom: boolean;
      /** 组里的模组数 */
      count: number;
    }): void;
  }
>();

// ---------- 列宽与排序：**状态都在 useResourceView** ----------
//
// 表格视图里分组是多个块、每块一个 ModTable 实例 —— 宽度 / 排序若各自持有一份，
// 改一组其它组不跟着变，列就不齐了。所以这两项由外层持有、这里只读 props + 发事件。

/** 本列的当前宽度；props 里没有就用 `MOD_COLS` 的默认值 */
function widthOf(col: ModCol): number {
  return props.colWidths[col.key] ?? col.width;
}

/** 栅格模板：24px 多选 + 各列固定宽，**全 px**；总宽超出时白底内部横向滚动 */
const gridTemplate = computed(() =>
  ["24px", ...MOD_COLS.map((col) => `${widthOf(col)}px`)].join(" "),
);

/**
 * 按住表头右边缘拖动改宽度
 *
 * 监听挂在 window 上，与 `useModDrag` 同一套做法：指针移出表头、甚至移出窗口也不会丢。
 * 每次移动只发事件，真正的宽度与落盘防抖都在 `useResourceView`。
 */
function startResize(event: PointerEvent, col: ModCol) {
  event.preventDefault();
  const startX = event.clientX;
  const startW = widthOf(col);
  const min = col.min ?? MIN_COL_WIDTH;

  const move = (e: PointerEvent) => {
    const next = Math.max(min, Math.round(startW + e.clientX - startX));
    emit("col-resize", col.key, next);
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", up);
  };

  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("pointercancel", up);
}

// ---------- 表头点击排序（正序 → 倒序 → 不排序） ----------

/** 点一下表头：没排序 → 正序 → 倒序 → 不排序，第三次点回到 modid 正序 */
function cycleSort(key: string) {
  const cur = props.sort;
  if (!cur || cur.key !== key) {
    emit("sort-change", { key, dir: "asc" });
  } else if (cur.dir === "asc") {
    emit("sort-change", { key, dir: "desc" });
  } else {
    emit("sort-change", null);
  }
}

/** 表头上的排序标记（不排序时为空串） */
function sortMark(key: string): string {
  const cur = props.sort;
  if (!cur || cur.key !== key) return "";
  return cur.dir === "asc" ? "↑" : "↓";
}



// ---------- 行的拍平（父行 + 已展开的内置模组行） ----------

/**
 * 表格里展开的行：父行 + 缩进的内部模组行拍平成一串
 *
 * 内置模组装在父 jar 里、没有独立文件：它们**没有**启用 / 备注 / 下载源这些东西，
 * 所以只在"名字"那一列缩进显示，其余列留空 —— 与它们在内核里的实际情况一致。
 */
interface TableRow {
  /** 这一行的模组条目 */
  item: ModItemDto;
  /** 层级：1 是模组行，大于 1 是内置模组行 */
  depth: number;
}

/**
 * **展开**的父条目 key；默认**收起**，与列表视图同一口径
 *
 * 内置模组（jar-in-jar）默认不铺开：一个包常带好几个内置库，全展开会把表撑得很长，
 * 想看的模组被挤到屏幕外。点名字前的箭头才展开。
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

/** 拍平：父行 + **已展开**的内置模组行（每一层各自排序） */
function flatten(items: ModItemDto[], depth: number, out: TableRow[]) {
  for (const item of orderedMods(items, props.sort)) {
    out.push({ item, depth });
    const key = modRowKey(item);
    if (item.jarInJar.length && isOpen(key)) flatten(item.jarInJar, depth + 1, out);
  }
}

/** 本表要显示的行；分组收起时为空，一级节点自己那一行由模板单独画 */
const rows = computed(() => {
  const out: TableRow[] = [];
  if (props.groupOpen) flatten(props.items, 1, out);
  return out;
});




// ---------- 备注：双击就地编辑 ----------

/**
 * 正在编辑的备注（同时只能有一个）
 *
 * **不弹输入框**：双击直接把这一格变成 `contenteditable` 的可书写区域，文字原地编辑 ——
 * 用户口径"不需要显示输入框，直接可以写文字"。提交（回车 / 失焦）时从单元格读文本，
 * 取消（Esc）把原文写回去。
 */
const editing = ref<{ key: string; item: ModItemDto } | null>(null);

/** 这一行是不是正在编辑备注 */
function isEditing(row: TableRow): boolean {
  return editing.value?.key === modRowKey(row.item);
}

/**
 * 双击备注格：这一格进入可书写状态，并把光标放到**末尾**（接着写最自然）
 *
 * 用 `event.currentTarget` 拿单元格，而不是模板 ref：`contenteditable` 只是切属性、
 * 元素不会重建，函数 ref 不会重新触发（拿到的会是旧的 / 空的）。
 */
async function startEdit(item: ModItemDto, event: MouseEvent) {
  if (props.busy) return;
  const el = event.currentTarget as HTMLElement | null;
  editing.value = { key: modRowKey(item), item };
  await nextTick(); // 等 `contenteditable` 生效，否则 span 不可聚焦
  if (!el) return;

  el.focus();
  const range = document.createRange();
  range.selectNodeContents(el);
  range.collapse(false);
  const sel = window.getSelection();
  sel?.removeAllRanges();
  sel?.addRange(range);
}

/**
 * 提交：文本没变就不落盘（省一次写配置）
 *
 * 注意**不能**指望 Vue 复原 DOM：内容没变时 `item.note` 与 vnode 文本都没变，
 * Vue 不会去动那个文本节点，用户敲进去的草稿会留在界面上 —— 得自己写回去。
 */
function commitEdit(event: Event) {
  const cur = editing.value;
  if (!cur) return;
  editing.value = null;

  const el = event.target as HTMLElement | null;
  const text = (el?.textContent ?? "").trim();
  if (text !== (cur.item.note ?? "")) {
    emit("note-save", cur.item, text);
  } else if (el) {
    // 只是多了空格之类 → 显示复原成已保存的那份
    el.textContent = cur.item.note ?? "";
  }
}

/** 取消：把原文写回去，理由同 `commitEdit` */
function cancelEdit(event: Event) {
  const cur = editing.value;
  editing.value = null;
  const el = event.target as HTMLElement | null;
  if (el) el.textContent = cur?.item.note ?? "";
}

/** 回车提交；中文输入法选字时的回车不算（`isComposing`） */
function onNoteEnter(event: KeyboardEvent) {
  if (event.isComposing) return;
  event.preventDefault();
  commitEdit(event);
  (event.target as HTMLElement | null)?.blur();
}
</script>

<template>
  <div class="mod-table" :style="{ gridTemplateColumns: gridTemplate }">
    <div class="mod-tr mod-th" :class="{ 'mod-th-hidden': hideHeader }"
      :style="{ gridTemplateColumns: gridTemplate }">
      <span class="mod-th-cell mod-th-select"></span>
      <span v-for="col in MOD_COLS" :key="col.key" class="mod-th-cell"
        :class="{ 'mod-th-center': col.center, 'mod-th-sortable': col.sortable }">
        <button v-if="col.sortable" class="mod-th-sort" :title="t('resource.colSortTip')" @click="cycleSort(col.key)">
          {{ t(col.label) }}<span v-if="sortMark(col.key)" class="mod-th-mark">{{ sortMark(col.key) }}</span>
        </button>
        <template v-else>{{ t(col.label) }}</template>
        <!-- 右边缘的拖宽把手（只在可拖的列上给） -->
        <span v-if="col.resizable" class="mod-th-resize" :title="t('resource.colResizeTip')"
          @pointerdown.stop="startResize($event, col)" />
      </span>
    </div>

    <!--
      一级节点：分组行。名字与箭头**都能点开 / 收起**（和列表视图的分组头同一口径），
      按住名字拖动 = 调整分组顺序，交给 ModPane 的 `onSectionPointerDown`。
      其余列留空占位 —— 栅格必须占满，否则这一行会整体左移、列全错位。
    -->
    <div class="mod-tr mod-group-row" :style="{ gridTemplateColumns: gridTemplate }"
      :class="{ 'mod-group-dragging': draggingGroup === groupKey }"
      @contextmenu.prevent="emit('group-menu', {
        event: $event, id: groupKey, label: groupLabel, custom, count: items.length,
      })">
      <span class="mod-cell mod-select-cell" />
      <span class="mod-cell mod-group-cell">
        <button class="mod-tree-caret" :class="{ collapsed: !groupOpen }" @click.stop="emit('toggle-group')">
          <GlyphIcon name="chevron-down" :size="13" :weight="2.4" />
        </button>
        <span class="mod-group-label" :title="groupLabel" @pointerdown.stop="emit('drag-group', $event)"
          @click.stop="emit('toggle-group')">{{ groupLabel }}</span>
        <span class="badge badge-dim">{{ t("resource.groupCountItems", { n: items.length }) }}</span>
      </span>
      <span v-for="col in MOD_COLS.slice(1)" :key="col.key" class="mod-cell" />
    </div>

    <div v-for="row in rows" :key="modRowKey(row.item)" class="mod-tr" :style="{
      gridTemplateColumns: gridTemplate,
      '--row-depth': row.depth - 1,
    }" :class="{
      'mod-dragging': !!row.item.sha1 && draggingKey === row.item.sha1,
      'mod-selected': selectedKeys.has(modSelectKey(row.item)),
      'mod-anchor': modSelectKey(row.item) === anchorKey,
      'mod-selecting': selectedKeys.size > 0,
      'mod-nested': row.depth > 1,
    }" @pointerdown="row.depth === 1 ? emit('drag-start', { event: $event, item: row.item }) : undefined"
      @contextmenu.prevent="row.depth === 1 ? emit('item-menu', { event: $event, item: row.item }) : undefined"
      @click="row.depth === 1 ? emit('row-click', { event: $event, item: row.item }) : undefined">
      <!-- 多选勾选框（与列表视图同一套）：内置行没有 SHA1，留等宽占位保持各列对齐 -->
      <span class="mod-cell mod-select-cell">
        <input v-if="row.depth === 1" type="checkbox" class="mod-select"
          :checked="selectedKeys.has(modSelectKey(row.item))" @pointerdown.stop @change.stop="emit('select', row.item)"
          @click.stop />
        <span v-else class="mod-select-gap" />
      </span>

      <!--
        树列在最左：**缩进 + 展开箭头都放这里**，用户要求"把展开放在左边"。
        层级靠 16px 一级的缩进；箭头只给带内置模组的行，其余留等宽占位。
        组名只在分组行那一行出现，模组行这一格是空的。
      -->
      <span class="mod-cell mod-group-cell">
        <span class="mod-tree-indent" :style="{ width: `${(row.depth - 1) * 16}px` }" />
        <button v-if="row.item.jarInJar.length" class="mod-tree-caret"
          :class="{ collapsed: !isOpen(modRowKey(row.item)) }" v-tip="isOpen(modRowKey(row.item))
              ? t('resource.modBuiltinCollapse')
              : t('resource.modBuiltinExpand')
            " @click.stop="toggleExpand(modRowKey(row.item))" @pointerdown.stop>
          <GlyphIcon name="chevron-down" :size="13" :weight="2.4" />
        </button>
        <span v-else class="mod-tree-caret-placeholder" />
      </span>

      <!-- 启用：复选框，勾 = 启用、取消 = 禁用；内置模组没有独立文件，不给 -->
      <span class="mod-cell mod-enable-cell" @pointerdown.stop>
        <input v-if="row.depth === 1" type="checkbox" class="mod-check" :checked="!row.item.disable"
          :disabled="busy || row.item.fail" @change="emit('toggle', row.item)" />
      </span>

      <span class="mod-cell mod-name-cell">
        <span class="mod-name" :title="row.item.name || row.item.file">
          {{ row.item.name || row.item.file }}
        </span>
        <!-- 内置的**库**：没有模组元数据的内置 jar。它不是模组，与"内置"分开标 -->
        <span v-if="row.item.library" class="badge badge-dim">
          {{ t("resource.modLibrary") }}
        </span>
        <span v-else-if="row.depth > 1" class="badge badge-dim">
          {{ t("resource.modBuiltin") }}
        </span>
        <span v-if="row.item.jarInJar.length" class="badge">
          {{ t("resource.modBuiltinCount", { n: row.item.jarInJar.length }) }}
        </span>
        <span v-if="row.item.fail" class="badge badge-red">{{ t("resource.modFail") }}</span>
        <span v-if="row.item.core" class="badge">{{ t("resource.modCore") }}</span>
      </span>

      <!--
        备注：紧挨名字右边，这是用户要求；**双击原地书写**，用 `contenteditable` 不弹输入框。
        内置行没有备注，双击也不进编辑（它没有独立文件，写不了备注）。
      -->
      <span class="mod-cell mod-note-cell" :class="{ 'mod-note-editing': isEditing(row) }"
        :contenteditable="isEditing(row) ? 'true' : undefined"
        :title="isEditing(row) ? '' : row.item.note || (row.depth === 1 ? t('resource.modNoteDoubleClick') : '')"
        @dblclick.stop="row.depth === 1 ? startEdit(row.item, $event) : undefined"
        @keydown.enter="onNoteEnter" @keydown.esc.prevent="cancelEdit($event)" @blur="commitEdit($event)"
        @pointerdown.stop>{{ row.item.note }}</span>

      <span class="mod-cell" :title="row.item.modId">{{ row.item.modId }}</span>
      <span class="mod-cell" :title="row.item.version">{{ row.item.version }}</span>
      <span class="mod-cell" :title="loaderNames(row.item.loaders)">
        {{ loaderNames(row.item.loaders) }}
      </span>
      <span class="mod-cell">{{ sideName(row.item.side) }}</span>
      <span class="mod-cell">{{ sourceName(row.item.source) }}</span>
      <span class="mod-cell" :title="row.item.projectId">{{ row.item.projectId }}</span>
      <span class="mod-cell" :title="row.item.fileId">{{ row.item.fileId }}</span>
      <!-- 路径不挂 title：用户明确不要悬浮 -->
      <span class="mod-cell mod-path">{{ row.item.path }}</span>
      <span class="mod-cell" :title="row.item.author">{{ row.item.author }}</span>
      <span class="mod-cell" :title="row.item.url">{{ row.item.url }}</span>
    </div>

    <!--
      空分组：一级节点那一行已经把组名画出来了（可折叠、可拖动排序），
      这里只补一句提示，横跨剩下的列。
    -->
    <div v-if="groupOpen && !rows.length" class="mod-tr mod-tr-empty"
      :style="{ gridTemplateColumns: gridTemplate }">
      <span class="mod-cell mod-select-cell" />
      <span class="mod-cell mod-empty-tip">{{ t("resource.groupEmptyHint") }}</span>
    </div>
  </div>
</template>
