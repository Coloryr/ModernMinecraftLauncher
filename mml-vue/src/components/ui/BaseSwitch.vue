<script setup lang="ts">
// 基础 Switch 开关：两态设置项用，如单/多窗口、左/右侧栏、动画开关
defineProps<{
  /** 开关状态，配合 `v-model` 使用 */
  modelValue: boolean;
  /** 是否禁用 */
  disabled?: boolean;
}>();

const emit = defineEmits<{
  /** `v-model` 更新：新的开关状态 */
  (e: "update:modelValue", v: boolean): void;
}>();
</script>

<template>
  <button type="button" role="switch" class="switch" :class="{ on: modelValue }" :aria-checked="modelValue"
    :disabled="disabled" @click="emit('update:modelValue', !modelValue)">
    <span class="knob" />
  </button>
</template>

<style scoped>
.switch {
  width: 40px;
  height: 22px;
  flex-shrink: 0;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-hover);
  cursor: pointer;
  position: relative;
  padding: 0;
  transition: background 0.15s, border-color 0.15s;
}

.switch:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--text-dim);
  transition: transform 0.15s, background 0.15s;
}

.switch.on {
  background: var(--accent);
  border-color: var(--accent);
}

.switch.on .knob {
  transform: translateX(18px);
  background: #fff;
}
</style>
