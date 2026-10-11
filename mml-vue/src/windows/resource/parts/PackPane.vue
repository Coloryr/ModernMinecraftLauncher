<script setup lang="ts">
// 材质包列表：启用 / 禁用（写 options.txt）、打开目录、删除
import { t } from "../../../lib/i18n";
import {
  deleteResourcepack,
  disableResourcepack,
  enableResourcepack,
} from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { PackItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，材质包列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化与确认弹窗从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { packs, instanceUuid, loading } = props.data;
const { busy, actLocal, askDelete, openFolder } = props.ops;

function remove(item: PackItemDto) {
  askDelete(item.file, () => deleteResourcepack(instanceUuid.value, item.file));
}

/**
 * 启用 / 禁用
 *
 * **不重拉列表**：启用状态只写在 options.txt 的 `resourcePacks` 里，包本身一个字节都没动。
 * 重拉一次要把每个包重新算 SHA1 / SHA256 并解一遍 `pack.mcmeta`，模组那边是重扫 jar，
 * 同一条理由。所以就地改这一行的 `enable`，走 `actLocal`：串行化 + 失败提示，不刷新。
 */
async function toggle(item: PackItemDto) {
  const next = !item.enable;
  await actLocal(async () => {
    if (next) {
      await enableResourcepack(instanceUuid.value, item.file);
    } else {
      await disableResourcepack(instanceUuid.value, item.file);
    }
    item.enable = next;
  });
}

/** 包格式那段文案；描述要过格式码，这一句不要，所以两段分开渲染 */
function formatLabel(item: PackItemDto): string {
  return t("resource.packFormat", { format: item.packFormat });
}
</script>

<template>
  <ContentHead :data="data">
  </ContentHead>

  <div v-if="loading" class="item-list">
    <ListSkeleton />
  </div>
  <div v-else class="item-list">
    <ResourceRow v-for="item in packs" :key="item.file" :icon="item.icon" :name="item.file">
      <template #badges>
        <span v-if="item.fail" class="badge badge-red">{{ t("resource.modFail") }}</span>
        <span v-if="item.enable" class="badge">{{ t("resource.packOn") }}</span>
      </template>
      <!--
        描述要过格式码，整合包作者常用 `§a` 这类上色；包格式那句不要 ——
        所以两段分开渲染、中间自己接分隔符，不能先拼成一个字符串再整体格式化
      -->
      <template #sub>
        <FormattedText v-if="item.description" :text="item.description" />
        <template v-if="item.description"> · </template>{{ formatLabel(item) }}
      </template>
      <template #actions>
        <button class="mini-btn" :disabled="busy" @click="toggle(item)">
          {{ item.enable ? t("resource.disable") : t("resource.enable") }}
        </button>
        <button class="mini-btn" :disabled="busy" @click="openFolder('resourcepacks', item.file)">
          {{ t("resource.openFolder") }}
        </button>
        <button class="mini-btn danger" :disabled="busy" @click="remove(item)">
          {{ t("resource.delete") }}
        </button>
      </template>
    </ResourceRow>
    <div v-if="!packs.length" class="empty-tip">{{ t("resource.empty") }}</div>
  </div>
</template>
