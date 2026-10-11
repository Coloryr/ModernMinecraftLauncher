<script setup lang="ts">
// 光影包列表：设为当前光影 / 取消、打开目录、删除
import { t } from "../../../lib/i18n";
import { deleteShaderpack, setShader } from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { ShaderItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，光影列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化与确认弹窗从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { shaders, instanceUuid, loading } = props.data;
const { busy, act, askDelete, openFolder } = props.ops;

/** 设为当前光影；点已选中的那个就是取消 */
function toggle(item: ShaderItemDto) {
  act(async () => {
    // 点已选中的那个 = 取消选择（后端传 null）
    await setShader(instanceUuid.value, item.selected ? null : item.file);
  });
}

function remove(item: ShaderItemDto) {
  askDelete(item.name || item.file, () => deleteShaderpack(instanceUuid.value, item.file));
}

/** 副标题：注释 · 文件名 */
function sub(item: ShaderItemDto): string {
  return [item.comment, item.file].filter(Boolean).join(" · ");
}
</script>

<template>
  <ContentHead :data="data">
  </ContentHead>

  <div v-if="loading" class="item-list">
    <ListSkeleton />
  </div>
  <div v-else class="item-list">
    <ResourceRow v-for="item in shaders" :key="item.file" :name="item.name || item.file">
      <template #badges>
        <span v-if="item.selected" class="badge">{{ t("resource.shaderOn") }}</span>
      </template>
      <template #sub>{{ sub(item) }}</template>
      <template #actions>
        <button class="mini-btn" :disabled="busy" @click="toggle(item)">
          {{ item.selected ? t("resource.disable") : t("resource.enable") }}
        </button>
        <button class="mini-btn" :disabled="busy" @click="openFolder('shaderpacks', item.file)">
          {{ t("resource.openFolder") }}
        </button>
        <button class="mini-btn danger" :disabled="busy" @click="remove(item)">
          {{ t("resource.delete") }}
        </button>
      </template>
    </ResourceRow>
    <div v-if="!shaders.length" class="empty-tip">{{ t("resource.empty") }}</div>
  </div>
</template>
