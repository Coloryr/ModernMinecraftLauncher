<script setup lang="ts">
// 自定义主页面示例：**服务器主页**
//
// 用途：服主把这一页当成自己服务器的门面——公告、活动、群号、赞助名单、进服引导，
// 再配合几个按钮直接驱动启动器（拉实例列表、启动游戏、看启动状态）。
// 页面会被启动器整页加载（iframe + mml-home:// 协议），能力来自注入的 window.mml。
//
// 改哪里：
//   - 下面 SERVER / EVENTS 两块就是全部"服务器内容"，改成你自己的即可；
//   - 启动器交互（拉实例、启动、状态）在 <script> 后半段，一般不用动；
//   - 配色在 src/style.css，已经支持启动器的暗色 / 亮色跟随。
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { InstanceInfoDto } from "mml-ipc";
import {
  STAGE_LABELS,
  commands,
  getMml,
  onGameExit,
  onLaunchError,
  onLaunchState,
  probeHost,
} from "./bridge";

// ==================== 服主自己改这里 ====================

/** 服务器名片：公告 / 群号 / 进服方式 / 赞助名单 */
const SERVER = {
  name: "Coloryr的服务器",
  slogan: "1.20.1 原版生存 · 白名单 · 长期开放",
  /** 顶部滚动公告（一句话说清"现在该干什么"） */
  ticker: "本周活动：周六 20:00 全服寻宝，冠军队伍得限定称号",
  /** 进服前必读 */
  rules: [
    "禁止任何作弊模组与脚本，被举报即封禁",
    "公共区域建筑请勿破坏，私人领地可申请圈地",
    "服务器 1.20.1 Java 版，正版账号可直接进",
  ],
  /** 进服方式：本整合包会把这些自动填好，玩家只需点启动 */
  join: [
    { k: "服务器地址", v: "api.coloryr.com" },
    { k: "群号", v: "571239090" },
    { k: "版本", v: "1.20.1 · Fabric" },
  ],
  /** 开服时间（给玩家一个预期） */
  schedule: "每天 08:00 – 24:00（维护会提前在群里通知）",
  /** 赞助名单（留名 / 感谢） */
  sponsors: ["Coloryr", "热心玩家 A", "热心玩家 B", "匿名服主", "建筑组的小王"],
  /** 页面底部的联系人 */
  contact: "问题反馈请在本群 @ 管理员，或在游戏内 /mail send 管理员",
};

/** 活动日历（最近几场，玩家最关心这个） */
const EVENTS = [
  { date: "本周六 20:00", title: "全服寻宝", desc: "地图各处埋了 30 个宝箱，先到先得" },
  { date: "下周三 19:30", title: "建筑大赛投票", desc: "作品提交截止到周二晚，群里投票" },
  { date: "每月 1 日", title: "服务器维护", desc: "换周目存档备份、插件更新，预计 1 小时" },
];

// ==================== 启动器交互（一般不用改） ====================

/** 宿主状态：probing = 正在握手，ready = 在启动器里，absent = 不在启动器里（浏览器预览） */
const host = ref<"probing" | "ready" | "absent">("probing");
const instances = ref<InstanceInfoDto[]>([]);
const loading = ref(false);
const error = ref("");
/** 正在请求启动的实例 uuid（按钮转圈用） */
const launching = ref("");
/** 当前启动阶段与进度（来自 launch-state 事件） */
const stage = ref("");
const progress = ref<number | null>(null);
/** 最近几条动态（启动阶段 / 退出 / 失败） */
const activities = ref<string[]>([]);

/** 事件取消订阅函数，组件卸载时统一调用 */
const offs: Array<() => void> = [];

/** 启动阶段文案 */
const stageLabel = computed(() =>
  stage.value ? (STAGE_LABELS[stage.value] ?? stage.value) : "",
);

/** 是否已经在跑（有实例处于运行中） */
const anyRunning = computed(() => instances.value.some((i) => i.running));

/** 服务器推荐实例：跟这一页定位一致的整合包（没有就用第一个） */
const serverInstance = computed(() => {
  const list = instances.value;
  return (
    list.find((i) => i.serverUrl === SERVER.join[0].v) ??
    list.find((i) => /server|整合|暮色/i.test(i.name)) ??
    list[0] ??
    null
  );
});

/** 浏览器预览用的示例数据：只在没连上启动器时显示，页面上会明确标注 */
const DEMO_INSTANCES: InstanceInfoDto[] = [
  {
    uuid: "demo-1",
    name: "暮色群岛 · 1.20.1",
    group: "服务器",
    version: "1.20.1",
    versionType: "release",
    quickPlay: false,
    autoSave: null,
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
    name: "暮色群岛 · 测试服",
    group: "服务器",
    version: "1.21.1",
    versionType: "release",
    quickPlay: false,
    autoSave: null,
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

/** 页面上要显示的列表：连上启动器就是真实数据，否则是示例数据 */
const shownInstances = computed(() =>
  host.value === "ready" ? instances.value : DEMO_INSTANCES,
);

/** 主按钮指向的实例 */
const mainTarget = computed(() =>
  host.value === "ready" ? serverInstance.value : DEMO_INSTANCES[0],
);

function nameOf(uuid: string): string {
  return (
    instances.value.find((i) => i.uuid === uuid)?.name ??
    DEMO_INSTANCES.find((i) => i.uuid === uuid)?.name ??
    uuid.slice(0, 8)
  );
}

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function log(text: string) {
  activities.value.unshift(text);
  if (activities.value.length > 5) activities.value.length = 5;
}

/** 拉实例列表（main_get_instances） */
async function refreshInstances() {
  loading.value = true;
  error.value = "";
  try {
    instances.value = await commands.main.getInstances();
  } catch (err) {
    error.value = messageOf(err);
  } finally {
    loading.value = false;
  }
}

/** 启动游戏（main_launch_game；本示例只关心实例 uuid，存档目录 / 强开 / 自动存档都留空） */
async function launch(inst: InstanceInfoDto | null) {
  if (!inst || host.value !== "ready") return;
  launching.value = inst.uuid;
  error.value = "";
  try {
    await commands.main.launchGame(inst.uuid, null, null, null);
    inst.running = true;
    log(`已请求启动：${inst.name}`);
  } catch (err) {
    error.value = messageOf(err);
    log(`启动请求失败：${messageOf(err)}`);
  } finally {
    launching.value = "";
  }
}

// ==================== 生命周期 ====================

onMounted(async () => {
  if (!getMml()) {
    host.value = "absent";
    return;
  }

  // 订阅先挂上：桥接脚本会把 subscribe 排到握手之后，不会漏掉早期事件
  offs.push(
    onLaunchState((e) => {
      stage.value = e.state;
      progress.value = e.progress;
      const pct = e.progress === null ? "" : ` ${e.progress}%`;
      log(`[${nameOf(e.uuid)}] ${STAGE_LABELS[e.state] ?? e.state}${pct}`);
    }),
  );
  offs.push(
    onGameExit((e) => {
      const inst = instances.value.find((i) => i.uuid === e.uuid);
      if (inst) inst.running = false;
      stage.value = "";
      progress.value = null;
      log(`[${nameOf(e.uuid)}] 游戏已退出（退出码 ${e.code}）`);
    }),
  );
  offs.push(
    onLaunchError((e) => {
      error.value = e.message;
      log(`启动失败：${e.message}`);
    }),
  );

  // 握手探测：确认页面确实跑在启动器里，再拉数据
  const ok = await probeHost();
  host.value = ok ? "ready" : "absent";
  if (ok) {
    await refreshInstances();
    log("已连接启动器");
  }
});

onUnmounted(() => {
  for (const off of offs) off();
  offs.length = 0;
});
</script>

<template>
  <div class="page">
    <!-- ① 服务器门面：名字 / 标语 / 滚动公告 -->
    <header class="hero">
      <div class="hero-row">
        <div>
          <h1>{{ SERVER.name }}</h1>
          <p class="slogan">{{ SERVER.slogan }}</p>
        </div>
        <div class="hero-actions">
          <button class="btn hero-btn" :disabled="host !== 'ready' || !mainTarget || anyRunning || launching !== ''"
            @click="launch(mainTarget)">
            {{
              anyRunning
                ? "游戏运行中"
                : launching
                  ? "正在启动…"
                  : host === "ready"
                    ? "一键进服"
                    : "一键进服（需在启动器中打开）"
            }}
          </button>
        </div>
      </div>
      <p class="ticker">📣 {{ SERVER.ticker }}</p>
    </header>

    <!-- ② 环境提示：不在启动器里打开时，命令与事件都用不了 -->
    <div v-if="host === 'probing'" class="notice">正在连接启动器…</div>
    <div v-else-if="host === 'absent'" class="notice warn">
      未检测到启动器（应该是用浏览器直接打开的）：下面是示例数据，「一键进服」不会真的启动游戏。
      要看真实效果，把本工程构建出的 zip 导入启动器：设置 → 客户端设置 → 自定义主页面。
    </div>

    <!-- ③ 主体：左边内容、右边进服信息 -->
    <div class="grid">
      <div class="col">
        <!-- 活动日历 -->
        <section class="card">
          <h2>最近活动</h2>
          <ul class="events">
            <li v-for="ev in EVENTS" :key="ev.title" class="event">
              <span class="event-date">{{ ev.date }}</span>
              <span class="event-body">
                <b>{{ ev.title }}</b>
                <em>{{ ev.desc }}</em>
              </span>
            </li>
          </ul>
        </section>

        <!-- 服务器规则：进服前必读 -->
        <section class="card">
          <h2>进服必读</h2>
          <ul class="rules">
            <li v-for="r in SERVER.rules" :key="r">{{ r }}</li>
          </ul>
        </section>

        <!-- 游戏实例（来自启动器，可按需切换） -->
        <section class="card">
          <div class="card-head">
            <h2>选择客户端</h2>
            <button class="btn ghost" :disabled="loading || host !== 'ready'" @click="refreshInstances">
              {{ loading ? "读取中…" : "刷新列表" }}
            </button>
          </div>
          <p v-if="host === 'ready' && !loading && instances.length === 0" class="empty">
            启动器里还没有游戏实例：先添加一个（或导入本服整合包），再回来点「一键进服」。
          </p>
          <ul class="inst-list">
            <li v-for="inst in shownInstances" :key="inst.uuid" class="inst">
              <div class="inst-info">
                <div class="inst-name">
                  {{ inst.name }}
                  <span v-if="inst.running" class="tag ok">运行中</span>
                  <span v-else-if="mainTarget && inst.uuid === mainTarget.uuid" class="tag">
                    推荐
                  </span>
                </div>
                <div class="inst-meta">
                  <span class="tag dim">{{ inst.version }}</span>
                  <span v-if="inst.loader" class="tag dim">{{ inst.loader }}</span>
                  <span v-if="inst.group" class="tag dim">{{ inst.group }}</span>
                </div>
              </div>
              <button class="btn primary" :disabled="host !== 'ready' || launching === inst.uuid || inst.running"
                @click="launch(inst)">
                {{ inst.running ? "运行中" : launching === inst.uuid ? "启动中…" : "启动" }}
              </button>
            </li>
          </ul>
        </section>
      </div>

      <aside class="col side">
        <!-- 进服方式 -->
        <section class="card">
          <h2>怎么进服</h2>
          <p v-for="row in SERVER.join" :key="row.k" class="kv">
            <span>{{ row.k }}</span><b>{{ row.v }}</b>
          </p>
          <p class="hint">点上面的「一键进服」即可；客户端会自动连上服务器。</p>
        </section>

        <!-- 开服时间 -->
        <section class="card">
          <h2>开服时间</h2>
          <p class="hint">{{ SERVER.schedule }}</p>
        </section>

        <!-- 赞助名单 -->
        <section class="card">
          <h2>赞助名单</h2>
          <ul class="sponsors">
            <li v-for="s in SERVER.sponsors" :key="s">{{ s }}</li>
          </ul>
          <p class="hint">感谢以上玩家的支持，服务器才能一直开下去。</p>
        </section>
      </aside>
    </div>

    <!-- ④ 启动状态：阶段文本 + 进度 + 最近动态（来自 launch-state / game-exit） -->
    <section class="card status">
      <div class="card-head">
        <h2>启动状态</h2>
        <span class="stage">{{ stageLabel || "空闲" }}</span>
      </div>
      <div class="track">
        <div v-if="progress !== null" class="bar" :style="{ width: progress + '%' }"></div>
        <div v-else-if="stage" class="bar rolling"></div>
      </div>
      <p v-if="error" class="err">{{ error }}</p>
      <ul v-if="activities.length" class="log">
        <li v-for="(line, i) in activities" :key="i">{{ line }}</li>
      </ul>
    </section>

    <!-- ⑤ 页脚 -->
    <footer class="foot">
      <p>{{ SERVER.contact }}</p>
      <p class="foot-dim">
        本页由服主定制 · 改 <code>src/App.vue</code> 的 SERVER / EVENTS 即可 · 能力说明见
        <code>README.md</code>
      </p>
    </footer>
  </div>
</template>

<style scoped>
.page {
  /* 铺满 iframe：整块撑满，宽度自适应（窄窗口也不裁切） */
  min-height: 100%;
  width: 100%;
  max-width: 1180px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* ① 门面 */
.hero {
  background: linear-gradient(120deg, var(--accent-2), var(--accent));
  border-radius: 14px;
  padding: 20px 24px;
  color: var(--hero-fg);
  box-shadow: var(--shadow);
}

.hero-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  flex-wrap: wrap;
}

.hero h1 {
  margin: 0;
  font-size: 26px;
  letter-spacing: 0.5px;
}

.slogan {
  margin: 4px 0 0;
  opacity: 0.88;
  font-size: 13px;
}

.hero-btn {
  background: rgba(255, 255, 255, 0.18);
  border: 1px solid rgba(255, 255, 255, 0.5);
  color: var(--hero-fg);
  font-weight: 600;
  padding: 10px 20px;
  font-size: 14px;
  backdrop-filter: blur(2px);
}

.hero-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.3);
}

.ticker {
  margin: 14px 0 0;
  padding-top: 12px;
  border-top: 1px solid rgba(255, 255, 255, 0.24);
  font-size: 13px;
}

/* ② 提示条 */
.notice {
  padding: 10px 14px;
  border-radius: 10px;
  background: var(--bg-card-2);
  border: 1px solid var(--line);
  color: var(--fg-dim);
  font-size: 13px;
}

.notice.warn {
  border-color: var(--warn);
  color: var(--warn);
}

/* ③ 两栏 */
.grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 300px;
  gap: 16px;
  align-items: start;
}

@media (max-width: 900px) {
  .grid {
    grid-template-columns: minmax(0, 1fr);
  }
}

.col {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

.card {
  background: var(--bg-card);
  border: 1px solid var(--line);
  border-radius: 12px;
  padding: 16px 18px;
}

.card h2 {
  margin: 0;
  font-size: 15px;
}

.card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

/* 活动 */
.events {
  list-style: none;
  margin: 12px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.event {
  display: flex;
  gap: 12px;
  align-items: baseline;
  padding: 9px 12px;
  border-radius: 10px;
  background: var(--bg-card-2);
}

.event-date {
  flex: 0 0 92px;
  color: var(--accent);
  font-size: 12px;
  font-weight: 600;
}

.event-body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.event-body em {
  font-style: normal;
  color: var(--fg-dim);
  font-size: 12px;
}

/* 规则 */
.rules {
  margin: 12px 0 0;
  padding-left: 18px;
  color: var(--fg);
  font-size: 13px;
}

.rules li+li {
  margin-top: 4px;
}

/* 实例列表 */
.inst-list {
  list-style: none;
  margin: 12px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.inst {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  background: var(--bg-card-2);
  border-radius: 10px;
  padding: 10px 12px;
}

.inst-info {
  min-width: 0;
}

.inst-name {
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.inst-meta {
  margin-top: 5px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.tag {
  font-size: 12px;
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
}

.tag.dim {
  background: var(--tag-dim-bg);
  color: var(--fg-dim);
}

.tag.ok {
  background: var(--ok-soft);
  color: var(--ok);
}

.empty,
.hint {
  color: var(--fg-dim);
  font-size: 13px;
}

.kv {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  margin: 10px 0 6px;
  font-size: 13px;
}

.kv b {
  color: var(--accent);
  letter-spacing: 0.5px;
}

.sponsors {
  margin: 10px 0 6px;
  padding-left: 18px;
  font-size: 13px;
}

/* 按钮 */
.btn {
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  transition: filter 0.15s ease, opacity 0.15s ease;
}

.btn:hover:not(:disabled) {
  filter: brightness(1.1);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn.primary {
  background: var(--accent-grad);
  color: #fff;
  font-weight: 600;
  flex-shrink: 0;
}

.btn.ghost {
  background: transparent;
  border-color: var(--line);
  color: var(--fg-dim);
}

/* ④ 启动状态 */
.status {
  margin-top: auto;
}

.stage {
  color: var(--accent);
  font-size: 13px;
}

.track {
  margin-top: 12px;
  height: 8px;
  border-radius: 999px;
  background: var(--bg-card-2);
  overflow: hidden;
}

.bar {
  height: 100%;
  background: var(--accent-grad);
  transition: width 0.25s ease;
}

.bar.rolling {
  width: 35%;
  animation: roll 1.1s ease-in-out infinite;
}

@keyframes roll {
  0% {
    margin-left: -35%;
  }

  100% {
    margin-left: 100%;
  }
}

.err {
  margin: 10px 0 0;
  color: var(--err);
  font-size: 13px;
}

.log {
  list-style: none;
  margin: 10px 0 0;
  padding: 0;
  color: var(--fg-dim);
  font-size: 12px;
  font-family: Consolas, "Cascadia Mono", monospace;
}

/* ⑤ 页脚 */
.foot {
  text-align: center;
  color: var(--fg-dim);
  font-size: 12px;
}

.foot p {
  margin: 0;
}

.foot-dim {
  margin-top: 4px;
  opacity: 0.75;
}

.foot code {
  color: var(--accent);
}
</style>
