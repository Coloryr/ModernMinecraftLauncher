<script setup lang="ts">
// 游戏统计窗口：汇总卡片 + 每实例启动次数 / 时长表
// 数据来自 Rust 侧 stats_get_data，即内核 game_count::CountObj 快照；运行中每 5 秒刷新
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import InstanceIcon from "../../components/InstanceIcon.vue";
import { api } from "../../lib/api";
import { t } from "../../lib/i18n";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import type { StatsDataDto } from "../../lib/bindings";

// 必须显式声明 close：不声明的话 Vue 会把父级的 @close 当 attrs 透传到根组件（WindowFrame），
// 与模板里的 @close="$emit('close')" 合并成两个处理器 —— 一次返回会调两遍 closeWindow()，
// 第二遍时 currentKind 已经回到 main，于是"返回"变成退出应用
/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

const data = ref<StatsDataDto | null>(null);

let timer: number | null = null;

async function load() {
  try {
    data.value = await api.getStatsData();
  } catch {
    // 拉取失败保留上次数据
  }
}

function startTimer() {
  if (timer === null) {
    timer = window.setInterval(load, 5000);
  }
}

function stopTimer() {
  if (timer !== null) {
    window.clearInterval(timer);
    timer = null;
  }
}

onMounted(async () => {
  await load();
  startTimer();
});

// 单窗口模式：组件被 KeepAlive 缓存，切走不卸载。切走停掉轮询（别在后台空转 IPC），
// 切回先补一次数据再继续，useWindowRefresh 就是"切回时补一次"
useWindowRefresh(load);
onActivated(startTimer);
onDeactivated(stopTimer);

// 多窗口模式：每个窗口是独立 webview，窗口被最小化 / 隐藏时**不会**触发 onDeactivated，
// 光靠上面那对钩子会一直空转。这里再按文档可见性停一次；单窗口模式下切页面时
// document 始终可见，两者互不干扰。
function onVisibilityChange() {
  if (document.hidden) {
    stopTimer();
  } else {
    void load();
    startTimer();
  }
}
onMounted(() => document.addEventListener("visibilitychange", onVisibilityChange));

onBeforeUnmount(() => {
  stopTimer();
  document.removeEventListener("visibilitychange", onVisibilityChange);
});

/** 总游戏时长，单位小时 */
const totalHours = computed(() =>
  data.value ? (data.value.totalSeconds / 3600).toFixed(1) : "0.0",
);

/** 时长列文本 */
function hoursOf(seconds: number) {
  return t("winStats.hours", { h: (seconds / 3600).toFixed(1) });
}

/** 最近游玩时间：epoch 毫秒转 YYYY-MM-DD HH:mm */
function fmtTime(ms: number | null) {
  if (!ms) return t("detail.none");
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
</script>

<template>
  <WindowFrame :title="t('features.stats')" body-gutter @close="$emit('close')">
    <div class="summary">
      <div class="summary-card">
        <span class="summary-num">{{ data?.instances.length ?? 0 }}</span>
        <span class="summary-label">{{ t("winStats.instances") }}</span>
      </div>
      <div class="summary-card">
        <span class="summary-num">{{ data?.launchCount ?? 0 }}</span>
        <span class="summary-label">{{ t("winStats.totalLaunch") }}</span>
      </div>
      <div class="summary-card">
        <span class="summary-num">{{ totalHours }}h</span>
        <span class="summary-label">{{ t("winStats.totalPlay") }}</span>
      </div>
    </div>

    <table class="stats-table">
      <thead>
        <tr>
          <th>{{ t("winStats.colInstance") }}</th>
          <th>{{ t("winStats.colCount") }}</th>
          <th>{{ t("winStats.colHours") }}</th>
          <th>{{ t("winStats.colLast") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="inst in data?.instances ?? []" :key="inst.uuid">
          <td class="inst-cell">
            <InstanceIcon :name="inst.name" :uuid="inst.uuid" :size="30" />
            <span>{{ inst.name }}</span>
            <span v-if="inst.running" class="running-tag">{{ t("winStats.running") }}</span>
          </td>
          <td>{{ inst.count }}</td>
          <td>{{ hoursOf(inst.seconds) }}</td>
          <td>{{ fmtTime(inst.last) }}</td>
        </tr>
        <tr v-if="!data || data.instances.length === 0">
          <td colspan="4" class="empty-cell">{{ t("winStats.noData") }}</td>
        </tr>
      </tbody>
    </table>

    <p class="foot-note">{{ t("winStats.foot") }}</p>
  </WindowFrame>
</template>

<style scoped>
.summary {
  display: flex;
  gap: 14px;
  margin-bottom: 18px;
}

.summary-card {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 16px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
}

.summary-num {
  font-size: 22px;
  font-weight: 800;
  color: var(--accent);
}

.summary-label {
  font-size: 12px;
  color: var(--text-dim);
}

.stats-table {
  width: 100%;
  border-collapse: collapse;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.stats-table th {
  text-align: left;
  font-size: 12px;
  color: var(--text-dim);
  font-weight: 600;
  padding: 12px 16px;
  background: var(--bg-side);
  border-bottom: 1px solid var(--border);
}

.stats-table td {
  padding: 11px 16px;
  font-size: 13px;
  border-bottom: 1px solid var(--border);
}

.stats-table tr:last-child td {
  border-bottom: none;
}

.inst-cell {
  display: flex;
  align-items: center;
  gap: 10px;
  font-weight: 600;
}

.running-tag {
  font-size: 10.5px;
  font-weight: 600;
  color: var(--accent);
  border: 1px solid var(--accent);
  border-radius: 6px;
  padding: 1px 7px;
}

.empty-cell {
  text-align: center;
  color: var(--text-dim);
  padding: 24px;
}

.foot-note {
  font-size: 11.5px;
  color: var(--text-dim);
  margin-top: 10px;
}
</style>
