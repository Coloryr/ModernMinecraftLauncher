// 设置窗口 · 设置项搜索索引
//
// 为什么是静态索引而不是扫 DOM：只有当前标签是渲染出来的（v-if），跨 6 个标签搜不到别页的项；
// 而索引里的 `group` 就是 SettingsGroup 的 id，也是 DOM 锚点 `#set-<id>`，命中后跳过去即可。
//
// 维护约定：**这里列的是"能搜到的设置项"**，标签页里加/改设置项时同步这里；
// 键名写错不会报错，只是搜不到，所以加完记得在搜索框里打一遍试试。
import { t } from "../../lib/i18n";
import { SETTINGS_TABS, type SettingsTab } from "./types";

/** 分组 id → 分组标题的 i18n 键（结果里显示「分组 › 项」用） */
export const GROUP_TITLES: Record<string, string> = {
  general: "winSettings.secGeneral",
  theme: "winSettings.secTheme",
  customCss: "winSettings.secCustomCss",
  customPage: "winSettings.secCustomPage",
  window: "winSettings.secWindow",
  mainWindow: "winSettings.secMainWindow",
  bgImage: "winSettings.bgImage",
  head: "winSettings.headDisplay",
  skin: "winSettings.skinDisplay",
  cape: "winSettings.capeDisplay",
  preview: "winSettings.secPreview",
  download: "winSettings.secDownload",
  proxy: "winSettings.secProxy",
  dns: "winSettings.dns",
  gameCheck: "winSettings.gameCheck",
  gameWindow: "winSettings.secGameWindow",
  memory: "winSettings.secMemory",
  jvm: "winSettings.secJvm",
  gameArgs: "winSettings.secGameArgs",
  launchCmd: "winSettings.secLaunchCmd",
  launchCheck: "winSettings.secLaunchCheck",
  javaAdd: "winSettings.javaAdd",
  servers: "winSettings.secServers",
  loginLock: "winSettings.secLoginLock",
  instanceLock: "winSettings.secInstanceLock",
  autoSave: "winSettings.secAutoSave",
  customHome: "winSettings.secCustomHome",
  music: "winSettings.secMusic",
  serverPack: "winSettings.secServerPack",
  clientLock: "winSettings.secClientLock",
  gameTitle: "winSettings.secGameTitle",
  gamepad: "gamepad.title",
  "window-look": "winSettings.secWindowLook",
  localFiles: "winSettings.secLocalFiles",
  experimental: "winSettings.secExperimental",
};

interface IndexEntry {
  tab: SettingsTab;
  group: string;
  /** 设置项标题的 i18n 键 */
  labelKey: string;
  /** 额外匹配词，可选；中英别名都列进来，与界面语言无关 */
  extra?: string[];
}

/** 设置项索引（按标签顺序，同组内按界面顺序） */
const SETTINGS_INDEX: IndexEntry[] = [
  // ---- 界面 ----
  { tab: "ui", group: "general", labelKey: "winSettings.language", extra: ["language", "语言"] },
  { tab: "ui", group: "general", labelKey: "winSettings.font", extra: ["font", "字体"] },
  { tab: "ui", group: "general", labelKey: "winSettings.animations", extra: ["animation", "动画"] },
  { tab: "ui", group: "theme", labelKey: "winSettings.theme", extra: ["theme", "dark", "light", "深色", "浅色"] },
  { tab: "ui", group: "theme", labelKey: "winSettings.accent", extra: ["accent", "color", "强调色", "主题色"] },
  { tab: "ui", group: "customCss", labelKey: "winSettings.customCssOn", extra: ["custom css", "style pack", "样式包", "自定义界面", "css", "样式", "换肤"] },
  { tab: "ui", group: "customCss", labelKey: "winSettings.customCssImport", extra: ["style pack", "css file", "import", "样式包", "选择文件", "导入"] },
  { tab: "ui", group: "customPage", labelKey: "winSettings.customPageOn", extra: ["custom page", "theme pack", "主题包", "html", "完全自定义界面", "整页"] },
  { tab: "ui", group: "customPage", labelKey: "winSettings.customPageImport", extra: ["theme pack", "zip", "import page", "主题包", "导入压缩包"] },
  { tab: "ui", group: "window", labelKey: "winSettings.windowMode", extra: ["window mode", "多窗口", "单窗口"] },
  {
    tab: "ui",
    group: "window-look",
    labelKey: "winSettings.windowEffect",
    extra: ["window effect", "mica", "acrylic", "blur", "材质", "窗口效果"],
  },
  {
    tab: "ui",
    group: "window-look",
    labelKey: "winSettings.windowOpacity",
    extra: ["window opacity", "alpha", "透明度", "窗口透明"],
  },
  { tab: "ui", group: "mainWindow", labelKey: "winSettings.startupPage", extra: ["startup", "home", "启动页", "主页", "项目列表"] },
  {
    tab: "ui",
    group: "mainWindow",
    labelKey: "winSettings.viewMode",
    extra: ["view mode", "group", "grid", "list", "分组", "平铺", "列表", "显示模式"],
  },
  { tab: "ui", group: "mainWindow", labelKey: "winSettings.sidebar", extra: ["sidebar", "侧栏"] },
  { tab: "ui", group: "bgImage", labelKey: "winSettings.bgImage", extra: ["background", "壁纸", "背景"] },
  { tab: "ui", group: "bgImage", labelKey: "winSettings.bgOpacity", extra: ["opacity", "不透明度"] },
  { tab: "ui", group: "bgImage", labelKey: "winSettings.bgBlur", extra: ["blur", "模糊"] },
  { tab: "ui", group: "bgImage", labelKey: "winSettings.bgNativeSize", extra: ["resolution", "分辨率"] },

  // ---- 皮肤与头像 ----
  { tab: "skin", group: "head", labelKey: "winSettings.headDisplay", extra: ["head", "头像"] },
  { tab: "skin", group: "head", labelKey: "winSettings.rotX", extra: ["rotate", "旋转"] },
  { tab: "skin", group: "head", labelKey: "winSettings.rotY", extra: ["rotate", "旋转"] },
  { tab: "skin", group: "skin", labelKey: "winSettings.skinDisplay", extra: ["skin", "皮肤"] },
  {
    tab: "skin",
    group: "skin",
    labelKey: "winSettings.rotX",
    extra: ["skin", "rotate", "皮肤", "旋转"],
  },
  {
    tab: "skin",
    group: "skin",
    labelKey: "winSettings.rotY",
    extra: ["skin", "rotate", "皮肤", "旋转"],
  },
  {
    tab: "skin",
    group: "cape",
    labelKey: "winSettings.capeDisplay",
    extra: ["cape", "elytra", "披风", "鞘翅"],
  },
  {
    tab: "skin",
    group: "cape",
    labelKey: "winSettings.rotX",
    extra: ["cape", "rotate", "披风", "旋转"],
  },
  {
    tab: "skin",
    group: "cape",
    labelKey: "winSettings.rotY",
    extra: ["cape", "rotate", "披风", "旋转"],
  },

  // ---- 网络与下载 ----
  { tab: "network", group: "download", labelKey: "winSettings.downloadSource", extra: ["source", "bmclapi", "下载源"] },
  { tab: "network", group: "download", labelKey: "winSettings.downloadThread", extra: ["thread", "线程", "并发"] },
  { tab: "network", group: "download", labelKey: "winSettings.checkFile", extra: ["sha1", "校验"] },
  { tab: "network", group: "download", labelKey: "winSettings.autoDownload", extra: ["auto download", "自动下载"] },
  { tab: "network", group: "proxy", labelKey: "winSettings.proxyWork", extra: ["proxy", "代理"] },
  { tab: "network", group: "proxy", labelKey: "winSettings.proxyLogin", extra: ["proxy", "代理"] },
  { tab: "network", group: "proxy", labelKey: "winSettings.proxyType", extra: ["socks", "http"] },
  { tab: "network", group: "proxy", labelKey: "winSettings.proxyIp", extra: ["ip", "address"] },
  { tab: "network", group: "proxy", labelKey: "winSettings.proxyPort", extra: ["port", "端口"] },
  { tab: "network", group: "proxy", labelKey: "winSettings.proxyUsername", extra: ["user", "用户名"] },
  { tab: "network", group: "proxy", labelKey: "winSettings.proxyPassword", extra: ["password", "密码"] },
  { tab: "network", group: "dns", labelKey: "winSettings.dnsEnable", extra: ["dns", "doh"] },
  { tab: "network", group: "dns", labelKey: "winSettings.dnsProxy", extra: ["dns", "proxy"] },
  { tab: "network", group: "dns", labelKey: "winSettings.dnsHttps", extra: ["doh", "https"] },
  { tab: "network", group: "gameCheck", labelKey: "winSettings.checkCore", extra: ["core", "核心"] },
  { tab: "network", group: "gameCheck", labelKey: "winSettings.checkLib", extra: ["library", "运行库"] },
  { tab: "network", group: "gameCheck", labelKey: "winSettings.checkAssets", extra: ["assets", "资源"] },
  { tab: "network", group: "gameCheck", labelKey: "winSettings.checkMod", extra: ["mod", "模组"] },
  { tab: "network", group: "gameCheck", labelKey: "winSettings.checkCoreSha1", extra: ["sha1"] },
  { tab: "network", group: "gameCheck", labelKey: "winSettings.checkLibSha1", extra: ["sha1"] },
  { tab: "network", group: "gameCheck", labelKey: "winSettings.checkAssetsSha1", extra: ["sha1"] },
  { tab: "network", group: "gameCheck", labelKey: "winSettings.checkModSha1", extra: ["sha1"] },

  // ---- 游戏启动 ----
  { tab: "launch", group: "gameWindow", labelKey: "winSettings.fullScreen", extra: ["fullscreen", "全屏"] },
  { tab: "launch", group: "gameWindow", labelKey: "winSettings.width", extra: ["width", "宽"] },
  { tab: "launch", group: "gameWindow", labelKey: "winSettings.height", extra: ["height", "高"] },
  { tab: "launch", group: "memory", labelKey: "winSettings.minMemory", extra: ["memory", "内存", "xmx"] },
  { tab: "launch", group: "memory", labelKey: "winSettings.maxMemory", extra: ["memory", "内存", "xmx"] },
  { tab: "launch", group: "jvm", labelKey: "winSettings.gcMode", extra: ["gc", "g1gc", "zgc"] },
  { tab: "launch", group: "jvm", labelKey: "winSettings.colorasm", extra: ["asmtools", "彩色日志"] },
  { tab: "launch", group: "jvm", labelKey: "winSettings.isolateHome", extra: ["home", "isolate", "user profile", "隔离", "用户目录"] },
  { tab: "launch", group: "jvm", labelKey: "winSettings.removeJvmArg", extra: ["jvm args"] },
  { tab: "launch", group: "jvm", labelKey: "winSettings.jvmEnv", extra: ["env", "环境变量"] },
  { tab: "launch", group: "jvm", labelKey: "winSettings.jvmArgs", extra: ["jvm args", "启动参数"] },
  { tab: "launch", group: "gameArgs", labelKey: "winSettings.removeGameArg", extra: ["game args"] },
  { tab: "launch", group: "gameArgs", labelKey: "winSettings.gameArgs", extra: ["game args", "游戏参数"] },
  { tab: "launch", group: "launchCmd", labelKey: "winSettings.preLaunch", extra: ["pre launch", "预启动"] },
  { tab: "launch", group: "launchCmd", labelKey: "winSettings.preSameTime", extra: ["pre launch"] },
  { tab: "launch", group: "launchCmd", labelKey: "winSettings.postLaunch", extra: ["post launch", "后置"] },
  { tab: "launch", group: "launchCmd", labelKey: "winSettings.preCmd", extra: ["command", "命令"] },
  { tab: "launch", group: "launchCmd", labelKey: "winSettings.postCmd", extra: ["command", "命令"] },
  { tab: "launch", group: "launchCheck", labelKey: "winSettings.checkLoader", extra: ["loader", "mod", "加载器", "模组"] },
  { tab: "launch", group: "launchCheck", labelKey: "winSettings.checkMemory", extra: ["memory", "mod", "内存", "模组"] },

  // ---- Java ----
  { tab: "java", group: "javaAdd", labelKey: "winSettings.javaAdd", extra: ["java", "添加"] },
  { tab: "java", group: "javaAdd", labelKey: "winSettings.javaName", extra: ["java name"] },
  { tab: "java", group: "javaAdd", labelKey: "winSettings.javaPath", extra: ["java path", "路径"] },
  { tab: "java", group: "javaAdd", labelKey: "winSettings.javaScan", extra: ["scan", "扫描"] },
  { tab: "java", group: "javaAdd", labelKey: "winSettings.javaScanDir", extra: ["scan folder", "扫描文件夹"] },
  { tab: "java", group: "javaAdd", labelKey: "winSettings.javaImport", extra: ["import", "导入"] },
  { tab: "java", group: "javaAdd", labelKey: "winSettings.javaDownload", extra: ["download", "下载"] },
  { tab: "java", group: "javaAdd", labelKey: "winSettings.javaRemoveAll", extra: ["remove", "删除"] },

  // ---- 客户端设置 ----
  { tab: "client", group: "gamepad", labelKey: "gamepad.title", extra: ["controller", "gamepad", "手柄", "死区", "摇杆"] },
  { tab: "client", group: "servers", labelKey: "winSettings.serverAddress", extra: ["server", "服务器地址"] },
  { tab: "client", group: "servers", labelKey: "winSettings.autoJoin", extra: ["auto join", "自动进服"] },
  { tab: "client", group: "servers", labelKey: "winSettings.motdCard", extra: ["motd", "服务器信息"] },
  { tab: "client", group: "servers", labelKey: "winSettings.motdInterval", extra: ["motd", "interval", "间隔"] },
  { tab: "client", group: "loginLock", labelKey: "winSettings.loginLockOn", extra: ["login lock", "登录方式锁定"] },
  { tab: "client", group: "loginLock", labelKey: "winSettings.loginLockAdd", extra: ["login lock", "添加"] },
  { tab: "client", group: "instanceLock", labelKey: "winSettings.instanceLockOn", extra: ["instance lock", "实例锁定"] },
  {
    tab: "client",
    group: "clientLock",
    labelKey: "winSettings.secClientLock",
    extra: ["lock client", "unlock", "password", "锁定客户端设置", "解锁", "密码"],
  },
  {
    tab: "client",
    group: "autoSave",
    labelKey: "winSettings.autoSaveNotice",
    extra: ["auto save", "save", "自动进入存档", "存档", "提醒"],
  },
  { tab: "client", group: "customHome", labelKey: "winSettings.customHomeOn", extra: ["custom home", "自定义主页面"] },
  { tab: "client", group: "customHome", labelKey: "winSettings.customHomeImport", extra: ["zip", "导入"] },
  { tab: "client", group: "customHome", labelKey: "winSettings.customHomeRemove", extra: ["remove", "删除"] },
  { tab: "client", group: "music", labelKey: "winSettings.musicOn", extra: ["music", "bgm", "自定义音乐"] },
  { tab: "client", group: "music", labelKey: "winSettings.musicSource", extra: ["music", "url", "音乐地址"] },
  { tab: "client", group: "gameTitle", labelKey: "winSettings.editTitle", extra: ["title", "标题"] },
  { tab: "client", group: "gameTitle", labelKey: "winSettings.randomTitle", extra: ["title", "随机标题"] },
  { tab: "client", group: "gameTitle", labelKey: "winSettings.cycleTitle", extra: ["title", "循环标题"] },
  { tab: "client", group: "gameTitle", labelKey: "winSettings.gameTitle", extra: ["title", "标题文字"] },
  { tab: "client", group: "gameTitle", labelKey: "winSettings.titleDelay", extra: ["title", "间隔"] },
  { tab: "client", group: "serverPack", labelKey: "winSettings.serverPackExport", extra: ["server pack", "serverpack", "服务器包", "导出", "分发"] },

  // ---- 本地文件 ----
  { tab: "files", group: "localFiles", labelKey: "winSettings.localFile.cache", extra: ["cache", "cleanup", "disk", "缓存", "清理", "占用"] },
  { tab: "files", group: "localFiles", labelKey: "winSettings.localFile.image", extra: ["image cache", "icon cache", "图片缓存", "图标缓存"] },
  { tab: "files", group: "localFiles", labelKey: "winSettings.localFile.download", extra: ["download", "下载"] },
];

export interface SettingsHit {
  tab: SettingsTab;
  group: string;
  /** 标签页显示名 */
  tabLabel: string;
  /** 分组显示名 */
  groupLabel: string;
  /** 设置项显示名 */
  label: string;
}

/**
 * 有"默认值"可恢复的分组 id
 *
 * 判断依据是默认值有没有真源：
 * - 前端持有的外观 / 皮肤 / 客户端设置取 `guiConfig.defaultConfig`；
 * - 核心配置（网络 / 启动 / 游戏窗口）取后端 `settings_get_defaults`；
 * - `javaAdd` 不在内：Java 列表是系统扫描出来的，没有"默认值"可言。
 */
export const RESETTABLE_GROUPS = [
  "general",
  "theme",
  "window",
  "window-look",
  "mainWindow",
  "bgImage",
  "head",
  "skin",
  "cape",
  "download",
  "proxy",
  "dns",
  "gameCheck",
  "gameWindow",
  "memory",
  "jvm",
  "gameArgs",
  "launchCmd",
  "launchCheck",
  "gameTitle",
  "servers",
  "loginLock",
  "instanceLock",
  "autoSave",
  "customHome",
  "customCss",
  "customPage",
  "music",
];

/**
 * 某个标签页包含哪些分组，按索引顺序去重
 *
 * 从搜索索引派生，保证能搜到的分组与会被重置的分组始终一致。
 *
 * - `tab`: 目标标签页
 *
 * # 返回值
 *
 * 该页含有的分组 id，供内容区右上角「恢复本页默认」派发
 */
export function groupsOfTab(tab: SettingsTab): string[] {
  const groups: string[] = [];
  for (const e of SETTINGS_INDEX) {
    if (e.tab === tab && !groups.includes(e.group)) groups.push(e.group);
  }
  return groups;
}

/**
 * 按关键词搜索设置项
 *
 * - `query`: 用户输入的关键词，空串直接返回空结果
 * - `limit`: 最多返回多少条，缺省 40
 *
 * # 返回值
 *
 * 命中项，含标签页 / 分组 / 设置项的显示名；高亮拼接由调用方做
 */
export function searchSettings(query: string, limit = 40): SettingsHit[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];

  const tabLabelOf = (tab: SettingsTab) => {
    const item = SETTINGS_TABS.find((x) => x.id === tab);
    return item ? t(`winSettings.tab.${item.id}`) : tab;
  };

  const hits: SettingsHit[] = [];
  for (const e of SETTINGS_INDEX) {
    const groupTitleKey = GROUP_TITLES[e.group];
    const groupLabel = groupTitleKey ? t(groupTitleKey) : e.group;
    const label = t(e.labelKey);
    const haystack = [label, groupLabel, tabLabelOf(e.tab), e.labelKey, ...(e.extra ?? [])]
      .join(" ")
      .toLowerCase();
    if (!haystack.includes(q)) continue;
    hits.push({
      tab: e.tab,
      group: e.group,
      tabLabel: tabLabelOf(e.tab),
      groupLabel,
      label,
    });
    if (hits.length >= limit) break;
  }
  return hits;
}
