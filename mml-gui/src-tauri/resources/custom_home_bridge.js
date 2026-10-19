// M²L 自定义主页面桥接脚本
//
// 由 `mml-home://` 协议按 `/__mml_bridge.js` 直接返回（内置资源，不在 custom_home/ 里），
// 并自动注入到每个返回的 HTML 的 `</head>` 之前。页面通过全局的 `window.mml` 使用：
//
//   window.mml = {
//     invoke(cmd, args) -> Promise<any>,   // 调任意已注册的启动器命令
//     on(event, cb) -> () => void,         // 订阅白名单事件，返回取消订阅函数
//     onTheme(cb) -> () => void,           // 主题变化回调，返回取消订阅函数
//     theme: "Dark" | "Light",             // 当前主题（启动器下发的，实时更新）
//     ready: Promise,                      // 父窗口握手完成
//   }
//
// 主题：iframe 是独立文档，拿不到启动器页面 `<html data-theme>` 上的 CSS 变量，所以启动器
// 通过 postMessage 下发当前主题，本脚本写到自己的 `<html data-mml-theme="Dark|Light">` 上，
// 页面用 `[data-mml-theme="Light"]` 切配色即可。父窗口还没应答之前，先用系统偏好
// （prefers-color-scheme）当默认值，避免首屏闪一下错误主题。
//
// **裸 HTML 兜底**：没有自己写主题样式的页面（纯 index.html + 一点 CSS，没用到 window.mml），
// 本脚本会按主题注入一份最小样式表，至少把页面底色与文字色跟启动器对齐，不至于亮色模式下
// 变成一块刺眼的白板。页面自己声明了配色（body 上有背景色 / 有 link[rel=stylesheet] /
// 有非空的 <style>）时不再注入，尊重页面自己的设计。
//
// 消息全部走 postMessage（iframe 是 opaque origin，没有 allow-same-origin，
// 只能 targetOrigin "*"）。消息格式见 TASK.md 与父窗口侧 `lib/customHomeBridge.ts`。
(function () {
  "use strict";

  // 页面自己提供了同名对象时让位（正常情况不会发生：本脚本注入在 head 末尾）
  if (window.mml) return;

  var PROTOCOL = 1;
  var READY_INTERVAL = 100;
  var READY_TRIES = 100;

  var seq = 0;
  /** invoke 请求 id -> { resolve, reject } */
  var pending = new Map();
  /** 订阅 id -> 回调 */
  var handlers = new Map();
  /** 主题回调（页面自己 onTheme 注册的） */
  var themeHandlers = [];

  var readyDone = false;
  var readyTimer = null;
  var readyTries = 0;
  var readyResolve;
  var ready = new Promise(function (resolve) {
    readyResolve = resolve;
  });

  /** 当前主题是不是我们自己按系统偏好兜底出来的（父窗口一旦下发就不再覆盖） */
  var themeFromSystemGuess = true;

  /** 系统深浅色媒体查询（浏览器不支持时为 null） */
  var darkMedia =
    window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)")
      ? window.matchMedia("(prefers-color-scheme: dark)")
      : null;
  var lightMedia =
    window.matchMedia && window.matchMedia("(prefers-color-scheme: light)")
      ? window.matchMedia("(prefers-color-scheme: light)")
      : null;

  // ---------- 主题 ----------

  /** 当前主题，父窗口下发前先用系统偏好兜底 */
  var themeValue = "Dark";

  /** 裸 HTML 兜底样式表：只定义颜色变量，页面自己的 CSS 不用改 */
  var FALLBACK_CSS =
    ":root{color-scheme:dark;--mml-bg:#14161a;--mml-fg:#e8eaed;--mml-dim:#9aa3af}" +
    '[data-mml-theme="Light"]{color-scheme:light;--mml-bg:#f4f6fa;--mml-fg:#1b2029;--mml-dim:#5c6675}' +
    "html,body{background:var(--mml-bg);color:var(--mml-fg)}" +
    "body{margin:0;padding:20px 24px;font-family:'Microsoft YaHei UI','Microsoft YaHei',system-ui,-apple-system,'Segoe UI',sans-serif;font-size:14px;line-height:1.6}" +
    "a{color:#4f8cff}[data-mml-theme=\"Light\"] a{color:#2f6fe4}" +
    "h1,h2,h3{line-height:1.3}";

  /**
   * 页面自己声明了配色吗
   *
   * 判据（保守，宁可判定为"有"而不注入）：body 上有内联背景色 / 有外部样式表 /
   * head 里有非空的 <style>。有就不注入兜底样式，避免盖掉页面设计。
   */
  function pageHasOwnStyling() {
    var doc = document;
    var body = doc.body;
    if (!body) return true;
    if (body.style && body.style.background) return true;
    var bodyInline = body.getAttribute && body.getAttribute("style");
    if (bodyInline && /background/i.test(bodyInline)) return true;
    if (doc.querySelector('link[rel~="stylesheet"]')) return true;
    var styles = doc.querySelectorAll("style");
    for (var i = 0; i < styles.length; i += 1) {
      if (styles[i].textContent && styles[i].textContent.trim().length > 0) return true;
    }
    return false;
  }

  /** 注入兜底样式（只注一次；页面自己带样式时不注） */
  function ensureFallbackStyle() {
    if (!document.body) return;
    if (pageHasOwnStyling()) return;
    if (document.getElementById("__mml_theme")) return;
    var style = document.createElement("style");
    style.id = "__mml_theme";
    style.textContent = FALLBACK_CSS;
    (document.head || document.documentElement).appendChild(style);
  }

  /** 应用主题：写到 <html data-mml-theme> + colorScheme（原生滚动条/表单控件跟着变） */
  function applyTheme(name, fromSystem) {
    var v = name === "Light" ? "Light" : "Dark";
    themeValue = v;
    // 父窗口下发的值优先级高于本地兜底：一旦下发过，以后系统再变也不覆盖
    if (!fromSystem) themeFromSystemGuess = false;
    var root = document.documentElement;
    root.dataset.mmlTheme = v;
    root.style.colorScheme = v === "Light" ? "light" : "dark";
    ensureFallbackStyle();
    if (window.mml) window.mml.theme = v;
    for (var i = 0; i < themeHandlers.length; i += 1) {
      try {
        themeHandlers[i](v);
      } catch (err) {
        console.error("[mml] theme handler error:", err);
      }
    }
  }

  /** 本地兜底：按系统偏好猜一个（只用到父窗口下发之前） */
  function guessSystemTheme() {
    return lightMedia && lightMedia.matches && !(darkMedia && darkMedia.matches) ? "Light" : "Dark";
  }

  applyTheme(guessSystemTheme(), true);

  /**
   * 系统深浅色变化：仅在父窗口还没下发过主题时跟随
   *
   * 两种情况下父窗口不会下发，只能靠这里：
   *   1. 页面不在启动器里（浏览器预览）；
   *   2. 启动器主题设为 System 且系统是在**窗口打开之后**才切换的——
   *      启动器侧 matchMedia 未必随窗口重开而更新，页面自己盯着更准。
   */
  function onSystemThemeChange() {
    if (!themeFromSystemGuess) return;
    applyTheme(guessSystemTheme(), true);
  }

  if (darkMedia && darkMedia.addEventListener) darkMedia.addEventListener("change", onSystemThemeChange);
  if (lightMedia && lightMedia.addEventListener) lightMedia.addEventListener("change", onSystemThemeChange);

  // body 由解析器创建后才好判断"页面自己有没有样式"；此时 head 里的 <style> 已解析完
  if (document.readyState === "loading" && document.addEventListener) {
    document.addEventListener("DOMContentLoaded", function () {
      ensureFallbackStyle();
    });
  } else {
    ensureFallbackStyle();
  }

  function post(msg) {
    msg.__mml = PROTOCOL;
    window.parent.postMessage(msg, "*");
  }

  function stopReadyRetry() {
    if (readyTimer !== null) {
      clearInterval(readyTimer);
      readyTimer = null;
    }
  }

  function finishReady() {
    if (readyDone) return;
    readyDone = true;
    stopReadyRetry();
    readyResolve();
  }

  function sendReady() {
    if (readyDone) return;
    post({ type: "ready" });
    readyTries += 1;
    if (readyTries >= READY_TRIES) {
      // 父窗口一直没应答（页面不是启动器加载的）：停止重试，让 ready 落地，
      // 之后的 invoke 会因无人应答而一直 pending——调用方自己加超时
      finishReady();
    }
  }

  // 握手带重试：桥接脚本可能先于父窗口的 message 监听就绪
  sendReady();
  readyTimer = setInterval(sendReady, READY_INTERVAL);

  window.addEventListener("message", function (e) {
    // 只认父窗口发来的、带协议标记的消息
    if (e.source !== window.parent) return;
    var d = e.data;
    if (!d || d.__mml !== PROTOCOL) return;

    if (d.type === "ready") {
      // 握手应答里带当前主题；老版本父窗口不带，那就继续用系统偏好兜底
      if (d.theme !== undefined) applyTheme(d.theme);
      finishReady();
      return;
    }

    // 主题变化（设置窗口切暗色/亮色、或 System 跟随系统变化时父窗口主动推）
    if (d.type === "theme") {
      applyTheme(d.theme);
      return;
    }

    if (d.type === "event") {
      var cb = handlers.get(d.id);
      if (!cb) return;
      try {
        cb(d.payload);
      } catch (err) {
        console.error("[mml] event handler error:", err);
      }
      return;
    }

    // invoke 应答：{ id, ok: true, data } / { id, ok: false, error }
    if (d.id === undefined) return;
    var p = pending.get(d.id);
    if (!p) return;
    pending.delete(d.id);
    if (d.ok) {
      p.resolve(d.data);
    } else {
      p.reject(new Error(d.error === undefined || d.error === null ? "mml invoke failed" : String(d.error)));
    }
  });

  /**
   * 调用启动器命令
   * @param {string} cmd 命令名（与 Rust 侧命令函数名一致，如 main_get_instances）
   * @param {any} [args] 命令参数对象（键名与 mml-ipc 的类型一致）
   * @returns {Promise<any>}
   */
  function invoke(cmd, args) {
    return ready.then(function () {
      return new Promise(function (resolve, reject) {
        var id = ++seq;
        pending.set(id, { resolve: resolve, reject: reject });
        post({
          id: id,
          type: "invoke",
          cmd: String(cmd),
          args: args === undefined ? null : args,
        });
      });
    });
  }

  /**
   * 订阅启动器事件（只支持父窗口白名单里的事件）
   *
   * 同步返回取消订阅函数；真正的 subscribe 会等握手完成后再发，
   * 所以在 ready 之前就取消订阅也不会漏发消息。
   *
   * @param {string} event 事件名（如 launch-state）
   * @param {(payload: any) => void} cb
   * @returns {() => void}
   */
  function on(event, cb) {
    if (typeof cb !== "function") {
      throw new TypeError("mml.on(event, cb): cb 必须是函数");
    }
    var id = ++seq;
    handlers.set(id, cb);
    ready.then(function () {
      if (handlers.has(id)) {
        post({ id: id, type: "subscribe", event: String(event) });
      }
    });
    return function off() {
      if (!handlers.delete(id)) return;
      if (readyDone) {
        post({ type: "unsubscribe", id: id });
      }
    };
  }

  /**
   * 订阅主题变化
   *
   * 只订阅本页面自己的回调（不上报父窗口）：主题是启动器单向下发的。
   * 注册后**不会**立刻回调一次；当前值直接读 `window.mml.theme`，
   * 或者干脆用 `<html data-mml-theme>` 写纯 CSS。
   *
   * @param {(theme: string) => void} cb 参数是 "Dark" / "Light"
   * @returns {() => void} 取消订阅函数
   */
  function onTheme(cb) {
    if (typeof cb !== "function") {
      throw new TypeError("mml.onTheme(cb): cb 必须是函数");
    }
    themeHandlers.push(cb);
    return function off() {
      var i = themeHandlers.indexOf(cb);
      if (i >= 0) themeHandlers.splice(i, 1);
    };
  }

  window.mml = {
    invoke: invoke,
    on: on,
    onTheme: onTheme,
    theme: themeValue,
    ready: ready,
  };
})();
