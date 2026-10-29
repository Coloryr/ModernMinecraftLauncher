<script setup lang="ts">
// 就地一行加载进度，跟在触发它的字段 / 控件下面
//
// 用处：添加实例窗口的"从头新建"（加载器类型 / 加载器版本）、实例设置面板、
// 添加文件夹的目录扫描。
// 两种形态：有总数时走确定进度条，如查询支持的加载器按加载器步进；
// 无总数的场景走**不确定滚动条**，小色块来回滚：拉取加载器版本、扫描目录。
//
// 摆放：就地，铺满所在字段那一列的宽度（由调用方那边排布）。
import { computed } from "vue";
import { t } from "../lib/i18n";

const props = defineProps<{
  /** 是否显示 */
  visible: boolean;
  /** 任务类型，决定默认文案与是否有步进；query = 查询支持的加载器，versions = 拉取加载器版本等无步进任务 */
  kind: "query" | "versions";
  /** 已完成的步数 */
  step: number;
  /** 总步数；为 0 时按不确定进度显示 */
  total: number;
  /** 自定义文案；不传则按 `kind` 取默认文案 */
  label?: string;
}>();

const label = computed(
  () =>
    props.label ??
    (props.kind === "query" ? t("add.loaderQuerying") : t("add.loaderVerLoading")),
);
/** 没有步数可报时用不确定动画（进度条宽度也没有意义） */
const determinate = computed(() => props.kind === "query" && props.total > 0);
const percent = computed(() => (props.total ? (props.step / props.total) * 100 : 0));
</script>

<template>
  <Transition name="load-progress">
    <div v-if="visible" class="load-progress" role="status" aria-live="polite">
      <span class="load-progress-spinner"></span>
      <span class="load-progress-label">{{ label }}</span>
      <div class="load-progress-bar">
        <div v-if="determinate" class="load-progress-fill" :style="{ width: percent + '%' }"></div>
        <div v-else class="load-progress-indet"></div>
      </div>
      <span v-if="determinate" class="load-progress-text">{{ step }} / {{ total }}</span>
    </div>
  </Transition>
</template>

<style scoped>
/* 字段下面的一行：与 .field-hint 同一档视觉重量（透明底、无描边），
   宽度铺满整列，与它上面那个下拉同宽 */
.load-progress {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  margin-top: 6px;
  font-size: 12.5px;
  color: var(--text);
}

.load-progress-spinner {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  animation: load-progress-spin 0.8s linear infinite;
  flex-shrink: 0;
}

@keyframes load-progress-spin {
  to {
    transform: rotate(360deg);
  }
}

.load-progress-label {
  color: var(--text-dim);
  white-space: nowrap;
}

/* 进度条吃掉剩余宽度：旋转圈 / 文案 / 步数是固定宽的，条子自己撑满剩下的部分 */
.load-progress-bar {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  background: var(--border);
  overflow: hidden;
}

.load-progress-fill {
  height: 100%;
  border-radius: 3px;
  background: var(--accent);
  transition: width 0.3s;
}

/* 不确定进度：小色块来回滚动 */
.load-progress-indet {
  height: 100%;
  width: 40%;
  border-radius: 3px;
  background: var(--accent);
  animation: load-progress-slide 1.1s ease-in-out infinite;
}

@keyframes load-progress-slide {
  from {
    transform: translateX(-100%);
  }

  to {
    transform: translateX(300%);
  }
}

.load-progress-text {
  color: var(--text-dim);
  min-width: 30px;
  text-align: right;
}

.load-progress-enter-active,
.load-progress-leave-active {
  transition: opacity 0.18s, transform 0.18s;
}

.load-progress-enter-from,
.load-progress-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
