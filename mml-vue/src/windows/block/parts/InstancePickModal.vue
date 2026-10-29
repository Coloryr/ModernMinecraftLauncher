<script setup lang="ts">
// 方块列表 · 设为实例图标：下拉选一个实例，点确认后父级把方块设成它的图标
import { computed, onMounted, ref } from "vue";
import { t } from "../../../lib/i18n";
import { useModalKeys } from "../../../composables/useModalKeys";
import BaseModal from "../../../components/ui/BaseModal.vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import { api } from "../../../lib/api";
import type { InstanceInfoDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 当前实例 uuid：下拉里默认选中它 */
  currentUuid: string | null;
  /** 正在设图标：禁用下拉与按钮，Esc 不关 */
  busy: boolean;
}>();

const emit = defineEmits<{
  /** 确认了一个实例，父级接着把方块设为它的图标 */
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
/** 下拉当前选中的实例 uuid；空串表示还没选 */
const selected = ref("");

/** 下拉里选中的实例；没选时为 null */
const picked = computed(() => instances.value.find((inst) => inst.uuid === selected.value) ?? null);

onMounted(async () => {
  try {
    instances.value = await api.getInstances();
    if (props.currentUuid && instances.value.some((inst) => inst.uuid === props.currentUuid)) {
      selected.value = props.currentUuid;
    }
  } catch {
    instances.value = [];
  } finally {
    loading.value = false;
  }
});

/**
 * 下拉选项文案：名字 · 版本，当前实例额外标出来
 *
 * - `inst`: 要显示的那个实例
 *
 * # 返回值
 *
 * 拼好的单行文案
 */
function labelOf(inst: InstanceInfoDto): string {
  const parts = [inst.name, inst.version];
  if (inst.uuid === props.currentUuid) parts.push(t("blocks.currentInstance"));
  return parts.join(" · ");
}

/** 确认选择：没选中实例时什么都不做 */
function confirm() {
  if (picked.value) emit("pick", picked.value);
}
</script>

<template>
  <BaseModal :title="t('blocks.pickInstance')" :width="380" :closable="false" @close="busy || emit('close')">
    <p class="pick-desc">{{ t("blocks.pickInstanceDesc") }}</p>

    <div v-if="loading" class="empty-tip">{{ t("blocks.loading") }}</div>
    <div v-else-if="!instances.length" class="empty-tip">{{ t("blocks.noInstances") }}</div>
    <select v-else v-model="selected" class="field-select" :disabled="busy">
      <option value="" disabled>{{ t("blocks.pickInstance") }}</option>
      <option v-for="inst in instances" :key="inst.uuid" :value="inst.uuid">{{ labelOf(inst) }}</option>
    </select>

    <div class="modal-actions">
      <BaseButton :disabled="busy" @click="emit('close')">{{ t("add.cancel") }}</BaseButton>
      <BaseButton v-if="instances.length" variant="primary" :disabled="!picked || busy" @click="confirm">
        {{ t("blocks.setIcon") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
.pick-desc {
  font-size: 12.5px;
  color: var(--text-dim);
  margin-bottom: 10px;
}
</style>
