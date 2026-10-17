<script setup lang="ts">
// 区域文件（`.mca`）视图：按"区域 → 区块 → 区块 NBT"的树展开
//
// 文件头那 1024 项位置表整份读出来，只有真的存了区块的表项才列；区块里的 NBT
// **按需拉取**（展开某个区块时才读它那一段）—— 打开一个区域文件就解析上千个区块不值当。
import { computed, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { readConfigMca, readConfigMcaChunk } from "../../../lib/api";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import McaNode from "./McaNode.vue";
import type { McaChunkDto, NbtNodeDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 来源，空串为实例本身，否则是存档目录名 */
  source: string;
  /** 来源内的相对文件路径 */
  file: string;
}>();

const rows = ref<McaChunkDto[]>([]);
const loading = ref(false);
/** 区域根节点是否展开 */
const regionOpen = ref(true);
/** 已展开的区块：表项下标 → 它的 NBT 成员 */
const chunks = ref(new Map<number, NbtNodeDto[]>());
/** 正在拉的区块下标，-1 表示没有 */
const loadingIndex = ref(-1);

/** 文件名，当树的根节点名字 */
const fileName = computed(() => props.file.split("/").pop() ?? props.file);

/** 区域坐标：文件名形如 `r.-1.-1.mca`，对不上就当 0 */
const region = computed(() => {
  const match = /^r\.(-?\d+)\.(-?\d+)\.mca$/i.exec(fileName.value);
  return match ? { x: Number(match[1]), z: Number(match[2]) } : { x: 0, z: 0 };
});

/** 区块的绝对坐标，截图里那种 (-32, -32) 口径 */
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

/** 区块行的悬停提示：位置表里那几项在这里，树上只看坐标 */
function chunkTip(row: McaChunkDto): string {
  return t("config.mcaChunkTip", {
    index: row.index,
    pos: row.pos,
    sectors: row.sectors,
    time: formatTime(row.time),
  });
}

/** 拉区块表 */
async function load() {
  loading.value = true;
  try {
    rows.value = await readConfigMca(props.uuid, props.source, props.file);
    chunks.value = new Map();
  } catch (e) {
    rows.value = [];
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 展开 / 收起一个区块；第一次展开才去拉 NBT */
async function toggleChunk(row: McaChunkDto) {
  const next = new Map(chunks.value);
  if (next.delete(row.index)) {
    chunks.value = next;
    return;
  }
  loadingIndex.value = row.index;
  try {
    chunks.value.set(row.index, await readConfigMcaChunk(props.uuid, props.source, props.file, row.index));
    chunks.value = new Map(chunks.value);
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loadingIndex.value = -1;
  }
}

// 换来源或换文件就重拉；`immediate` 让首次打开就有内容
watch(
  () => [props.uuid, props.source, props.file],
  () => void load(),
  { immediate: true },
);
</script>

<template>
  <div class="mca-tree">
    <!-- 根：区域文件本身，右边是区块个数 -->
    <button class="nbt-line" @click="regionOpen = !regionOpen">
      <GlyphIcon class="nbt-chevron" :class="{ open: regionOpen }" name="chevron-down" :size="11" :weight="2.4" />
      <span class="nbt-kind">[]</span>
      <span class="nbt-name">{{ fileName }}</span>
      <span class="nbt-value">[{{ rows.length }}]</span>
    </button>

    <template v-if="regionOpen">
      <div v-for="row in rows" :key="row.index">
        <button class="nbt-line" v-tip="chunkTip(row)" @click="toggleChunk(row)">
          <GlyphIcon class="nbt-chevron" :class="{ open: chunks.has(row.index) }" name="chevron-down" :size="11"
            :weight="2.4" />
          <span class="nbt-kind">{}</span>
          <span class="nbt-name">({{ chunkX(row) }}, {{ chunkZ(row) }})</span>
        </button>
        <template v-if="chunks.has(row.index)">
          <McaNode v-for="child in chunks.get(row.index)" :key="child.name" :node="child" :depth="2" />
        </template>
        <p v-else-if="loadingIndex === row.index" class="empty-tip">{{ t("config.loading") }}</p>
      </div>
      <p v-if="loading" class="empty-tip">{{ t("config.loading") }}</p>
      <p v-else-if="!rows.length" class="empty-tip">{{ t("config.mcaEmpty") }}</p>
    </template>
  </div>
</template>

<style scoped>
/* 树自己滚：与 NBT 树同一边界口径 */
.mca-tree {
  height: 100%;
  min-height: 0;
  overflow: auto;
  padding: 6px 0 12px;
}

.nbt-line {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 1px 8px 1px 0;
  border: none;
  background: transparent;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: default;
}

.nbt-line:hover {
  background: var(--bg-hover);
}

.nbt-chevron {
  flex-shrink: 0;
  color: var(--text-dim);
  transform: rotate(-90deg);
  transition: transform 0.12s;
}

.nbt-chevron.open {
  transform: none;
}

.nbt-kind {
  width: 26px;
  flex-shrink: 0;
  color: var(--text-dim);
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  font-size: 11px;
}

.nbt-name {
  flex-shrink: 0;
  font-size: 12px;
}

.nbt-value {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-dim);
  font-size: 12px;
}
</style>
