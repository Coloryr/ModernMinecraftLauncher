<script setup lang="ts">
// 方块列表 · 设为实例图标：列出所有实例，点条目即设（当前实例高亮并带标记）
import { onMounted, ref } from "vue";
import { t } from "../../../lib/i18n";
import { useModalKeys } from "../../../composables/useModalKeys";
import BaseModal from "../../../components/ui/BaseModal.vue";
import InstanceIcon from "../../../components/InstanceIcon.vue";
import { api } from "../../../lib/api";
import type { InstanceInfoDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 当前实例 uuid（列表里高亮它） */
  currentUuid: string | null;
  /** 正在设图标：禁用条目，Esc 不关 */
  busy: boolean;
}>();

const emit = defineEmits<{
  /** 选中一个实例，父级接着把方块设为它的图标 */
  (e: "pick", inst: InstanceInfoDto): void;
  /** 请求关闭弹窗，设图标进行中不会触发 */
  (e: "close"): void;
}>();

useModalKeys((e) => {
  if (e.key === "Escape" && !props.busy) emit("close");
});

const instances = ref<InstanceInfoDto[]>([]);
/** 首次拉取中：先别显示「还没有游戏实例」，否则会闪一下错提示 */
const loading = ref(true);

onMounted(async () => {
  try {
    instances.value = await api.getInstances();
  } catch {
    instances.value = [];
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <BaseModal :title="t('blocks.pickInstance')" :width="430" @close="busy || emit('close')">
    <p class="pick-desc">{{ t("blocks.pickInstanceDesc") }}</p>

    <div v-if="instances.length" class="pick-list">
      <button v-for="inst in instances" :key="inst.uuid" type="button" class="pick-item"
        :class="{ on: inst.uuid === currentUuid }" :disabled="busy" @click="emit('pick', inst)">
        <InstanceIcon :name="inst.name" :uuid="inst.uuid" :size="30" />
        <span class="pick-text">
          <span class="pick-name">{{ inst.name }}</span>
          <span class="pick-sub">{{ inst.version }}</span>
        </span>
        <span v-if="inst.uuid === currentUuid" class="pick-cur">
          {{ t("blocks.currentInstance") }}
        </span>
      </button>
    </div>
    <div v-else-if="loading" class="empty-tip">{{ t("blocks.loading") }}</div>
    <div v-else class="empty-tip">{{ t("blocks.noInstances") }}</div>
  </BaseModal>
</template>

<style scoped>
.pick-desc {
  font-size: 12.5px;
  color: var(--text-dim);
  margin-bottom: 10px;
}

.pick-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 320px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行不往左缩（槽位由上面的 scrollbar-gutter 常驻），滚动条画在这 8px 上
     （与项目列表 .res-list 同一做法，见 styles/scrollbar.css） */
  margin-right: -8px;
  padding-right: 8px;
}

.pick-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid transparent;
  border-radius: 10px;
  background: transparent;
  color: var(--text);
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}

.pick-item:hover:not(:disabled) {
  background: var(--bg-hover);
  border-color: var(--accent);
}

.pick-item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.pick-item:disabled {
  cursor: default;
  opacity: 0.6;
}

.pick-item.on {
  background: var(--accent-soft);
  border-color: var(--accent-border);
}

.pick-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  line-height: 1.3;
}

.pick-name {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pick-sub {
  font-size: 11.5px;
  color: var(--text-dim);
}

.pick-cur {
  flex-shrink: 0;
  font-size: 10px;
  padding: 0 6px;
  border-radius: 8px;
  background: var(--accent-soft);
  color: var(--accent);
  line-height: 1.7;
}
</style>
