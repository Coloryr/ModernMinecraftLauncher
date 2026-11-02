<script setup lang="ts">
// 手柄面板：设备下拉 + 重新扫描 + 外观 + 实时状态图
//
// 复用内核采样快照，本身不发送输入，也不启动第二套设备线程。
// 外观优先用后端按手柄认出来的那份，认不出才回落到配置里手动选的那个；
// A / B 交换始终按配置来，由调用方传下来、改完再 emit 回去。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  commands,
  type GamepadDto,
  type GamepadLayoutDto,
  type GamepadStateDto,
} from "../../../lib/bindings";
import { t, tErr } from "../../../lib/i18n";
import { usePageActive } from "../../../lib/pageActive";
import { isTauri } from "../../windowManager";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import GamepadDiagram from "./GamepadDiagram.vue";

const props = defineProps<{
  /** 配置里保存的外观；没有可编辑的配置时为 null，选项区不显示 */
  layout: GamepadLayoutDto | null;
  /** 配置里的 A / B 交换；没有可编辑的配置时为 null */
  swapAb: boolean | null;
}>();

const emit = defineEmits<{
  /** 后端认出来的外观；没接手柄或认不出时为 null */
  (e: "detected", value: GamepadLayoutDto | null): void;
  /** 手动换外观：xbox / playstation */
  (e: "update:layout", value: GamepadLayoutDto): void;
  /** 换 A / B 位置 */
  (e: "update:swap-ab", value: boolean): void;
}>();

const devices = ref<GamepadDto[]>([]);
const selected = ref<number | null>(null);
const state = ref<GamepadStateDto | null>(null);
const error = ref("");
const refreshing = ref(false);
const active = usePageActive();
let disposed = false;
let timer: ReturnType<typeof setTimeout> | undefined;

/** 当前选中的设备 */
const device = computed(() => devices.value.find((item) => item.id === selected.value) ?? null);

/** 后端认出来的外观；认不出时为 null */
const detected = computed(() => device.value?.layout ?? null);

/** 画图用的外观：认得出用手柄的，认不出用配置里那份 */
const effectiveLayout = computed(() => detected.value ?? props.layout ?? "xbox");

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
 * 认出来的外观叫什么
 *
 * - `value`: xbox / playstation
 */
function layoutName(value: GamepadLayoutDto): string {
  return value === "playstation" ? t("winGamepad.layoutPlaystation") : t("winGamepad.layoutXbox");
}

/**
 * 下拉项的文字
 *
 * 系统名后面带上认出来的外观：名字是 Windows 给的通用名（HID-compliant game controller）
 * 时，用户也能一眼看出有没有被认成 Xbox / PlayStation。
 *
 * - `item`: 手柄
 */
function deviceLabel(item: GamepadDto): string {
  return item.layout ? `${item.name} · ${layoutName(item.layout)}` : item.name;
}

// 认出来的外观要报给上层：按钮映射表里的记号跟着它变
watch(detected, (value) => emit("detected", value), { immediate: true });

/**
 * 选中一个设备
 *
 * - `id`: 设备编号；null 表示没有可用的手柄
 */
function select(id: number | null) {
  if (id === selected.value) return;
  selected.value = id;
  // 换设备后旧快照不属于它，等下一轮轮询再画
  state.value = null;
}

/**
 * 下拉换设备
 *
 * - `event`: 下拉的 change 事件
 */
function onPickDevice(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  select(value === "" ? null : Number(value));
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

/**
 * 问一次设备列表与选中设备的快照
 *
 * 选中项跟着列表走：断开后回到第一个，一个都没有就清空。
 */
async function sync() {
  const available = await commands.gamepad.list();
  if (disposed) return;
  devices.value = available;
  if (!available.some((item) => item.id === selected.value)) {
    select(available[0]?.id ?? null);
  }
  const id = selected.value;
  const snapshot = id === null ? null : await commands.gamepad.state(id);
  if (disposed) return;
  if (id === selected.value) state.value = snapshot;
}

/** 重新扫描：轮询本身也在刷新，这颗按钮让“刚连上”立刻出现在下拉里 */
async function refresh() {
  if (refreshing.value || !isTauri()) return;
  refreshing.value = true;
  try {
    await sync();
    error.value = "";
  } catch (reason) {
    error.value = tErr(reason);
  } finally {
    refreshing.value = false;
  }
}

/** 串行查询，窗口停用或销毁后不再轮询 */
async function poll() {
  if (disposed || !isTauri()) return;
  if (active.value) {
    try {
      await sync();
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

    <!-- 手柄下拉 + 重新扫描：选中哪个就画哪个，断开后回到第一个 -->
    <div class="gp-device-row">
      <select v-if="devices.length" class="field-select gp-device-select"
        :value="selected === null ? '' : String(selected)" :aria-label="t('winGamepad.devices')"
        @change="onPickDevice">
        <option v-for="item in devices" :key="item.id" :value="String(item.id)">{{ deviceLabel(item) }}</option>
      </select>
      <p v-else class="field-hint gp-device-empty">{{ t("winGamepad.testEmpty") }}</p>
      <button class="ui-btn-icon" :disabled="refreshing" v-tip="t('winGamepad.refresh')"
        :aria-label="t('winGamepad.refresh')" @click="refresh">
        <GlyphIcon name="refresh" :size="14" :class="{ 'is-spin': refreshing }" />
      </button>
    </div>

    <p v-if="error" class="gp-error" role="alert">{{ error }}</p>

    <!-- 外观只影响记号，不改映射；没有可编辑的配置时不显示 -->
    <div v-if="layout !== null" class="gp-look-wrap">
      <div class="gp-look">
        <div class="gp-look-item">
          <span class="field-label">{{ t("winGamepad.layoutTitle") }}</span>
          <!-- 认得出手柄就用它的外观，认不出才给手动选项 -->
          <span v-if="detected" class="gp-detected">{{ t("winGamepad.detected", { name: layoutName(detected) }) }}</span>
          <SegmentedTabs v-else :model-value="layout" :options="layoutOptions"
            @update:model-value="pickLayout" />
        </div>
        <div class="gp-look-item">
          <span class="field-label">{{ t("winGamepad.swapAbTitle") }}</span>
          <SegmentedTabs :model-value="swapAb ? 'swapped' : 'normal'" :options="swapOptions"
            @update:model-value="pickSwap" />
        </div>
      </div>
      <p class="field-hint">{{ t("winGamepad.layoutHint") }}</p>
    </div>

    <GamepadDiagram v-if="state" :state="state" :layout="effectiveLayout" :swap-ab="swapAb ?? false" />
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

/* 设备下拉：占住宽度，扫描按钮跟在后面 */
.gp-device-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.gp-device-select {
  flex: 0 1 320px;
  min-width: 0;
}

.gp-device-empty {
  margin: 0;
  flex: 1;
}

/* 自动认出来的外观：与旁边的分段控件同高、单行显示 */
.gp-detected {
  display: flex;
  align-items: center;
  min-height: var(--ctl-h);
  font-size: var(--ctl-font);
  color: var(--text);
}

/* 扫描中：图标转起来，和依赖检测那颗同一个节奏 */
.is-spin {
  animation: gp-spin 0.9s linear infinite;
}

@keyframes gp-spin {
  to {
    transform: rotate(360deg);
  }
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
