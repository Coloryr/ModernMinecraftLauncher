<script setup lang="ts">
// 启动时的旧数据升级进度：盖住整个界面，升级结束后自动消失
//
// 升级跑在核心加载之前，前端可能比它晚挂载，所以挂载时先补读一次状态，
// 再订阅进度事件；没有升级任务时整块不存在。
// 用户可以随时停止：旧文件留着，下次启动接着导，已导入的记录不会丢。
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { t } from "../lib/i18n";
import { commands, type UpgradeStatusDto } from "../lib/bindings";
import { UpgradeProgress } from "../lib/listens";
import { showToast } from "../lib/toast";
import BaseButton from "./ui/BaseButton.vue";

/** 当前升级状态；还没查到时按"没有任务"处理 */
const status = ref<UpgradeStatusDto>({
  running: false,
  done: true,
  cancelled: false,
  now: 0,
  total: 0,
});

/** 只在升级进行中显示 */
const visible = computed(() => status.value.running);

/** 进度百分比；总数还不知道时返回 `null`，进度条走无限滚动 */
const percent = computed(() => {
  const total = status.value.total;
  return total > 0 ? Math.min(100, (status.value.now / total) * 100) : null;
});

/** 停止本次升级：已经导入的记录保留，下次启动接着导 */
async function stop() {
  await commands.windows.cancelUpgrade().catch(() => undefined);
}

// 聊胜于无地给一个反馈：停止后遮罩消失，用轻提示补一句
watch(
  () => [status.value.cancelled, status.value.running] as const,
  ([cancelled, running]) => {
    if (cancelled && !running) {
      showToast(t("upgrade.stopped"));
    }
  },
);

/** 进度事件订阅的退订函数 */
let unlisten: UnlistenFn | null = null;
/** 组件已卸载：订阅晚到就立刻退掉 */
let disposed = false;

onMounted(async () => {
  const off = await listen<UpgradeStatusDto>(UpgradeProgress, (event) => {
    status.value = event.payload;
  });
  if (disposed) {
    off();
    return;
  }
  unlisten = off;
  // 挂载得比升级晚：补读一次当前状态
  status.value = await commands.windows.upgradeStatus().catch(() => status.value);
});

onUnmounted(() => {
  disposed = true;
  unlisten?.();
  unlisten = null;
});
</script>

<template>
  <div v-if="visible" class="upgrade-layer">
    <div class="upgrade-card">
      <span class="upgrade-title">{{ t("upgrade.title") }}</span>
      <span class="upgrade-hint">{{ t("upgrade.hint") }}</span>
      <div class="progress-track">
        <div class="progress-fill" :class="{ 'is-roll': percent === null }"
          :style="percent === null ? undefined : { width: percent + '%' }" />
      </div>
      <div class="upgrade-foot">
        <span v-if="percent !== null" class="upgrade-count">{{ status.now }} / {{ status.total }}</span>
        <span v-else class="upgrade-count" />
        <BaseButton size="sm" @click="stop">{{ t("upgrade.stop") }}</BaseButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 启动画面级别的遮罩：必须盖住一切，直到数据升级完成或用户停止 */
.upgrade-layer {
  position: fixed;
  inset: 0;
  z-index: 999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg);
}

.upgrade-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: 360px;
  padding: 24px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-modal);
}

.upgrade-title {
  font-size: 15px;
  color: var(--text);
}

.upgrade-hint {
  font-size: 12.5px;
  color: var(--text-sub);
}

.upgrade-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.upgrade-count {
  font-size: 11.5px;
  color: var(--text-sub);
}
</style>
