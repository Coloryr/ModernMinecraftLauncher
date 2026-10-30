<script setup lang="ts">
// 发现旧数据：ColorMC 与官方启动器 .minecraft 放在同一个弹窗里，各一节
//
// 两节各自独立：谁没问过就显示谁（"问过了"标记在后端，见 windows/colormc 与
// windows/minecraft），答完一节另一节照常操作。关掉整个弹窗不算回答，下次还问。
import { computed, ref } from "vue";
import type { ColorMcInfoDto, MinecraftInfoDto } from "../../lib/bindings";
import { t } from "../../lib/i18n";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import ColorMcMigrateSection from "./ColorMcMigrateSection.vue";
import MinecraftImportSection from "./MinecraftImportSection.vue";

const props = defineProps<{
  /** ColorMC 探测结果；null = 本机没有或已经问过 */
  colormc: ColorMcInfoDto | null;
  /** .minecraft 探测结果；null = 本机没有或已经问过 */
  minecraft: MinecraftInfoDto | null;
}>();

/** `done` 表示有一节搬完了数据，主窗口据此重拉实例 / 分组 / 账户 */
const emit = defineEmits<{ (e: "close"): void; (e: "done"): void }>();

/** 正在搬数据的节数：两节可以同时跑，按计数算，不能用一个布尔 */
const busySections = ref(0);

/** 有任务在跑时不许关弹窗 */
const busy = computed(() => busySections.value > 0);

/**
 * 子节报告忙碌状态
 *
 * - `value`: 是否正在搬数据
 */
function onBusy(value: boolean) {
  busySections.value = Math.max(0, busySections.value + (value ? 1 : -1));
}
</script>

<template>
  <!-- 右上角不画 ✕：关闭入口只留底部那个 -->
  <BaseModal :title="t('oldData.title')" :width="560" :closable="false" @close="emit('close')">
    <div class="old-modal">
      <p class="old-hint">{{ t("oldData.hint") }}</p>

      <ColorMcMigrateSection v-if="props.colormc" :info="props.colormc" @done="emit('done')"
        @busy="onBusy" />
      <MinecraftImportSection v-if="props.minecraft" :info="props.minecraft" @done="emit('done')"
        @busy="onBusy" />

      <div class="modal-actions">
        <BaseButton variant="accent" :disabled="busy" @click="emit('close')">{{
          t("oldData.close") }}</BaseButton>
      </div>
    </div>
  </BaseModal>
</template>

<style scoped>
.old-hint {
  margin: 0 0 12px;
  font-size: 12.5px;
  color: var(--text-dim);
}

/* 两节共用的外观：定义在弹窗这一层、用 :deep 穿透到子组件 ——
   同一套卡片样式在 ColorMC 与 .minecraft 各写一份就会走样，见 AGENTS §16.4 */
.old-modal :deep(.old-section) {
  padding: 12px 0;
  border-top: 1px solid var(--border);
}

.old-modal :deep(.old-section:first-of-type) {
  border-top: none;
  padding-top: 0;
}

.old-modal :deep(.old-head) {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 4px;
}

.old-modal :deep(.old-name) {
  font-size: 13px;
  font-weight: 700;
}

.old-modal :deep(.old-meta) {
  font-size: 11.5px;
  color: var(--text-dim);
}

/* 来源路径：等宽 + 任意位置断行（长路径不该撑破弹窗） */
.old-modal :deep(.src) {
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  color: var(--text);
  word-break: break-all;
}

.old-modal :deep(.warn) {
  margin: 10px 0 0;
  color: var(--text-dim);
  font-size: 12.5px;
}

.old-modal :deep(.error) {
  margin: 10px 0 0;
  color: var(--red);
  font-size: 12.5px;
  word-break: break-all;
}

.old-modal :deep(.settled) {
  margin: 8px 0 0;
  font-size: 12.5px;
  color: var(--text-dim);
}

.old-modal :deep(.old-actions) {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 10px;
}

/* 版本 / 实例名两行：标签定宽，控件占满剩余宽度 */
.old-modal :deep(.old-form) {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.old-modal :deep(.old-form .field-label) {
  flex-shrink: 0;
  width: 52px;
}

.old-modal :deep(.old-form .field-input),
.old-modal :deep(.old-form .field-select) {
  flex: 1;
  min-width: 0;
}

.old-modal :deep(.old-loader) {
  flex-shrink: 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

/* ---------- 进度 ---------- */

.old-modal :deep(.bar) {
  height: 6px;
  margin: 10px 0 6px;
  border-radius: 3px;
  overflow: hidden;
  background: var(--bg-hover);
}

.old-modal :deep(.bar-fill) {
  height: 100%;
  border-radius: 3px;
  background: var(--accent-grad, var(--accent));
  transition: width 0.15s ease;
}

.old-modal :deep(.bar.indet .bar-fill) {
  width: 40%;
  animation: old-data-run 1.1s ease-in-out infinite;
}

@keyframes old-data-run {
  0% {
    transform: translateX(-110%);
  }

  100% {
    transform: translateX(260%);
  }
}

.old-modal :deep(.run-info) {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  font-size: 12px;
  color: var(--text-dim);
}

.old-modal :deep(.run-info .cur) {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---------- 结果 ---------- */

.old-modal :deep(.line) {
  font-size: 12.5px;
  line-height: 1.7;
  word-break: break-all;
}

.old-modal :deep(.line.ok) {
  color: var(--text);
}

.old-modal :deep(.line.bad) {
  color: var(--red);
}

.old-modal :deep(.line.warn) {
  color: var(--text-dim);
}

.old-modal :deep(.compat) {
  margin-top: 10px;
}

.old-modal :deep(.compat-head) {
  font-size: 12px;
  color: var(--text-dim);
  margin-bottom: 4px;
}

.old-modal :deep(.compat-line) {
  display: flex;
  gap: 8px;
  align-items: baseline;
  margin-bottom: 4px;
}

.old-modal :deep(.tag) {
  flex: none;
  padding: 1px 7px;
  border-radius: 6px;
  font-size: 11px;
  background: var(--bg-hover);
  color: var(--text-dim);
  white-space: nowrap;
}

.old-modal :deep(.tag.ok) {
  background: color-mix(in srgb, var(--green) 14%, transparent);
  color: var(--green);
}

.old-modal :deep(.tag.partial) {
  background: color-mix(in srgb, var(--yellow) 16%, transparent);
  color: var(--yellow, var(--text));
}

.old-modal :deep(.names) {
  min-width: 0;
  font-size: 12px;
  color: var(--text-dim);
  line-height: 1.7;
}

.old-modal :deep(.names code) {
  font-family: "Cascadia Code", Consolas, monospace;
  margin-right: 8px;
  color: var(--text);
}
</style>
