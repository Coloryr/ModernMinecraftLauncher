// 截图用的 Tauri 运行时替身：在页面脚本之前注入，拦截全部 IPC。
//
// 数据来自 window.__SHOT__（由 scripts/screenshot/run.mjs 在注入前拼好）：
// - commands: 命令名 → 返回值；没命中的命令按插件默认值处理，仍没有就记进 unknown
// - label: 本窗口标签，决定 App 挂哪个窗口（见 windows/windowManager.ts 的 resolveKind）
(() => {
  const cfg = window.__SHOT__ || {};
  const data = cfg.commands || {};
  const calls = [];
  const unknown = [];
  const errors = [];
  const listeners = new Map();
  const callbacks = new Map();
  let nextId = 1;

  const clone = (value) => (value === undefined ? null : JSON.parse(JSON.stringify(value)));
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  /** 插件命令的兜底值：窗口 / 事件 / 对话框 / 路径这些页面启动就要用 */
  function pluginDefault(cmd, args) {
    if (cmd === "plugin:event|listen") {
      const id = nextId++;
      listeners.set(id, { event: args?.event, handler: args?.handler });
      return id;
    }
    if (cmd === "plugin:event|unlisten") {
      listeners.delete(args?.eventId);
      return null;
    }
    if (cmd === "plugin:event|emit" || cmd === "plugin:event|emit_to") return null;
    if (cmd === "plugin:dialog|save") return cfg.savePath || "C:\\Users\\shot\\export.txt";
    if (cmd === "plugin:dialog|open") return cfg.openPath || null;
    if (cmd === "plugin:dialog|ask" || cmd === "plugin:dialog|confirm") return true;
    if (cmd === "plugin:dialog|message") return null;
    if (cmd.startsWith("plugin:path|")) return cfg.homeDir || "C:\\Users\\shot";
    if (cmd === "plugin:app|version") return "0.0.1";
    if (cmd === "plugin:app|name") return "M²L";
    if (cmd === "plugin:app|tauri_version") return "2.0.0";
    if (cmd === "plugin:app|bundle_type") return "app";
    if (cmd === "plugin:window|is_maximized") return false;
    if (cmd === "plugin:window|is_fullscreen") return false;
    if (cmd === "plugin:window|is_minimized") return false;
    if (cmd === "plugin:window|is_focused") return true;
    if (cmd === "plugin:window|is_visible") return true;
    if (cmd === "plugin:window|is_decorated") return true;
    if (cmd === "plugin:window|theme") return "light";
    if (cmd === "plugin:window|scale_factor") return 1;
    if (cmd === "plugin:window|title") return cfg.title || "M²L";
    if (cmd === "plugin:window|inner_size" || cmd === "plugin:window|outer_size") {
      return { width: cfg.width || 1280, height: cfg.height || 800 };
    }
    if (cmd === "plugin:window|inner_position" || cmd === "plugin:window|outer_position") {
      return { x: 0, y: 0 };
    }
    if (cmd === "plugin:window|cursor_position") return { x: 0, y: 0 };
    if (cmd === "plugin:window|current_monitor" || cmd === "plugin:window|primary_monitor") return null;
    if (cmd === "plugin:window|available_monitors") return [];
    if (cmd === "plugin:webview|position" || cmd === "plugin:webview|size") return null;
    return undefined;
  }

  const internals = {
    invoke: async (cmd, args) => {
      calls.push({ cmd, args });
      if (Object.prototype.hasOwnProperty.call(data, cmd)) {
        const entry = data[cmd];
        // 场景可用 { __delay, value } / { __error } 模拟慢查询与失败
        if (entry && typeof entry === "object" && !Array.isArray(entry)) {
          if (entry.__delay) await sleep(entry.__delay);
          if (entry.__error) throw new Error(entry.__error);
          if ("value" in entry) return clone(entry.value);
        }
        return clone(entry);
      }
      const fallback = pluginDefault(cmd, args);
      if (fallback !== undefined) return fallback;
      unknown.push(cmd);
      return null;
    },
    transformCallback: (callback, once) => {
      const id = nextId++;
      callbacks.set(id, { callback, once });
      return id;
    },
    unregisterCallback: (id) => callbacks.delete(id),
    unregisterListener: (id) => listeners.delete(id),
    convertFileSrc: (filePath) => filePath,
    metadata: {
      currentWindow: { label: cfg.label || "mml-main" },
      currentWebview: { label: cfg.label || "mml-main", windowLabel: cfg.label || "mml-main" },
      windows: [{ label: cfg.label || "mml-main" }],
      webviews: [{ label: cfg.label || "mml-main", windowLabel: cfg.label || "mml-main" }],
    },
  };

  window.__TAURI_INTERNALS__ = internals;
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener(event, eventId) {
      listeners.delete(eventId);
      void event;
    },
  };
  window.__SHOT__ = {
    ...cfg,
    calls,
    unknown,
    errors,
    /** 向页面派发一条 Tauri 事件；payload 原样传给监听者 */
    emit(name, payload) {
      for (const [id, item] of listeners) {
        if (item.event !== name) continue;
        const entry = callbacks.get(item.handler);
        if (!entry) continue;
        try {
          entry.callback({ event: name, id, payload });
        } catch (e) {
          errors.push(String(e));
        }
      }
    },
    callsOf(cmd) {
      return calls.filter((call) => call.cmd === cmd);
    },
  };

  window.addEventListener("error", (e) => errors.push(String(e.message || e.error)));
  window.addEventListener("unhandledrejection", (e) => errors.push(String(e.reason)));
})();
