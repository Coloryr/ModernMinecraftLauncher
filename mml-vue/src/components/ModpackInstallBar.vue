<script setup lang="ts">
// 整合包安装进度条（多任务）：总进度条 + 任务数，每个任务的详情
// 阶段 / 主子进度 / 取消按钮，默认可以点击标题行折叠；终态任务带状态标签，可一键清除。
// 当前由右下角的进度弹窗（ModpackPopup）使用；内容区与主窗口改用标题栏指示器。
import { computed, ref } from "vue";
import CollapsePanel from "./ui/CollapsePanel.vue";
import { api } from "../lib/api";
import { t, tErr } from "../lib/i18n";
import { showToast } from "../lib/toast";
import type { ModPackStatusDto, ModPackTaskDto } from "../lib/bindings";
import AsyncImage from "./ui/AsyncImage.vue";
import { isRunning } from "../lib/modpackTasks";

const props = withDefaults(
  defineProps<{
    /** 整合包安装任务状态 */
    status: ModPackStatusDto;
    /** 任务详情是否可以折叠。默认可以；
     *  进度弹窗传 false —— 那里本来就是点开来看详情的，再折一层等于白点一次 */
    collapsible?: boolean;
    /** 撑满父容器剩余高度、任务多了内部自己滚（弹窗锁死高度时用）；
     *  不传时卡片跟着内容自适应 */
    fillHeight?: boolean;
    /** 底部是否给「关闭」按钮；弹窗形态用 */
    showClose?: boolean;
  }>(),
  { collapsible: true, fillHeight: false, showClose: false },
);

const emit = defineEmits<{
  /** 点底部的「关闭」；由弹窗收起自己 */
  (e: "close"): void;
}>();

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

/** 单任务主进度，单位 %；总数未知时按 0 算，别凭"不是进行中"就当成完成 */
function taskProgress(task: ModPackTaskDto): number {
  if (task.done) return 100;
  return task.total ? (task.now / task.total) * 100 : 0;
}

/**
 * 把解压条目的完整路径压成一行，保留末尾文件名
 *
 * - `path`: 压缩包内路径
 * - `max`: 最多显示多少字符，超出时优先保留上级目录与扩展名
 *
 * # 返回值
 *
 * 形如 `.../父目录/文件名`；文件名本身过长时缩短中段
 */
function shortPath(path: string, max = 64): string {
  const normalized = path.replace(/\\/g, "/");
  if (normalized.length <= max) return normalized;

  const parts = normalized.split("/").filter(Boolean);
  const file = parts.pop() ?? normalized;
  const parent = parts.pop();
  const dot = file.lastIndexOf(".");
  const ext = dot > 0 && dot < file.length - 1 ? file.slice(dot) : "";
  const base = ext ? file.slice(0, dot) : file;

  /** 把文件名缩成 head...tail */
  const shortenBase = (budget: number) => {
    if (base.length <= budget) return base;
    const head = Math.max(1, Math.ceil(budget / 2));
    return `${base.slice(0, head)}...${base.slice(-(budget - head))}`;
  };

  const prefix = parent ? `${parent}/` : "";
  const budget = max - 4 - prefix.length - ext.length - 3;
  if (budget >= 8) {
    return `.../${prefix}${shortenBase(budget)}${ext}`;
  }

  // 上级目录太长：只保留文件名，仍然缩短中段并保留扩展名
  const onlyFileBudget = Math.max(8, max - 4 - ext.length - 3);
  return `.../${shortenBase(onlyFileBudget)}${ext}`;
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
    <!-- 总进度条：只有多个任务时才画；单个任务下面那条就是它自己的进度，
         再画一条总进度只会重复。全结束时也不画，满格蓝杠没有信息量 -->
    <div v-if="running.length > 1" class="progress-track">
      <div class="progress-fill" :style="{ width: percent + '%' }" />
    </div>

    <!-- 展开的任务详情；fillHeight 时滚动落在 CollapsePanel 内层，见 .mp-bar.fill-h -->
    <CollapsePanel :open="open">
      <div class="mp-task-list">
        <div v-for="task in status.tasks" :key="task.uuid" class="mp-task">
          <div class="mp-task-head">
            <!-- 整合包图标：一眼认出装的是哪个包；没有就用首字母占位 -->
            <AsyncImage v-if="task.icon" class="mp-task-icon" :src="task.icon" alt="" />
            <span v-else class="mp-task-icon mp-task-icon-fallback">{{ task.name.slice(0, 1).toUpperCase() }}</span>
            <span class="mp-task-name">{{ task.name }}</span>
            <span v-if="task.done" class="mp-tag done">{{ t("add.packState.done") }}</span>
            <span v-else-if="task.failed" class="mp-tag failed" v-tip="task.error ? tErr(task.error) : ''">{{ t("modpack.bar.failed")
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
               已完成才走 100%。进行中有子进度时让位，避免一条阶段条压在子进度条上面不动 -->
          <div v-if="!task.failed && !task.cancelled && !(isRunning(task) && task.subTotal > 0)"
            class="progress-track sub">
            <div class="progress-fill" :style="{ width: taskProgress(task) + '%' }" />
          </div>
          <template v-if="isRunning(task) && (task.subText || task.subTotal)">
            <div class="mp-task-sub" v-tip="task.subText || ''">{{ shortPath(task.subText || "") }}</div>
            <div class="progress-track sub">
              <div class="progress-fill"
                :style="{ width: task.subTotal ? (task.subNow / task.subTotal) * 100 + '%' : '0%' }" />
            </div>
          </template>
        </div>
      </div>
      <!-- 清掉已结束的任务：放在列表底部 —— 放标题行右侧时紧挨着标题，
           和弹窗自己的 ✕ 挤在一起，分不清哪个是哪个 -->
      <div v-if="!fillHeight && finished.length" class="mp-footer">
        <button class="mp-clear-btn" @click="clearDone">
          {{ t("modpack.bar.clearDone") }}
        </button>
      </div>
    </CollapsePanel>

    <!-- 弹窗形态：操作条常驻在弹窗底部，清除已完成与关闭都靠右，不再爬标题栏那一行 -->
    <div v-if="fillHeight && (finished.length || showClose)" class="mp-actions">
      <button v-if="finished.length" class="mp-clear-btn" @click="clearDone">
        {{ t("modpack.bar.clearDone") }}
      </button>
      <button v-if="showClose" class="mp-close-btn" @click="emit('close')">
        {{ t("actions.close") }}
      </button>
    </div>
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
   高度由弹窗那侧传 fillHeight 打开 */
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

/* 弹窗底部操作条：与任务列表用一条分隔线隔开，按钮全部靠右 */
.mp-actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
  margin-top: 10px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}

/* 关闭：与主操作按钮同档，但不是"危险/强调"，用次要外观 */
.mp-close-btn {
  height: var(--ctl-h);
  padding: 0 20px;
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  font-size: var(--ctl-font);
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}

.mp-close-btn:hover {
  background: var(--bg-hover);
  border-color: var(--accent);
}

/* 展开指示：用 SVG 画的箭头。原来是 12px 的字符「▾」，太小且粗细随字体变形 */
.mp-bar-chevron {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  color: var(--text-dim);
  transition: transform 0.18s;
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
  font-size: 12.5px;
  color: var(--text-dim);
}

.mp-task-state span {
  margin-left: 6px;
}

.mp-task-sub {
  font-size: 12.5px;
  color: var(--text-dim);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mp-task-error {
  font-size: 12.5px;
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

.progress-track.sub {
  height: 6px;
}

/* ---------- 弹窗形态：任务排成项目列表，不再套"一个大框" ---------- */
/* 只作用于 fill-h（弹窗那侧传的）；默认形态仍保留底色与描边 */
.mp-bar.fill-h {
  background: none;
  border: none;
  border-radius: 0;
  padding: 0;
}

.mp-bar.fill-h .mp-task-list {
  gap: 0;
  padding-top: 6px;
}

/* 弹窗里一条任务就是一"项目"：第一行图标 + 名字 + 操作，下面阶段 / 进度条 / 文件名
   统一缩进到名字下面，读起来是同一块，而不是几行左对齐的长条 */
.mp-bar.fill-h .mp-task {
  padding: 10px 0 12px;
  gap: 6px;
  border-bottom: 1px solid var(--border);
}

.mp-bar.fill-h .mp-task:last-child {
  border-bottom: none;
}

.mp-bar.fill-h .mp-task-icon {
  width: 28px;
  height: 28px;
  border-radius: 7px;
}

/* 28px 图标 + 8px 间距：详情与名字左边缘对齐 */
.mp-bar.fill-h .mp-task-state,
.mp-bar.fill-h .mp-task-sub,
.mp-bar.fill-h .mp-task-error,
.mp-bar.fill-h .mp-task>.progress-track {
  margin-left: 36px;
}
.mp-task-icon {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  flex-shrink: 0;
  object-fit: cover;
}

/* 没有图标时用首字母占位，与实例图标的回退口径一致 */
.mp-task-icon-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-hover);
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
}
</style>
