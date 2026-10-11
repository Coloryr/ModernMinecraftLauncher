<script setup lang="ts">
// 账户类型徽章：平铺 / 列表 / 详情三个视图共用一套配色
import { computed } from "vue";
import { t } from "../lib/i18n";
import { typeLabelKey } from "../lib/accountStore";

const props = defineProps<{
  /** 账户认证类型，取值如 microsoft / littleskin / authlib / nide8，决定配色 */
  authType: string;
}>();

const label = computed(() => t(typeLabelKey(props.authType)));
</script>

<template>
  <span class="acc-type" :class="authType">{{ label }}</span>
</template>

<style scoped>
.acc-type {
  display: inline-block;
  font-size: 10.5px;
  padding: 2px 8px;
  border-radius: 20px;
  white-space: nowrap;
  background: var(--bg-hover);
  color: var(--text-dim);
}

.acc-type.microsoft {
  background: rgba(63, 140, 255, 0.16);
  color: #8fb0ff;
}

.acc-type.littleskin {
  background: rgba(168, 85, 247, 0.16);
  color: #c084fc;
}

.acc-type.authlib {
  background: rgba(245, 158, 11, 0.16);
  color: var(--yellow);
}

.acc-type.nide8 {
  background: rgba(6, 182, 212, 0.16);
  color: #22d3ee;
}

/* 亮色主题：浅色文字落在白卡片上看不清，换深色系 */
[data-theme="Light"] .acc-type.microsoft {
  color: #2563eb;
}

[data-theme="Light"] .acc-type.littleskin {
  color: #9333ea;
}

[data-theme="Light"] .acc-type.nide8 {
  color: #0e7490;
}
</style>
