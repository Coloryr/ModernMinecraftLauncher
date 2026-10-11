<script setup lang="ts">
// 游戏日志窗口：实例运行日志与日志文件浏览，日志用 Monaco 只读渲染，含实时流与历史快照
//
// 实例下拉的空白项 =「运行中的进程」：聚合当前所有运行中实例的实时日志，
// 列表由 LaunchState / GameExit 事件驱动刷新。
// 目标实例按优先级取：openWindow 带入的目标参数 → `log-focus` 事件 → 空白。
// 目标参数单窗口模式走 windowParams，多窗口走 URL uuid；
// `log-focus` 是窗口已存在时再次打开，由壳层推送。
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
// Monaco 编辑器按需加载：它一块就是 ~2.6 MB（gzip 674 kB）。
// 异步化不能省掉这次下载（默认就停在"运行日志"页），但能让窗口外壳、实例下拉与
// 工具条先画出来，Monaco 在这之后流式加载并替换占位；切到"日志文件"页时不加载。
const MonacoLogView = defineAsyncComponent(() => import("../../components/MonacoLogView.vue"));
import { t } from "../../lib/i18n";
import { api, onGameExit, onGameLog, onLaunchState, onLogFocus } from "../../lib/api";
import { targetUuid, windowParams } from "../windowManager";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import type { InstanceInfoDto, LogLine } from "../../lib/bindings";

/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

/** 每个实例的日志内存上限，与内核 InstanceRuntimeLog 保持一致 */
const MAX_LOG_LINES = 10000;

const instances = ref<InstanceInfoDto[]>([]);
/** 空 = 运行中的进程（聚合视图） */
const currentUuid = ref("");
const view = ref<"run" | "files">("run");

const viewOptions = computed(() => [
  { value: "run", label: t("logWindow.runLog") },
  { value: "files", label: t("logWindow.files") },
]);

// ---------------- 运行日志：按实例分段存储，视图按选择聚合 ----------------

const runningUuids = ref<string[]>([]);
const segmentLogs = ref(new Map<string, LogLine[]>());

const runLogs = computed<LogLine[]>(() => {
  if (currentUuid.value) {
    return segmentLogs.value.get(currentUuid.value) ?? [];
  }
  // 空白：聚合运行中实例；各实例段内保持时间序，段间按运行列表顺序拼接
  return runningUuids.value.flatMap((u) => segmentLogs.value.get(u) ?? []);
});

/** 拉一次快照；失败也留一个空段，避免之后反复重拉 */
async function loadSnapshot(uuid: string) {
  try {
    segmentLogs.value.set(uuid, await api.getRuntimeLog(uuid));
  } catch (e) {
    console.error("[LogWindow] 加载运行日志失败", e);
    segmentLogs.value.set(uuid, []);
  }
}

/** 刷新运行中实例列表并补拉新进程的快照 */
async function refreshRunning() {
  let ids: string[] = [];
  try {
    ids = await api.getRunning();
  } catch (e) {
    console.error("[LogWindow] 获取运行中实例失败", e);
  }
  for (const id of ids) {
    if (!segmentLogs.value.has(id)) await loadSnapshot(id);
  }
  runningUuids.value = ids;
}

// ---------------- 日志文件 ----------------

const logFiles = ref<string[]>([]);
const filesLoadedFor = ref("");
const activeFile = ref("");
const fileLogs = ref<LogLine[]>([]);
const loadingFile = ref(false);

/** 崩溃报告目录的文件单独分组 */
const crashGroup = computed(() => logFiles.value.filter((p) => p.includes("crash-reports")));
const logGroup = computed(() => logFiles.value.filter((p) => !p.includes("crash-reports")));

function fileName(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() ?? path;
}

async function loadFiles() {
  if (!currentUuid.value) return;
  try {
    logFiles.value = await api.getLogFiles(currentUuid.value);
    filesLoadedFor.value = currentUuid.value;
  } catch (e) {
    console.error("[LogWindow] 加载日志文件列表失败", e);
    logFiles.value = [];
  }
}

async function openFile(path: string) {
  if (loadingFile.value) return;
  loadingFile.value = true;
  activeFile.value = path;
  try {
    fileLogs.value = await api.readLogFile(currentUuid.value, path);
  } catch (e) {
    console.error("[LogWindow] 读取日志文件失败", e);
    fileLogs.value = [];
  } finally {
    loadingFile.value = false;
  }
}

// ---------------- 实例切换 ----------------

watch(currentUuid, (uuid) => {
  activeFile.value = "";
  fileLogs.value = [];
  filesLoadedFor.value = "";
  logFiles.value = [];
  if (uuid) {
    // 选定实例：拉该实例快照，实时行由 game-log 事件继续追加
    if (!segmentLogs.value.has(uuid)) loadSnapshot(uuid);
  } else {
    // 空白：跟随运行中的进程
    refreshRunning();
  }
  // 文件视图是懒加载：切回文件视图时再拉列表
  if (view.value === "files") loadFiles();
});

watch(view, (v) => {
  if (v === "files" && currentUuid.value && filesLoadedFor.value !== currentUuid.value) {
    loadFiles();
  }
});

// ---------------- 事件 ----------------

let unlistenGameLog: UnlistenFn | null = null;
let unlistenFocus: UnlistenFn | null = null;
let unlistenLaunchState: UnlistenFn | null = null;
let unlistenGameExit: UnlistenFn | null = null;

/** 追加一行并裁到上限，超出丢最旧的 */
function pushLine(uuid: string, line: LogLine) {
  const seg = segmentLogs.value.get(uuid) ?? [];
  seg.push(line);
  if (seg.length > MAX_LOG_LINES) seg.splice(0, seg.length - MAX_LOG_LINES);
  segmentLogs.value.set(uuid, seg);
}

/** 定位到 openWindow 带入的目标实例；没有或不存在就保持空白，即运行中的进程 */
function applyTarget() {
  const uuid = targetUuid();
  if (uuid && instances.value.some((i) => i.uuid === uuid)) {
    currentUuid.value = uuid;
  }
}

/** 重新拉实例列表并定位目标实例（首次挂载、切回本窗口都用） */
async function syncInstances() {
  try {
    instances.value = await api.getInstances();
    applyTarget();
  } catch (e) {
    console.error("[LogWindow] 初始化失败", e);
  }
}

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载，自己补一次列表与定位
useWindowRefresh(syncInstances);

// 已经停在本窗口时又被打开：openWindow 只更新参数、不换组件，跟着参数换实例
watch(() => windowParams.value.uuid, applyTarget);

onMounted(async () => {
  await syncInstances();

  unlistenGameLog = await onGameLog((e) => {
    // 选定实例：只收该实例；空白：收所有运行中实例
    const accept = currentUuid.value
      ? e.uuid === currentUuid.value
      : runningUuids.value.includes(e.uuid);
    if (!accept) return;
    if (e.clear) {
      segmentLogs.value.set(e.uuid, []);
      return;
    }
    pushLine(e.uuid, {
      time: e.time,
      text: e.text,
      thread: e.thread,
      level: e.level,
      category: e.category,
    });
  });

  // 窗口已存在时再次打开：壳层推送新的目标实例
  unlistenFocus = await onLogFocus((uuid) => {
    if (instances.value.some((i) => i.uuid === uuid)) {
      currentUuid.value = uuid;
    }
  });

  // 空白视图跟随进程起止：启动 / 退出时刷新运行列表
  unlistenLaunchState = await onLaunchState((e) => {
    if (!currentUuid.value && e.state === "launching") refreshRunning();
  });
  unlistenGameExit = await onGameExit(() => {
    if (!currentUuid.value) refreshRunning();
  });
});

onBeforeUnmount(() => {
  unlistenGameLog?.();
  unlistenFocus?.();
  unlistenLaunchState?.();
  unlistenGameExit?.();
});
</script>

<template>
  <WindowFrame body-fill :title="t('logWindow.title')" @close="$emit('close')">
    <div class="log-window">
      <div class="topbar">
        <select v-model="currentUuid" class="field-select inst-select" v-tip="t('logWindow.instance')">
          <option value="">{{ t("logWindow.runningProcess") }}</option>
          <option v-for="i in instances" :key="i.uuid" :value="i.uuid">{{ i.name }}</option>
        </select>
        <SegmentedTabs v-model="view" :options="viewOptions" />
      </div>

      <!-- 运行日志（实时流 + 历史快照；空白 = 运行中的进程聚合） -->
      <MonacoLogView v-if="view === 'run'" :logs="runLogs" fill
        :empty-hint="currentUuid ? undefined : t('logWindow.noRunning')" />

      <!-- 日志文件 / 崩溃报告（需选定具体实例） -->
      <div v-else-if="currentUuid" class="files-view">
        <div class="file-list">
          <template v-if="logFiles.length > 0">
            <div v-if="logGroup.length > 0" class="group-label">{{ t("logWindow.logsDir") }}</div>
            <button v-for="f in logGroup" :key="f" class="file-item" :class="{ active: f === activeFile }" v-tip="f"
              @click="openFile(f)">
              {{ fileName(f) }}
            </button>
            <template v-if="crashGroup.length > 0">
              <div class="group-label crash">{{ t("logWindow.crashReports") }}</div>
              <button v-for="f in crashGroup" :key="f" class="file-item" :class="{ active: f === activeFile }" v-tip="f"
                @click="openFile(f)">
                {{ fileName(f) }}
              </button>
            </template>
          </template>
          <div v-else class="no-files">{{ t("logWindow.noFiles") }}</div>
        </div>
        <div class="file-content">
          <div v-if="loadingFile" class="pick-hint">{{ t("logWindow.loading") }}</div>
          <MonacoLogView v-else-if="activeFile" :logs="fileLogs" fill />
          <div v-else class="pick-hint">{{ t("logWindow.selectFile") }}</div>
        </div>
      </div>
      <div v-else class="pick-hint full">{{ t("logWindow.filesNeedInstance") }}</div>
    </div>
  </WindowFrame>
</template>

<style scoped>
.log-window {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  min-height: 0;
  padding: 12px 14px;
  box-sizing: border-box;
}

.topbar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.inst-select {
  flex: 1;
  min-width: 0;
  /* 外观全部走 .field-select，同箭头、不透明底、主题边框、手型光标；这里只覆盖尺寸：
     原来自己抄了一整份外观，字号也是单独一份，改成"挂 .field-select + 只改尺寸"免得再走偏 */
  padding: 6px 24px 6px 8px;
  font-size: 13px;
  min-height: 0;
  background-position: right 6px center;
}

.topbar .seg-tabs {
  margin-left: auto;
}

/* 日志文件视图：左侧文件列表 + 右侧内容 */
.files-view {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 10px;
}

.file-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 8px;
}

.group-label {
  font-size: 11px;
  color: var(--text-secondary, #8a919c);
  padding: 6px 6px 4px;
  user-select: none;
}

.group-label.crash {
  margin-top: 6px;
  border-top: 1px solid var(--border);
  padding-top: 8px;
}

.file-item {
  text-align: left;
  font-size: 12.5px;
  color: var(--text);
  background: transparent;
  border: none;
  border-radius: 6px;
  padding: 6px 8px;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-item:hover {
  background: var(--border);
}

.file-item.active {
  background: var(--border);
  font-weight: 600;
}

.no-files {
  color: var(--text-secondary, #8a919c);
  font-size: 12.5px;
  text-align: center;
  margin-top: 24px;
}

.file-content {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.pick-hint {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary, #8a919c);
  font-size: 13px;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 10px;
}

.pick-hint.full {
  margin: 0;
}
</style>
