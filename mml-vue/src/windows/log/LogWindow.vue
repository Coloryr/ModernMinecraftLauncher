<script setup lang="ts">
// 游戏日志窗口：**一个实例一个**，打开时锁定，窗口内不再选择实例
//
// 多窗口模式下是独立真实窗口，标签形如 `mml-log-<实例uuid>`（身份由后端按实例派生，
// 同一实例重复打开只聚焦）；单窗口模式下与别的窗口一样是应用内的一页，实例由
// windowParams 带进来，已在本页上时参数变化就是换实例，见 lockedUuid 的 watch。
//
// 布局照 ColorMC 的 GameLogControl：一行"日志文件 + 刷新 / 搜索 / 上传 / 测试启动"，
// 一行"自动换行 / 自动下拉 + 五个等级"，一行"线程 / 分组"，最后是日志正文。
// 等级与自动下拉存进实例的 `gui_setting.json`（一个实例一套）；**自动换行不跟实例走** ——
// 它是全局偏好（lib/editorWrap），与配置编辑器共用一份，Alt+Z 在任一处都能切。
// 线程与分组只筛当前这份日志，不落盘。
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseSwitch from "../../components/ui/BaseSwitch.vue";
import { toggleWordWrap, wordWrap } from "../../lib/editorWrap";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import InstanceChip from "../../components/ui/InstanceChip.vue";
import LaunchProgress from "../../components/LaunchProgress.vue";
import DownloadTitleIndicator from "../../components/DownloadTitleIndicator.vue";
// Monaco 编辑器按需加载：它一块就是 ~2.6 MB（gzip 674 kB）。
// 异步化不能省掉这次下载（打开就是要看日志），但能让窗口外壳与工具条先画出来。
const MonacoLogView = defineAsyncComponent(() => import("../../components/MonacoLogView.vue"));
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { copyText } from "../../lib/clipboard";
import { api, onGameExit, onGameLog, onLaunchError, onLaunchState } from "../../lib/api";
import { stateText } from "../../lib/launchState";
import { useDownloadProgress } from "../../composables/useDownloadProgress";
import { familyInstanceFromLabel, targetLogBottom, targetLogFile, targetUuid } from "../windowManager";
import { latestCrash } from "../../lib/crash";
import { launchGame } from "../../lib/launch";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { usePageActive } from "../../lib/pageActive";
import type { CrashAnalysisDto, InstanceInfoDto, LogLine, LogSettingDto } from "../../lib/bindings";
import { uniq } from "../../lib/util";

/** 请求关闭本窗口 / 收起面板 */
defineEmits<{ (e: "close"): void }>();

/** 每个实例的日志内存上限，与内核 InstanceRuntimeLog 保持一致 */
const MAX_LOG_LINES = 10000;

/**
 * 本窗口锁定的实例 uuid
 *
 * 多窗口模式按窗口标签，那是后端派生窗口身份时用的同一份实例；
 * 单窗口模式按本次跳转带入的 windowParams，已在本页上换个实例打开时它就是新值。
 * 浏览器预览没有标签，同样退回 URL / windowParams 上的参数。
 */
const lockedUuid = computed(() => familyInstanceFromLabel("log") ?? targetUuid() ?? "");

/**
 * 锁定实例的信息，只用来在标题栏显示 chip
 *
 * 与资源管理 / 添加资源窗口同一个 chip，但**只给名字不给版本**：日志窗口里版本没有用，
 * 标题栏这一排控件本来就多。所以窗口标题里也不再重复实例名。
 */
const instance = ref<InstanceInfoDto | null>(null);

/** 标题栏 chip 的数据；只取名字，见上面 */
const instanceChip = computed(() => (instance.value ? { name: instance.value.name } : null));

/** 窗口标题：只有窗口名，实例在标题栏的 chip 上 */
const title = computed(() => t("logWindow.title"));

// ---------------- 设置：自动下拉 / 五个等级（换行是全局偏好，不在这里） ----------------

/** 默认值与后端 `GameLogSettingObj::default` 一致：自动下拉关、等级全开 */
const setting = ref<LogSettingDto>({
  // wordWrap 只为对齐后端结构留着：界面上的换行读全局偏好，不读它
  wordWrap: true,
  auto: false,
  autoShow: false,
  enableNone: true,
  enableInfo: true,
  enableWarn: true,
  enableError: true,
  enableDebug: true,
});

/** 设置是否已从磁盘读过：读完之前不写回，否则会把默认值盖到用户设置上 */
let settingLoaded = false;

/** 等级开关与文案键，顺序即显示顺序 */
const LEVELS = [
  { key: "enableInfo", label: "logWindow.levelInfo" },
  { key: "enableWarn", label: "logWindow.levelWarn" },
  { key: "enableError", label: "logWindow.levelError" },
  { key: "enableDebug", label: "logWindow.levelDebug" },
  { key: "enableNone", label: "logWindow.levelNone" },
] as const;

/**
 * 这一行该不该显示
 *
 * - `level`: 日志行的等级，取自内核解析结果，可能是空串
 *
 * # 返回值
 *
 * 该等级开着时返回 true；解析不出等级的普通行归「其他」
 */
function levelOn(level: string): boolean {
  if (level === "Info") return setting.value.enableInfo;
  if (level === "Warn") return setting.value.enableWarn;
  if (level === "Error" || level === "Fatal") return setting.value.enableError;
  if (level === "Debug") return setting.value.enableDebug;
  return setting.value.enableNone;
}

/** 读一次设置；实例不存在或读失败都退回默认值 */
async function loadSetting() {
  if (!lockedUuid.value) return;
  try {
    setting.value = await api.getLogSetting(lockedUuid.value);
  } catch (e) {
    console.error("[LogWindow] 读取日志设置失败", e);
  } finally {
    settingLoaded = true;
  }
}

// 改一项就整份回写：设置项少，后端按文件去重合并写入，不会积压
watch(
  setting,
  () => {
    if (!settingLoaded || !lockedUuid.value) return;
    void api.setLogSetting(lockedUuid.value, { ...setting.value }).catch((e) => {
      console.error("[LogWindow] 保存日志设置失败", e);
    });
  },
  { deep: true },
);

// ---------------- 日志来源：运行日志 / 某个日志文件 ----------------

/** 日志文件列表，绝对路径 */
const logFiles = ref<string[]>([]);
/** 当前来源；空串表示运行日志（实时） */
const file = ref("");
/** 当前来源的全部日志行 */
const sourceLogs = ref<LogLine[]>([]);
/** 正在读文件 */
const loadingSource = ref(false);

/** 线程 / 分组筛选；空串表示全部 */
const thread = ref("");
const category = ref("");

/** 日志行是否属于当前运行日志来源（只有它跟着 game-log 事件增长） */
const liveSource = computed(() => file.value === "");

function fileName(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() ?? path;
}

/** 按等级 / 线程 / 分组筛出要显示的行 */
const shownLogs = computed(() =>
  sourceLogs.value.filter(
    (l) =>
      levelOn(l.level) &&
      (thread.value === "" || l.thread === thread.value) &&
      (category.value === "" || l.category === category.value),
  ),
);

const threads = computed(() => uniq(sourceLogs.value.map((l) => l.thread)));
const categories = computed(() => uniq(sourceLogs.value.map((l) => l.category)));

/** 刷新日志文件列表；失败按空处理 */
async function refreshList() {
  if (!lockedUuid.value) return;
  try {
    logFiles.value = await api.getLogFiles(lockedUuid.value);
  } catch (e) {
    console.error("[LogWindow] 加载日志文件列表失败", e);
    logFiles.value = [];
  }
}

/** 本次打开是否还要滚到底；加载完成后消费 */
let pendingBottom = false;

/** 加载完当前来源后，若本次打开要求滚到底就滚一次 */
async function scrollIfPending() {
  if (!pendingBottom) return;
  await nextTick();
  // Monaco 视图是异步组件，还没挂上时先留着，等 viewRef 的 watch 再滚
  if (!viewRef.value) return;
  pendingBottom = false;
  viewRef.value.scrollBottom();
}

/** 当前崩溃报告的分析结果；不是崩溃报告时为 null */
const analysis = ref<CrashAnalysisDto | null>(null);
/** 分析请求序号：换文件后旧请求的结果直接丢掉 */
let analyzeSeq = 0;

/** Mixin 相关结论；没有时不显示横幅 */
const mixinFinding = computed(
  () => analysis.value?.findings.find((f) => f.id === "mixin_failure") ?? null,
);

/** 横幅上显示的可疑模组，最多三个 */
const mixinSuspects = computed(() => (mixinFinding.value?.mods ?? []).slice(0, 3).join(", "));

/** 横幅上显示的 Mixin 配置名 */
const mixinConfig = computed(() => {
  const detail = mixinFinding.value?.details.find((d) => d.startsWith("mixin_config="));
  return detail ? detail.slice("mixin_config=".length) : "";
});

/** 分析当前日志文件；运行日志不分析，历史文件打开时都跑一遍 */
async function analyzeCurrent() {
  const seq = ++analyzeSeq;
  if (!lockedUuid.value || liveSource.value) {
    analysis.value = null;
    return;
  }

  try {
    const result = await api.analyzeLog(lockedUuid.value, file.value);
    if (seq === analyzeSeq) analysis.value = result;
  } catch (e) {
    console.error("[LogWindow] 分析崩溃报告失败", e);
    if (seq === analyzeSeq) analysis.value = null;
  }
}

/** 载入当前来源的全部日志行 */
async function loadSource() {
  if (!lockedUuid.value) return;
  // 换了来源，旧的分析结果与在途请求都作废
  analyzeSeq += 1;
  analysis.value = null;
  // 换了来源，旧的筛选选项不再成立；待入列的实时行也不属于新来源
  dropPending();
  thread.value = "";
  category.value = "";
  if (liveSource.value) {
    try {
      sourceLogs.value = await api.getRuntimeLog(lockedUuid.value);
    } catch (e) {
      console.error("[LogWindow] 加载运行日志失败", e);
      sourceLogs.value = [];
    }
    await scrollIfPending();
    return;
  }

  loadingSource.value = true;
  try {
    sourceLogs.value = await api.readLogFile(lockedUuid.value, file.value);
  } catch (e) {
    console.error("[LogWindow] 读取日志文件失败", e);
    sourceLogs.value = [];
  } finally {
    loadingSource.value = false;
  }
  void analyzeCurrent();
  await scrollIfPending();
}

watch(file, loadSource);

/**
 * 应用本次打开带入的日志参数
 *
 * 指定了文件且列表里有就切过去，没有就回运行日志；要求滚到底时等来源加载完再滚。
 */
async function applyOpenParams() {
  await refreshList();

  const wanted = targetLogFile();
  if (targetLogBottom()) pendingBottom = true;

  if (wanted && logFiles.value.includes(wanted)) {
    if (file.value !== wanted) {
      // 由 file 的 watch 负责加载，加载完消费 pendingBottom
      file.value = wanted;
    } else {
      await scrollIfPending();
    }
    return;
  }

  if (file.value !== "") {
    // 没有可用的崩溃文件：回运行日志，由 watch 加载
    file.value = "";
  } else {
    await loadSource();
  }
}

// ---------------- 运行状态：测试启动 / 停止 ----------------

const running = ref(false);

/**
 * 启动中：点了"测试启动"、进程还没起来的那一段
 *
 * 后端要到**进程真正起来**才把实例算作运行中（内核 add_run_game），这期间 getRunning() 还是
 * false；所以"启动中"由这份本地状态顶住，"运行中"以后端为准（进程起来后内核推一次
 * LaunchState::End，我们据此刷新，见 onLaunchState）。
 */
const launching = ref(false);
/** 当前阶段名与进度，来自 launch-state 事件 */
const stageKey = ref("");
/** 内核给的阶段百分比：按 LaunchState 等权近似，下载阶段恒为 36 */
const statePct = ref<number | null>(null);

/** 下载队列的真实进度 */
const downloadProgress = useDownloadProgress();

/** 阶段文案；还没收到阶段就按"正在启动…" */
const stageText = computed(() => (stageKey.value ? stateText(stageKey.value) : t("launch.launching")));

/** 按钮上的百分比：下载阶段用真实下载进度，其它阶段用内核的阶段近似值 */
const stagePct = computed(() =>
  stageKey.value === "download" && downloadProgress.hasTasks.value
    ? Math.round(downloadProgress.percent.value)
    : statePct.value,
);

/** 拉一次该实例是否在运行 */
async function refreshRunning() {
  if (!lockedUuid.value) return;
  try {
    running.value = (await api.getRunning()).includes(lockedUuid.value);
    // 进程起来了：启动阶段结束，按钮该换成停止键
    if (running.value) launching.value = false;
  } catch (e) {
    console.error("[LogWindow] 读取运行状态失败", e);
  }
}

/** 测试启动：与主页面同一个命令，日志会流进本窗口 */
async function testLaunch() {
  // 点了就先按住按钮：后端起进程之前 getRunning() 还是 false，光靠它按钮会一直像"可点"
  launching.value = true;
  stageKey.value = "";
  statePct.value = null;
  // 自己点的启动：立刻切回运行日志，不等第一阶段事件
  file.value = "";
  try {
    const started = await launchGame(lockedUuid.value);
    if (!started) {
      launching.value = false;
      return;
    }
    // 自己启动的实例就是要盯着它的输出，顺手把自动下拉打开（与参考实现一致）
    setting.value.auto = true;
    await refreshRunning();
  } catch (e) {
    launching.value = false;
    showToast(tErr(e));
  }
}

/** 停止确认弹窗 */
const stopAsk = ref(false);

/** 强制停止该实例 */
async function doStop() {
  stopAsk.value = false;
  try {
    await api.stopGame(lockedUuid.value);
    running.value = false;
  } catch (e) {
    showToast(tErr(e));
  }
}

// ---------------- 搜索 / 上传 ----------------

/** 日志视图的公开方法，见 MonacoLogView 的 defineExpose */
const viewRef = ref<{ openFind: () => void; scrollBottom: () => void } | null>(null);

// Monaco 视图异步加载：挂上之后补一次待办的滚底
watch(viewRef, (view) => {
  if (view) void scrollIfPending();
});

/** 打开页内查找框：工具栏搜索按钮与 Ctrl+F 都走这里 */
function onSearch() {
  viewRef.value?.openFind();
}

/** 本页是否在前台；单窗口模式切走时不能继续抢全局快捷键 */
const active = usePageActive();

/** Ctrl+F：焦点不在编辑器里时开 Monaco 的查找框，别落到浏览器自带的页内查找 */
function onFindKey(e: KeyboardEvent) {
  if (e.defaultPrevented) return;
  if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return;
  if (e.key.toLowerCase() !== "f") return;
  // 无论焦点在工具栏还是编辑器里，都先吃掉这次按键，再开编辑器自己的查找框
  e.preventDefault();
  onSearch();
}

watch(
  active,
  (on) => {
    if (on) window.addEventListener("keydown", onFindKey, true);
    else window.removeEventListener("keydown", onFindKey, true);
  },
  { immediate: true },
);

/** 上传确认弹窗 */
const uploadAsk = ref(false);
/** 上传中，避免连点 */
const uploading = ref(false);
/** 上传成功后的分享地址，非空即弹结果框 */
const uploadUrl = ref("");

/** 上传的是**整份日志**（当前来源），不是界面上筛过的那几行 */
async function doUpload() {
  uploadAsk.value = false;
  crashAsk.value = false;
  uploading.value = true;
  try {
    uploadUrl.value = await api.uploadLog(lockedUuid.value, liveSource.value ? null : file.value);
    // 与参考实现一致：结果框弹出的同时就把地址放进剪贴板，省得再点一次
    await copyUrl();
  } catch (e) {
    showToast(tErr(e));
  } finally {
    uploading.value = false;
  }
}

/** 游戏异常退出时问一句要不要上传日志 */
const crashAsk = ref(false);
/** 异常退出的返回码，只用于文案 */
const crashCode = ref(0);

/** 复制分享地址 */
async function copyUrl() {
  if (await copyText(uploadUrl.value)) {
    showToast(t("logWindow.copied"));
  }
}

// ---------------- 初始化 ----------------

/**
 * 补一次实例名、设置、文件列表与当前日志
 *
 * - `applyParams`: 是否应用本次打开带入的文件 / 滚底参数；切回面板时只刷新当前来源
 */
async function syncInstance(applyParams = false) {
  if (!lockedUuid.value) return;
  try {
    const list: InstanceInfoDto[] = await api.getInstances();
    instance.value = list.find((i) => i.uuid === lockedUuid.value) ?? null;
  } catch (e) {
    console.error("[LogWindow] 读取实例列表失败", e);
  }
  await loadSetting();
  if (applyParams) {
    await applyOpenParams();
  } else {
    await refreshList();
    await loadSource();
  }
  await refreshRunning();
}

// 单窗口模式：本页被 KeepAlive 缓存，切回时补一次数据；只刷新当前来源，不重放打开参数
useWindowRefresh(() => syncInstance(false));

// 单窗口模式：已停在本页时再开另一个实例就是换锁定对象 —— 会走这里。
// 来源先清空（file 的 watch 会先拉一遍，syncInstance 再整补一次，读同一份数据幂等），
// 否则换实例头一眼看到的还是上一个实例的正文
watch(
  lockedUuid,
  (now, before) => {
    if (!now || now === before) return;
    sourceLogs.value = [];
    void syncInstance(true);
  },
);

// 单窗口模式：已经停在日志页时 openWindow 换了文件 / 要求滚底，参数变化走这里。
// 换实例由上面 lockedUuid 的 watch 整补，这里跳过，免得同一份数据拉两遍。
watch(
  () => [targetUuid(), targetLogFile(), targetLogBottom()] as const,
  (now, before) => {
    if (!now[0] || now[0] !== before?.[0]) return;
    if (now[1] === before?.[1] && now[2] === before?.[2]) return;
    void applyOpenParams();
  },
);

let unlistenGameLog: UnlistenFn | null = null;
let unlistenLaunchState: UnlistenFn | null = null;
let unlistenLaunchError: UnlistenFn | null = null;
let unlistenGameExit: UnlistenFn | null = null;

/** 攒批周期，单位 ms */
const FLUSH_INTERVAL = 100;

/** 还没并入 sourceLogs 的行
 *
 * 事件是**逐行**来的，收一行就 push 一次的话，一次渲染的活儿（筛选、线程 / 分组去重、
 * Monaco 追加、滚到底）会跟着行数跑几千遍 —— 游戏一次性刷几千行时窗口就卡住了。
 */
let pendingLines: LogLine[] = [];
let flushTimer = 0;

/** 收一行：先攒着，到点一次性并入 */
function enqueueLine(line: LogLine) {
  pendingLines.push(line);
  // 一次性灌几十万行时只留最近这一段：上限本来也是 MAX_LOG_LINES，攒太多只是白占内存
  if (pendingLines.length > MAX_LOG_LINES * 2) {
    pendingLines.splice(0, pendingLines.length - MAX_LOG_LINES * 2);
  }
  if (flushTimer === 0) flushTimer = window.setTimeout(flushPending, FLUSH_INTERVAL);
}

/** 把攒下的行一次并入，超出上限丢最旧的 */
function flushPending() {
  flushTimer = 0;
  if (pendingLines.length === 0) return;
  const merged = sourceLogs.value.concat(pendingLines);
  pendingLines = [];
  sourceLogs.value = merged.length > MAX_LOG_LINES ? merged.slice(-MAX_LOG_LINES) : merged;
}

/** 丢掉还没入列的行：清空、换实例、换来源时都要，别让旧行事后冒出来 */
function dropPending() {
  pendingLines = [];
  if (flushTimer !== 0) {
    clearTimeout(flushTimer);
    flushTimer = 0;
  }
}

onMounted(async () => {
  await syncInstance(true);

  unlistenGameLog = await onGameLog((e) => {
    // 锁定了实例：只收它自己的日志行；看历史文件时不追加实时行
    if (e.uuid !== lockedUuid.value || !liveSource.value) return;
    if (e.clear) {
      // 清屏并收下跟在清屏之后的行（"游戏启动中…"这类就在这批里）
      dropPending();
      sourceLogs.value = [...e.lines];
      return;
    }
    for (const line of e.lines) enqueueLine(line);
  });

  // 本实例的启停：切换「测试启动 / 停止」按钮
  unlistenLaunchState = await onLaunchState((e) => {
    if (e.uuid !== lockedUuid.value) return;
    // 用户在主窗口点了「取消启动」：启动线程退出，收回启动态
    if (e.state === "cancelled") {
      launching.value = false;
      stageKey.value = "";
      statePct.value = null;
      return;
    }
    // 启动一开始（不管从主窗口还是这里的测试启动）就切回运行日志：
    // 停在历史文件上会看不到这次启动的输出
    if (!liveSource.value && !running.value) file.value = "";
    stageKey.value = e.state;
    statePct.value = e.progress ?? null;
    void refreshRunning();
  });
  unlistenLaunchError = await onLaunchError((e) => {
    if (e.uuid && e.uuid !== lockedUuid.value) return;
    launching.value = false;
    running.value = false;
  });
  unlistenGameExit = await onGameExit((e) => {
    if (e.uuid !== lockedUuid.value) return;
    launching.value = false;
    running.value = false;
    // 非 0 退出：刷一次文件列表（崩溃报告刚写出来），问一句要不要上传，
    // 并切到最新崩溃报告；没有就回运行日志并滚到底
    if (e.code !== 0) {
      crashCode.value = e.code;
      crashAsk.value = true;
      void refreshList().then(() => {
        // 主窗口已经带着崩溃文件重开时，URL 上会有滚底参数，这里不再抢着切
        if (targetLogBottom()) return;
        const crash = latestCrash(logFiles.value);
        pendingBottom = true;
        if (crash && crash !== file.value) {
          file.value = crash;
        } else if (!crash && file.value !== "") {
          file.value = "";
        } else {
          void scrollIfPending();
        }
      });
    }
  });
});

onBeforeUnmount(() => {
  dropPending();
  window.removeEventListener("keydown", onFindKey, true);
  unlistenGameLog?.();
  unlistenLaunchState?.();
  unlistenLaunchError?.();
  unlistenGameExit?.();
});
</script>

<template>
  <WindowFrame :title="title" body-fill hide-download-indicator @close="$emit('close')">
    <!-- 日志来源与操作都放标题栏：不占内容区高度；除「测试启动」外只留图标，文字进提示 -->
    <template #head-right>
      <div class="head-log">
        <!-- 当前实例：与资源管理窗口同一个 chip，只是不带版本 -->
        <InstanceChip :instance="instanceChip" />
        <select v-model="file" class="field-select head-select" v-tip="t('logWindow.sourceTip')"
          :aria-label="t('logWindow.sourceTip')">
          <option value="">{{ t("logWindow.runLog") }}</option>
          <option v-for="f in logFiles" :key="f" :value="f">{{ fileName(f) }}</option>
        </select>
        <!-- 下载入口紧跟日志来源下拉框：从这儿测试启动时，缺文件的下载就在手边 -->
        <DownloadTitleIndicator />
        <BaseButton size="sm" variant="ghost" class="icon-btn-head" v-tip="t('logWindow.refreshList')"
          :aria-label="t('logWindow.refreshList')" @click="refreshList">
          <GlyphIcon name="refresh" :size="15" :weight="1.8" />
        </BaseButton>
        <BaseButton size="sm" variant="ghost" class="icon-btn-head" v-tip="t('logWindow.search')"
          :aria-label="t('logWindow.search')" @click="onSearch">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
            stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="6.5" />
            <path d="m16 16 4.5 4.5" />
          </svg>
        </BaseButton>
        <BaseButton size="sm" variant="ghost" class="icon-btn-head" :disabled="uploading"
          v-tip="uploading ? t('logWindow.uploading') : t('logWindow.upload')"
          :aria-label="uploading ? t('logWindow.uploading') : t('logWindow.upload')" @click="uploadAsk = true">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 16V4" />
            <path d="m7 9 5-5 5 5" />
            <path d="M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />
          </svg>
        </BaseButton>
        <!-- 测试启动 / 停止：也是图标按钮，但它会改变实例状态，所以给一档有色变体与三个幽灵按钮区分 -->
        <BaseButton v-if="running" size="sm" variant="danger" class="icon-btn-head" v-tip="t('logWindow.stop')"
          :aria-label="t('logWindow.stop')" @click="stopAsk = true">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
            stroke-linecap="round" stroke-linejoin="round">
            <rect x="6.5" y="6.5" width="11" height="11" rx="2" />
          </svg>
        </BaseButton>
        <BaseButton v-else size="sm" :variant="launching ? 'primary' : 'accent'" class="icon-btn-head"
          :class="{ 'launch-live': launching }" :disabled="launching"
          v-tip="launching ? stageText : t('logWindow.testLaunch')"
          :aria-label="launching ? stageText : t('logWindow.testLaunch')" @click="testLaunch">
          <LaunchProgress v-if="launching" :text="stageText" :percent="stagePct" />
          <GlyphIcon v-else name="play" :size="15" />
        </BaseButton>
      </div>
    </template>

    <div class="log-window">
      <!-- 第一行：换行 / 自动下拉 / 等级。等级也走开关，与旁边两个同款；
           "等级"这行标签与第二行的"线程 / 分组"同一套 -->
      <div class="bar">
        <label class="sw">
          <!-- 开关读的是全局换行偏好（lib/editorWrap），与配置编辑器共用一份 -->
          <BaseSwitch :model-value="wordWrap" @update:model-value="toggleWordWrap" />
          <span>{{ t("logWindow.wordWrap") }}</span>
        </label>
        <label class="sw">
          <BaseSwitch v-model="setting.auto" />
          <span>{{ t("logWindow.autoScroll") }}</span>
        </label>
        <label class="sw">
          <BaseSwitch v-model="setting.autoShow" />
          <span>{{ t("logWindow.autoOpen") }}</span>
        </label>
        <span class="lbl">{{ t("logWindow.level") }}</span>
        <label v-for="lv in LEVELS" :key="lv.key" class="sw">
          <BaseSwitch v-model="setting[lv.key]" />
          <span>{{ t(lv.label) }}</span>
        </label>
      </div>

      <!-- 第二行：线程 / 分组 -->
      <div class="bar">
        <span class="lbl">{{ t("logWindow.thread") }}</span>
        <select v-model="thread" class="field-select pick">
          <option value="">{{ t("logWindow.filterAll") }}</option>
          <option v-for="th in threads" :key="th" :value="th">{{ th }}</option>
        </select>
        <span class="lbl">{{ t("logWindow.category") }}</span>
        <select v-model="category" class="field-select pick">
          <option value="">{{ t("logWindow.filterAll") }}</option>
          <option v-for="c in categories" :key="c" :value="c">{{ c }}</option>
        </select>
      </div>

      <!-- 崩溃分析：Mixin 错误直接点名可疑模组 -->
      <div v-if="mixinFinding" class="analysis-card">
        <GlyphIcon class="analysis-icon" name="alert" :size="15" :weight="2" />
        <div class="analysis-body">
          <div class="analysis-title">
            {{ t("logWindow.mixinFailure") }}
            <span v-if="mixinSuspects" class="analysis-suspect">
              {{ t("logWindow.suspectMod", { mods: mixinSuspects }) }}
            </span>
            <span v-else class="analysis-suspect">{{ t("logWindow.analysisNoMod") }}</span>
          </div>
          <div v-if="mixinConfig" class="analysis-detail">
            {{ t("logWindow.mixinConfig", { config: mixinConfig }) }}
          </div>
        </div>
        <button class="analysis-close" :aria-label="t('logWindow.analysisClose')"
          v-tip="t('logWindow.analysisClose')" @click="analysis = null">×</button>
      </div>

      <!-- 日志正文 -->
      <MonacoLogView ref="viewRef" :logs="shownLogs" :auto-scroll="setting.auto"
        :empty-hint="loadingSource ? t('logWindow.loading') : undefined" fill
        @scroll-away="setting.auto = false" />
    </div>

    <!-- 上传确认：日志会发到第三方站点，先问一句 -->
    <BaseModal v-if="uploadAsk" :title="t('logWindow.upload')" @close="uploadAsk = false">
      <p class="modal-text">{{ t("logWindow.uploadConfirm") }}</p>
      <div class="modal-actions">
        <BaseButton @click="uploadAsk = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="doUpload">{{ t("logWindow.upload") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 异常退出：多半就是来找日志的，直接问一句要不要传 -->
    <BaseModal v-if="crashAsk" :title="t('logWindow.crashTitle')" @close="crashAsk = false">
      <p class="modal-text">{{ t("logWindow.crashUpload", { code: crashCode }) }}</p>
      <div class="modal-actions">
        <BaseButton @click="crashAsk = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="doUpload">{{ t("logWindow.upload") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 上传结果：只读地址 + 复制 -->
    <BaseModal v-if="uploadUrl" :title="t('logWindow.uploadDone')" @close="uploadUrl = ''">
      <input class="field-input url" :value="uploadUrl" readonly spellcheck="false" />
      <div class="modal-actions">
        <BaseButton @click="uploadUrl = ''">{{ t("logWindow.close") }}</BaseButton>
        <BaseButton variant="primary" @click="copyUrl">{{ t("logWindow.copyUrl") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 停止确认：强制结束游戏 -->
    <BaseModal v-if="stopAsk" :title="t('logWindow.stop')" @close="stopAsk = false">
      <p class="modal-text">{{ t("logWindow.stopConfirm") }}</p>
      <div class="modal-actions">
        <BaseButton @click="stopAsk = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="doStop">{{ t("logWindow.stop") }}</BaseButton>
      </div>
    </BaseModal>
  </WindowFrame>
</template>

<style scoped>
.log-window {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  min-height: 0;
  /* 四周留白交给外壳的 .frame-body（上下 22 / 左右 26），
     这里**不能再加一份 padding**，否则顶部 34、左右 40，比别的窗口鼓一圈
     —— 与导出窗口同一条规矩，见 ExportWindow 的 .export-window */
  box-sizing: border-box;
}

/* 工具条：内容不换行，宽度不够时下拉自己缩 */
.bar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  flex-wrap: wrap;
}

/* ---------- 标题栏里的一组控件（head-right 插槽） ---------- */

/* 与方块窗口的工具条同一条规矩：标题栏里空间紧，这一组整体可收缩，先挤下拉不裁按钮 */
.head-log {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 1;
  min-width: 0;
}

/* 按钮不缩、图标键方正化、chip 可压、下拉宽度与省略号：都在 styles/titlebar.css 的标题栏标准里 */

/* 开关 + 文案：文案跟着开关走，点哪都算切换 */
.sw {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text);
  user-select: none;
  cursor: pointer;
}

.lbl {
  font-size: 12.5px;
  color: var(--text-dim);
}

.pick {
  flex: 1;
  min-width: 140px;
}

.url {
  width: 100%;
  box-sizing: border-box;
}

/* 崩溃分析横幅：Mixin 错误时贴在日志正文上方 */
.analysis-card {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 10px;
  border: 1px solid var(--red);
  border-radius: 10px;
  background: color-mix(in srgb, var(--red) 8%, var(--bg));
}

.analysis-icon {
  flex: none;
  margin-top: 2px;
  color: var(--red);
}

.analysis-body {
  flex: 1;
  min-width: 0;
}

.analysis-title {
  font-size: 12.5px;
  color: var(--text);
}

.analysis-suspect {
  margin-left: 6px;
  color: var(--red);
  font-weight: 600;
}

.analysis-detail {
  margin-top: 2px;
  font-size: 11.5px;
  color: var(--text-dim);
  word-break: break-all;
}

.analysis-close {
  flex: none;
  padding: 0 2px;
  border: 0;
  background: transparent;
  color: var(--text-dim);
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
}

.analysis-close:hover {
  color: var(--text);
}

.modal-text {
  margin: 0 0 14px;
  font-size: 13px;
  color: var(--text);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
