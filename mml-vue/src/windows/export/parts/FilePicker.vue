<script setup lang="ts">
// 导出用的文件树：一层一层向 Rust 要目录内容，勾选状态留在父组件
//
// 勾选沿目录继承：勾了目录，里面的东西都算勾上；取消勾选记成排除项，父目录勾着也能去掉某一条。
// 展平成一维行渲染（目录 + 文件的先后即显示顺序），吸顶靠行高换算：
// 滚动位置 / 行高 = 顶部那一行，往回找到最近的目录行，把它单独画在滚动区外面——
// 不用 position: sticky，那一套在嵌套分组里对祖先的高度很敏感，容易错位。
import { computed, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { api } from "../../../lib/api";
import type { ExportEntryDto } from "../../../lib/bindings";
import FileTreeRow from "./FileTreeRow.vue";
import { fileCheckStates } from "../picked";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 已勾选的条目表，键是相对根目录的路径；由父组件持有 */
  picked: Map<string, unknown>;
  /** 取消勾选的路径；父目录勾着时单独去掉某一条，默认空 */
  unpicked?: Set<string>;
  selectionLoading?: boolean;
  /** 根目录：game = 游戏目录（默认），instance = 实例目录；「直接压缩」与 MMC 用后者 */
  base?: "game" | "instance";
  /** 初始展开的目录（相对根目录）；默认勾选落在子目录里时靠它露出来 */
  expand?: string[];
  /** 必选项判定：命中时勾选框固定勾上并禁用，默认没有必选项 */
  locked?: (path: string) => boolean;
}>();

const emit = defineEmits<{
  /** 勾选 / 取消勾选一个条目 */
  (e: "toggle", entry: ExportEntryDto, checked: boolean): void;
}>();

/** 相对路径 → 该目录下的子项；根目录用空串当键 */
const children = ref(new Map<string, ExportEntryDto[]>());
/** 展开着的目录 */
const opened = ref(new Set<string>());
/** 正在拉的目录，防重复请求 */
const loading = ref(new Set<string>());
const rootError = ref("");

/** 展平后的可见行：只含已展开目录的后代，顺序就是显示顺序 */
interface TreeRow {
  /** 这一行的条目 */
  entry: ExportEntryDto;
  /** 缩进层级，根目录的子项为 0 */
  depth: number;
}

/** 行高，单位 px：与 .tree-row 的 height 一致，吸顶行按它换算 */
const ROW_H = 27;

const rows = computed<TreeRow[]>(() => {
  const list: TreeRow[] = [];
  const walk = (dir: string, depth: number) => {
    for (const entry of children.value.get(dir) ?? []) {
      list.push({ entry, depth });
      if (entry.dir && opened.value.has(entry.path)) walk(entry.path, depth + 1);
    }
  };
  walk("", 0);
  return list;
});

/** 滚动容器 */
const scrollEl = ref<HTMLElement | null>(null);
/** 当前的滚动距离，吸顶行按它算 */
const scrollTop = ref(0);

/** 滚动时重算吸顶行 */
function onScroll() {
  scrollTop.value = scrollEl.value?.scrollTop ?? 0;
}

/** 吸顶行：视口顶部那一行**所属**的那一层目录；没滚动或已经在根上时不显示 */
const pinned = computed<TreeRow | null>(() => {
  const top = scrollTop.value;
  const list = rows.value;
  if (top <= 0 || !list.length) return null;
  const start = Math.min(Math.floor(top / ROW_H), list.length - 1);
  const row = list[start];
  // 顶部这行本身就是展开着的目录：它自己就是当前所在的那一层
  if (row.entry.dir && opened.value.has(row.entry.path)) return row;
  // 否则往上找最近的上一层目录：DFS 里它就是这行的父目录。
  // 不能简单取「最近的目录行」—— 那会挑中已经划过去的、同层的兄弟目录
  for (let i = start - 1; i >= 0; i--) {
    if (list[i].depth < row.depth && list[i].entry.dir) return list[i];
  }
  return null;
});

/** 没有排除项时共用的空表，免得每渲染一行就新建一个 */
const NO_UNPICKED: ReadonlySet<string> = new Set();

const checkStates = computed(() => fileCheckStates(children.value, props.picked, props.unpicked ?? NO_UNPICKED));

function toggleEntry(entry: ExportEntryDto) {
  if (props.selectionLoading || isLocked(entry.path)) return;
  emit("toggle", entry, checkStates.value.get(entry.path) !== "checked");
}

/** 必选条目：勾选框固定勾上并禁用，点了也不改勾选 */
function isLocked(path: string): boolean {
  return props.locked?.(path) === true;
}

/** 按根目录选接口：实例目录给「直接压缩」用 */
function listDir(dir: string) {
  return props.base === "instance"
    ? api.listExportInstanceDir(props.uuid, dir)
    : api.listExportDir(props.uuid, dir);
}

/** 拉一个目录的内容，拉过就不再拉 */
async function load(dir: string) {
  const table = children.value;
  const pending = loading.value;
  if (table.has(dir) || pending.has(dir)) return;
  pending.add(dir);
  try {
    const list = await listDir(dir);
    if (table === children.value) table.set(dir, list);
  } catch (e) {
    if (table !== children.value) return;
    if (dir) showToast(tErr(e));
    else rootError.value = tErr(e);
  } finally {
    pending.delete(dir);
  }
}

/** 展开 / 收起一个目录 */
async function toggleDir(dir: string) {
  if (opened.value.has(dir)) {
    opened.value.delete(dir);
    return;
  }
  opened.value.add(dir);
  await load(dir);
}

/** 回到顶部，并把吸顶行一起清掉 */
function resetScroll() {
  if (scrollEl.value) scrollEl.value.scrollTop = 0;
  scrollTop.value = 0;
}

// 展开 / 收起会改变内容高度，浏览器可能顺手挪动滚动位置：重新读一次
watch(rows, () => {
  scrollTop.value = scrollEl.value?.scrollTop ?? 0;
}, { flush: "post" });

// 换实例或换根目录就把整棵树清掉重来，再把指定的目录展开
watch(
  [() => props.uuid, () => props.base],
  async (_current, _previous, onCleanup) => {
    const table = children.value;
    onCleanup(() => {
      children.value = new Map();
      opened.value = new Set();
      loading.value = new Set();
    });
    rootError.value = "";
    resetScroll();
    if (!props.uuid) return;
    await load("");
    for (const dir of props.expand ?? []) {
      if (table !== children.value) return;
      if (!table.get("")?.some((e) => e.dir && e.path === dir)) continue;
      opened.value.add(dir);
      await load(dir);
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="tree" :aria-busy="selectionLoading || loading.has('')">
    <!-- 吸顶行画在滚动区之外，滚动时由前面的脚本换成当前那一层目录 -->
    <div v-if="pinned && !selectionLoading" class="tree-pin">
      <FileTreeRow :entry="pinned.entry" :depth="pinned.depth" :open="opened.has(pinned.entry.path)"
        :loading="loading.has(pinned.entry.path)"
        :checked="checkStates.get(pinned.entry.path) === 'checked'" :indeterminate="checkStates.get(pinned.entry.path) === 'mixed'"
        :locked="isLocked(pinned.entry.path)" pinned
        @toggle="toggleEntry" @open="toggleDir" />
    </div>
    <div ref="scrollEl" class="tree-scroll" @scroll="onScroll">
      <div v-if="selectionLoading || loading.has('')" class="px-2" role="status" :aria-label="t('winExport.loadingFiles')">
        <div v-for="row in 8" :key="row" class="flex h-[27px] items-center gap-2" aria-hidden="true">
          <span class="sk size-3.5" />
          <span class="sk sk-line" :class="row % 2 ? 'w40' : 'w60'" />
        </div>
      </div>
      <p v-else-if="!rows.length" class="empty-tip" :class="{ 'load-error': rootError }">
        {{ rootError || t("winExport.noFiles") }}
      </p>
      <template v-else>
        <FileTreeRow v-for="row in rows" :key="row.entry.path" :entry="row.entry" :depth="row.depth"
          :open="opened.has(row.entry.path)" :loading="loading.has(row.entry.path)"
          :checked="checkStates.get(row.entry.path) === 'checked'" :indeterminate="checkStates.get(row.entry.path) === 'mixed'"
          :locked="isLocked(row.entry.path)"
          @toggle="toggleEntry" @open="toggleDir" />
      </template>
    </div>
  </div>
</template>

<style scoped>
/* 外框只管边框与圆角，滚动交给里面那层；圆角裁掉滚动条与吸顶行 */
.tree {
  position: relative;
  flex: 1;
  min-height: 0;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-solid, var(--bg-card));
  overflow: hidden;
}

.tree-scroll {
  position: absolute;
  inset: 4px;
  overflow: auto;
  overflow-y: scroll;
  /* 只留滚动条那一侧的槽：条目左右内边距本来就一样，两侧都留反而把内容推向右 */
  scrollbar-gutter: stable;
}

/* 吸顶行：贴在同一条内缩线上，右缘让出 9px 的滚动条槽，
   宽度才和滚动区里的行一致 */
.tree-pin {
  position: absolute;
  top: 4px;
  left: 4px;
  right: 13px;
  z-index: 2;
}
</style>
