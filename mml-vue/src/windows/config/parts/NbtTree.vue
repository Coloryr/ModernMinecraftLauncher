<script setup lang="ts">
// NBT 文件视图：拉整个文件的树，交给 NbtNode 递归画
import { ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { readConfigNbt } from "../../../lib/api";
import type { NbtNodeDto } from "../../../lib/bindings";
import NbtNode from "./NbtNode.vue";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 相对游戏目录的文件路径 */
  file: string;
}>();

const tree = ref<NbtNodeDto | null>(null);
const loading = ref(false);

/** 拉这棵 NBT 树 */
async function load() {
  loading.value = true;
  try {
    tree.value = await readConfigNbt(props.uuid, props.file);
  } catch (e) {
    tree.value = null;
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

// 换文件就重拉；`immediate` 让首次打开就有内容
watch(
  () => [props.uuid, props.file],
  () => void load(),
  { immediate: true },
);
</script>

<template>
  <div class="nbt-tree">
    <div v-if="loading" class="empty-tip">{{ t("config.loading") }}</div>
    <NbtNode v-else-if="tree" :node="tree" :depth="0" />
    <p v-else class="empty-tip">{{ t("config.nbtEmpty") }}</p>
  </div>
</template>

<style scoped>
.nbt-tree {
  height: 100%;
  overflow: auto;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
}
</style>
