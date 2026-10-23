<script setup lang="ts">
// 方块图标选择：从已渲染的方块列表里挑一个，设为实例图标
//
// 网格直接复用方块窗口的 BlockGrid（自带虚拟滚动、搜索高亮与空态），
// 这里只负责拉列表、按关键字过滤、点选后写图标。
import { computed, onMounted, ref } from "vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import SearchBox from "../../components/ui/SearchBox.vue";
import BlockGrid from "../block/parts/BlockGrid.vue";
import { blockSetIcon, getBlockList } from "../../lib/api";
import { locale, t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import type { BlockItemDto } from "../../lib/bindings";

const props = defineProps<{
  /** 目标实例 uuid */
  uuid: string;
  /** 目标实例名，成功提示里用 */
  name: string;
}>();

const emit = defineEmits<{ (e: "close"): void }>();

/** 全部方块；打开时按界面语言拉一次 */
const items = ref<BlockItemDto[]>([]);
/** 搜索关键字，空串 = 不过滤 */
const keyword = ref("");
const loading = ref(true);
/** 正在写图标：连点只算一次 */
const busy = ref(false);

/** 关键字同时匹配 id 与显示名 */
const shown = computed(() => {
  const q = keyword.value.trim().toLowerCase();
  if (!q) return items.value;
  return items.value.filter((b) => b.id.toLowerCase().includes(q) || b.name.toLowerCase().includes(q));
});

onMounted(async () => {
  try {
    items.value = await getBlockList(locale.value === "en_us" ? "en_us" : "zh_cn");
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
});

/**
 * 点一个方块：直接设为图标，成功就关掉
 *
 * - `block`: 被点中的方块条目
 */
async function pick(block: BlockItemDto) {
  if (busy.value) return;
  busy.value = true;
  try {
    await blockSetIcon(props.uuid, block.id);
    showToast(t("blocks.setIconOk", { name: props.name }));
    emit("close");
  } catch (e) {
    showToast(tErr(e));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <BaseModal :title="t('blocks.pickIconTitle')" :width="620" fixed-height="560px" @close="emit('close')">
    <div class="pick">
      <SearchBox v-model="keyword" :placeholder="t('blocks.search')" :clear-label="t('blocks.searchClear')"
        width="100%" />
      <div class="pick-grid">
        <p v-if="loading" class="pick-hint">{{ t("blocks.loading") }}</p>
        <BlockGrid v-else :items="shown" :keyword="keyword" size="sm" :active-id="null" skin-cat=""
          :filtered="!!keyword.trim()" @open="pick" @clear-filters="keyword = ''" />
      </div>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 弹窗内容区在 fixed-height 下是 flex 列，这里让网格吃掉剩余高度 */
.pick {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-height: 0;
}

.pick-grid {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.pick-hint {
  margin: auto;
  color: var(--text-dim);
  font-size: var(--ctl-font);
}
</style>
