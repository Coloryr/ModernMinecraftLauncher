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
  <!-- 排版走工具类：字号 / 间距取的是启动器主题的档位，颜色仍由 custom.css 的渐变负责 -->
  <button type="button"
    class="mml-custom-launch inline-flex items-center justify-center gap-2 text-heading font-bold cursor-pointer"
    @click="emit('launch')">
    <span class="mml-custom-launch-icon" aria-hidden="true">▶</span>
    <span>{{ text }}</span>
  </button>
</template>
