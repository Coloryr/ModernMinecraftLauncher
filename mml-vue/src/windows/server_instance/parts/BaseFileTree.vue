<script setup lang="ts">
// 基础文件树：根条目由窗口层给，目录按需向 Rust 要子项；勾选是精确路径，勾目录表示整目录进包
import { computed, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { api } from "../../../lib/api";
import type { ExportEntryDto } from "../../../lib/bindings";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 根目录条目，已由窗口层过滤掉启动器元数据与运行期目录 */
  root: ExportEntryDto[];
  /** 已勾选的相对路径，精确匹配 */
  picked: Set<string>;
  /** 生成中时整棵树不能改 */
  disabled?: boolean;
}>();

const emit = defineEmits<{
  /** 勾选 / 取消一个条目 */
  (e: "toggle", path: string): void;
}>();

/** .minecraft 里的清单文件不能进基础包，后端会拒绝，树里不列 */
const RESERVED = new Set([
  ".minecraft/server.json",
  ".minecraft/server.json.sha1",
  ".minecraft/server.json.old",
]);

/** 目录路径 → 已拉取的子项 */
const children = ref(new Map<string, ExportEntryDto[]>());
/** 展开着的目录 */
const opened = ref(new Set<string>());
/** 正在拉的目录，防重复请求 */
const loading = ref(new Set<string>());

interface Row {
  /** 这一行的条目 */
  entry: ExportEntryDto;
  /** 缩进层级，根目录的子项为 0 */
  depth: number;
}

const rows = computed<Row[]>(() => {
  const list: Row[] = [];
  const walk = (entries: ExportEntryDto[], depth: number) => {
    for (const entry of entries) {
      list.push({ entry, depth });
      if (entry.dir && opened.value.has(entry.path)) {
        walk(children.value.get(entry.path) ?? [], depth + 1);
      }
    }
  };
  walk(props.root, 0);
  return list;
});

/** 拉一个目录的子项，拉过就不再拉 */
async function load(dir: string) {
  if (children.value.has(dir) || loading.value.has(dir)) return;
  const table = children.value;
  loading.value.add(dir);
  try {
    const list = await api.listExportInstanceDir(props.uuid, dir);
    if (table === children.value) {
      table.set(dir, list.filter((entry) => !RESERVED.has(entry.path)));
    }
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value.delete(dir);
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

/** 文件大小，按档显示 */
function sizeText(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

// 换实例就整棵树重来；目录默认全部收起，由用户自己展开
watch(
  () => [props.uuid, props.root] as const,
  () => {
    children.value = new Map();
    opened.value = new Set();
  },
  { immediate: true },
);
</script>

<template>
  <div v-if="!rows.length" class="empty-tip">{{ t("serverInstance.baseEmpty") }}</div>
  <template v-else>
    <div v-for="row in rows" :key="row.entry.path" class="si-row-item"
      :style="{ paddingLeft: `${8 + row.depth * 16}px` }">
      <span class="si-caret" :class="{ open: opened.has(row.entry.path) }" @click="toggleDir(row.entry.path)">
        <GlyphIcon v-if="row.entry.dir" name="chevron-down" :size="12" :weight="2.4" />
      </span>
      <input type="checkbox" class="si-box" :checked="picked.has(row.entry.path)" :disabled="disabled"
        @change="emit('toggle', row.entry.path)" />
      <span class="si-name" :class="{ dir: row.entry.dir }" :title="row.entry.path"
        @click="row.entry.dir ? toggleDir(row.entry.path) : emit('toggle', row.entry.path)">
        {{ row.entry.path.split("/").pop() }}{{ row.entry.dir ? "/" : "" }}
      </span>
      <span class="si-counter">{{ row.entry.dir ? "—" : sizeText(row.entry.size) }}</span>
    </div>
  </template>
</template>
