<script setup lang="ts">
// 游戏日志窗口：**一个实例一个**，打开时锁定，窗口内不再选择实例
//
// 多窗口模式下是独立真实窗口，标签形如 `mml-log-<实例uuid>`（身份由后端按实例派生，
// 同一实例重复打开只聚焦）；单窗口模式下是叠在当前页面之上的浮动面板，实例由父组件传入
// （见 LogPopups.vue），可以同时开多个。
// 两种外壳同形：WindowFrame 与 FloatingPanel 都是 title + close 事件 + 默认插槽。
//
// 布局照 ColorMC 的 GameLogControl：一行"日志文件 + 刷新 / 搜索 / 上传 / 测试启动"，
// 一行"自动换行 / 自动下拉 + 五个等级"，一行"线程 / 分组"，最后是日志正文。
// 等级、换行、下拉都存进实例的 `gui_setting.json`（一个实例一套），线程与分组只筛当前这份。
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import FloatingPanel from "../../components/ui/FloatingPanel.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseSwitch from "../../components/ui/BaseSwitch.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import InstanceChip from "../../components/ui/InstanceChip.vue";
// Monaco 编辑器按需加载：它一块就是 ~2.6 MB（gzip 674 kB）。
// 异步化不能省掉这次下载（打开就是要看日志），但能让窗口外壳与工具条先画出来。
const MonacoLogView = defineAsyncComponent(() => import("../../components/MonacoLogView.vue"));
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { copyText } from "../../lib/clipboard";
import { api, onGameExit, onGameLog, onLaunchState } from "../../lib/api";
import { logInstanceFromLabel, multiWindow, targetUuid } from "../windowManager";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import type { InstanceInfoDto, LogLine, LogSettingDto } from "../../lib/bindings";

/** 请求关闭本窗口 / 收起面板 */
defineEmits<{ (e: "close"): void }>();

const props = defineProps<{
  /** 锁定的实例 uuid；单窗口模式的浮动面板由父组件传，独立窗口留空、从窗口标签取 */
  uuid?: string;
}>();

/** 每个实例的日志内存上限，与内核 InstanceRuntimeLog 保持一致 */
const MAX_LOG_LINES = 10000;

/**
 * 本窗口锁定的实例 uuid
 *
 * 浮动面板按父组件传入的参数；独立窗口按窗口标签，那是后端派生窗口身份时用的同一份实例；
 * 浏览器预览没有标签，退回 URL 上的参数。
 */
const lockedUuid = computed(() => props.uuid ?? logInstanceFromLabel() ?? targetUuid() ?? "");

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

/**
 * 外壳两用：多窗口模式下是独立窗口 `WindowFrame`；单窗口模式下是**浮动面板**
 * `FloatingPanel`，无遮罩、不挡页面，可叠放多个，见 LogPopups.vue
 */
const shell = computed(() => (multiWindow.value ? WindowFrame : FloatingPanel));
const shellProps = computed(() =>
  multiWindow.value
    ? { title: title.value, bodyFill: true }
    : {
      title: title.value,
      width: 820,
      // 固定高度：日志行一直在增长，不锁的话面板会跟着长高；内容区自己滚，
      // 视口太矮时由 FloatingPanel 的 max-height 兜住
      height: "580px",
    },
);

// ---------------- 设置：换行 / 自动下拉 / 五个等级 ----------------

/** 默认值与后端 `GameLogSettingObj::default` 一致：换行开、自动下拉关、等级全开 */
const setting = ref<LogSettingDto>({
  wordWrap: true,
  auto: false,
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

/** 去空去重排序 */
function uniq(values: string[]): string[] {
  return [...new Set(values.filter(Boolean))].sort();
}

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

/** 载入当前来源的全部日志行 */
async function loadSource() {
  if (!lockedUuid.value) return;
  // 换了来源，旧的筛选选项不再成立
  thread.value = "";
  category.value = "";
  if (liveSource.value) {
    try {
      sourceLogs.value = await api.getRuntimeLog(lockedUuid.value);
    } catch (e) {
      console.error("[LogWindow] 加载运行日志失败", e);
      sourceLogs.value = [];
    }
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
}

watch(file, loadSource);

// ---------------- 运行状态：测试启动 / 停止 ----------------

const running = ref(false);

/** 拉一次该实例是否在运行 */
async function refreshRunning() {
  if (!lockedUuid.value) return;
  try {
    running.value = (await api.getRunning()).includes(lockedUuid.value);
  } catch (e) {
    console.error("[LogWindow] 读取运行状态失败", e);
  }
}

/** 测试启动：与主页面同一个命令，日志会流进本窗口 */
async function testLaunch() {
  try {
    await api.launchGame(lockedUuid.value);
    // 自己启动的实例就是要盯着它的输出，顺手把自动下拉打开（与参考实现一致）
    setting.value.auto = true;
    await refreshRunning();
  } catch (e) {
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
const viewRef = ref<{ openFind: () => void } | null>(null);

/** 打开页内查找框：浏览器的 Ctrl+F 被壳层关掉了，只能按钮触发 */
function onSearch() {
  viewRef.value?.openFind();
}

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

/** 补一次实例名、设置、文件列表与当前日志；首次挂载、单窗口模式下切回面板都用 */
async function syncInstance() {
  if (!lockedUuid.value) return;
  try {
    const list: InstanceInfoDto[] = await api.getInstances();
    instance.value = list.find((i) => i.uuid === lockedUuid.value) ?? null;
  } catch (e) {
    console.error("[LogWindow] 读取实例列表失败", e);
  }
  await loadSetting();
  await refreshList();
  await loadSource();
  await refreshRunning();
}

// 单窗口模式：面板被 KeepAlive 缓存，切回时补一次数据
useWindowRefresh(syncInstance);

let unlistenGameLog: UnlistenFn | null = null;
let unlistenLaunchState: UnlistenFn | null = null;
let unlistenGameExit: UnlistenFn | null = null;

/** 追加一行并裁到上限，超出丢最旧的 */
function pushLine(line: LogLine) {
  sourceLogs.value.push(line);
  if (sourceLogs.value.length > MAX_LOG_LINES) {
    sourceLogs.value.splice(0, sourceLogs.value.length - MAX_LOG_LINES);
  }
}

onMounted(async () => {
  await syncInstance();

  unlistenGameLog = await onGameLog((e) => {
    // 锁定了实例：只收它自己的日志行；看历史文件时不追加实时行
    if (e.uuid !== lockedUuid.value || !liveSource.value) return;
    if (e.clear) {
      sourceLogs.value = [];
      return;
    }
    pushLine({
      time: e.time,
      text: e.text,
      thread: e.thread,
      level: e.level,
      category: e.category,
    });
  });

  // 本实例的启停：切换「测试启动 / 停止」按钮
  unlistenLaunchState = await onLaunchState((e) => {
    if (e.uuid === lockedUuid.value) void refreshRunning();
  });
  unlistenGameExit = await onGameExit((e) => {
    if (e.uuid !== lockedUuid.value) return;
    running.value = false;
    // 非 0 退出：顺手刷一次文件列表（崩溃报告刚写出来），并问一句要不要把日志传出去
    if (e.code !== 0) {
      crashCode.value = e.code;
      crashAsk.value = true;
      void refreshList();
    }
  });
});

onBeforeUnmount(() => {
  unlistenGameLog?.();
  unlistenLaunchState?.();
  unlistenGameExit?.();
});
</script>

<template>
  <component :is="shell" v-bind="shellProps" @close="$emit('close')">
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
        <BaseButton v-else size="sm" variant="accent" class="icon-btn-head" v-tip="t('logWindow.testLaunch')"
          :aria-label="t('logWindow.testLaunch')" @click="testLaunch">
          <GlyphIcon name="play" :size="15" />
        </BaseButton>
      </div>
    </template>

    <div class="log-window" :class="{ fill: !multiWindow }">
      <!-- 第一行：换行 / 自动下拉 / 等级 -->
      <div class="bar">
        <label class="sw">
          <BaseSwitch v-model="setting.wordWrap" />
          <span>{{ t("logWindow.wordWrap") }}</span>
        </label>
        <label class="sw">
          <BaseSwitch v-model="setting.auto" />
          <span>{{ t("logWindow.autoScroll") }}</span>
        </label>
        <label v-for="lv in LEVELS" :key="lv.key" class="lv">
          <input type="checkbox" v-model="setting[lv.key]" />
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

      <!-- 日志正文 -->
      <MonacoLogView ref="viewRef" :logs="shownLogs" :word-wrap="setting.wordWrap" :auto-scroll="setting.auto"
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
  </component>
</template>

<style scoped>
.log-window {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  min-height: 0;
  /* 四周留白交给外壳：独立窗口是 .frame-body（上下 22 / 左右 26），浮动面板是 .fp-body。
     这里**不能再加一份 padding**，否则顶部 34、左右 40，比别的窗口鼓一圈
     —— 与导出窗口同一条规矩，见 ExportWindow 的 .export-window */
  box-sizing: border-box;
}

/* 浮动面板里高度由 .fp-body 的剩余空间给，不再是撑满父级的 100% */
.log-window.fill {
  flex: 1;
  height: auto;
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

/* 等级勾选：一排小标签，与开关同高 */
.lv {
  display: flex;
  align-items: center;
  gap: 4px;
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
