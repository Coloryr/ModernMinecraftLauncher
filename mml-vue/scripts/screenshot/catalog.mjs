// 截图场景的名称与中文说明：docs/screenshots.md 与 `--list=md` 共用这一份。
//
// 说明由场景 id 按「窗口前缀 + 词元」拼出来；词元表覆盖当前全部场景，
// 新增场景若用了生词，`node scripts/screenshot/docs.mjs` 会原样输出该词，方便发现漏项。

/** 窗口前缀 → 中文窗口名 */
const WINDOW_LABEL = {
  main: "主窗口",
  settings: "设置",
  resource: "资源管理",
  account: "账户",
  add: "添加实例",
  addmodpack: "下载整合包",
  "add-resource": "添加资源",
  collect: "收藏",
  download: "下载",
  java: "Java 下载",
  block: "方块与物品",
  log: "游戏日志",
  export: "导出实例",
  lobby: "联机大厅",
  config: "配置文件",
  "server-instance": "服务器实例",
  "server-pack": "服务器包",
  help: "帮助",
  stats: "统计",
  control: "控件",
};

/** 场景 id 里的词元 → 中文；表里没有的按原词输出 */
const TOKEN = {
  account: "账户",
  add: "添加",
  advanced: "扩展参数",
  all: "全部",
  archive: "压缩包",
  authlib: "外置登录",
  auto: "自动",
  bar: "工具条",
  block: "方块",
  card: "卡片",
  check: "检查",
  chunk: "区块",
  clear: "清空",
  client: "客户端",
  close: "关闭",
  collect: "收藏",
  config: "配置",
  configs: "配置",
  confirm: "确认",
  conflict: "冲突",
  continue: "继续",
  context: "右键",
  copy: "复制",
  crash: "崩溃",
  ctx: "右键",
  curseforge: "CurseForge",
  custom: "自定义",
  day: "日期",
  delete: "删除",
  dep: "依赖",
  detail: "详情",
  done: "完成",
  download: "下载",
  drop: "下拉",
  edit: "编辑",
  empty: "空状态",
  error: "错误",
  experimental: "实验性功能",
  export: "导出",
  fail: "失败",
  file: "文件",
  files: "文件",
  filled: "已填充",
  folder: "文件夹",
  gamepad: "手柄",
  graph: "图表",
  grid: "网格",
  group: "分组",
  help: "帮助",
  hex: "十六进制",
  hmcl: "HMCL",
  home: "主页",
  image: "图片",
  install: "安装",
  instance: "实例",
  java: "Java",
  launch: "游戏启动",
  list: "列表",
  littleskin: "LittleSkin",
  loaded: "已加载",
  loader: "加载器",
  loading: "加载中",
  local: "本地",
  lock: "锁定",
  log: "日志",
  main: "主窗口",
  manual: "手动",
  mark: "标记",
  mca: "MCA",
  menu: "菜单",
  meta: "元数据",
  microsoft: "微软",
  mmc: "MMC",
  mod: "模组",
  modal: "弹窗",
  modrinth: "Modrinth",
  mods: "模组",
  move: "移动",
  multi: "多选",
  name: "名称",
  nbt: "NBT",
  network: "网络",
  new: "新建",
  note: "备注",
  oauth: "设备码登录",
  online: "在线地址",
  other: "其他",
  pack: "包",
  packs: "资源包",
  paused: "已暂停",
  popup: "弹窗",
  probing: "探测中",
  profile: "档案",
  progress: "进度",
  read: "读取",
  relogin: "重新登录",
  remove: "移除",
  rename: "重命名",
  rendered: "已渲染",
  rendering: "渲染中",
  reset: "恢复默认",
  resource: "资源",
  row: "行",
  saves: "存档",
  scan: "扫描",
  scanning: "扫描中",
  schematics: "投影",
  screenshots: "截图",
  scroll: "滚动",
  search: "搜索",
  server: "服务器",
  servers: "服务器",
  set: "设置",
  settings: "设置",
  shaders: "光影",
  single: "单",
  skin: "皮肤",
  start: "启动",
  stats: "统计",
  stop: "停止",
  table: "表格",
  test: "测试",
  token: "令牌",
  tooltip: "提示",
  tree: "目录树",
  type: "类型",
  ui: "界面",
  unstar: "取消收藏",
  update: "更新",
  upload: "上传",
  version: "版本",
  waiting: "等待中",
  xbox: "Xbox",
  xsts: "XSTS",
};

/** 几个按词拼接读着别扭的固定搭配，拼完再合并 */
const PHRASE = [
  ["右键 · 菜单", "右键菜单"],
  ["分组 · 弹窗", "分组弹窗"],
  ["分组 · 菜单", "分组菜单"],
  ["分组 · 下拉", "分组下拉"],
  ["类型 · 下拉", "类型下拉"],
  ["更新 · 检查", "更新检查"],
  ["更新 · 日志", "更新日志"],
  ["删除 · 分组", "删除分组"],
  ["移除 · 全部", "全部移除"],
  ["多选 · 工具条", "多选工具条"],
  ["移动 · 分组", "移动分组"],
  ["读取 · 错误", "读取失败"],
  ["扫描 · 错误", "扫描失败"],
  ["令牌 · 交换", "令牌交换"],
  ["拉取 · 档案", "拉取档案"],
  ["重命名 · 弹窗", "重命名弹窗"],
  ["复制 · 弹窗", "复制弹窗"],
  ["添加 · 分组弹窗", "添加分组弹窗"],
  ["新建 · 分组", "新建分组"],
  ["设置 · 分组", "设置分组"],
  ["分组 · 重命名", "分组重命名"],
  ["分组 · 移除", "移除分组"],
  ["卡片 · 菜单", "卡片菜单"],
  ["移除 · 弹窗", "移除弹窗"],
  ["清空 · 弹窗", "清空弹窗"],
  ["停止 · 弹窗", "停止弹窗"],
  ["上传 · 弹窗", "上传弹窗"],
  ["导出 · 弹窗", "导出弹窗"],
  ["安装 · 错误", "安装失败"],
];

/** 单独给出的完整说明，优先于词元拼接 */
const OVERRIDE = {
  "main-grid": "主窗口 · 网格视图",
  "main-group": "主窗口 · 分组视图",
  "main-detail": "主窗口 · 实例详情",
  "main-detail-scroll": "主窗口 · 实例详情 · 滚动",
  "main-advanced": "主窗口 · 实例设置 · 扩展参数",
  "main-advanced-scroll": "主窗口 · 实例设置 · 扩展参数 · 滚动",
  "main-move-group": "主窗口 · 转移分组",
  "main-multi-bar": "主窗口 · 多选工具条",
  "main-multi-delete": "主窗口 · 批量删除",
  "settings-client-lock": "设置 · 客户端设置 · 登录方式锁定",
  "settings-local-clear": "设置 · 本地文件 · 清理确认",
  "settings-gamepad-new": "设置 · 手柄 · 新建配置",
  "settings-reset-modal": "设置 · 恢复默认弹窗",
  "settings-java-remove-all": "设置 · Java · 删除全部",
  "settings-client": "设置 · 客户端设置",
  "settings-local": "设置 · 本地文件",
  "settings-network": "设置 · 网络与下载",
  "settings-skin": "设置 · 皮肤与头像",
  "resource-row-menu": "资源管理 · 行右键菜单",
  "resource-update-log": "资源管理 · 更新记录",
  "resource-dep-test": "资源管理 · 依赖检测",
  "resource-dep-graph": "资源管理 · 依赖图",
  "resource-auto-mark": "资源管理 · 自动标记",
  "account-list": "账户 · 列表视图",
  "account-detail": "账户 · 详情视图",
  "account-list-no-match": "账户 · 列表视图 · 无匹配",
  "account-detail-no-match": "账户 · 详情视图 · 无匹配",
  "account-add-invalid": "账户 · 添加 · 未填用户名",
  "account-add-invalid-authlib": "账户 · 添加 · 未填服务器与账密",
  "account-add-ok": "账户 · 添加成功（断言命令）",
  "settings-custom-css": "设置 · 界面 · 自定义样式包",
  "addmodpack-install-done": "下载整合包 · 安装完成",
  "add-resource": "添加资源 · 默认页",
  "add-resource-loaded": "添加资源 · 搜索结果",
  "add-resource-loading": "添加资源 · 加载中",
  "add-resource-error": "添加资源 · 加载失败",
  "add-resource-detail": "添加资源 · 项目详情",
  "add-resource-downloading": "添加资源 · 下载中",
  "add-resource-save": "添加资源 · 存档类型",
  "add-resource-save-pick": "添加资源 · 选择存档",
  "add-resource-instance-pick": "添加资源 · 选择实例",
  "add-resource-deps": "添加资源 · 前置选择",
  "add-resource-deps-skip": "添加资源 · 跳过前置确认",
  "main-launch": "主窗口 · 点击启动（断言命令）",
  "main-create-group": "主窗口 · 新建分组（断言命令）",
  "settings-save": "设置 · 改动保存（断言命令）",
  "java-download-start": "Java 下载 · 点击下载（断言命令）",
  "download-pause": "下载 · 暂停全部（断言命令）",
  "java-download-empty": "Java 下载 · 无匹配",
  "download-failed": "下载 · 有失败任务",
  "stats-empty": "统计 · 无数据",
  "log-empty": "日志 · 无输出",
  "account-oauth": "账户 · 设备码登录",
  "account-oauth-xbox": "账户 · 设备码登录 · Xbox",
  "account-oauth-xsts": "账户 · 设备码登录 · XSTS",
  "account-oauth-token": "账户 · 设备码登录 · 令牌交换",
  "account-oauth-profile": "账户 · 设备码登录 · 拉取档案",
  "account-oauth-fail": "账户 · 设备码登录 · 失败",
  "add-continue-modal": "添加实例 · 继续安装弹窗",
  "add-name-conflict-modal": "添加实例 · 重名弹窗",
  "add-pack-progress-modal": "添加实例 · 整合包进度弹窗",
  "add-archive-read-error": "添加实例 · 压缩包读取失败",
  "add-folder-scan-error": "添加实例 · 文件夹扫描失败",
  "addmodpack-install-popup": "下载整合包 · 安装进度弹窗",
  "collect-group-pick": "收藏 · 选中分组",
  "collect-group-empty": "收藏 · 空分组",
  "collect-group-menu": "收藏 · 分组菜单",
  "collect-drag-group": "收藏 · 拖到分组",
  "collect-install-modal": "收藏 · 安装确认弹窗",
  "download-stop-modal": "下载 · 停止确认弹窗",
  "log-crash-modal": "游戏日志 · 崩溃分析",
  "config-folder-context": "配置文件 · 文件夹右键菜单",
  "stats-single-day": "统计 · 单日",
  "block-skin-add": "方块与物品 · 添加皮肤",
  "block-close-confirm": "方块与物品 · 关闭确认",
  "java-download": "Java 下载 · 列表",
  "java-loading": "Java 下载 · 加载中",
  "export-meta-filled": "导出实例 · 元数据已填",
  "control-buttons": "控件 · 按钮",
  "control-inputs": "控件 · 输入框",
  "control-selects": "控件 · 下拉框",
  "control-select-open": "控件 · 下拉框 · 展开",
  "control-switches": "控件 · 开关",
  "control-segmented": "控件 · 分段切换",
  "control-steppers": "控件 · 数字步进器",
  "control-search": "控件 · 搜索框",
  "control-form": "控件 · 表单行",
  "control-date": "控件 · 日期选择",
  "control-date-open": "控件 · 日期选择 · 展开",
  "control-modal": "控件 · 弹窗",
  "control-collapse": "控件 · 折叠面板",
  "control-tabs": "控件 · 图标页签",
  "control-icons": "控件 · 图标",
};

/** 场景分组：docs 里按这个顺序分节 */
export const GROUPS = [
  { key: "main", title: "主窗口" },
  { key: "settings", title: "设置" },
  { key: "resource", title: "资源管理" },
  { key: "account", title: "账户" },
  { key: "add", title: "添加实例" },
  { key: "addmodpack", title: "下载整合包" },
  { key: "add-resource", title: "添加资源" },
  { key: "collect", title: "收藏" },
  { key: "download", title: "下载" },
  { key: "java", title: "Java 下载" },
  { key: "block", title: "方块与物品" },
  { key: "log", title: "游戏日志" },
  { key: "export", title: "导出实例" },
  { key: "lobby", title: "联机大厅" },
  { key: "config", title: "配置文件" },
  { key: "server-instance", title: "服务器实例" },
  { key: "server-pack", title: "服务器包" },
  { key: "help", title: "帮助" },
  { key: "stats", title: "统计" },
  { key: "control", title: "控件" },
];

/**
 * 取场景所属分组
 *
 * - `id`: 场景 id
 *
 * # 返回值
 *
 * 见 [`GROUPS`] 的 key；未知前缀按第一段返回
 */
export function groupOf(id) {
  if (id === "add-resource") return "add-resource";
  if (id.startsWith("addmodpack")) return "addmodpack";
  if (id.startsWith("add-")) return "add";
  if (id.startsWith("server-instance")) return "server-instance";
  if (id.startsWith("server-pack")) return "server-pack";
  return id.split("-")[0];
}

/** 场景 id 对应的窗口前缀，长的先匹配，避免 `add-resource` 被当成 `add` */
function prefixOf(id) {
  for (const prefix of ["server-instance", "server-pack", "add-resource", "addmodpack"]) {
    if (id === prefix || id.startsWith(prefix + "-")) return prefix;
  }
  if (id.startsWith("add-")) return "add";
  return id.split("-")[0];
}

/**
 * 场景的中文说明
 *
 * - `id`: 场景 id，即图片文件名去掉 `.png`
 *
 * # 返回值
 *
 * 形如 `主窗口 · 实例详情 · 滚动` 的一行说明
 */
export function describe(id) {
  if (OVERRIDE[id]) return OVERRIDE[id];

  const prefix = prefixOf(id);
  const rest = id.slice(prefix.length).replace(/^-/, "");
  const parts = [WINDOW_LABEL[prefix] ?? prefix];
  if (rest) {
    for (const token of rest.split("-")) parts.push(TOKEN[token] ?? token);
  }
  let text = parts.join(" · ");
  for (const [from, to] of PHRASE) text = text.replaceAll(from, to);
  return text;
}

/** 场景视口，形如 `1280×800` */
export function viewportOf(scenario) {
  const [w, h] = scenario.size ?? [1280, 800];
  return w + "×" + h;
}

/**
 * Markdown 表格
 *
 * - `items`: 场景表
 * - `describeFn`: id → 说明，默认用 [`describe`]
 *
 * # 返回值
 *
 * 带表头的多行 Markdown
 */
export function markdownTable(items, describeFn = describe) {
  const lines = [
    "| 图片 | 画面 | 窗口 | 视口 |",
    "| --- | --- | --- | --- |",
  ];
  for (const s of items) {
    lines.push("| `" + s.id + "` | " + describeFn(s.id) + " | `" + s.kind + "` | " + viewportOf(s) + " |");
  }
  return lines.join("\n");
}
