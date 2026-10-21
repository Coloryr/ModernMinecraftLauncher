// 整页 HTML 主题示例：不用框架，window.mml 由启动器的 mml-ui 协议注入。
//
// 在启动器里：拉真实实例、真的调用 main_launch_game、订阅启动阶段。
// 直接打开 src/index.html 或在浏览器里预览时没有 window.mml，用一份假数据把界面跑起来，
// 窗口按钮与主题切换只在本页生效。

const host = window.mml;
const root = document.documentElement;
const $ = (id) => document.getElementById(id);

/** 浏览器预览用的假实例，字段与 InstanceInfoDto 对齐 */
const DEMO_INSTANCES = [
  { uuid: "preview-1", name: "星野生存", version: "1.20.1", loader: "Forge", loaderVersion: "47.2.0", running: false },
  { uuid: "preview-2", name: "轻量原版", version: "1.21.4", loader: "Fabric", loaderVersion: "0.16.9", running: false },
  { uuid: "preview-3", name: "创造建筑服", version: "1.20.4", loader: "NeoForge", loaderVersion: "20.4.5", running: true },
];

/** 启动阶段文案；表里没有的原样显示，方便启动器加新阶段 */
const STAGE_LABELS = {
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

const el = {
  instances: $("instances"),
  empty: $("instance-empty"),
  refresh: $("refresh"),
  selectedAvatar: $("selected-avatar"),
  selectedName: $("selected-name"),
  selectedMeta: $("selected-meta"),
  launch: $("launch"),
  launchLabel: $("launch-label"),
  progress: $("progress"),
  progressFill: $("progress-fill"),
  stage: $("stage-text"),
  badge: $("launch-state"),
  error: $("error-text"),
  hostHint: $("host-hint"),
  hostState: $("host-state"),
  themeToggle: $("theme-toggle"),
  titlebar: document.querySelector("[data-titlebar]"),
};

let instances = [];
let selected = null;
/** 正在等启动命令返回的实例 uuid；空串表示没有请求在飞 */
let launching = "";
let progress = null;
let stageText = "准备就绪";
let errorText = "";
let previewTimer = null;
let flashTimer = null;

/** 调启动器命令；不在启动器里时直接拒绝，避免界面卡在等待上 */
function invoke(cmd, args) {
  if (!host) return Promise.reject(new Error("当前不在启动器中打开，window.mml 不可用"));
  return host.invoke(cmd, args);
}

/** 异常转成一句能显示的话 */
function messageOf(err) {
  return err instanceof Error ? err.message : String(err);
}

/** 取名字首字符，兼容代理对 */
function initial(name) {
  const chars = Array.from((name || "").trim());
  return chars.length > 0 ? chars[0].toUpperCase() : "?";
}

/** 实例副标题：版本 · 加载器 · 加载器版本 */
function instanceMeta(inst) {
  return [inst.version, inst.loader, inst.loaderVersion].filter(Boolean).join(" · ") || "未知版本";
}

/** 清掉模拟启动的定时器 */
function stopPreview() {
  if (previewTimer !== null) {
    clearTimeout(previewTimer);
    previewTimer = null;
  }
}

/** 底部状态栏闪一条提示，再恢复连接状态 */
function flashHostState(text) {
  if (flashTimer !== null) clearTimeout(flashTimer);
  el.hostState.textContent = text;
  flashTimer = setTimeout(() => {
    flashTimer = null;
    el.hostState.textContent = host ? "已连接启动器" : "浏览器预览 · 模拟数据";
  }, 2400);
}

function showError(text) {
  errorText = text;
  el.error.textContent = text;
  el.error.hidden = false;
  renderStage();
}

function clearError() {
  errorText = "";
  el.error.hidden = true;
}

function renderInstances() {
  el.instances.replaceChildren();
  for (const inst of instances) {
    const item = document.createElement("button");
    item.type = "button";
    item.className = "instance" + (selected && selected.uuid === inst.uuid ? " is-active" : "");
    item.setAttribute("role", "option");
    item.setAttribute("aria-selected", selected && selected.uuid === inst.uuid ? "true" : "false");

    const avatar = document.createElement("span");
    avatar.className = "avatar";
    avatar.textContent = initial(inst.name);

    const info = document.createElement("span");
    info.className = "instance-info";
    const name = document.createElement("strong");
    name.textContent = inst.name || "未命名实例";
    const meta = document.createElement("span");
    meta.className = "muted";
    meta.textContent = instanceMeta(inst);
    info.append(name, meta);

    item.append(avatar, info);
    if (inst.running) {
      const running = document.createElement("span");
      running.className = "running-dot";
      running.textContent = "运行中";
      item.append(running);
    }

    item.addEventListener("click", () => selectInstance(inst));
    el.instances.append(item);
  }
  el.empty.hidden = instances.length > 0;
}

function renderSelected() {
  el.selectedAvatar.textContent = selected ? initial(selected.name) : "?";
  el.selectedName.textContent = selected ? selected.name : "未选择实例";
  el.selectedMeta.textContent = selected ? instanceMeta(selected) : "从左侧选一个实例";
  const busy = Boolean(selected && launching === selected.uuid);
  el.launch.disabled = !selected || busy || selected.running;
  el.launchLabel.textContent = busy ? "启动中…" : selected && selected.running ? "运行中" : "开始冒险";
}

function renderStage() {
  el.stage.textContent = stageText;
  el.progress.hidden = progress === null;
  if (progress !== null) el.progressFill.style.width = Math.min(100, Math.max(0, progress)) + "%";

  let label = "待机";
  let state = "";
  if (errorText) {
    label = "启动失败";
    state = "error";
  } else if (selected && launching === selected.uuid) {
    label = "启动中";
    state = "busy";
  } else if (selected && selected.running) {
    label = "运行中";
    state = "busy";
  }
  el.badge.textContent = label;
  el.badge.dataset.state = state;
}

/** 三块一起重画：实例列表、选中详情、启动状态 */
function render() {
  renderInstances();
  renderSelected();
  renderStage();
}

/** 选中实例时把上一轮的启动状态清掉 */
function selectInstance(inst) {
  if (selected && selected.uuid === inst.uuid) return;
  stopPreview();
  selected = inst;
  // 预览里的启动是本地定时器，换实例就停；真实启动请求继续跑，下面按 uuid 对
  if (!host) launching = "";
  progress = null;
  clearError();
  stageText = inst.running ? "游戏正在运行" : "准备就绪";
  render();
}

async function loadInstances() {
  el.refresh.disabled = true;
  try {
    const list = host ? await invoke("main_get_instances") : DEMO_INSTANCES.map((item) => ({ ...item }));
    instances = Array.isArray(list) ? list : [];

    const kept = selected ? instances.find((item) => item.uuid === selected.uuid) : null;
    selected = kept || instances[0] || null;
    stopPreview();
    if (!host) launching = "";
    progress = null;
    clearError();
    stageText = selected && selected.running
      ? "游戏正在运行"
      : selected && launching === selected.uuid
        ? "正在请求启动…"
        : "准备就绪";
    render();
  } catch (err) {
    showError("读取实例失败：" + messageOf(err));
  } finally {
    el.refresh.disabled = false;
  }
}

async function launch() {
  if (!selected || launching || selected.running) return;
  const target = selected;

  clearError();
  launching = target.uuid;
  progress = null;
  stageText = "正在请求启动…";
  render();

  if (!host) {
    simulateLaunch(target);
    return;
  }

  try {
    await invoke("main_launch_game", { uuid: target.uuid, world: null, force: null, autoSave: null });
    const inst = instances.find((item) => item.uuid === target.uuid);
    if (inst) inst.running = true;
    if (selected && selected.uuid === target.uuid) {
      stageText = "启动请求已提交，等待启动器回报阶段…";
    }
  } catch (err) {
    if (selected && selected.uuid === target.uuid) {
      stageText = "准备就绪";
      showError("启动失败：" + messageOf(err));
    }
  } finally {
    if (launching === target.uuid) launching = "";
    render();
  }
}

/** 预览模式的假启动：按阶段推进，最后一个阶段标成完成 */
function simulateLaunch(target) {
  const stages = [
    ["login", 8],
    ["check", 22],
    ["readinfo", 36],
    ["download", 62],
    ["jvm", 80],
    ["pre", 90],
    ["launching", 96],
    ["end", 100],
  ];
  let index = 0;

  const tick = () => {
    if (!selected || selected.uuid !== target.uuid) {
      stopPreview();
      return;
    }
    const [state, value] = stages[index];
    index += 1;
    progress = value;
    stageText = STAGE_LABELS[state] || state;
    if (state === "end") {
      target.running = true;
      if (launching === target.uuid) launching = "";
      stageText = "启动完成（模拟）";
    }
    render();
    if (index < stages.length) {
      previewTimer = setTimeout(tick, 650);
    } else {
      previewTimer = null;
    }
  };

  previewTimer = setTimeout(tick, 350);
}

/** 启动阶段事件：列表状态都更新，进度只画当前选中的实例 */
function onLaunchState(event) {
  const inst = instances.find((item) => item.uuid === event.uuid);
  const isCurrent = Boolean(selected && selected.uuid === event.uuid);

  if (event.state === "end") {
    if (inst) inst.running = true;
    if (launching === event.uuid) launching = "";
  }

  if (!isCurrent) {
    renderInstances();
    return;
  }

  progress = typeof event.progress === "number" ? event.progress : null;
  stageText = STAGE_LABELS[event.state] || event.state;
  render();
}

function onLaunchError(event) {
  if (launching === event.uuid) launching = "";

  if (!selected || (event.uuid && event.uuid !== selected.uuid)) {
    renderInstances();
    return;
  }

  progress = null;
  stageText = "准备就绪";
  showError("启动失败：" + (event.message || "启动器没有给出原因"));
  renderSelected();
}

function onGameExit(event) {
  const inst = instances.find((item) => item.uuid === event.uuid);
  if (inst) inst.running = false;

  if (selected && selected.uuid === event.uuid) {
    if (launching === event.uuid) launching = "";
    progress = null;
    stageText = "游戏已退出";
    render();
  } else if (inst) {
    renderInstances();
  }
}

async function toggleTheme() {
  const current = root.dataset.mmlTheme === "Light" ? "Light" : "Dark";
  const next = current === "Dark" ? "Light" : "Dark";

  if (!host) {
    root.dataset.mmlTheme = next;
    return;
  }

  try {
    const config = await invoke("window_get_gui_config");
    await invoke("window_save_gui_config", { config: { ...config, theme: next } });
  } catch (err) {
    flashHostState("切换主题失败：" + messageOf(err));
  }
}

function noop() {
  /* 窗口命令失败时没有可恢复的动作，忽略 */
}

/** 右上角窗口按钮；预览时只闪一条提示 */
function bindWindowControls() {
  document.querySelectorAll("[data-window]").forEach((button) => {
    button.addEventListener("click", () => {
      if (!host) {
        flashHostState("浏览器预览：窗口按钮不生效");
        return;
      }
      const action = button.dataset.window;
      if (action === "minimize") {
        invoke("window_minimize").catch(noop);
      } else if (action === "maximize") {
        invoke("window_toggle_maximize").catch(noop);
      } else if (action === "close") {
        invoke("window_close_window", { kind: "main", instance: null }).catch(noop);
      }
    });
  });
}

/** 标题栏拖动：单击拖动、双击最大化，和内置标题栏一致 */
function bindTitlebar() {
  if (!el.titlebar) return;
  let lastDownAt = 0;
  let lastTarget = null;

  el.titlebar.addEventListener("pointerdown", (event) => {
    if (!host || event.button !== 0) return;
    if (!(event.target instanceof Element)) return;
    if (event.target.closest("button, a, input, select, textarea, [data-no-drag]")) return;

    const now = Date.now();
    if (now - lastDownAt < 400 && event.target === lastTarget) {
      lastDownAt = 0;
      lastTarget = null;
      invoke("window_toggle_maximize").catch(noop);
      return;
    }
    lastDownAt = now;
    lastTarget = event.target;
    invoke("window_start_dragging").catch(noop);
  });
}

function boot() {
  if (host) {
    el.hostState.textContent = "已连接启动器";
  } else {
    el.hostState.textContent = "浏览器预览 · 模拟数据";
    el.hostHint.hidden = false;
  }

  bindWindowControls();
  bindTitlebar();
  el.refresh.addEventListener("click", () => void loadInstances());
  el.launch.addEventListener("click", () => void launch());
  el.themeToggle.addEventListener("click", () => void toggleTheme());

  if (host) {
    host.on("launch-state", onLaunchState);
    host.on("launch-error", onLaunchError);
    host.on("game-exit", onGameExit);
    host.on("instance-change", () => void loadInstances());
  }

  void loadInstances();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}
