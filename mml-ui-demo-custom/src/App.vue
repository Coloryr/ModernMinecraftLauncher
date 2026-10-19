<script setup lang="ts">
// 整页自定义界面示例：整个主窗口都交给这一页
//
// 与「自定义主页面」的区别：那边只换主窗口内容区，这里连标题栏 / 侧栏 / 内容区一起换。
// 页面通过 window.mml 调启动器命令；窗口拖动、最小化、最大化、关闭都由页面自己接。
import { computed, onMounted, onUnmounted, ref } from "vue";
import type {
  ErrorEvent,
  ExitEvent,
  InstanceChangeEvent,
  InstanceInfoDto,
  StateEvent,
} from "mml-ipc";
import {
  commands,
  getMml,
  onGameExit,
  onInstanceChange,
  onLaunchError,
  onLaunchState,
  probeHost,
} from "./bridge";

/** 启动阶段文案：表里没有的原样显示，后端加阶段不会空着 */
const STAGE_LABELS: Record<string, string> = {
  launching: "正在启动",
  login: "登录账户",
  check: "检查游戏文件",
  readinfo: "读取版本信息",
  download: "下载缺失文件",
  jvm: "准备启动参数",
  pre: "执行启动前命令",
  post: "执行启动后命令",
  end: "启动完成",
  loadserverpack: "加载服务器包",
  checkserverpack: "检查服务器包",
  downloadserverpack: "下载服务器包",
};

/** 宿主状态：probing = 握手，ready = 在启动器里，absent = 浏览器预览 */
const host = ref<"probing" | "ready" | "absent">("probing");
const instances = ref<InstanceInfoDto[]>([]);
const selectedUuid = ref("");
const loading = ref(false);
const error = ref("");
const launching = ref("");
const stage = ref("");
const progress = ref<number | null>(null);
const activities = ref<string[]>([]);
const offs: Array<() => void> = [];

const selected = computed(
  () => instances.value.find((i) => i.uuid === selectedUuid.value) ?? instances.value[0] ?? null,
);
const runningCount = computed(() => instances.value.filter((i) => i.running).length);
const stageLabel = computed(() => (stage.value ? (STAGE_LABELS[stage.value] ?? stage.value) : "空闲"));

/** 浏览器预览用的示例数据 */
const DEMO_INSTANCES: InstanceInfoDto[] = [
  {
    uuid: "demo-1",
    name: "暮色群岛 · 1.20.1",
    group: "服务器",
    version: "1.20.1",
    versionType: "release",
    quickPlay: false,
    loader: "fabric",
    loaderVersion: "0.15.11",
    dir: "（示例数据）",
    running: false,
    modpackType: null,
    pid: null,
    fid: null,
    serverUrl: "play.example.com",
    lang: null,
    logEncoding: null,
    source: null,
    order: 0,
  },
  {
    uuid: "demo-2",
    name: "原版测试服",
    group: "服务器",
    version: "1.21.1",
    versionType: "release",
    quickPlay: false,
    loader: "",
    loaderVersion: null,
    dir: "（示例数据）",
    running: false,
    modpackType: null,
    pid: null,
    fid: null,
    serverUrl: null,
    lang: null,
    logEncoding: null,
    source: null,
    order: 1,
  },
];

const hostLabel = computed(() =>
  host.value === "ready" ? "已连接启动器" : host.value === "probing" ? "正在连接…" : "浏览器预览",
);

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function log(text: string) {
  activities.value.unshift(text);
  if (activities.value.length > 6) activities.value.length = 6;
}

/** 拉实例列表；不在启动器里时用示例数据 */
async function refresh() {
  if (host.value !== "ready") {
    instances.value = DEMO_INSTANCES.map((i) => ({ ...i }));
    selectedUuid.value = instances.value[0]?.uuid ?? "";
    return;
  }
  loading.value = true;
  error.value = "";
  try {
    instances.value = await commands.main.getInstances();
    if (!instances.value.some((i) => i.uuid === selectedUuid.value)) {
      selectedUuid.value = instances.value[0]?.uuid ?? "";
    }
  } catch (err) {
    error.value = messageOf(err);
  } finally {
    loading.value = false;
  }
}

/** 启动选中实例 */
async function launch(inst: InstanceInfoDto | null) {
  if (!inst || host.value !== "ready" || inst.running) return;
  launching.value = inst.uuid;
  error.value = "";
  try {
    await commands.main.launchGame(inst.uuid, null);
    inst.running = true;
    log("已请求启动：" + inst.name);
  } catch (err) {
    error.value = messageOf(err);
    log("启动请求失败：" + messageOf(err));
  } finally {
    launching.value = "";
  }
}

/** 打开内置设置窗口：整页模式下也要留一条回去改配置的路 */
async function openSettings() {
  try {
    await commands.windows.openWindow("settings", null, null);
  } catch (err) {
    log("打开设置失败：" + messageOf(err));
  }
}

function minimize() {
  void commands.windows.minimize().catch(() => { });
}

function toggleMaximize() {
  void commands.windows.toggleMaximize().catch(() => { });
}

function closeMain() {
  void commands.windows.closeWindow("main", null).catch(() => { });
}

/** 自绘标题栏拖动：按钮 / 输入框上不触发 */
function onHeaderPointerDown(e: PointerEvent) {
  const target = e.target as HTMLElement | null;
  if (target?.closest("button, input, select, textarea, a")) return;
  void commands.windows.startDragging().catch(() => { });
}

/** 双击标题栏最大化 / 还原 */
function onHeaderDblClick(e: MouseEvent) {
  const target = e.target as HTMLElement | null;
  if (target?.closest("button, input, select, textarea, a")) return;
  toggleMaximize();
}

onMounted(async () => {
  if (!getMml()) {
    host.value = "absent";
    await refresh();
    return;
  }

  offs.push(
    onLaunchState((e: StateEvent) => {
      stage.value = e.state;
      progress.value = e.progress;
      if (e.state === "end") {
        const inst = instances.value.find((i) => i.uuid === e.uuid);
        if (inst) inst.running = true;
      }
      log((STAGE_LABELS[e.state] ?? e.state) + (e.progress === null ? "" : " " + e.progress + "%"));
    }),
  );
  offs.push(
    onGameExit((e: ExitEvent) => {
      const inst = instances.value.find((i) => i.uuid === e.uuid);
      if (inst) inst.running = false;
      stage.value = "";
      progress.value = null;
      log("游戏已退出（退出码 " + e.code + "）");
    }),
  );
  offs.push(
    onLaunchError((e: ErrorEvent) => {
      error.value = e.message;
      log("启动失败：" + e.message);
    }),
  );
  offs.push(
    onInstanceChange((_: InstanceChangeEvent) => {
      void refresh();
    }),
  );

  const ok = await probeHost();
  host.value = ok ? "ready" : "absent";
  await refresh();
});

onUnmounted(() => {
  for (const off of offs) off();
  offs.length = 0;
});
</script>

<template>
  <div class="app">
    <!-- 自绘标题栏：整页替代内置窗口框后，拖动区与窗口按钮要由页面补 -->
    <header class="bar" @pointerdown="onHeaderPointerDown" @dblclick="onHeaderDblClick">
      <div class="brand">M²L <span>自定义界面</span></div>
      <span class="chip" :class="{ on: host === 'ready' }">{{ hostLabel }}</span>
      <span class="spacer" />
      <button class="win-btn" title="最小化" @click="minimize">─</button>
      <button class="win-btn" title="最大化 / 还原" @click="toggleMaximize">□</button>
      <button class="win-btn close" title="关闭" @click="closeMain">✕</button>
    </header>

    <div v-if="host === 'absent'" class="banner">
      当前是浏览器预览：没有 window.mml，实例与启动按钮都是示例数据。
    </div>

    <div class="body">
      <!-- 侧栏：实例列表 + 设置入口，完全由这一页自己画 -->
      <aside class="side">
        <div class="side-head">
          <span>实例{{ loading ? " · 读取中" : "" }}</span>
          <button class="link" @click="refresh">刷新</button>
        </div>
        <button
          v-for="inst in instances"
          :key="inst.uuid"
          class="inst"
          :class="{ active: selected?.uuid === inst.uuid }"
          @click="selectedUuid = inst.uuid"
        >
          <span class="dot" :class="{ running: inst.running }" />
          <span class="inst-name">{{ inst.name }}</span>
          <span class="inst-meta">
            {{ inst.version }}{{ inst.loader ? " · " + inst.loader : "" }}
          </span>
        </button>
        <p v-if="!instances.length" class="empty">还没有实例，先在内置设置里添加。</p>
        <div class="side-foot">
          <button class="btn ghost" @click="openSettings">打开内置设置</button>
        </div>
      </aside>

      <main class="main">
        <section v-if="selected" class="hero">
          <div>
            <p class="eyebrow">当前实例</p>
            <h1>{{ selected.name }}</h1>
            <p class="desc">
              {{ selected.version }} · {{ selected.loader || "原版" }}
              <template v-if="selected.loaderVersion"> {{ selected.loaderVersion }}</template>
            </p>
            <p class="path">{{ selected.dir }}</p>
          </div>
          <button
            class="launch"
            :disabled="host !== 'ready' || selected.running || launching !== ''"
            @click="launch(selected)"
          >
            {{
              selected.running
                ? "运行中"
                : launching === selected.uuid
                  ? "启动中…"
                  : "启动游戏"
            }}
          </button>
        </section>

        <section class="card status">
          <div class="status-head">
            <h2>启动状态</h2>
            <span class="stage">{{ stageLabel }}</span>
          </div>
          <div class="track">
            <div class="bar-fill" :style="{ width: (progress ?? 0) + '%' }" />
          </div>
          <p v-if="error" class="err">{{ error }}</p>
          <ul v-if="activities.length" class="log">
            <li v-for="(line, i) in activities" :key="i">{{ line }}</li>
          </ul>
          <p v-if="host === 'ready' && runningCount > 0" class="log">
            {{ runningCount }} 个实例正在运行
          </p>
        </section>

        <div class="grid">
          <section class="card">
            <h2>这是「整页重写」示例</h2>
            <p>
              整个主窗口都换成这一页：标题栏、侧栏、内容区、窗口按钮全部自定义。
              标题栏按住可拖动，双击可最大化 / 还原。
            </p>
          </section>
          <section class="card">
            <h2>能做什么</h2>
            <ul>
              <li>commands.main.* 拉实例、启动游戏、读写配置</li>
              <li>commands.windows.* 开设置窗口、最小化、最大化、关闭</li>
              <li>onLaunchState / onGameExit 等事件实时更新界面</li>
              <li>想改内置界面又不想重写整页，用「自定义 CSS」模式</li>
            </ul>
          </section>
        </div>
      </main>
    </div>

    <footer class="foot">
      整页自定义界面示例 · 通过 window.mml 调启动器命令 · 不给 allow-same-origin
    </footer>
  </div>
</template>
