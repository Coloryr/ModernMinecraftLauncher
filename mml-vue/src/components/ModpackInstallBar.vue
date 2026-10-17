<script setup lang="ts">
// 整合包安装进度条（多任务）：总进度条 + 任务数，每个任务的详情
// 阶段 / 主子进度 / 取消按钮，默认可以点击标题行折叠；终态任务带状态标签，可一键清除。
// 下载整合包窗口常显；右下角的进度弹窗与多窗口模式的主窗口也用它（由调用方 v-if 控制）。
import { computed, ref } from "vue";
import CollapsePanel from "./ui/CollapsePanel.vue";
import { api } from "../lib/api";
import { t, tErr } from "../lib/i18n";
import { showToast } from "../lib/toast";
import type { ModPackStatusDto, ModPackTaskDto } from "../lib/bindings";

const props = withDefaults(
  defineProps<{
    /** 整合包安装任务状态 */
    status: ModPackStatusDto;
    /** 任务详情是否可以折叠。默认可以（页面里为了省地方）；
     *  弹窗形态传 false —— 那里本来就是点开来看详情的，再折一层等于白点一次 */
    collapsible?: boolean;
    /** 撑满父容器剩余高度、任务多了内部自己滚（弹窗锁死高度时用）。
     *  页面里那两处不传：卡片跟着内容自适应即可 */
    fillHeight?: boolean;
  }>(),
  { collapsible: true, fillHeight: false },
);

/** 是否展开任务详情：不可折叠时恒为展开 */
const open = ref(!props.collapsible);

/** 进行中的任务（终态之外） */
const running = computed(() =>
  props.status.tasks.filter((task) => isRunning(task)),
);

/** 已结束的任务（完成 / 失败 / 取消） */
const finished = computed(() =>
  props.status.tasks.filter((task) => !isRunning(task)),
);

/** 总进度 = 各任务主进度的均值 */
const percent = computed(() => {
  if (!running.value.length) {
    return 100;
  }
  const sum = running.value.reduce(
    (acc, task) => acc + (task.total ? (task.now / task.total) * 100 : 0),
    0,
  );
  return sum / running.value.length;
});

/** 任务是否还在跑 */
function isRunning(task: ModPackTaskDto): boolean {
  return !task.done && !task.failed && !task.cancelled;
}

/** 单任务主进度，单位 %；总数未知时按 0 算，别凭"不是进行中"就当成完成 */
function taskProgress(task: ModPackTaskDto): number {
  if (task.done) return 100;
  return task.total ? (task.now / task.total) * 100 : 0;
}

/** 标题行文案：还有在装的按"正在安装 N 个"说，全结束了改说"已结束"（别再报 0 个正在安装） */
const headText = computed(() =>
  running.value.length
    ? t("modpack.bar.running", { count: running.value.length })
    : t("modpack.bar.finished", { count: props.status.tasks.length }),
);

async function cancel(task: ModPackTaskDto) {
  try {
    await api.cancelModpackInstall(task.pid, task.fid);
  } catch (e) {
    showToast(t("modpack.bar.cancelFail", { msg: tErr(e) }));
  }
}

async function clearDone() {
  try {
    await api.clearModpackDone();
  } catch (e) {
    showToast(tErr(e));
  }
}
</script>

<template>
  <div class="mp-bar" :class="{ 'fill-h': fillHeight }">
    <!-- 总览：标题（任务数）+ 展开开关；可折叠时点击整行切换 -->
    <!-- "清除已完成"在任务列表底部，见下面的 .mp-footer -->
    <div class="mp-bar-head" :class="{ 'no-toggle': !collapsible }" @click="collapsible && (open = !open)">
      <span class="mp-bar-title">{{ headText }}</span>
      <span v-if="collapsible" class="mp-bar-chevron" :class="{ up: open }" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </span>
    </div>
    <!-- 总进度条：只在真有任务在跑时画。全结束时它是条满格蓝杠，什么信息都没表达，
         还占着标题下面一整行，配合"清除已完成"的 ✕ 整块看着像还在下载 -->
    <div v-if="running.length" class="progress-track">
      <div class="progress-fill" :style="{ width: percent + '%' }" />
    </div>

    <!-- 展开的任务详情；fillHeight 时滚动落在 CollapsePanel 内层，见 .mp-bar.fill-h -->
    <CollapsePanel :open="open">
      <div class="mp-task-list">
        <div v-for="task in status.tasks" :key="task.uuid" class="mp-task">
          <div class="mp-task-head">
            <span class="mp-task-name">{{ task.name }}</span>
            <span v-if="task.done" class="mp-tag done">{{ t("add.packState.done") }}</span>
            <span v-else-if="task.failed" class="mp-tag failed" v-tip="task.error || ''">{{ t("modpack.bar.failed")
            }}</span>
            <span v-else-if="task.cancelled" class="mp-tag cancelled">{{ t("modpack.bar.cancelled") }}</span>
            <button v-else class="mp-cancel" @click="cancel(task)">{{ t("modpack.bar.cancel") }}</button>
          </div>
          <div v-if="isRunning(task)" class="mp-task-state">
            {{ t(`add.packState.${task.state}`) }}
            <span v-if="task.total">{{ task.now }} / {{ task.total }}</span>
          </div>
          <div v-else-if="task.failed && task.error" class="mp-task-error">
            {{ tErr(task.error) }}
          </div>
          <!-- 任务进度：失败 / 取消的任务不再画条。原来兜底给 100%，失败的任务下面就
               挂着一条满格的灰条，看着像"下载完了"，与旁边的"失败"标签自相矛盾。
               已完成才走 100% -->
          <div v-if="!task.failed && !task.cancelled" class="progress-track sub">
            <div class="progress-fill" :style="{ width: taskProgress(task) + '%' }" />
          </div>
          <template v-if="isRunning(task) && (task.subText || task.subTotal)">
            <div class="mp-task-sub">{{ task.subText || "" }}</div>
            <div class="progress-track sub">
              <div class="progress-fill"
                :style="{ width: task.subTotal ? (task.subNow / task.subTotal) * 100 + '%' : '0%' }" />
            </div>
          </template>
        </div>
      </div>
      <!-- 清掉已结束的任务：放在列表底部 —— 放标题行右侧时紧挨着标题，
           和弹窗自己的 ✕ 挤在一起，分不清哪个是哪个 -->
      <div v-if="finished.length" class="mp-footer">
        <button class="mp-clear-btn" @click="clearDone">
          {{ t("modpack.bar.clearDone") }}
        </button>
      </div>
    </CollapsePanel>
  </div>
</template>

<style scoped>
/* 左右 18px：卡片边缘 26 + 18 = 44，卡内内容与同窗口其它卡片对齐。
   不用 gap 分行：收起时 CollapsePanel 高度为 0，flex 的 gap 仍会在它前面留一道间隔，
   卡片就成了上 10px 下 16px。间距改由各块自己带，收起时随内层一起被裁掉 */
.mp-bar {
  display: flex;
  flex-direction: column;
  padding: 10px 18px;
  border-radius: 10px;
  background: var(--bg-card);
  border: 1px solid var(--border);
}

/* 标题与总进度条的间距，原来由 .mp-bar 的 gap 提供 */
.mp-bar>.progress-track {
  margin-top: 6px;
}

/* 弹窗形态：外层 BaseModal 锁了高度，内容区是 flex 列。
   本卡片撑满剩余高度，任务多了由任务区自己滚，不再把弹窗顶高。
   高度由弹窗那侧传 fillHeight 打开，页面里那两处保持自适应 */
.mp-bar.fill-h {
  flex: 1;
  min-height: 0;
}

/* 滚动落在 CollapsePanel 的内层：外层是 grid，靠 grid-template-rows 做展开过渡、
   自身不滚。展开状态下 1fr 行撑满，内层 overflow 出来就是滚动条 */
.mp-bar.fill-h :deep(.collapse) {
  flex: 1;
  min-height: 0;
}

.mp-bar.fill-h :deep(.collapse.open) {
  grid-template-rows: 1fr;
  overflow: hidden;
}

.mp-bar.fill-h :deep(.collapse.open > .collapse-inner) {
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行不往左缩（槽位由上面的 scrollbar-gutter 常驻），滚动条画在这 8px 上
     （与项目列表 .res-list 同一做法，见 styles/scrollbar.css） */
  margin-right: -8px;
  padding-right: 8px;
}

.mp-bar-head {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;
}

/* 不折叠时这一行只是标题：别给手型光标，免得看着像能点 */
.mp-bar-head.no-toggle {
  cursor: default;
}

.mp-bar-title {
  flex: 1;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

/* 清掉已结束的任务：列表底部一条文字按钮，与任务列表隔一条分隔线。
   靠右放：与弹窗、表单底部的按钮同一个位置口径，居中看着像块独立内容 */
.mp-footer {
  display: flex;
  justify-content: flex-end;
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--border);
}

.mp-clear-btn {
  height: var(--ctl-h);
  padding: 0 14px;
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text-dim);
  font-size: var(--ctl-font);
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: color 0.12s, border-color 0.12s;
}

.mp-clear-btn:hover {
  color: var(--text);
  border-color: var(--accent);
}

/* 展开指示：用 SVG 画的箭头。原来是 12px 的字符「▾」，太小且粗细随字体变形 */
.mp-bar-chevron {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  color: var(--text-dim);
  transition: transform 0.22s ease;
}

.mp-bar-chevron.up {
  transform: rotate(180deg);
}

.mp-task-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  /* 10px = 原来 .mp-bar 的 6px gap 加这里的 4px。间距放进内层，
     收起时才会跟着被裁掉，不在卡片底部留下空白 */
  padding-top: 10px;
}

.mp-task {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.mp-task-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mp-task-name {
  flex: 1;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  word-break: break-all;
}

.mp-task-state {
  font-size: 12px;
  color: var(--text-dim);
}

.mp-task-state span {
  margin-left: 6px;
}

.mp-task-sub {
  font-size: 12px;
  color: var(--text-dim);
  word-break: break-all;
}

.mp-task-error {
  font-size: 12px;
  color: var(--red);
  word-break: break-all;
}

.mp-tag {
  font-size: 11.5px;
  padding: 1px 8px;
  border-radius: 999px;
  flex-shrink: 0;
}

.mp-tag.done {
  color: var(--green);
  background: color-mix(in srgb, var(--green) 14%, transparent);
}

.mp-tag.failed {
  color: var(--red);
  background: color-mix(in srgb, var(--red) 14%, transparent);
}

.mp-tag.cancelled {
  color: var(--text-dim);
  background: var(--bg-hover);
}

.mp-cancel {
  display: inline-flex;
  align-items: center;
  height: var(--ctl-h);
  font-size: var(--ctl-font);
  font-weight: 600;
  color: var(--red);
  background: var(--bg-raised);
  border: 1px solid color-mix(in srgb, var(--red) 40%, transparent);
  border-radius: var(--ctl-radius);
  padding: 0 14px;
  cursor: pointer;
  flex-shrink: 0;
  font-family: inherit;
}

.mp-cancel:hover {
  background: color-mix(in srgb, var(--red) 12%, transparent);
}

.progress-track {
  height: 8px;
  border-radius: 4px;
  background: var(--bg-hover);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 4px;
  background: var(--accent-grad);
  transition: width 0.3s;
}

.progress-fill.dim {
  background: var(--text-dim);
  opacity: 0.5;
}

.progress-track.sub {
  height: 6px;
}
</style>
