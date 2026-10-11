<script setup lang="ts">
// 自定义执行：启动前 / 启动后命令（可开关）
import { t } from "../lib/i18n";
import type { InstanceArgsDto } from "../lib/bindings";

const props = defineProps<{
  /** 当前实例的启动参数 */
  args: InstanceArgsDto;
}>();

const emit = defineEmits<{
  /** 回传改动后的整份启动参数 */
  (e: "update:args", v: InstanceArgsDto): void;
}>();

/** 合并 patch 后回传整份参数 */
function update(patch: Partial<InstanceArgsDto>) {
  emit("update:args", { ...props.args, ...patch });
}
</script>

<template>
  <div class="exec-panel">
    <div class="exec-row">
      <span class="exec-label">{{ t("exec.pre") }}</span>
      <input type="checkbox" class="exec-chk" :checked="args.preEnabled"
        @change="update({ preEnabled: ($event.target as HTMLInputElement).checked })" />
    </div>
    <textarea class="exec-text" rows="2" :value="args.preCmd" :disabled="!args.preEnabled"
      :placeholder="t('exec.preContent')" spellcheck="false"
      @input="update({ preCmd: ($event.target as HTMLTextAreaElement).value })"></textarea>

    <div class="exec-row">
      <span class="exec-label">{{ t("exec.post") }}</span>
      <input type="checkbox" class="exec-chk" :checked="args.postEnabled"
        @change="update({ postEnabled: ($event.target as HTMLInputElement).checked })" />
    </div>
    <textarea class="exec-text" rows="2" :value="args.postCmd" :disabled="!args.postEnabled"
      :placeholder="t('exec.postContent')" spellcheck="false"
      @input="update({ postCmd: ($event.target as HTMLTextAreaElement).value })"></textarea>
  </div>
</template>

<style scoped>
.exec-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px 16px;
}

.exec-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.exec-label {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-dim);
}

.exec-chk {
  accent-color: var(--accent);
}

.exec-text {
  width: 100%;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg);
  color: var(--text);
  font-size: 12.5px;
  font-family: "Cascadia Code", Consolas, "Courier New", monospace;
  resize: vertical;
  outline: none;
}

.exec-text:focus {
  border-color: var(--accent);
}

.exec-text:disabled {
  opacity: 0.5;
}
</style>
