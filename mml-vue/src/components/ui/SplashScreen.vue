<script setup lang="ts">
// 启动画面：初始化中显示 splash，初始化失败显示错误页（splashError）+ 问题反馈入口
//
// 加载期间**主窗口顶栏照常渲染**，见 MainWindow 模板，所以这里不需要自己的关闭按钮：
// 系统装饰在窗口一出现就被插件摘掉，关闭由顶栏那三个窗口按钮负责。
import { t } from "../../lib/i18n";
import BaseButton from "./BaseButton.vue";

withDefaults(
  defineProps<{
    /** 是否显示启动画面；为 false 且 `splashError` 非空时显示错误页 */
    splashVisible: boolean;
    /** 初始化失败信息；空串表示没失败 */
    splashError: string;
  }>(),
  {
    splashVisible: true,
    splashError: "",
  },
);

/** 点错误页上的"问题反馈" */
const emit = defineEmits<{
  (e: "feedback"): void;
}>();
</script>

<template>
  <!-- 启动画面：初始化中，由 closeSplash() 关闭 -->
  <div v-if="splashVisible" class="splash">
    <div class="splash-logo">MC</div>
    <div class="splash-name">{{ t("app.name") }}</div>
    <div class="splash-spinner"></div>
    <div class="splash-text">{{ t("init.splash") }}</div>
  </div>

  <!-- 初始化失败错误页：closeSplash(error) 后显示 -->
  <div v-else-if="splashError" class="boot-error">
    <div class="boot-error-card">
      <div class="boot-error-icon">!</div>
      <h1>{{ t("init.failed") }}</h1>
      <p class="boot-error-text">{{ splashError }}</p>
      <BaseButton class="boot-error-btn" @click="emit('feedback')">
        {{ t("init.feedback") }}
      </BaseButton>
    </div>
  </div>
</template>

<style scoped>
/* 初始化期间的关闭按钮：浮在右上角，不占布局 */
.splash-close {
  position: fixed;
  top: 10px;
  right: 10px;
  width: 34px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  cursor: default;
  transition: background 0.12s, color 0.12s;
}

.splash-close:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.splash {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  background: linear-gradient(160deg, var(--bg-side) 0%, var(--bg) 100%);
}

.splash-logo {
  width: 72px;
  height: 72px;
  border-radius: 18px;
  background: linear-gradient(135deg, #4f8cff, #7c5cff);
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 800;
  font-size: 30px;
  color: #fff;
  box-shadow: 0 8px 24px rgba(79, 140, 255, 0.35);
}

.splash-name {
  font-size: 20px;
  font-weight: 700;
  color: var(--text);
}

.splash-spinner {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: 3px solid var(--border);
  border-top-color: var(--accent);
  animation: splash-rotate 0.9s linear infinite;
  margin-top: 8px;
}

@keyframes splash-rotate {
  to {
    transform: rotate(360deg);
  }
}

.splash-text {
  font-size: 13px;
  color: var(--text-dim);
}

/* 初始化失败错误页 */
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
  background: rgba(255, 99, 99, 0.14);
  color: #ff6363;
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
