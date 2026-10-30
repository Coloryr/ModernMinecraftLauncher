<script setup lang="ts">
// ColorMC 迁移：「发现旧数据」弹窗里的一节
//
// 数据由主窗口在加载期间探测，见 MainWindow 的 oldDataReady；这一节只负责"问一次"：
// 复制 / 移动 / 不迁移。三个按钮都算回答，不选就一直留着（关掉整个弹窗不算回答）。
//
// 文案一律从简：来源 + 实例数 + 一句话提醒 + 三个按钮；结果页只列条目名，
// 每条兼容性结论的长说明挂在 `title` 上（悬停才看）。
import { computed, ref } from "vue";
import type { ColorMcInfoDto, ColorMcProgressDto, ColorMcReportDto } from "../../lib/bindings";
import { commands } from "../../lib/bindings";
import { api, onColormcProgress } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import BaseButton from "../../components/ui/BaseButton.vue";

/** ColorMC 探测结果：数据目录、命中来源与可迁移实例数 */
const props = defineProps<{ info: ColorMcInfoDto }>();
/** `done` 表示数据已搬完，父组件据此重拉实例与分组；`busy` 表示正在搬，弹窗不许关 */
const emit = defineEmits<{
  (e: "done"): void;
  (e: "busy", busy: boolean): void;
}>();

/** 迁移进行中（按钮禁用 + 不允许关闭） */
const running = ref(false);
/** 已经回答过（迁移完成或选择不迁移），按钮收起 */
const settled = ref(false);
/** 最近一次进度 */
const progress = ref<ColorMcProgressDto | null>(null);
/** 结果报告（非空 → 显示结果页） */
const report = ref<ColorMcReportDto | null>(null);
/** 失败信息（后端给的是 i18n key） */
const error = ref("");

/** 进度事件取消订阅 */
let unlisten: (() => void) | null = null;

/** 迁移完成后到重启的等待，单位毫秒；留一点时间让用户看到结果 */
const RESTART_DELAY_MS = 1500;

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

/** 兼容性结论按级别分档（每档一行条目名）；"用不到"那一档不展示 */
const compatGroups = computed(() => {
  const list = report.value?.compat ?? [];
  return (["ok", "partial"] as const)
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
  emit("busy", true);
  error.value = "";
  report.value = null;
  progress.value = null;
  try {
    unlisten = await onColormcProgress((p) => {
      progress.value = p;
    });
    report.value = await api.migrateColorMc(props.info.path, mode);
    emit("done");
    // 实例是"文件夹搬进来"的，运行中的核心不会重新扫实例目录：迁移完强制重启
    window.setTimeout(() => {
      void commands.windows.restartApp();
    }, RESTART_DELAY_MS);
  } catch (e) {
    error.value = tErr(e);
  } finally {
    unlisten?.();
    unlisten = null;
    running.value = false;
    // 只有搬成功才算答完；失败时收起按钮会看不到错误，也重试不了
    if (report.value) settled.value = true;
    emit("busy", false);
  }
}

/** 不迁移：记下标记后收起 */
async function skip() {
  try {
    await api.skipColorMc();
  } catch {
    /* 标记没写上顶多下次再问一次 */
  }
  settled.value = true;
}

</script>

<template>
  <section class="old-section">
    <div class="old-head">
      <span class="old-name">ColorMC</span>
      <span class="old-meta">{{ fromText }} · {{ t("colormc.instances", { n: info.instances }) }}</span>
    </div>
    <div class="src">{{ info.path }}</div>

    <!-- ---------- 1. 还没开始 ---------- -->
    <template v-if="!running && !report && !settled">
      <p class="warn">{{ t("colormc.hint") }}</p>
      <p v-if="error" class="error">{{ error }}</p>
      <div class="old-actions">
        <BaseButton @click="skip">{{ t("colormc.skip") }}</BaseButton>
        <BaseButton @click="run('copy')">{{ t("colormc.copy") }}</BaseButton>
        <BaseButton variant="accent" @click="run('move')">{{ t("colormc.move") }}</BaseButton>
      </div>
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
        {{ t("colormc.instOk", { n: report.instancesOk.length }) }}
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
      <div v-if="compatGroups.length" class="compat">
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

      <p class="settled">{{ t("colormc.restarting") }}</p>
    </template>

    <!-- ---------- 4. 跳过了 ---------- -->
    <p v-else-if="settled && !report" class="settled">{{ t("colormc.skipped") }}</p>
  </section>
</template>
