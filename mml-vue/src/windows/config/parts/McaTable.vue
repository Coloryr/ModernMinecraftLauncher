<script setup lang="ts">
// 区域文件（`.mca`）视图：区块列表，点一条就翻到该区块的 NBT 编辑器
//
// 区块内容直接复用 NBT 编辑器（NbtTree）：每个区块是一份独立的 NBT 会话，
// 编辑、保存、基准 SHA1 核对都走编辑器自己那条路。区块列表的行沿用 NBT 树的口径
// （左列键、右列值、中间一条竖线），进出两页观感是同一个编辑器。
//
// 「查找实体 / 方块」的入口在窗口标题栏，查到的区块通过 `chunk` 与 `reveal` 传进来。
import { computed, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { readConfigMca } from "../../../lib/api";
import NbtTree from "./NbtTree.vue";
import type { McaChunkDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 来源，空串为实例本身，否则是存档目录名 */
  source: string;
  /** 来源内的相对文件路径 */
  file: string;
  /** 直接打开的区块下标，来自「查找实体 / 方块」；不传就停在列表 */
  chunk?: number;
  /** 打开后要定位的节点路径，同样是查找给的 */
  reveal?: string[];
}>();

const rows = ref<McaChunkDto[]>([]);
const loading = ref(false);
/** 正在编辑的区块下标；null 表示停在区块列表 */
const editing = ref<number | null>(props.chunk ?? null);
/** 交给编辑器的定位路径 */
const reveal = ref<string[] | null>(props.reveal ?? null);
/** 编辑器有没有未保存的改动，决定返回条上要不要给保存键 */
const dirty = ref(false);
/** 区块编辑器实例，保存键按它调 save() */
const treeRef = ref<InstanceType<typeof NbtTree> | null>(null);

/** 文件名，当列表标题 */
const fileName = computed(() => props.file.split("/").pop() ?? props.file);

/** 区域坐标：文件名形如 `r.-1.-1.mca`，对不上就当 0 */
const region = computed(() => {
  const match = /^r\.(-?\d+)\.(-?\d+)\.mca$/i.exec(fileName.value);
  return match ? { x: Number(match[1]), z: Number(match[2]) } : { x: 0, z: 0 };
});

/** 区块的绝对坐标 */
function chunkX(row: McaChunkDto): number {
  return region.value.x * 32 + row.x;
}

function chunkZ(row: McaChunkDto): number {
  return region.value.z * 32 + row.z;
}

/** 写入时间：后端给的是 Unix 秒 */
function formatTime(seconds: number): string {
  if (!seconds) return "--";
  return new Date(seconds * 1000).toLocaleString();
}

/** 右列的说明：扇区数与写入时间，位置表里那几项 */
function chunkMeta(row: McaChunkDto): string {
  return t("config.mcaChunkMeta", { sectors: row.sectors, time: formatTime(row.time) });
}

/** 正在编辑的区块坐标，返回条上显示 */
const editingLabel = computed(() => {
  const row = rows.value.find((item) => item.index === editing.value);
  return row ? `(${chunkX(row)}, ${chunkZ(row)})` : "";
});

/** 保存当前区块的改动 */
async function saveChunk() {
  await treeRef.value?.save();
}

/** 拉区块表 */
async function load() {
  loading.value = true;
  try {
    rows.value = await readConfigMca(props.uuid, props.source, props.file);
    dirty.value = false;
  } catch (e) {
    rows.value = [];
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

// 换来源或换文件就重拉；`immediate` 让首次打开就有内容
watch(
  () => [props.uuid, props.source, props.file],
  () => {
    editing.value = props.chunk ?? null;
    reveal.value = props.reveal ?? null;
    void load();
  },
  { immediate: true },
);

// 查找换了目标（同一份文件里再查一次）也要跟着翻页
watch(
  () => [props.chunk, props.reveal],
  () => {
    editing.value = props.chunk ?? null;
    reveal.value = props.reveal ?? null;
  },
);
</script>

<template>
  <div class="mca-view">
    <!-- 区块列表：一行一个区块，键列是坐标、值列是扇区与写入时间 -->
    <div v-if="editing === null" class="mca-list">
      <div class="mca-head">{{ fileName }}</div>
      <button v-for="row in rows" :key="row.index" class="mca-row"
        @click="reveal = null; editing = row.index">
        <span class="mca-key">
          <span class="mca-arrow" aria-hidden="true">›</span>
          ({{ chunkX(row) }}, {{ chunkZ(row) }})
        </span>
        <span class="mca-val">{{ chunkMeta(row) }}</span>
      </button>
      <p v-if="loading" class="empty-tip">{{ t("config.loading") }}</p>
      <p v-else-if="!rows.length" class="empty-tip">{{ t("config.mcaEmpty") }}</p>
    </div>

    <!-- 区块内容：上方一条返回条，下面是同一个 NBT 编辑器 -->
    <template v-else>
      <div class="mca-bar">
        <button class="mca-back" @click="editing = null">
          <span class="mca-back-arrow" aria-hidden="true">‹</span>
          {{ t("config.mcaBack") }}
        </button>
        <span class="mca-bar-title">{{ editingLabel }}</span>
        <button v-if="dirty" class="mca-save" @click="saveChunk">{{ t("config.save") }}</button>
      </div>
      <NbtTree ref="treeRef" :uuid="uuid" :source="source" :file="file" :chunk="editing"
        :reveal="reveal ?? undefined" @dirty="dirty = $event" />
    </template>
  </div>
</template>

<style scoped>
/* 两页共用一列纵向布局：列表自己滚，编辑器占满剩余高度 */
.mca-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.mca-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding-bottom: 12px;
}

/* 列表标题：与树的根节点同一处口径 */
.mca-head {
  flex-shrink: 0;
  padding: 6px 10px;
  color: var(--text-dim);
  font-size: 11.5px;
  font-weight: 600;
}

/* 行与 NBT 树一致：左列键、右列值、中间一条竖线 */
.mca-row {
  display: grid;
  grid-template-columns: minmax(0, 42%) minmax(0, 1fr);
  align-items: center;
  width: 100%;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: default;
}

.mca-row:hover {
  background: var(--bg-hover);
}

.mca-key {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 10px;
  border-right: 1px solid var(--border);
  font-size: 12.5px;
}

.mca-arrow {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 13px;
}

.mca-val {
  padding: 3px 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-dim);
  font-size: 12px;
}

/* 返回条：贴顶部一条 */
.mca-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border);
}

.mca-back {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 3px 10px 3px 6px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  font-size: 12px;
  cursor: pointer;
}

.mca-back:hover {
  border-color: var(--border);
  background: var(--bg-hover);
  color: var(--text);
}

.mca-back-arrow {
  font-size: 15px;
  line-height: 1;
}

.mca-bar-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-dim);
  font-size: 12px;
}

.mca-save {
  flex-shrink: 0;
  padding: 2px 10px;
  border: 1px solid var(--accent-border);
  border-radius: 6px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 11.5px;
  cursor: pointer;
}

/* 编辑器占满返回条以下的高度 */
.mca-view :deep(.nbt-view) {
  flex: 1;
  min-height: 0;
}
</style>
