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
  border-radius: 999px;
  white-space: nowrap;
  background: var(--bg-hover);
  color: var(--text-dim);
}

.acc-type.microsoft {
  background: var(--auth-microsoft-bg);
  color: var(--auth-microsoft);
}

.acc-type.littleskin {
  background: var(--auth-littleskin-bg);
  color: var(--auth-littleskin);
}

.acc-type.authlib {
  background: var(--auth-authlib-bg);
  color: var(--yellow);
}

.acc-type.nide8 {
  background: var(--auth-nide8-bg);
  color: var(--auth-nide8);
}

</style>
