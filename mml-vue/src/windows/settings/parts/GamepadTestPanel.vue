<script setup lang="ts">
// 手柄面板：设备列表 + 外观选项 + 实时状态图
//
// 复用内核采样快照，本身不发送输入，也不启动第二套设备线程。
// 外观与 A / B 交换存在配置里，由调用方传下来、改完再 emit 回去。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  commands,
  type GamepadDto,
  type GamepadLayoutDto,
  type GamepadStateDto,
} from "../../../lib/bindings";
import { t, tErr } from "../../../lib/i18n";
import { usePageActive } from "../../../lib/pageActive";
import { isTauri } from "../../windowManager";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import GamepadDiagram from "./GamepadDiagram.vue";

defineProps<{
  /** 当前配置的外观；没有可编辑的配置时为 null，选项区不显示 */
  layout: GamepadLayoutDto | null;
  /** 当前配置的 A / B 交换；没有可编辑的配置时为 null */
  swapAb: boolean | null;
}>();

const emit = defineEmits<{
  /** 换外观：xbox / playstation */
  (e: "update:layout", value: GamepadLayoutDto): void;
  /** 换 A / B 位置 */
  (e: "update:swap-ab", value: boolean): void;
}>();

const devices = ref<GamepadDto[]>([]);
const selected = ref<number | null>(null);
const state = ref<GamepadStateDto | null>(null);
const error = ref("");
const active = usePageActive();
let disposed = false;
let timer: ReturnType<typeof setTimeout> | undefined;

/** 外观选项 */
const layoutOptions = computed(() => [
  { value: "xbox", label: t("winGamepad.layoutXbox") },
  { value: "playstation", label: t("winGamepad.layoutPlaystation") },
]);

/** A / B 选项：值是"是否交换"，这里转成两档字符串给分段控件 */
const swapOptions = computed(() => [
  { value: "normal", label: t("winGamepad.swapAbNormal") },
  { value: "swapped", label: t("winGamepad.swapAbSwapped") },
]);

/**
 * 选中一个设备
 *
 * - `id`: 设备编号
 */
function select(id: number) {
  if (id === selected.value) return;
  selected.value = id;
  // 换设备后旧快照不属于它，等下一轮轮询再画
  state.value = null;
}

/**
 * 换外观
 *
 * - `value`: `xbox` / `playstation`
 */
function pickLayout(value: string) {
  emit("update:layout", value === "playstation" ? "playstation" : "xbox");
}

/**
 * 换 A / B 位置
 *
 * - `value`: `normal` / `swapped`
 */
function pickSwap(value: string) {
  emit("update:swap-ab", value === "swapped");
}

/** 串行查询，窗口停用或销毁后不再轮询。 */
async function poll() {
  if (disposed || !isTauri()) return;
  if (active.value) {
    try {
      const available = await commands.gamepad.list();
      if (disposed) return;
      devices.value = available;
      if (!available.some((device) => device.id === selected.value)) {
        selected.value = available[0]?.id ?? null;
        state.value = null;
      }
      const id = selected.value;
      const snapshot = id === null ? null : await commands.gamepad.state(id);
      if (disposed) return;
      if (id === selected.value) state.value = snapshot;
      error.value = "";
    } catch (reason) {
      if (!disposed) {
        state.value = null;
        error.value = tErr(reason);
      }
    }
  }
  if (!disposed) timer = setTimeout(() => void poll(), 120);
}

onMounted(() => void poll());
onBeforeUnmount(() => {
  disposed = true;
  if (timer) clearTimeout(timer);
});
</script>

<template>
  <section class="gp-test" :aria-label="t('winGamepad.testTitle')">
    <div class="gp-test-head">
      <h3 class="gp-test-title">{{ t("winGamepad.devices") }}</h3>
      <p class="field-hint">{{ t("winGamepad.testHint") }}</p>
    </div>

    <!-- 手柄列表：选中哪个就画哪个，断开后回到列表第一个 -->
    <div v-if="devices.length" class="gp-devices">
      <button v-for="device in devices" :key="device.id" type="button" class="gp-device"
        :class="{ 'is-active': device.id === selected }" @click="select(device.id)">
        {{ device.name }}
      </button>
    </div>
    <p v-else class="field-hint">{{ t("winGamepad.testEmpty") }}</p>

    <p v-if="error" class="gp-error" role="alert">{{ error }}</p>

    <!-- 外观只影响记号，不改映射；没有可编辑的配置时不显示 -->
    <div v-if="layout !== null" class="gp-look-wrap">
      <div class="gp-look">
        <div class="gp-look-item">
          <span class="field-label">{{ t("winGamepad.layoutTitle") }}</span>
          <SegmentedTabs :model-value="layout" :options="layoutOptions" @update:model-value="pickLayout" />
        </div>
        <div class="gp-look-item">
          <span class="field-label">{{ t("winGamepad.swapAbTitle") }}</span>
          <SegmentedTabs :model-value="swapAb ? 'swapped' : 'normal'" :options="swapOptions"
            @update:model-value="pickSwap" />
        </div>
      </div>
      <p class="field-hint">{{ t("winGamepad.layoutHint") }}</p>
    </div>

    <GamepadDiagram v-if="state" :state="state" :layout="layout ?? 'xbox'" :swap-ab="swapAb ?? false" />
  </section>
</template>

<style scoped>
.gp-test {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  margin-bottom: 20px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
}

.gp-test-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex-wrap: wrap;
}

.gp-test-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}

/* 设备列表：一行一个胶囊，选中用强调色描边 */
.gp-devices {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.gp-device {
  padding: 6px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-card);
  color: var(--text);
  font-family: inherit;
  font-size: 12.5px;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s, color 0.12s;
}

.gp-device:hover {
  border-color: var(--accent-border);
}

.gp-device.is-active {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
}

.gp-look {
  display: flex;
  gap: 24px;
  flex-wrap: wrap;
}

.gp-look-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.gp-error {
  margin: 0;
  color: var(--red);
  font-size: var(--ctl-font);
}
</style>