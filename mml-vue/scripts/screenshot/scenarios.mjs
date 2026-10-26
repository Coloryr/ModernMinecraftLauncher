// 截图场景表：一个场景 = 一个窗口 + 一份数据 + 可选的操作序列。
//
// - kind / label: 决定挂哪个窗口（label 是 Tauri 窗口标签，见 windowManager 的 resolveKind）
// - size: 视口尺寸；不同窗口按实际最小尺寸给
// - query: 附加到 URL 的参数（窗口族要 uuid）
// - commands: 覆盖 data.mjs 的公共命令
// - actions: 字符串按 JS 执行；{ wait } 等待；{ eval } 执行后等一会
import {
  collectWithData, commonCommands, downloadStatus, groups, instances, javaDownloadList,
  javaList, resourceView, saves, schematics, servers, shaders,
} from "./data.mjs";

function base(extra = {}, view = {}) {
  const cfg = commonCommands().window_get_gui_config;
  return commonCommands({
    main_get_instances: instances,
    main_get_groups: groups,
    settings_scan_java: javaList,
    ...extra,
    window_get_gui_config: {
      ...cfg,
      mainWindow: { ...cfg.mainWindow, ...view },
      ...(extra.window_get_gui_config || {}),
    },
  });
}

const withView = (category, extra = {}, modView = "list") =>
  base({
    resource_view_get: { ...resourceView, category, modView },
    ...extra,
  });

const click = (selector) =>
  "(() => { const el = document.querySelector(" + JSON.stringify(selector) + "); if (el) { el.click(); return true; } return false; })()";

const clickTab = (text) =>
  "(() => { const b = [...document.querySelectorAll('.tab-rail .tab-item')].find((x) => x.textContent.includes(" +
  JSON.stringify(text) +
  ")); if (b) { b.click(); return true; } return false; })()";

/** 在指定容器里按可见文字点按钮；先精确匹配，再退回包含匹配 */
const clickText = (text, scope = "body") =>
  "(() => { const root = document.querySelector(" + JSON.stringify(scope) + "); if (!root) return false;" +
  " const els = [...root.querySelectorAll('button, [role=\"button\"], .tab-item, .seg-item')];" +
  " const el = els.find((x) => x.textContent.trim() === " + JSON.stringify(text) + ") ||" +
  " els.find((x) => x.textContent.includes(" + JSON.stringify(text) + "));" +
  " if (!el) return false; el.click(); return true; })()";

/** 给下拉框设值并触发 change，绕开原生弹层 */
const selectValue = (selector, value) =>
  "(() => { const el = document.querySelector(" + JSON.stringify(selector) + "); if (!el) return false;" +
  " el.value = " + JSON.stringify(value) + "; el.dispatchEvent(new Event('change', { bubbles: true })); return true; })()";

export const scenarios = [
  /* ---------------- 主窗口 ---------------- */
  { id: "main-grid", kind: "main", label: "mml-main", size: [1280, 800], commands: base({}, { viewMode: "grid", startupPage: "List" }) },
  { id: "main-group", kind: "main", label: "mml-main", size: [1280, 800], commands: base({}, { viewMode: "group", startupPage: "List" }) },
  { id: "main-detail", kind: "main", label: "mml-main", size: [1280, 800], commands: base({}, { viewMode: "list", startupPage: "List" }) },
  { id: "main-home", kind: "main", label: "mml-main", size: [1280, 800], commands: base({}, { viewMode: "grid", startupPage: "Home" }) },
  { id: "main-empty", kind: "main", label: "mml-main", size: [1280, 800], commands: base({ main_get_instances: [], main_get_groups: [] }, { viewMode: "grid" }) },
  {
    id: "main-ctx-menu",
    kind: "main",
    label: "mml-main",
    size: [1280, 800],
    commands: base({}, { viewMode: "group", startupPage: "List" }),
    actions: [{ eval: "(() => { const row = [...document.querySelectorAll('.inst-row')].find((x) => x.textContent.includes('纯净 1.20.4')) || document.querySelector('.inst-row'); if (!row) return false; row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 320, clientY: 520, button: 2 })); return true; })()", after: 400 }],
  },
  {
    id: "main-rename-modal",
    kind: "main",
    label: "mml-main",
    size: [1280, 800],
    commands: base({}, { viewMode: "group", startupPage: "List" }),
    actions: [
      { eval: "(() => { const row = [...document.querySelectorAll('.inst-row')].find((x) => x.textContent.includes('纯净 1.20.4')) || document.querySelector('.inst-row'); if (!row) return false; row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 320, clientY: 520, button: 2 })); return true; })()", after: 350 },
      { eval: clickText("重命名", ".ctx-menu"), after: 450 },
    ],
  },
  {
    id: "main-copy-modal",
    kind: "main",
    label: "mml-main",
    size: [1280, 800],
    commands: base({}, { viewMode: "group", startupPage: "List" }),
    actions: [
      { eval: "(() => { const row = [...document.querySelectorAll('.inst-row')].find((x) => x.textContent.includes('纯净 1.20.4')) || document.querySelector('.inst-row'); if (!row) return false; row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 320, clientY: 520, button: 2 })); return true; })()", after: 350 },
      { eval: clickText("复制实例", ".ctx-menu"), after: 450 },
    ],
  },
  {
    id: "main-add-group-modal",
    kind: "main",
    label: "mml-main",
    size: [1280, 800],
    commands: base({}, { viewMode: "group" }),
    actions: [{ eval: click(".sidebar-slot .add-group-row"), after: 450 }],
  },

  /* ---------------- 设置 ---------------- */
  { id: "settings-ui", kind: "settings", label: "mml-settings", size: [1100, 760], commands: base(), actions: [{ eval: clickTab("界面"), after: 400 }] },
  { id: "settings-java", kind: "settings", label: "mml-settings", size: [1100, 860], commands: base(), actions: [{ eval: clickTab("Java"), after: 500 }] },
  { id: "settings-network", kind: "settings", label: "mml-settings", size: [1100, 780], commands: base(), actions: [{ eval: clickTab("网络与下载"), after: 500 }] },
  { id: "settings-launch", kind: "settings", label: "mml-settings", size: [1100, 780], commands: base(), actions: [{ eval: clickTab("游戏启动"), after: 500 }] },
  { id: "settings-client", kind: "settings", label: "mml-settings", size: [1100, 780], commands: base(), actions: [{ eval: clickTab("客户端设置"), after: 500 }] },
  { id: "settings-gamepad", kind: "settings", label: "mml-settings", size: [1100, 860], commands: base(), actions: [{ eval: clickTab("手柄"), after: 600 }] },
  { id: "settings-skin", kind: "settings", label: "mml-settings", size: [1100, 800], commands: base(), actions: [{ eval: clickTab("皮肤与头像"), after: 600 }] },
  { id: "settings-local", kind: "settings", label: "mml-settings", size: [1100, 760], commands: base(), actions: [{ eval: clickTab("本地文件"), after: 500 }] },
  { id: "settings-experimental", kind: "settings", label: "mml-settings", size: [1100, 760], commands: base(), actions: [{ eval: clickTab("实验性功能"), after: 500 }] },
  {
    id: "settings-reset-modal",
    kind: "settings",
    label: "mml-settings",
    size: [1100, 760],
    commands: base(),
    actions: [{ eval: clickTab("界面"), after: 400 }, { eval: clickText("恢复本页默认"), after: 450 }],
  },
  {
    id: "settings-java-remove-all",
    kind: "settings",
    label: "mml-settings",
    size: [1100, 860],
    commands: base(),
    actions: [{ eval: clickTab("Java"), after: 500 }, { eval: clickText("删除全部"), after: 450 }],
  },
  {
    id: "settings-client-lock",
    kind: "settings",
    label: "mml-settings",
    size: [1100, 780],
    commands: base(),
    actions: [{ eval: clickTab("客户端设置"), after: 500 }, { eval: clickText("设置密码并锁定"), after: 450 }],
  },
  {
    id: "settings-local-clear",
    kind: "settings",
    label: "mml-settings",
    size: [1100, 760],
    commands: base(),
    actions: [{ eval: clickTab("本地文件"), after: 700 }, { eval: clickText("清理", ".local-files-list"), after: 450 }],
  },
  {
    id: "settings-gamepad-new",
    kind: "settings",
    label: "mml-settings",
    size: [1100, 860],
    commands: base(),
    actions: [{ eval: clickTab("手柄"), after: 600 }, { eval: clickText("新建", ".gp-side"), after: 450 }],
  },

  /* ---------------- 资源管理 ---------------- */
  { id: "resource-mods", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("mods") },
  { id: "resource-mods-table", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("mods", {}, "table") },
  { id: "resource-packs", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("resourcepacks") },
  { id: "resource-saves", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("saves", { resource_list_saves: saves }) },
  { id: "resource-shaders", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("shaders", { resource_list_shaderpacks: shaders }) },
  { id: "resource-servers", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("servers", { resource_list_servers: servers }) },
  { id: "resource-schematics", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("schematics", { resource_list_schematics: schematics }) },
  { id: "resource-screenshots", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("screenshots") },
  { id: "resource-empty", kind: "resource", label: "mml-resource-inst-1", size: [1280, 800], query: "uuid=inst-1", commands: withView("mods", { resource_list_mods: [] }) },
  {
    id: "resource-group-modal",
    kind: "resource",
    label: "mml-resource-inst-1",
    size: [1280, 800],
    query: "uuid=inst-1",
    commands: withView("mods"),
    actions: [{ eval: click(".item-list .add-row-btn"), after: 450 }],
  },
  {
    id: "resource-mod-note",
    kind: "resource",
    label: "mml-resource-inst-1",
    size: [1280, 800],
    query: "uuid=inst-1",
    commands: withView("mods"),
    actions: [
      {
        eval: "(() => { const row = document.querySelector('.mod-rows > *'); if (!row) return false; row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 620, clientY: 320, button: 2 })); return true; })()",
        after: 400,
      },
      { eval: clickText("备注"), after: 450 },
    ],
  },
  {
    id: "resource-auto-mark",
    kind: "resource",
    label: "mml-resource-inst-1",
    size: [1280, 800],
    query: "uuid=inst-1",
    commands: withView("mods"),
    actions: [{ eval: click('[aria-label="自动标记"]'), after: 450 }],
  },
  {
    id: "resource-server-add",
    kind: "resource",
    label: "mml-resource-inst-1",
    size: [1280, 800],
    query: "uuid=inst-1",
    commands: withView("servers"),
    actions: [{ eval: clickText("添加服务器", ".resource-layout"), after: 500 }],
  },
  {
    id: "resource-update-log",
    kind: "resource",
    label: "mml-resource-inst-1",
    size: [1280, 800],
    query: "uuid=inst-1",
    commands: withView("mods"),
    actions: [{ eval: click('[aria-label="更新记录"]'), after: 500 }],
  },
  {
    id: "resource-export-modal",
    kind: "resource",
    label: "mml-resource-inst-1",
    size: [1280, 800],
    query: "uuid=inst-1",
    commands: withView("mods"),
    actions: [{ eval: click('[aria-label="导出模组列表"]'), after: 450 }],
  },

  /* ---------------- 账户 ---------------- */
  { id: "account-loaded", kind: "account", label: "mml-account", size: [1000, 700], commands: base() },
  { id: "account-empty", kind: "account", label: "mml-account", size: [1000, 700], commands: base({ account_get_accounts: { accounts: [], currentUuid: null, locked: false } }) },
  { id: "account-list", kind: "account", label: "mml-account", size: [1000, 700], commands: base(), actions: [{ eval: clickText("列表", ".head-tabs"), after: 450 }] },
  { id: "account-detail", kind: "account", label: "mml-account", size: [1000, 700], commands: base(), actions: [{ eval: clickText("详情", ".head-tabs"), after: 450 }] },
  { id: "account-add", kind: "account", label: "mml-account", size: [1000, 700], commands: base(), actions: [{ eval: click(".acc-card.add-card"), after: 450 }] },
  {
    id: "account-add-microsoft",
    kind: "account",
    label: "mml-account",
    size: [1000, 700],
    commands: base(),
    actions: [
      { eval: click(".acc-card.add-card"), after: 300 },
      { eval: selectValue(".modal select", "microsoft"), after: 350 },
    ],
  },
  {
    id: "account-add-littleskin",
    kind: "account",
    label: "mml-account",
    size: [1000, 700],
    commands: base(),
    actions: [
      { eval: click(".acc-card.add-card"), after: 300 },
      { eval: selectValue(".modal select", "littleskin"), after: 350 },
    ],
  },
  {
    id: "account-add-authlib",
    kind: "account",
    label: "mml-account",
    size: [1000, 700],
    commands: base(),
    actions: [
      { eval: click(".acc-card.add-card"), after: 300 },
      { eval: selectValue(".modal select", "authlib"), after: 350 },
    ],
  },
  {
    id: "account-edit",
    kind: "account",
    label: "mml-account",
    size: [1000, 700],
    commands: base(),
    actions: [{
      eval: "(() => { const c = [...document.querySelectorAll('.acc-card')].find((x) => x.textContent.includes('Alex')); const b = c?.querySelector('.acc-actions button'); if (b) { b.click(); return true; } return false; })()",
      after: 450,
    }],
  },
  {
    id: "account-delete",
    kind: "account",
    label: "mml-account",
    size: [1000, 700],
    commands: base(),
    actions: [{
      eval: "(() => { const c = [...document.querySelectorAll('.acc-card')].find((x) => x.textContent.includes('Alex')); const bs = c?.querySelectorAll('.acc-actions button'); if (bs?.length) { bs[bs.length - 1].click(); return true; } return false; })()",
      after: 450,
    }],
  },
  {
    id: "account-relogin",
    kind: "account",
    label: "mml-account",
    size: [1000, 700],
    commands: base(),
    actions: [{
      eval: "(() => { const c = [...document.querySelectorAll('.acc-card')].find((x) => x.textContent.includes('LittleSkinUser')); const bs = c?.querySelectorAll('.acc-actions button'); if (bs?.length) { bs[bs.length - 2].click(); return true; } return false; })()",
      after: 450,
    }],
  },
  {
    id: "account-oauth",
    kind: "account",
    label: "mml-account",
    size: [1000, 700],
    commands: base(),
    actions: [{
      eval: "window.__SHOT__.emit('account-oauth', { code: 'K7Q2-9XPM', url: 'https://www.microsoft.com/link' })",
      after: 600,
    }],
  },
  {
    id: "account-skin",
    kind: "account",
    label: "mml-account",
    size: [1000, 720],
    commands: base(),
    actions: [{
      eval: "(() => { const c = [...document.querySelectorAll('.acc-card')].find((x) => x.textContent.includes('Steve')); const b = c?.querySelector('.acc-actions button'); if (b) { b.click(); return true; } return false; })()",
      after: 900,
    }],
  },

  /* ---------------- 添加实例 / 下载资源 ---------------- */
  { id: "add-new", kind: "add", label: "mml-add", size: [1100, 760], commands: base() },
  { id: "add-archive", kind: "add", label: "mml-add", size: [1100, 760], commands: base(), actions: [{ eval: clickText("导入压缩包"), after: 450 }] },
  { id: "add-folder", kind: "add", label: "mml-add", size: [1100, 760], commands: base(), actions: [{ eval: clickText("添加文件夹"), after: 450 }] },
  { id: "add-online", kind: "add", label: "mml-add", size: [1100, 760], commands: base(), actions: [{ eval: clickText("在线实例"), after: 450 }] },
  { id: "add-modpack", kind: "add_modpack", label: "mml-add_modpack", size: [1150, 800], commands: base() },
  { id: "add-resource", kind: "add_resource", label: "mml-add_resource-inst-1", size: [1200, 800], query: "uuid=inst-1", commands: base() },

  /* ---------------- 收藏 ---------------- */
  { id: "collect-loaded", kind: "collect", label: "mml-collect", size: [1100, 760], commands: base({ collect_get_data: collectWithData }) },
  {
    id: "collect-add-group-modal",
    kind: "collect",
    label: "mml-collect",
    size: [1100, 760],
    commands: base({ collect_get_data: collectWithData }),
    actions: [{ eval: clickText("添加分组", ".collect-body"), after: 450 }],
  },
  {
    id: "collect-install-modal",
    kind: "collect",
    label: "mml-collect",
    size: [1100, 760],
    commands: base({ collect_get_data: collectWithData }),
    actions: [
      { eval: clickText("多选"), after: 300 },
      { eval: click(".card-check"), after: 300 },
      { eval: clickText("安装", ".multi-bar"), after: 500 },
    ],
  },
  { id: "collect-empty", kind: "collect", label: "mml-collect", size: [1100, 760], commands: base({ collect_get_data: { items: [], groups: {} } }) },

  /* ---------------- 下载 / Java ---------------- */
  { id: "download-loaded", kind: "download", label: "mml-download", size: [1000, 700], commands: base({ download_get_status: downloadStatus }) },
  { id: "download-empty", kind: "download", label: "mml-download", size: [1000, 700], commands: base({ download_get_status: { tasks: [], threads: [], speed: 0, paused: false } }) },
  {
    id: "download-stop-modal",
    kind: "download",
    label: "mml-download",
    size: [1000, 700],
    commands: base({ download_get_status: downloadStatus }),
    actions: [{ eval: clickText("停止"), after: 450 }],
  },
  { id: "java-download", kind: "java_download", label: "mml-java_download", size: [1000, 720], commands: base({ java_download_get_list: javaDownloadList }) },

  /* ---------------- 其它窗口 ---------------- */
  { id: "block-empty", kind: "block", label: "mml-block", size: [1150, 800], commands: base() },
  { id: "log", kind: "log", label: "mml-log-inst-1", size: [1100, 720], query: "uuid=inst-1", commands: base() },
  {
    id: "log-upload-modal",
    kind: "log",
    label: "mml-log-inst-1",
    size: [1100, 720],
    query: "uuid=inst-1",
    commands: base(),
    actions: [{ eval: click('[aria-label="上传日志"]'), after: 450 }],
  },
  { id: "export", kind: "export", label: "mml-export", size: [1100, 760], query: "uuid=inst-1", commands: base() },
  {
    id: "export-progress",
    kind: "export",
    label: "mml-export",
    size: [1100, 760],
    query: "uuid=inst-1",
    commands: base(),
    actions: [{
      eval: "window.__SHOT__.emit('export-progress', { state: 'running', now: 42, total: 100, text: 'mods/sodium-0.5.11.jar', error: null, file: 'mods/sodium-0.5.11.jar' })",
      after: 600,
    }],
  },
  { id: "lobby", kind: "lobby", label: "mml-lobby", size: [1050, 720], commands: base() },
  { id: "config", kind: "config", label: "mml-config-inst-1", size: [1240, 820], query: "uuid=inst-1", commands: base() },
  {
    id: "config-file",
    kind: "config",
    label: "mml-config-inst-1",
    size: [1240, 820],
    query: "uuid=inst-1",
    commands: base(),
    actions: [{ eval: clickText("options.txt"), after: 700 }],
  },
  { id: "server-instance", kind: "server_instance", label: "mml-server_instance", size: [1050, 760], query: "uuid=inst-1", commands: base() },
  { id: "server-pack", kind: "server_pack", label: "mml-server_pack", size: [1100, 760], query: "uuid=inst-1", commands: base() },
  { id: "help", kind: "help", label: "mml-help", size: [1050, 760], commands: base() },
  { id: "stats", kind: "stats", label: "mml-stats", size: [1150, 800], commands: base() },
];
