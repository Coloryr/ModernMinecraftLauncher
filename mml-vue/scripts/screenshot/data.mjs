// 截图用的假数据：只覆盖界面真正会读到的字段。
//
// 新增窗口 / 新状态时优先改这里，scenarios.mjs 只负责"用哪份数据 + 怎么点"。

/** 简单的 SVG 占位图，转成 data URI；避免截图时去请求外网图片 */
export function svgImage(color, text = "") {
  const label = text.replace(/[<>&]/g, "");
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128">' +
    '<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">' +
    '<stop offset="0" stop-color="' + color + '"/><stop offset="1" stop-color="#ffffff"/>' +
    "</linearGradient></defs>" +
    '<rect width="128" height="128" rx="18" fill="url(#g)"/>' +
    (label ? '<text x="64" y="78" font-size="44" text-anchor="middle" fill="#ffffff" font-family="sans-serif">' + label + "</text>" : "") +
    "</svg>";
  return "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
}

/** 主题 / 窗口 / 客户端设置：与 Rust GuiConfigDto 同形 */
export const baseConfig = {
  theme: "Light",
  accent: "Default",
  customAccent: "",
  locale: "zh_cn",
  windowMode: "Multi",
  mainWindow: {
    sidebarSide: "Left",
    sidebarCollapsed: false,
    viewMode: "list",
    startupPage: "List",
    selectedInstance: "inst-1",
  },
  head: { headType: "Head2DB", x: 0, y: 0 },
  collect: { modpack: true, showMod: true, resourcePack: true, shaderpack: true },
  client: {
    motdCard: false,
    motdInterval: 0,
    loginLockOn: false,
    loginLock: [],
    autoJoin: false,
    accountCheck: false,
    autoSaveNotice: false,
    autoJoinServer: "",
    motdServer: "",
    lockInstance: "",
    lockClient: false,
    customHome: false,
    customCssOn: false,
    customCss: [],
    customPageOn: false,
    musicOn: false,
    musicSource: "",
    windowEffect: "none",
    windowOpacity: 100,
  },
  experimental: { launcherAdmin: false, instanceAdmin: false, uriLaunch: true, sandboxLaunch: false },
  launchCheck: { checkLoader: true, checkMemory: true },
  font: "",
  skin: { display: "Skin2DB", pitch: 15, yaw: 45 },
  cape: { display: "Cape2D", pitch: 0, yaw: 30 },
  bgSource: "",
  bgOpacity: 100,
  bgBlur: 0,
  bgNativeSize: 0,
};

export const groups = [
  { uuid: "group-1", name: "生存" },
  { uuid: "group-2", name: "模组测试" },
];

const instance = (uuid, name, version, loader, loaderVersion, group, extra = {}) => ({
  uuid,
  name,
  group,
  version,
  versionType: "release",
  quickPlay: false,
  autoSave: null,
  loader,
  loaderVersion,
  dir: "C:\\Users\\shot\\instances\\" + name,
  running: false,
  modpackType: null,
  pid: null,
  fid: null,
  serverUrl: null,
  lang: "zh_cn",
  logEncoding: "utf8",
  source: null,
  order: 0,
  busy: false,
  ...extra,
});

export const instances = [
  instance("inst-1", "1.21.4 NeoForge", "1.21.4", "neoforge", "21.4.100", "group-1"),
  instance("inst-2", "1.20.1 Forge", "1.20.1", "forge", "47.3.0", "group-1", { running: true }),
  instance("inst-3", "Fabric 1.21.1", "1.21.1", "fabric", "0.16.9", "group-2"),
  instance("inst-4", "纯净 1.20.4", "1.20.4", "normal", null, null),
  instance("inst-5", "整合包 · Better MC", "1.20.1", "forge", "47.2.0", "group-2", {
    modpackType: "curseforge",
    pid: "better-mc",
    fid: "1234567",
    serverUrl: "https://example.com/server.json",
  }),
];

export const addVersions = [
  { id: "1.21.4", versionType: "release" },
  { id: "1.21.1", versionType: "release" },
  { id: "1.20.1", versionType: "release" },
  { id: "1.20.4", versionType: "release" },
  { id: "1.19.2", versionType: "release" },
  { id: "24w14a", versionType: "snapshot" },
];

export const accounts = {
  accounts: [
    {
      userName: "Steve",
      uuid: "acc-1",
      authType: "microsoft",
      loginTime: "2026-10-25 19:12:00",
      avatarColor: "#4f8cff",
      skin: "",
      tokenStatus: "ok",
      avatar: svgImage("#4f8cff", "S"),
      server: null,
      ext1: null,
      ext2: null,
      canRefresh: true,
      canRelogin: true,
      canEdit: false,
    },
    {
      userName: "Alex",
      uuid: "acc-2",
      authType: "offline",
      loginTime: "2026-10-20 10:00:00",
      avatarColor: "#34d399",
      skin: "",
      tokenStatus: "ok",
      avatar: null,
      server: null,
      ext1: null,
      ext2: null,
      canRefresh: false,
      canRelogin: true,
      canEdit: true,
    },
    {
      userName: "LittleSkinUser",
      uuid: "acc-3",
      authType: "littleskin",
      loginTime: "2026-09-30 08:30:00",
      avatarColor: "#fb923c",
      skin: "",
      tokenStatus: "expired",
      avatar: null,
      server: "https://littleskin.cn/api/yggdrasil",
      ext1: null,
      ext2: null,
      canRefresh: true,
      canRelogin: true,
      canEdit: true,
    },
  ],
  currentUuid: "acc-1",
  locked: false,
};

export const javaList = [
  { name: "Java 17", path: "C:\\Program Files\\Java\\jdk-17\\bin\\java.exe", version: "17.0.12", major: 17, javaType: "JDK", arch: "x86_64" },
  { name: "Java 21", path: "C:\\Program Files\\Java\\jdk-21\\bin\\java.exe", version: "21.0.4", major: 21, javaType: "JDK", arch: "x86_64" },
  { name: "Java 8", path: "C:\\Program Files\\Java\\jre-8\\bin\\java.exe", version: "1.8.0_421", major: 8, javaType: "JRE", arch: "x86_64" },
];

export const news = [
  {
    id: 1,
    title: "M²L 0.0.1 预发布：全新的实例管理与资源下载",
    date: "2026-10-25",
    tag: "更新",
    image: svgImage("#4f8cff", "M"),
    url: "https://example.com/news/1",
  },
  {
    id: 2,
    title: "CurseForge 整合包安装速度优化",
    date: "2026-10-18",
    tag: "公告",
    image: svgImage("#a78bfa", "C"),
    url: "https://example.com/news/2",
  },
  {
    id: 3,
    title: "联机房间与映射大厅测试中",
    date: "2026-10-10",
    tag: "联机",
    image: svgImage("#34d399", "L"),
    url: "https://example.com/news/3",
  },
];

export const instanceArgs = {
  memory: 4096,
  minMemory: 1024,
  fullscreen: false,
  width: 1280,
  height: 720,
  javaName: "",
  javaPath: "",
  gc: "auto",
  gcCustom: "",
  mainClass: "",
  jvmArgs: [],
  gameArgs: [],
  classPath: [],
  envVars: [],
  isolateHome: true,
  lang: "zh_cn",
  logEncoding: "utf8",
  preEnabled: false,
  preCmd: "",
  postEnabled: false,
  postCmd: "",
  proxyIp: "",
  proxyPort: 0,
  proxyUser: "",
  proxyPass: "",
  serverIp: "",
  serverPort: 0,
  joinServer: false,
};

export const customUiStatus = {
  cssOn: false,
  css: [],
  pageOn: false,
  pageInstalled: false,
  kind: "page",
  name: "",
  version: "",
  declared: false,
  pageEntryUrl: "",
  pageScriptUrl: "",
  pageStyleUrl: "",
  pageVersion: "",
  pageFileCount: 0,
};

export const customHomeStatus = {
  installed: false,
  enabled: false,
  kind: "home",
  name: "",
  version: "",
  declared: false,
  entryUrl: "",
  scriptUrl: "",
  styleUrl: "",
  contentVersion: "",
  fileCount: 0,
};

export const buildInfo = { version: "0.0.1", channel: "prerelease" };

/** 每个窗口启动时都会用到的公共命令：各场景都带上 */

/* ---------- 其余窗口的公共假数据 ---------- */

export const settingsLaunch = {
  removeJvmArg: false,
  removeGameArg: false,
  jvmArgs: "-XX:+UseG1GC",
  gameArgs: "",
  jvmEnv: "",
  isolateHome: true,
  gcMode: "auto",
  minMemory: 1024,
  maxMemory: 4096,
  colorasm: false,
  launchPreRun: false,
  preRunWithGame: true,
  launchPostRun: false,
  preRunArg: "",
  postRunArg: "",
};

export const settingsWindow = {
  fullScreen: false,
  width: 1280,
  height: 720,
  editTitle: false,
  gameTitle: "",
  randomTitle: false,
  cycleTitle: false,
  titleDelay: 3,
};

export const settingsNetwork = {
  source: "default",
  downloadThread: 64,
  proxyIp: "",
  proxyPort: 0,
  proxyUser: "",
  proxyPassword: "",
  workProxy: "",
  workProxyType: "http",
  loginProxy: "",
  loginProxyType: "http",
  checkFile: true,
  autoDownload: true,
  dns: { enable: false, https: ["https://1.1.1.1/dns-query"], httpProxy: false },
  check: { core: true, lib: true, assets: true, gameMod: true, coreSha1: false, libSha1: false, assetsSha1: false, modSha1: false },
};

const mod = (uuid, file, modId, name, version, extra = {}) => ({
  uuid,
  sha1: "534fda8d43b95f5d1017d4362ec852a9eee27bc6",
  file,
  path: "mods\\" + file,
  disable: false,
  fail: false,
  core: false,
  library: false,
  modId,
  name,
  version,
  author: "作者",
  description: "这是一个用于界面截图的示例模组。",
  loaders: ["neoforge"],
  side: "both",
  url: "https://modrinth.com/mod/" + modId,
  source: "modrinth",
  projectId: modId,
  fileId: "fid-" + uuid,
  note: "",
  icon: "",
  jarInJar: [],
  hasDependencies: false,
  ...extra,
});

export const mods = [
  mod("mod-1", "appliedenergistics2-15.2.1.jar", "ae2", "Applied Energistics 2", "15.2.1"),
  mod("mod-2", "geckolib-4.7.jar", "geckolib", "GeckoLib 4", "4.7.0", { loaders: ["neoforge"] }),
  mod("mod-3", "jei-1.21.4.jar", "jei", "Just Enough Items", "19.21.0.247"),
  mod("mod-4", "jade-15.10.0.jar", "jade", "Jade", "15.10.0"),
  mod("mod-5", "sodium-0.6.5.jar", "sodium", "Sodium", "0.6.5", { loaders: ["fabric"], disable: true }),
  mod("mod-6", "iris-1.8.8.jar", "iris", "Iris Shaders", "1.8.8", { loaders: ["fabric"], disable: true }),
  mod("mod-7", "create-6.0.4.jar", "create", "Create", "6.0.4"),
  mod("mod-8", "farmers-delight.jar", "farmersdelight", "Farmer's Delight", "1.2.7"),
];

export const resourceView = { order: [], category: "mods", modView: "list", modColWidths: {} };
export const modGroups = [];
export const blockGroups = { order: [], groups: [], members: {}, favorites: [] };
export const blockItems = [];
export const collectData = { items: [], groups: {} };

export const lobbyView = {
  account: "Steve",
  accountUuid: "acc-1",
  version: "1.21.4",
  sessionGeneration: 1,
  ready: true,
  downloading: false,
  downloadPercent: 0,
  nodes: [],
  state: { state: "idle", index: -1, room: null, address: null, difficulty: null, exception: null, profiles: [] },
};

export const gamepadActions = [
  "KeyW", "KeyA", "KeyS", "KeyD", "Space", "ShiftLeft", "ControlLeft", "Escape",
  "Digit1", "Digit2", "F1", "F5", "Enter", "Tab",
  "MouseLeft", "MouseRight", "MouseMiddle", "MouseX", "MouseY",
  "ScrollUp", "ScrollDown",
];
export const gamepadProfiles = [{ name: "默认配置", active: true, error: null }];
export const gamepadConfig = {
  selected: true,
  buttons: [
    { button: "south", modifier: null, action: "Space" },
    { button: "east", modifier: null, action: "Escape" },
    { button: "leftTrigger", modifier: null, action: "ShiftLeft" },
    { button: "rightTrigger", modifier: null, action: "MouseLeft" },
  ],
  axes: [
    { axis: "leftStickX", from: -0.6, to: -0.3, action: "KeyA" },
    { axis: "leftStickX", from: 0.3, to: 0.6, action: "KeyD" },
    { axis: "rightStickX", from: -1, to: 1, action: "MouseX" },
  ],
  mouse: { cursorStick: "left", turnStick: "right", cursorSpeed: 50, turnSpeed: 50, deadzone: 0.2 },
};

const entry = (path, size, dir = false) => ({
  path,
  dir,
  size,
  url: "",
  modId: null,
  fileId: null,
  sha1: "",
});
export const exportInfo = {
  canBundleLauncher: true,
  name: "1.21.4 NeoForge",
  customLoader: false,
  mods: [
    entry("mods/appliedenergistics2-15.2.1.jar", 4_200_000),
    entry("mods/jei-1.21.4.jar", 2_400_000),
    entry("mods/jade-15.10.0.jar", 1_100_000),
  ],
};

export const statsPlaytime = {
  days: [
    { date: "2026-10-20", seconds: 5400, count: 2, failed: 0 },
    { date: "2026-10-21", seconds: 12600, count: 3, failed: 1 },
    { date: "2026-10-22", seconds: 0, count: 0, failed: 0 },
    { date: "2026-10-23", seconds: 7200, count: 1, failed: 0 },
    { date: "2026-10-24", seconds: 18000, count: 4, failed: 0 },
    { date: "2026-10-25", seconds: 9600, count: 2, failed: 0 },
    { date: "2026-10-26", seconds: 3600, count: 1, failed: 0 },
  ],
  totalSeconds: 68400,
  activeDays: 6,
  byInstance: [
    { uuid: "inst-1", name: "1.21.4 NeoForge", seconds: 43200, count: 8, failed: 1 },
    { uuid: "inst-2", name: "1.20.1 Forge", seconds: 18000, count: 3, failed: 0 },
    { uuid: "inst-3", name: "Fabric 1.21.1", seconds: 7200, count: 2, failed: 0 },
  ],
};

export const logRuntime = [
  { time: "19:58:01", text: "[main/INFO] [NeoForge] Mod loading complete", thread: "main", level: "INFO", category: "NeoForge" },
  { time: "19:58:02", text: "[Render thread/INFO] [Minecraft] Setting user: Steve", thread: "Render thread", level: "INFO", category: "Minecraft" },
  { time: "19:58:03", text: "[Worker-Main-1/WARN] [AE2] Missing optional dependency", thread: "Worker-Main-1", level: "WARN", category: "AE2" },
  { time: "19:58:05", text: "[Render thread/INFO] [Minecraft] Loaded 124 mods", thread: "Render thread", level: "INFO", category: "Minecraft" },
];
export const configFiles = [
  { name: "options.txt", fileType: "text" },
  { name: "servers.dat", fileType: "nbt" },
  { name: "config", fileType: "text" },
  { name: "logs", fileType: "text" },
];
export const saves = [
  { dir: "世界一", levelName: "新的世界", lastPlayed: 1761390000, gameType: 0, hardCore: false, difficulty: 2, broken: false, seed: "123456789", icon: "", backups: 2, autoEnter: false },
  { dir: "创造测试", levelName: "创造测试", lastPlayed: 1761290000, gameType: 1, hardCore: false, difficulty: 1, broken: false, seed: "-987654321", icon: "", backups: 0, autoEnter: false },
];


export const packs = [
  { file: "Faithful 32x.zip", description: "经典风格的 32x 材质包", packFormat: 34, minFormat: 34, maxFormat: 34, fail: false, enable: true, icon: "" },
  { file: "FreshAnimations.zip", description: "让生物动作更自然", packFormat: 34, minFormat: 34, maxFormat: 34, fail: false, enable: true, icon: "" },
  { file: "Stay True.zip", description: "保留原版风格的细节增强", packFormat: 34, minFormat: 34, maxFormat: 15, fail: false, enable: false, icon: "" },
];
export const servers = [
  { name: "Hypixel", ip: "mc.hypixel.net", acceptTextures: true, icon: "" },
  { name: "2b2t", ip: "2b2t.org", acceptTextures: false, icon: "" },
  { name: "本地测试服", ip: "127.0.0.1:25565", acceptTextures: true, icon: "" },
];
export const shaders = [
  { file: "ComplementaryReimagined_r5.3.zip", name: "Complementary Reimagined", comment: "综合观感与性能的常用光影", selected: true },
  { file: "BSL_v8.2.08.zip", name: "BSL Shaders", comment: "色彩鲜艳", selected: false },
];
export const schematics = [
  { file: "castle.litematic", name: "中世纪城堡", author: "Steve", description: "带城墙与塔楼的城堡", typeName: "Litematica", width: 64, height: 42, length: 64, blockCount: 182340, blockTypes: 47, fail: false },
  { file: "farm.schem", name: "自动农场", author: "Alex", description: "小型自动甘蔗农场", typeName: "Schematic", width: 12, height: 8, length: 9, blockCount: 864, blockTypes: 6, fail: false },
];
export const datapacks = [
  { name: "更好的合成", file: "better_recipes.zip", description: "增加一批实用合成配方", packFormat: 48, enable: true },
  { name: "平衡调整", file: "balance.zip", description: "调整部分原版数值", packFormat: 48, enable: false },
];
export const collectItems = [
  { uuid: "col-1", name: "Sodium", source: "modrinth", fileType: "mod", pid: "sodium", icon: null, url: "https://modrinth.com/mod/sodium", favTime: 1761390000000, loaders: ["fabric"], gameVersions: ["1.21.1"] },
  { uuid: "col-2", name: "Iris Shaders", source: "modrinth", fileType: "shaderpack", pid: "iris", icon: null, url: "https://modrinth.com/mod/iris", favTime: 1761290000000, loaders: ["fabric"], gameVersions: ["1.21.1"] },
  { uuid: "col-3", name: "Create", source: "curseforge", fileType: "mod", pid: "create", icon: null, url: "https://www.curseforge.com/minecraft/mc-mods/create", favTime: 1761190000000, loaders: ["forge"], gameVersions: ["1.20.1"] },
];
export const collectWithData = { items: collectItems, groups: { "模组": ["col-1", "col-3"], "光影包": ["col-2"] } };
export const downloadStatus = {
  tasks: [
    { id: 1, total: 3, completed: 2, failed: 0, allBytes: 73400320, nowBytes: 52428800, elapsedMs: 42000, paused: false },
    { id: 2, total: 1, completed: 1, failed: 0, allBytes: 1048576, nowBytes: 1048576, elapsedMs: 3000, paused: false },
  ],
  threads: [
    { thread: 1, name: "sodium-0.6.5.jar", state: "下载中", progress: 62, nowBytes: 1048576, allBytes: 2097152, speed: 3145728 },
    { thread: 2, name: "iris-1.8.8.jar", state: "下载中", progress: 31, nowBytes: 655360, allBytes: 2097152, speed: 2097152 },
  ],
  speed: 5242880,
  paused: false,
};
export const javaDownloadList = [
  { uuid: "jdk-21", name: "Temurin 21 (LTS)", javaVersion: "21.0.4", filename: "OpenJDK21U-jdk_x64_windows_hotspot_21.0.4_7.zip", size: 198180864 },
  { uuid: "jdk-17", name: "Temurin 17 (LTS)", javaVersion: "17.0.12", filename: "OpenJDK17U-jdk_x64_windows_hotspot_17.0.12_7.zip", size: 190840832 },
  { uuid: "jre-8", name: "Temurin 8 (LTS)", javaVersion: "1.8.0_422", filename: "OpenJDK8U-jre_x64_windows_hotspot_8u422b05.zip", size: 41943040 },
];

export const javaOptions = {
  types: ["adoptium", "zulu", "openj9", "foojay"],
  majors: [8, 17, 21],
  systems: ["windows", "linux", "mac"],
  archs: ["x64", "arm64"],
};

/** 其余窗口启动时会问到的命令；给空值也让界面能画出来 */
export function moreCommands(extra = {}) {
  return {
    settings_get_launch: { javaList, run: settingsLaunch, window: settingsWindow },
    settings_get_defaults: { network: settingsNetwork, run: settingsLaunch, window: settingsWindow },
    main_get_motd: null,
    settings_get_network: settingsNetwork,
    settings_get_local_files: [
      { id: "cache", path: "E:\\M2L\\cache", size: 73400320, deletable: true },
      { id: "image", path: "E:\\M2L\\image", size: 12582912, deletable: true },
      { id: "download", path: "E:\\M2L\\download", size: 268435456, deletable: true },
      { id: "webview", path: "E:\\M2L\\webview", size: 10485760, deletable: true },
      { id: "instances", path: "E:\\M2L\\instances", size: 3221225472, deletable: false },
      { id: "java", path: "E:\\M2L\\java", size: 536870912, deletable: false },
    ],
    settings_get_system_fonts: ["Microsoft YaHei UI", "Segoe UI", "Consolas"],
    resource_view_get: resourceView,
    resource_mod_groups: modGroups,
    resource_mods_update_log: [
      {
        projectId: "sodium",
        name: "Sodium",
        update: [
          { time: 1761400000000, oldVersion: "0.5.8", version: "0.5.11", oldFileId: "4101", fileId: "4288", sha1: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678" },
          { time: 1760200000000, oldVersion: "0.5.5", version: "0.5.8", oldFileId: "3900", fileId: "4101", sha1: "b2c3d4e5f60718293a4b5c6d7e8f901234567890" },
        ],
      },
      {
        projectId: "iris",
        name: "Iris Shaders",
        update: [
          { time: 1761100000000, oldVersion: "1.7.0", version: "1.7.3", oldFileId: "5120", fileId: "5301", sha1: "c3d4e5f60718293a4b5c6d7e8f90123456789012" },
        ],
      },
    ],
    resource_mod_groups_collapsed: [],
    resource_list_mods: mods,
    resource_view_set: null,
    resource_mod_groups_collapsed_set: null,
    add_get_version_types: ["release", "snapshot", "old_beta", "old_alpha"],
    add_get_pack_types: ["curseforge", "modrinth", "mmc", "hmcl"],
    add_get_support_loaders: ["normal", "forge", "neoforge", "fabric", "quilt", "optifine", "liteloader", "cleanroom"],
    add_set_close_guard: null,
    add_create_new: "new-instance-uuid",
    window_ensure_model: null,
    window_drop_model: null,
    add_modpack_last_source: "curseforge",
    add_modpack_set_last_source: null,
    add_resource_set_last_source: null,
    add_resource_categories: {},
    add_resource_game_versions: ["1.21.4", "1.21.1", "1.20.1", "1.19.2"],
    add_resource_sort_type: ["relevance", "downloads", "follows", "newest", "updated"],
    resource_mod_group_order: [],
    resource_mod_group_order_set: null,
    settings_save_launch: null,
    account_get_textures: {
      skins: [
        { sha1: "skin-1", name: "经典 Steve", variant: "classic", active: true },
        { sha1: "skin-2", name: "纤细 Alex", variant: "slim", active: false },
      ],
      capes: [
        { sha1: "cape-1", name: "Migrator", active: true },
      ],
    },
    add_resource_last_source: "curseforge",
    add_resource_source_type: ["mod", "resourcepack", "shaderpack", "save", "datapack"],
    collect_get_data: collectData,
    collect_project_icon: null,
    collect_image_url: null,
    java_download_get_types: javaOptions.types,
    java_download_get_options: javaOptions,
    java_download_get_list: [],
    block_get_groups: blockGroups,
    block_list: blockItems,
    log_get_files: [],
    log_get_runtime: logRuntime,
    log_set_setting: null,
    export_get_info: exportInfo,
    export_list_dir: [],
    export_list_instance_dir: [],
    lobby_prepare: lobbyView,
    lobby_status: lobbyView,
    config_tree_width: 240,
    config_get_files: configFiles,
    config_get_file: {
      text: "# 由截图脚本提供的示例配置\nmaxFps: 120\nrenderDistance: 12\nguiScale: 2\nlanguage: zh_cn\n",
      sha1: "0123456789abcdef",
    },
    resource_list_saves: saves,
    gamepad_get_actions: gamepadActions,
    gamepad_get_profiles: gamepadProfiles,
    gamepad_get_config: gamepadConfig,
    app_build_exe_info: { path: "C:\\Program Files\\M²L\\mml-gui.exe", sha1: "a1b2c3d4e5f6" },
    stats_get_playtime: statsPlaytime,
    add_modpack_list: { items: [], count: 0 },
    add_resource_list: { items: [], count: 0 },
    resource_list_resourcepacks: packs,
    resource_list_servers: servers,
    resource_list_shaderpacks: shaders,
    resource_list_schematics: schematics,
    resource_list_datapacks: datapacks,
    resource_list_screenshots: [],
    download_get_status: { tasks: [], threads: [], speed: 0, paused: false },
    ...extra,
  };
}

export function commonCommands(extra = {}) {
  return {
    window_get_gui_config: baseConfig,
    custom_ui_status: customUiStatus,
    custom_home_status: customHomeStatus,
    app_build_info: buildInfo,
    account_get_accounts: accounts,
    main_get_java_list: javaList,
    main_get_running: [],
    main_get_news: news,
    main_get_system_memory: { total: 32768, free: 16384 },
    main_load_state: { ok: true, error: null },
    main_get_instance_args: instanceArgs,
    main_get_instance_langs: [
      { code: "zh_cn", name: "简体中文" },
      { code: "en_us", name: "English" },
    ],
    add_modpack_status: { windowOpen: false, tasks: [] },
    add_resource_status: { windowOpen: false, tasks: [] },
    main_music_base_url: "",
    main_image_base_url: "",
    main_take_start_arg: null,
    main_get_versions: addVersions,
    main_refresh_versions: addVersions,
    add_get_loader_versions: ["0.16.10", "0.16.9", "0.15.11"],
    add_list_archive: [
      "manifest.json",
      "mods/",
      "mods/sodium.jar",
      "mods/iris.jar",
      "config/",
      "config/sodium-options.json",
    ],
    add_detect_archive: { packType: "curseforge", name: "Better MC" },
    add_scan_folder: [
      { path: "E:\\M2L\\instances\\Better MC", name: "Better MC" },
      { path: "E:\\M2L\\instances\\RLCraft", name: "RLCraft" },
    ],
    add_list_dir: [
      { name: "mods", isDir: true },
      { name: "config", isDir: true },
      { name: "options.txt", isDir: false },
    ],
    add_probe_server_pack: {
      url: "https://example.com/pack/server.json",
      name: "Better MC Server",
      version: "1.20.1",
      loader: "forge",
      loaderVersion: "47.2.0",
      packVersion: "1.0.0",
      text: "服务端整合包，包含 128 个模组",
      fileCount: 132,
      archiveCount: 4,
    },
    main_get_custom_json: [],
    settings_get_bg: { source: "", dataUrl: "", opacity: 100, blur: 0, nativeSize: 0 },
    log_get_setting: {
      wordWrap: false,
      auto: true,
      enableNone: true,
      enableInfo: true,
      enableWarn: true,
      enableError: true,
      enableDebug: false,
      autoShow: false,
    },
    download_get_status: { tasks: [], threads: [], speed: 0, paused: false },
    stats_get_data: { launchCount: 0, launchDoneCount: 0, launchErrorCount: 0, totalSeconds: 0, instances: [] },
    block_status: { rendered: false, version: "", running: false, now: 0, total: 0, text: null, error: null },
    window_save_gui_config: null,
    window_check_colormc: null,
    window_is_maximized: false,
    window_is_fullscreen: false,
    window_set_title: null,
    window_activate_decoration: "custom",
    ...moreCommands(),
    ...extra,
  };
}
