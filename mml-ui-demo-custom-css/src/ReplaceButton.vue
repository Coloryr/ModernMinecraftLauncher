<script setup lang="ts">
// 替换后的实例启动按钮：入口函数经 ctx.replace 把它挂到原按钮位置
defineProps<{
  /** 按钮文字 */
  text: string;
}>();

const emit = defineEmits<{
  /** 点击启动 */
  (e: "launch"): void;
}>();
</script>

<template>
  <button type="button" class="mml-custom-launch" @click="emit('launch')">
    <span class="mml-custom-launch-icon" aria-hidden="true">▶</span>
    <span>{{ text }}</span>
  </button>
</template>
