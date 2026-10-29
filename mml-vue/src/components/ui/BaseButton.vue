<script setup lang="ts">
// 统一按钮，五种变体：primary 主操作、accent 强调、plain 次要、danger 危险、ghost 图标类。
// **外观全在 styles/controls.css**（.ui-btn / .v-* / .s-*），组件只负责把类拼起来，
// 这样裸 <button class="ui-btn v-plain s-sm"> 与组件是同一副外观，不会各写一份。
withDefaults(
  defineProps<{
    /** 外观变体；primary 主操作、accent 强调、plain 次要、danger 危险、ghost 图标类 */
    variant?: "primary" | "accent" | "plain" | "danger" | "ghost";
    /** 尺寸档；sm / md 定高 --ctl-h（32px），只差左右内边距；lg 不给高度，靠内边距撑开且字号大一档 */
    size?: "sm" | "md" | "lg";
    /** 是否禁用 */
    disabled?: boolean;
    /** 是否占满整行宽度 */
    block?: boolean;
  }>(),
  { variant: "plain", size: "md", disabled: false, block: false },
);
</script>

<template>
  <button class="ui-btn" :class="[`v-${variant}`, `s-${size}`, { block }]" :disabled="disabled">
    <slot />
  </button>
</template>
