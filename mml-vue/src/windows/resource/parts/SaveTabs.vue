<script setup lang="ts">
// 存档分类的子页切换：存档 / 数据包（存档页与数据包页共用这一段）
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import { t } from "../../../lib/i18n";
import type { useResourceData } from "../composables/useResourceData";
import type { SaveTab } from "../types";

const props = defineProps<{
  /** 数据层，子页选中值存在它里面，切换即写回 */
  data: ReturnType<typeof useResourceData>;
}>();
const { saveTab } = props.data;
</script>

<template>
  <SegmentedTabs :model-value="saveTab" :options="[
    { value: 'saves', label: t('resource.saves') },
    { value: 'datapacks', label: t('resource.datapacks') },
  ]" @update:model-value="saveTab = $event as SaveTab" />
</template>
