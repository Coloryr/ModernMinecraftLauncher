<script setup lang="ts">
// 区域文件（`.mca`）视图：文件头那 1024 项位置表，只有真的存了区块的表项会列出来
//
// 后端只读 8 KiB 文件头、不解析区块内容，所以这里没有"每个区块里是什么"；
// 要看某个区块的原始字节，用二进制视图按 `pos` 翻过去。
import { ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { readConfigMca } from "../../../lib/api";
import type { McaChunkDto } from "../../../lib/bindings";

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

/** 拉区块表 */
async function load() {
  loading.value = true;
  try {
    rows.value = await readConfigMca(props.uuid, props.source, props.file);
  } catch (e) {
    rows.value = [];
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 写入时间：后端给的是 Unix 秒 */
function formatTime(seconds: number): string {
  if (!seconds) return "--";
  return new Date(seconds * 1000).toLocaleString();
}

// 换来源或换文件就重拉；`immediate` 让首次打开就有内容
watch(
  () => [props.uuid, props.source, props.file],
  () => void load(),
  { immediate: true },
);
</script>

<template>
  <div class="mca-table">
    <div class="mca-head">
      <span class="mca-index">{{ t("config.mcaIndex") }}</span>
      <span class="mca-xyz">X</span>
      <span class="mca-xyz">Z</span>
      <span class="mca-pos">{{ t("config.mcaPos") }}</span>
      <span class="mca-sectors">{{ t("config.mcaSectors") }}</span>
      <span class="mca-time">{{ t("config.mcaTime") }}</span>
    </div>
    <div class="mca-rows">
      <div v-for="row in rows" :key="row.index" class="mca-row">
        <span class="mca-index">{{ row.index }}</span>
        <span class="mca-xyz">{{ row.x }}</span>
        <span class="mca-xyz">{{ row.z }}</span>
        <span class="mca-pos">{{ row.pos }}</span>
        <span class="mca-sectors">{{ row.sectors }}</span>
        <span class="mca-time">{{ formatTime(row.time) }}</span>
      </div>
      <p v-if="loading" class="empty-tip">{{ t("config.loading") }}</p>
      <p v-else-if="!rows.length" class="empty-tip">{{ t("config.mcaEmpty") }}</p>
    </div>
  </div>
</template>

<style scoped>
.mca-table {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
  overflow: hidden;
}

.mca-head,
.mca-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 10px;
  font-size: 12px;
}

.mca-head {
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
  font-weight: 600;
}

.mca-rows {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.mca-row:hover {
  background: var(--bg-hover);
}

/* 数值列等宽数字，右对齐不跳列 */
.mca-index,
.mca-xyz,
.mca-pos,
.mca-sectors {
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}

.mca-index,
.mca-xyz {
  width: 56px;
}

.mca-pos,
.mca-sectors {
  width: 84px;
}

/* 时间列吃掉剩余宽度；窄窗口下先挤它 */
.mca-time {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-dim);
}
</style>
