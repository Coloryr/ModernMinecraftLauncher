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
  /* 比全 app 控件那一档（--ctl-h = 32px）矮一档：开关是成对出现的小控件，
     32px 高的胶囊放在工具条里比旁边的文案与勾选都抢眼。
     宽高比与圆点直径只在这一处定，圆点的位移由它们推出来 */
  --sw-h: 24px;
  --sw-knob: calc(var(--sw-h) - 6px);
  width: calc(var(--sw-h) * 1.5);
  height: var(--sw-h);
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
  width: var(--sw-knob);
  height: var(--sw-knob);
  border-radius: 50%;
  background: var(--text-dim);
  transition: transform 0.15s, background 0.15s;
}

.switch.on {
  background: var(--accent);
  border-color: var(--accent);
}

.switch.on .knob {
  /* 位移 = 内部可用宽 - 圆点 - 两侧内缩 = 1.5H - (H - 6) - 6 = H / 2。
     原来写的 H - 8 会多推一段，圆点在"开"时凸出开关右缘 */
  transform: translateX(calc(var(--sw-h) / 2));
  background: #fff;
}
</style>
