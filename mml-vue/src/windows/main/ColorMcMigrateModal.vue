<script setup lang="ts">
// 加载期间问一次：把 ColorMC 的数据搬过来（复制 / 移动 / 不迁移）
//
// 由主窗口在**加载页还盖着的时候**发起，见 MainWindow 的 colorMcReady：答完或直接关掉
// 才关启动页，所以搬运发生在主页面之前。三个按钮都算回答，右上角关掉不算 —— 下次还问。
//
// 文案一律从简：来源 + 实例数 + 一句话提醒 + 三个按钮；结果页只列条目名，
// 每条兼容性结论的长说明挂在 `title` 上（悬停才看）。
import { computed, ref } from "vue";
import type { ColorMcInfoDto, ColorMcProgressDto, ColorMcReportDto } from "../../lib/bindings";
import { commands } from "../../lib/bindings";
import { api, onColormcProgress } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";

/** ColorMC 探测结果：数据目录、命中来源与可迁移实例数 */
const props = defineProps<{ info: ColorMcInfoDto }>();
/** 收起弹窗 / 迁移成功；`done` 表示数据已搬完，父组件据此重拉实例与分组 */
const emit = defineEmits<{ (e: "close"): void; (e: "done"): void }>();

/** 迁移进行中（按钮禁用 + 不允许关闭） */
const running = ref(false);
/** 最近一次进度 */
const progress = ref<ColorMcProgressDto | null>(null);
/** 结果报告（非空 → 显示结果页） */
const report = ref<ColorMcReportDto | null>(null);
/** 失败信息（后端给的是 i18n key） */
const error = ref("");

/** 进度事件取消订阅 */
let unlisten: (() => void) | null = null;

/** 进行中的阶段文案 */
const stageText = computed(() => (progress.value ? t(`colormc.stage.${progress.value.stage}`) : ""));

/** 百分比；total 为 0（未知）返回 -1 = 不确定进度 */
const percent = computed(() => {
  const p = progress.value;
  if (!p || p.total <= 0) return -1;
  return Math.min(100, Math.max(0, Math.round((p.done / p.total) * 100)));
});

/** 命中来源；后端只给 run / app / default / fallback */
const fromText = computed(() => t(`colormc.from.${props.info.from}`));

/** 兼容性结论按级别分档（每档一行条目名） */
const compatGroups = computed(() => {
  const list = report.value?.compat ?? [];
  return (["ok", "partial", "extra"] as const)
    .map((level) => ({ level, items: list.filter((item) => item.level === level) }))
    .filter((group) => group.items.length > 0);
});

/**
 * 开始迁移：订阅进度事件，跑完把结果放进 `report`
 *
 * - `mode`: `copy` 保留源数据 / `move` 搬走后删除源数据
 */
async function run(mode: "copy" | "move") {
  if (running.value) return;
  running.value = true;
  error.value = "";
  report.value = null;
  progress.value = null;
  try {
    unlisten = await onColormcProgress((p) => {
      progress.value = p;
    });
    report.value = await api.migrateColorMc(props.info.path, mode);
    emit("done");
  } catch (e) {
    error.value = tErr(e);
  } finally {
    unlisten?.();
    unlisten = null;
    running.value = false;
  }
}

/** 不迁移：记下标记后关闭 */
async function skip() {
  try {
    await api.skipColorMc();
  } catch {
    /* 标记没写上顶多下次再问一次 */
  }
  emit("close");
}

/** 重启启动器：实例是"文件夹搬进来"的，重启后核心才会重新扫到 */
async function restart() {
  await commands.windows.restartApp();
}
</script>

<template>
  <BaseModal :title="t('colormc.title')" :width="480" :closable="!running"
    @close="emit('close')">
    <!-- ---------- 1. 还没开始 ---------- -->
    <template v-if="!running && !report">
      <div class="src">{{ info.path }}</div>
      <div class="modal-sub">
        {{ fromText }} · {{ t("colormc.instances", { n: info.instances }) }}
      </div>
      <p class="warn">{{ t("colormc.hint") }}</p>
      <p v-if="error" class="error">{{ error }}</p>
    </template>

    <!-- ---------- 2. 迁移中 ---------- -->
    <template v-else-if="running">
      <p class="modal-text">{{ stageText }}</p>
      <div class="bar" :class="{ indet: percent < 0 }">
        <div class="bar-fill" :style="percent < 0 ? undefined : { width: percent + '%' }" />
      </div>
      <div class="run-info">
        <span v-if="progress && progress.total > 0">{{ progress.done }} / {{ progress.total }}</span>
        <span class="cur">{{ progress?.text }}</span>
      </div>
    </template>

    <!-- ---------- 3. 结果 ---------- -->
    <template v-else-if="report">
      <p class="modal-text">{{ t("colormc.entriesDone", { n: report.entries }) }}</p>
      <div v-if="report.accounts" class="line ok">
        {{ t("colormc.accountsDone", { n: report.accounts }) }}
      </div>

      <div v-if="report.instancesOk.length" class="line ok">
        {{ t("colormc.instOk", { n: report.instancesOk.length }) }}{{ t("common.detailSep") }}{{
          report.instancesOk.join(t("common.listSep"))
        }}
      </div>
      <div v-if="report.instancesBad.length" class="line bad">
        {{ t("colormc.instBad", { n: report.instancesBad.length }) }}
      </div>
      <div v-if="report.instancesLegacyGui.length" class="line warn">
        {{ t("colormc.instLegacyGui", { n: report.instancesLegacyGui.length }) }}
      </div>
      <div v-if="report.failed.length" class="line bad">
        {{ t("colormc.failedHead", { n: report.failed.length }) }}{{ report.failed.join(t("common.listSep")) }}
      </div>

      <!-- 兼容性：只列条目名，说明挂在悬停提示上 -->
      <div v-if="report.compat.length" class="compat">
        <div class="compat-head">{{ t("colormc.compatHead") }}</div>
        <div v-for="group in compatGroups" :key="group.level" class="compat-line">
          <span class="tag" :class="group.level">{{ t(`colormc.level.${group.level}`) }}</span>
          <span class="names">
            <code v-for="item in group.items" :key="item.name" :title="t(item.note)">{{
              item.name
            }}</code>
          </span>
        </div>
      </div>
    </template>

    <div class="modal-actions">
      <template v-if="!running && !report">
        <BaseButton @click="skip">{{ t("colormc.skip") }}</BaseButton>
        <BaseButton @click="run('copy')">{{ t("colormc.copy") }}</BaseButton>
        <BaseButton variant="accent" @click="run('move')">{{ t("colormc.move") }}</BaseButton>
      </template>
      <template v-else-if="report">
        <BaseButton v-if="report.instancesOk.length" @click="restart">
          {{ t("colormc.restart") }}
        </BaseButton>
        <BaseButton variant="accent" @click="emit('close')">{{ t("colormc.finish") }}</BaseButton>
      </template>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 来源路径：等宽 + 任意位置断行（长路径不该撑破弹窗） */
.src {
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  color: var(--text);
  word-break: break-all;
}

.warn {
  margin: 10px 0 0;
  color: var(--text-dim);
  font-size: 12.5px;
}

.error {
  margin: 10px 0 0;
  color: var(--red);
  font-size: 12.5px;
  word-break: break-all;
}

/* ---------- 进度 ---------- */

.bar {
  height: 6px;
  margin: 12px 0 6px;
  border-radius: 3px;
  overflow: hidden;
  background: var(--bg-hover);
}

.bar-fill {
  height: 100%;
  border-radius: 3px;
  background: var(--accent-grad, var(--accent));
  transition: width 0.15s ease;
}

.bar.indet .bar-fill {
  width: 40%;
  animation: colormc-run 1.1s ease-in-out infinite;
}

@keyframes colormc-run {
  0% {
    transform: translateX(-110%);
  }

  100% {
    transform: translateX(260%);
  }
}

.run-info {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  font-size: 12px;
  color: var(--text-dim);
}

.run-info .cur {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---------- 结果 ---------- */

.line {
  font-size: 12.5px;
  line-height: 1.7;
  word-break: break-all;
}

.line.ok {
  color: var(--text);
}

.line.bad {
  color: var(--red);
}

.line.warn {
  color: var(--text-dim);
}

.compat {
  margin-top: 10px;
}

.compat-head {
  font-size: 12px;
  color: var(--text-dim);
  margin-bottom: 4px;
}

.compat-line {
  display: flex;
  gap: 8px;
  align-items: baseline;
  margin-bottom: 4px;
}

.tag {
  flex: none;
  padding: 1px 7px;
  border-radius: 6px;
  font-size: 11px;
  background: var(--bg-hover);
  color: var(--text-dim);
  white-space: nowrap;
}

.tag.ok {
  background: color-mix(in srgb, var(--green) 14%, transparent);
  color: var(--green);
}

.tag.partial {
  background: color-mix(in srgb, var(--yellow) 16%, transparent);
  color: var(--yellow, var(--text));
}

.names {
  min-width: 0;
  font-size: 12px;
  color: var(--text-dim);
  line-height: 1.7;
}

.names code {
  font-family: "Cascadia Code", Consolas, monospace;
  margin-right: 8px;
  color: var(--text);
}
</style>
