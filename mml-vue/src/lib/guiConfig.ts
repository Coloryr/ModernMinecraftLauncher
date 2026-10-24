// GUI 状态由 Rust 提供：gui_config.json 存界面状态，window_save.json 存窗口几何
// 主窗口使用固定 uuid，与 Rust window_manager.rs 一致。
// IPC wire 使用 TS 命名（camelCase），经 Rust dtos::GuiConfigDto 转换；
// 磁盘 gui_config.json 使用 Rust 命名（snake_case），前端不直接接触。
// 枚举值即 Rust 变体名：theme/windowMode/sidebarSide 为 PascalCase，locale 为 zh_cn/en_us。
import { commands } from "./bindings";
import { KEYS, readFlag, readNumber, readRaw, readString } from "./storage";

/** 主题：Dark / Light / System */
export type Theme = "Dark" | "Light" | "System";
/** 与 core Lang 变体同名：zh_cn / en_us */
export type Locale = "zh_cn" | "en_us";
/** 多窗口 / 单窗口 */
export type WindowMode = "Multi" | "Single";
/** 侧栏在左还是在右 */
export type SidebarSide = "Left" | "Right";
/** 实例列表显示模式：默认 list，用户改过后用用户的值 */
export type ViewMode = "list" | "group" | "grid";
/** 启动器启动时主窗口先显示哪一屏：枚举值即 Rust 变体名 */
export type StartupPage = "Home" | "List";
/** 皮肤显示模式：账户界面皮肤预览形态，枚举值即 Rust 变体名 */
export type SkinDisplay = "Skin2DA" | "Skin2DB" | "Skin3D" | "Skin3DD" | "Skin3DRotate";
/** 披风显示模式：账户界面披风 / 鞘翅预览形态，枚举值即 Rust 变体名 */
export type CapeDisplay = "Cape2D" | "Cape3D" | "Elytra";

/**
 * 把任意值规范成合法的皮肤显示模式
 *
 * - `value`: 待规范的原始值，非法 / 缺省时为 null 或 undefined
 *
 * # 返回值
 *
 * 合法模式，非法 / 缺省时回落 `Skin2DB`
 */
export function normalizeSkinDisplay(value: string | null | undefined): SkinDisplay {
  return value === "Skin2DA" ||
    value === "Skin3D" ||
    value === "Skin3DD" ||
    value === "Skin3DRotate"
    ? value
    : "Skin2DB";
}

/** 皮肤显示设置，对应 Rust SkinConfig，wire 为 skin */
export interface SkinConfig {
  /** Skin2DA / Skin2DB / Skin3D / Skin3DD / Skin3DRotate */
  display: SkinDisplay;
  /** 3D 旋转 X，单位度 */
  pitch: number;
  /** 3D 旋转 Y，单位度 */
  yaw: number;
}

/** 皮肤 3D 旋转角度默认值：X 15 / Y 45，与 Rust SkinConfig::default 一致 */
export const DEFAULT_SKIN_PITCH = 15;
export const DEFAULT_SKIN_YAW = 45;

/** 披风显示设置，对应 Rust CapeConfig，wire 为 cape */
export interface CapeConfig {
  /** Cape2D / Cape3D / Elytra */
  display: CapeDisplay;
  /** 披风 3D 旋转 X，单位度 */
  pitch: number;
  /** 披风 3D 旋转 Y，单位度 */
  yaw: number;
}

/** 披风 3D 角度默认值：X 0 / Y 30，与 Rust CapeConfig::default 一致 */
export const DEFAULT_CAPE_PITCH = 0;
export const DEFAULT_CAPE_YAW = 30;

/**
 * 把任意值规范成合法的披风显示模式
 *
 * - `value`: 待规范的原始值，非法 / 缺省时为 null 或 undefined
 *
 * # 返回值
 *
 * 合法模式，非法 / 缺省时回落 `Cape2D`
 */
export function normalizeCapeDisplay(value: string | null | undefined): CapeDisplay {
  return value === "Cape3D" || value === "Elytra" ? value : "Cape2D";
}

/**
 * 把任意值规范成合法的显示模式
 *
 * - `value`: 待规范的原始值，非法 / 缺省时为 null 或 undefined
 *
 * # 返回值
 *
 * 合法模式，非法 / 缺省时回落 `list`
 */
export function normalizeViewMode(value: string | null | undefined): ViewMode {
  return value === "group" || value === "grid" ? value : "list";
}

/**
 * 把任意值规范成合法的启动页
 *
 * - `value`: 待规范的原始值，非法 / 缺省时为 null 或 undefined
 *
 * # 返回值
 *
 * 合法取值，非法 / 缺省时回落 `Home`
 */
export function normalizeStartupPage(value: string | null | undefined): StartupPage {
  return value === "List" ? "List" : "Home";
}

/**
 * 把任意值规范成合法的头像类型
 *
 * - `value`: 待规范的原始值，非法 / 缺省时为 null 或 undefined
 *
 * # 返回值
 *
 * 合法类型，非法 / 缺省时回落 `Head2DB`
 */
export function normalizeHeadType(value: string | null | undefined): HeadType {
  return value === "Head3DA" || value === "Head3DB" || value === "Head3DC" || value === "Head2DA" ? value : "Head2DB";
}

/** 主窗口配置，对应 Rust MainWindowConfig，wire 为 mainWindow */
export interface MainWindowConfig {
  /** Left / Right */
  sidebarSide: SidebarSide;
  /** 是否收起侧栏 */
  sidebarCollapsed: boolean;
  /** list / group / grid */
  viewMode: ViewMode;
  /** 启动时先显示哪一屏：Home / List */
  startupPage: StartupPage;
  /** 当前选中实例 uuid（空串 = 未选中） */
  selectedInstance: string;
}

/** 头像类型（枚举值即 Rust 变体名） */
export type HeadType = "Head2DA" | "Head3DA" | "Head3DB" | "Head3DC" | "Head2DB";

/** 头像设置，对应 Rust HeadConfig，wire 为 head */
export interface HeadConfig {
  /** Head2DA / Head3DA / Head3DB / Head3DC / Head2DB */
  headType: HeadType;
  /** 3D 旋转 X */
  x: number;
  /** 3D 旋转 Y */
  y: number;
}

/** 收藏窗口的类型过滤，对应 Rust CollectConfig，wire 为 collect */
export interface CollectConfig {
  /** 显示整合包 */
  modpack: boolean;
  /** 显示模组 */
  showMod: boolean;
  /** 显示资源包 */
  resourcePack: boolean;
  /** 显示光影包 */
  shaderpack: boolean;
}

/** 登录方式锁定的一个条目 */
export interface LoginLockItem {
  /** 账户添加类型：offline / microsoft / littleskin / selflittleskin / authlib / nide8 */
  ty: string;
  /** 登录模型名字：添加账户时下拉里的显示名，空 = 用类型名 / 服务器地址显示 */
  name: string;
  /** 锁定的服务器：authlib = 认证服务器地址，nide8 = 服务器 ID，空 = 不指定 */
  server: string;
}

/** 客户端设置，对应 Rust ClientConfig，wire 为 client */
/** 实验性功能开关，对应 Rust ExperimentalConfig，wire 为 experimental */
export interface ExperimentalConfig {
  /** 启动器自身以管理员身份运行 */
  launcherAdmin: boolean;
  /** 启动实例时提升到管理员身份 */
  instanceAdmin: boolean;
  /** 允许通过 URI 直接启动实例 */
  uriLaunch: boolean;
  /** 在沙盒里启动实例 */
  sandboxLaunch: boolean;
}

/** 一条已安装的自定义 CSS */
export interface CustomCssItem {
  /** 条目 id，CSS 包是 custom_ui/css/<id>.zip */
  id: string;
  /** 显示名，导入时的文件名 */
  name: string;
  /** 是否启用；停用的条目保留文件但不注入 */
  enabled: boolean;
}

export interface ClientConfig {
  /** 主窗口实例设置里显示 MOTD 卡片 */
  motdCard: boolean;
  /** MOTD 卡片刷新间隔，单位秒 */
  motdInterval: number;
  /** 登录方式锁定总开关；false 表示锁定列表不生效 */
  loginLockOn: boolean;
  /** 登录方式锁定列表；空表示不锁定，可添加多个条目 */
  loginLock: LoginLockItem[];
  /** 游戏自动进服，启动时自动进入 autoJoinServer */
  autoJoin: boolean;
  /** 启动实例时检查账户是否已被其它运行中的实例占用 */
  accountCheck: boolean;
  /** 实例配了自动进入存档时，启动前是否弹窗确认 */
  autoSaveNotice: boolean;
  /** 游戏自动进服地址：host 或 host:port，空表示不自动进服 */
  autoJoinServer: string;
  /** 窗口材质：auto（云母→亚克力→模糊降级）/ mica / acrylic / blur / none */
  windowEffect: string;
  /** 窗口底色不透明度，单位百分比 60–100 */
  windowOpacity: number;
  /** MOTD 显示地址：host 或 host:port，空表示不显示真实服务器信息 */
  motdServer: string;
  /** 实例锁定：锁定的实例 uuid（空 = 未锁定）；锁定时主窗口不允许切换到其它实例 */
  lockInstance: string;
  /** 锁定客户端设置：设置页隐藏客户端设置，需要密码解锁；只读，真值来自运行目录的隐藏锁文件 */
  lockClient: boolean;
  /** 自定义主页面：启用后主窗口的启动器主页换成导入的页面 */
  customHome: boolean;
  /** 自定义 CSS 总开关；关闭后已安装的样式保留但不生效 */
  customCssOn: boolean;
  /** 已安装的自定义 CSS，顺序即注入顺序 */
  customCss: CustomCssItem[];
  /** 完全自定义界面总开关；关闭后主窗口回内置界面，整页包保留 */
  customPageOn: boolean;
  /** 自定义音乐：启用后主窗口播放 `musicSource` */
  musicOn: boolean;
  /** 自定义音乐来源：本地音频文件路径或 http(s) 直链，空串表示没配 */
  musicSource: string;
}

export interface GuiConfig {
  /** Dark / Light / System */
  theme: Theme;
  /** 强调色：预设 id（blue / green / …）或 custom */
  accent: string;
  /** 自定义强调色，#rrggbb */
  customAccent: string;
  /** zh_cn / en_us */
  locale: Locale;
  /** Multi / Single */
  windowMode: WindowMode;
  mainWindow: MainWindowConfig;
  head: HeadConfig;
  collect: CollectConfig;
  client: ClientConfig;
  /** 实验性功能开关（界面先行，行为未接线） */
  experimental: ExperimentalConfig;
  /** 界面字体族名（空串 = 默认字体栈） */
  font: string;
  /** 皮肤显示设置：Skin2DA / Skin2DB / Skin3D / Skin3DD / Skin3DRotate 与旋转角度 */
  skin: SkinConfig;
  /** 披风显示设置：Cape2D / Cape3D / Elytra 与 3D 角度 */
  cape: CapeConfig;
  /** 背景图来源：文件路径或网址，空串表示无背景图 */
  bgSource: string;
  /** 背景图不透明度，单位 %，取值 5–100 */
  bgOpacity: number;
  /** 背景图模糊，单位 px，取值 0–40 */
  bgBlur: number;
  /** 背景图原始分辨率，单位 %，取值 10–100 */
  bgNativeSize: number;
}

export interface WindowState {
  uuid: string;
  /** 窗口标签：main / mml-settings … */
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * 读取 GUI 状态
 *
 * # 返回值
 *
 * 完整配置；非 Tauri 环境或后端调用失败时为 `null`，调用方回退本地存储
 */
export async function loadGuiConfig(): Promise<GuiConfig | null> {
  try {
    return await commands.windows.getGuiConfig();
  } catch {
    return null;
  }
}

/** GUI 状态局部更新：mainWindow / head 内部可只给一个字段，saveGuiConfig 会深合并 */
export interface GuiConfigPatch {
  theme?: Theme;
  locale?: Locale;
  windowMode?: WindowMode;
  mainWindow?: Partial<MainWindowConfig>;
  head?: Partial<HeadConfig>;
  collect?: Partial<CollectConfig>;
  client?: Partial<ClientConfig>;
  experimental?: Partial<ExperimentalConfig>;
  font?: string;
  accent?: string;
  customAccent?: string;
  skin?: Partial<SkinConfig>;
  cape?: Partial<CapeConfig>;
  bgSource?: string;
  bgOpacity?: number;
  bgBlur?: number;
  bgNativeSize?: number;
}

/**
 * 合并保存 GUI 状态到 gui_config.json
 *
 * mainWindow / head / collect / client / skin / cape 内部做深合并，避免互相覆盖。
 *
 * - `patch`: 要更新的字段，未给的字段保持原值
 */
export async function saveGuiConfig(patch: GuiConfigPatch): Promise<void> {
  try {
    const cur = (await loadGuiConfig()) ?? defaultConfig();
    await commands.windows.saveGuiConfig({
      ...cur,
      ...patch,
      mainWindow: { ...cur.mainWindow, ...patch.mainWindow },
      head: { ...cur.head, ...patch.head },
      collect: { ...cur.collect, ...patch.collect },
      client: { ...cur.client, ...patch.client },
      experimental: { ...cur.experimental, ...patch.experimental },
      skin: { ...cur.skin, ...patch.skin },
      cape: { ...cur.cape, ...patch.cape },
    });
  } catch {
    /* 浏览器环境忽略 */
  }
}

/** 把本地存储里的登录锁定 JSON 解析成条目列表：兼容旧的纯字符串数组，非法 / 缺省 → 空列表 */
function safeLockList(raw: string | null): LoginLockItem[] {
  if (!raw) return [];
  try {
    const v = JSON.parse(raw);
    if (!Array.isArray(v)) return [];
    return v
      .map((x): LoginLockItem | null => {
        if (typeof x === "string") return { ty: x, name: "", server: "" };
        if (x && typeof x === "object" && typeof x.ty === "string") {
          return {
            ty: x.ty,
            name: typeof x.name === "string" ? x.name : "",
            server: typeof x.server === "string" ? x.server : "",
          };
        }
        return null;
      })
      .filter((x): x is LoginLockItem => x !== null);
  } catch {
    return [];
  }
}

/**
 * 默认配置，浏览器回退用，也是设置窗口「恢复默认」的取值来源
 *
 * 口径与 Rust 侧 `GuiConfig::default()` 一致，改这里要同步那边。
 *
 * 每一项都从本地存储的**同名键**取，键登记在 storage.ts 的 `KEYS`，
 * 所以这里不再手写一串字符串字面量 —— 老版本是手工重列的，加一个键就得记得补一处，
 * 漏了就静默回落默认值。读取一律走 storage.ts 的便捷函数，默认值口径也在那边。
 *
 * # 返回值
 *
 * 完整默认配置
 */
export function defaultConfig(): GuiConfig {
  const storedTheme = readString(KEYS.theme);
  return {
    // 没有显式选择时跟随系统深浅色，与 theme.ts 的首绘兜底一致
    theme:
      storedTheme === "Light" || storedTheme === "Dark" || storedTheme === "System"
        ? storedTheme
        : matchMedia("(prefers-color-scheme: dark)").matches
          ? "Dark"
          : "Light",
    // 与 theme.ts 的首绘兜底一致：没读到就用默认强调色
    accent: readString(KEYS.accent) || "blue",
    customAccent: readString(KEYS.accentCustom, "#4f8cff"),
    locale: readString(KEYS.locale) === "en_us" ? "en_us" : "zh_cn",
    windowMode: readString(KEYS.windowMode) === "Single" ? "Single" : "Multi",
    mainWindow: {
      sidebarSide: readString(KEYS.sidebarSide) === "Right" ? "Right" : "Left",
      sidebarCollapsed: readFlag(KEYS.sidebarCollapsed, true),
      viewMode: normalizeViewMode(readString(KEYS.viewMode)),
      startupPage: normalizeStartupPage(readString(KEYS.startupPage)),
      selectedInstance: readString(KEYS.selectedInstance),
    },
    head: {
      headType: normalizeHeadType(readString(KEYS.headType)),
      // 与 settings.ts 同语义：没存过 / 非法值回落到默认角度
      // 默认 x 15 / y 65。别用 || 0，未存过时会把默认角度覆盖成 0/0
      x: readNumber(KEYS.headX, 15),
      y: readNumber(KEYS.headY, 65),
    },
    collect: {
      modpack: readFlag(KEYS.collectModpack, true),
      showMod: readFlag(KEYS.collectShowMod, true),
      resourcePack: readFlag(KEYS.collectResourcePack, true),
      shaderpack: readFlag(KEYS.collectShaderpack, true),
    },
    client: {
      motdCard: readFlag(KEYS.motdCard, true),
      motdInterval: readNumber(KEYS.motdInterval, 15),
      loginLockOn: readFlag(KEYS.loginLockOn),
      loginLock: safeLockList(readRaw(KEYS.loginLock)),
      autoJoin: readFlag(KEYS.autoJoin),
      // 账户占用检查默认开；真值以 gui_config.json 为准
      accountCheck: true,
      // 自动进入存档的启动提醒默认开，同样以 gui_config.json 为准
      autoSaveNotice: true,
      autoJoinServer: readString(KEYS.autoJoinServer),
      // 这两项不做 localStorage 首帧缓存：真值一律来自 gui_config.json
      windowEffect: "auto",
      windowOpacity: 85,
      motdServer: readString(KEYS.motdServer),
      lockInstance: readString(KEYS.lockInstance),
      lockClient: false,
      customHome: readFlag(KEYS.customHome),
      // 自定义界面的开关与条目只认 gui_config，前端不给本地兜底
      customCssOn: false,
      customCss: [],
      customPageOn: false,
      // 自定义音乐：不给 localStorage 兜底，缺省就是没配
      musicOn: false,
      musicSource: "",
    },
    experimental: {
      launcherAdmin: false,
      instanceAdmin: false,
      uriLaunch: false,
      sandboxLaunch: false,
    },
    font: readString(KEYS.font),
    skin: {
      display: normalizeSkinDisplay(readString(KEYS.skinDisplay)),
      // 与 settings.ts 同语义：没存过 / 非法值回落到默认角度
      pitch: readNumber(KEYS.skinPitch, DEFAULT_SKIN_PITCH),
      yaw: readNumber(KEYS.skinYaw, DEFAULT_SKIN_YAW),
    },
    cape: {
      display: normalizeCapeDisplay(readString(KEYS.capeDisplay)),
      // 与 settings.ts 同语义：没存过 / 非法值回落到默认角度
      pitch: readNumber(KEYS.capePitch, DEFAULT_CAPE_PITCH),
      yaw: readNumber(KEYS.capeYaw, DEFAULT_CAPE_YAW),
    },
    bgSource: "",
    bgOpacity: readNumber(KEYS.bgOpacity, 100),
    bgBlur: readNumber(KEYS.bgBlur, 0),
    bgNativeSize: 100,
  };
}
