<script setup lang="ts">
// 初始化失败页：错误原文 + 问题反馈入口，由 SplashScreen 在 splashError 非空时挂出。
import { t } from "../../lib/i18n";
import BaseButton from "./BaseButton.vue";

defineProps<{
  /** 初始化失败信息，原样显示 */
  error: string;
}>();

const emit = defineEmits<{
  /** 点「问题反馈」 */
  (e: "feedback"): void;
}>();
</script>

<template>
  <div class="boot-error">
    <div class="boot-error-card">
      <div class="boot-error-icon">!</div>
      <h1>{{ t("init.failed") }}</h1>
      <p class="boot-error-text">{{ error }}</p>
      <BaseButton class="boot-error-btn" @click="emit('feedback')">
        {{ t("init.feedback") }}
      </BaseButton>
    </div>
  </div>
</template>

<style scoped>
.boot-error {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(160deg, var(--bg-side) 0%, var(--bg) 100%);
}

.boot-error-card {
  width: 440px;
  padding: 36px;
  border-radius: 14px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  box-shadow: var(--shadow-lg);
  display: flex;
  flex-direction: column;
  align-items: center;
}

.boot-error-icon {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: var(--splash-error-bg);
  color: var(--splash-error);
  font-size: 30px;
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
}

.boot-error-card h1 {
  font-size: 18px;
  margin: 14px 0 10px;
  color: var(--text);
}

.boot-error-text {
  width: 100%;
  margin-bottom: 22px;
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--bg-side);
  border: 1px solid var(--border);
  color: var(--text-dim);
  font-size: 13px;
  word-break: break-all;
  max-height: 180px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
}

.boot-error-btn {
  min-width: 120px;
}
</style>
