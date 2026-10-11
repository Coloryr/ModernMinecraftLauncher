<script setup lang="ts">
// 数据包子页：先选存档（下拉），再列该存档里的数据包，可启用 / 禁用 / 删除
import { t } from "../../../lib/i18n";
import { stripFormatting } from "../../../lib/formatting";
import { deleteDatapack, toggleDatapack } from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import SaveTabs from "./SaveTabs.vue";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { DataPackItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，存档下拉、数据包列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化与确认弹窗从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { saves, datapacks, dpSave, instanceUuid, loading } = props.data;
const { busy, act, askDelete, openFolder } = props.ops;

function toggle(item: DataPackItemDto) {
  act(async () => {
    await toggleDatapack(instanceUuid.value, dpSave.value, item.name);
  });
}

function remove(item: DataPackItemDto) {
  askDelete(item.file, () => deleteDatapack(instanceUuid.value, dpSave.value, item.name));
}

/** 启用状态的显示名：开着 / 关着 / 没登记进 level.dat */
function state(item: DataPackItemDto): string {
  if (item.enable === true) return t("resource.dpOn");
  if (item.enable === false) return t("resource.dpOff");
  return t("resource.dpNone");
}

/** 显示名：去掉 NBT 登记名里的 `file/` 前缀 */
function name(item: DataPackItemDto): string {
  return item.name.startsWith("file/") ? item.name.slice(5) : item.file || item.name;
}

/** 包格式那段文案；描述要过格式码，这一句不要，所以两段分开渲染 */
function formatLabel(item: DataPackItemDto): string {
  return t("resource.packFormat", { format: item.packFormat });
}
</script>

<template>
  <ContentHead :data="data">
    <SaveTabs :data="data" />
  </ContentHead>

  <div class="item-list">
    <select v-model="dpSave" class="field-select dp-select">
      <option value="" disabled>{{ t("resource.selectSave") }}</option>
      <option v-for="s in saves" :key="s.dir" :value="s.dir">
        {{ stripFormatting(s.levelName || s.dir) }}
      </option>
    </select>

    <template v-if="dpSave">
      <!-- 加载中用骨架行占位：与其它分类同一套；这里只换行，存档下拉要一直留着 -->
      <ListSkeleton v-if="loading" />
      <ResourceRow v-for="item in datapacks" :key="item.name" :name="name(item)">
        <template #badges>
          <span class="badge" :class="{ 'badge-red': item.enable === false }">
            {{ state(item) }}
          </span>
        </template>
        <template #sub>
          <!-- 描述过格式码，数据包作者会用 `§a` 上色；包格式那句不过 -->
          <FormattedText v-if="item.description" :text="item.description" />
          <template v-if="item.description"> · </template>{{ formatLabel(item) }}
        </template>
        <template #actions>
          <button class="mini-btn" :disabled="busy" @click="toggle(item)">
            {{ item.enable === false ? t("resource.enable") : t("resource.disable") }}
          </button>
          <button class="mini-btn" :disabled="busy" @click="openFolder('datapacks', null, dpSave)">
            {{ t("resource.openFolder") }}
          </button>
          <button class="mini-btn danger" :disabled="busy" @click="remove(item)">
            {{ t("resource.delete") }}
          </button>
        </template>
      </ResourceRow>
      <div v-if="!datapacks.length && !loading" class="empty-tip">{{ t("resource.empty") }}</div>
    </template>
    <div v-else class="empty-tip">{{ t("resource.selectSave") }}</div>
  </div>
</template>
