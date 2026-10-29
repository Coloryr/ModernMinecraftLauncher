<script setup lang="ts">
// 实例的自定义启动配置：json/ 目录下每个文件一行的摘要表
//
// 与 ColorMC 游戏编辑窗口 tab1 的那张表同一组列（排序 / 名称 / UID / 版本 / 路径）。
// 数据是读目录现算的，不进 gui_config —— 用户可以手改 json 目录，拉一次就是最新的。
import { ref, watch } from "vue";
import { t } from "../lib/i18n";
import { api } from "../lib/api";
import type { CustomJsonItemDto } from "../lib/bindings";

const props = defineProps<{
  /** 实例 uuid；换实例时重新拉一次 */
  uuid: string;
}>();

const list = ref<CustomJsonItemDto[]>([]);
/** 拉取代次：切实例时丢弃旧一发的返回 */
let loadSeq = 0;

/** 拉一次列表；失败按空表处理，不打断详情页 */
async function load() {
  const mine = ++loadSeq;
  if (!props.uuid) {
    list.value = [];
    return;
  }
  try {
    const items = await api.getCustomJson(props.uuid);
    if (mine === loadSeq) list.value = items;
  } catch {
    if (mine === loadSeq) list.value = [];
  }
}

watch(() => props.uuid, load, { immediate: true });
</script>

<template>
  <div class="json-panel">
    <p v-if="list.length === 0" class="json-empty">{{ t("customJson.empty") }}</p>
    <table v-else class="json-table">
      <thead>
        <tr>
          <th class="col-order">{{ t("customJson.order") }}</th>
          <th>{{ t("customJson.name") }}</th>
          <th>{{ t("customJson.uid") }}</th>
          <th>{{ t("customJson.version") }}</th>
          <th>{{ t("customJson.file") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="item in list" :key="item.file">
          <td class="col-order">{{ item.order }}</td>
          <td>{{ item.name }}</td>
          <td class="mono">{{ item.uid }}</td>
          <td>{{ item.version }}</td>
          <td class="mono">{{ item.file }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.json-panel {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 12px 16px;
  overflow-x: auto;
}

.json-empty {
  margin: 0;
  font-size: 12.5px;
  color: var(--text-dim);
}

.json-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
}

.json-table th {
  padding: 4px 8px 6px;
  text-align: left;
  font-weight: 600;
  color: var(--text-dim);
  white-space: nowrap;
}

.json-table td {
  padding: 5px 8px;
  border-top: 1px solid var(--border);
}

.col-order {
  width: 48px;
}

/* uid 与文件名是标识，等宽字更好认 */
.mono {
  font-family: "Cascadia Code", Consolas, monospace;
  color: var(--text-dim);
}
</style>
