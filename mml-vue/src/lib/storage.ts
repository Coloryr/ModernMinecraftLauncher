// 界面本地存储：**全仓唯一的 localStorage 出入口**
//
// 为什么要收成一个文件：
// - 各家自己 `localStorage.getItem(...)` 时，"键长什么样""默认值是什么""非法值怎么兜"
//   全靠约定，同一份数据在两处口径不一致就会出现"设置里改了、主界面没变"；
// - 键名散在十几个文件里，改一处漏一处。guiConfig.ts 的浏览器回退分支就是手工重列了
//   一整份键，漏一个就静默丢设置；
// - 将来要换掉 localStorage，比如换 IndexedDB 或走后端文件，只有这一个文件要改。
//
// 键名一律 `mml.` 前缀 + 小驼峰，集中登记在 [`KEYS`]；**不要在别处再写字符串字面量**。
//
// 跨窗口同步：原来靠 `window.addEventListener("storage")`——那是 localStorage 的事件，
// **只有"另一个文档"改了值才触发**，同一个文档里写不触发。换成后端文件后没有这个事件，
// 所以这里自己派发同名事件，见 [`emitChange`]，行为与原来一致：
// 同源（同一窗口）不触发，其它窗口都触发。
//
// WebView2 的用户数据目录已经搬到 `<cache>/webview`，见 mml-gui 的 windows/mod.rs，
// 所以 localStorage 本身也跟着运行目录走了 —— 这一层不再需要额外落盘一份镜像。
import { isTauri } from "./util";

/** 自派发的变更事件名，与原生 `storage` 分开，避免和真事件混淆 */
const CHANGE_EVENT = "mml:storage";

/**
 * 全部存储键（**唯一登记处**）
 *
 * 命名口径：`mml.<域>[.<子项>]`，小驼峰。新增键一律加在这里，
 * 不要在业务代码里写 `localStorage.getItem("mml.xxx")`。
 */
export const KEYS = {
  // ---- 外观 / 主题 ----
  /** 主题：Dark / Light / System */
  theme: "mml.theme",
  /** 强调色预设 id 或 custom */
  accent: "mml.accent",
  /** 自定义强调色 #rrggbb */
  accentCustom: "mml.accentCustom",
  /** 界面字体族名（空串 = 默认字体栈） */
  font: "mml.font",
  /** 动画开关（"0" = 关） */
  animations: "mml.animations",
  /** 背景图 dataURL：浏览器回退用，Tauri 下以后端落盘的图为准 */
  bgImage: "mml.bgImage",
  /** 背景不透明度 %（浏览器回退用） */
  bgOpacity: "mml.bgOpacity",
  /** 背景模糊 px */
  bgBlur: "mml.bgBlur",

  // ---- 窗口 / 主界面 ----
  /** 多窗口 / 单窗口：Single 之外都算 Multi */
  windowMode: "mml.windowMode",
  /** 侧栏在左还是在右 */
  sidebarSide: "mml.sidebarSide",
  /** 侧栏是否收起（"0" = 展开） */
  sidebarCollapsed: "mml.sidebarCollapsed",
  /** 实例列表显示模式 list / group / grid */
  viewMode: "mml.viewMode",
  /** 启动时先显示哪一屏 Home / List */
  startupPage: "mml.startupPage",
  /** 当前选中实例 uuid */
  selectedInstance: "mml.selectedInstance",
  /** 语言 zh_cn / en_us */
  locale: "mml.locale",
  /** 设置窗口上次所在的页签 */
  settingsTab: "mml.settingsTab",
  /** 图标裁剪上次选的边长 */
  iconPickSize: "mml.iconPickSize",

  // ---- 账户显示 ----
  /** 皮肤显示模式 Skin2DA / Skin2DB / Skin3D / Skin3DD / Skin3DRotate */
  skinDisplay: "mml.skinDisplay",
  /** 皮肤 3D 旋转 X */
  skinPitch: "mml.skinPitch",
  /** 皮肤 3D 旋转 Y */
  skinYaw: "mml.skinYaw",
  /** 披风显示模式 Cape2D / Cape3D / Elytra */
  capeDisplay: "mml.capeDisplay",
  /** 披风 3D 旋转 X */
  capePitch: "mml.capePitch",
  /** 披风 3D 旋转 Y */
  capeYaw: "mml.capeYaw",
  /** 头像渲染模式 */
  headType: "mml.headType",
  /** 3D 头像旋转 X */
  headX: "mml.headX",
  /** 3D 头像旋转 Y */
  headY: "mml.headY",

  // ---- 客户端设置：settings 窗口改，主界面读 ----
  /** MOTD 卡片开关（"0" = 关） */
  motdCard: "mml.client.motdCard",
  /** MOTD 刷新间隔，单位秒 */
  motdInterval: "mml.client.motdInterval",
  /** 登录方式锁定总开关 */
  loginLockOn: "mml.client.loginLockOn",
  /** 登录方式锁定列表（JSON） */
  loginLock: "mml.client.loginLock",
  /** 自动进服开关 */
  autoJoin: "mml.client.autoJoin",
  /** 自动进服地址 */
  autoJoinServer: "mml.client.autoJoinServer",
  /** MOTD 显示地址 */
  motdServer: "mml.client.motdServer",
  /** 实例锁定 uuid */
  lockInstance: "mml.client.lockInstance",
  /** 自定义主页面开关 */
  customHome: "mml.client.customHome",

  // ---- 收藏窗口类型过滤（"0" = 不显示） ----
  collectModpack: "mml.collect.modpack",
  collectShowMod: "mml.collect.showMod",
  collectResourcePack: "mml.collect.resourcePack",
  collectShaderpack: "mml.collect.shaderpack",

  // ---- 编辑器（日志视图与配置编辑器共用这一份） ----
  /** 自动换行，"0" = 关；Alt+Z 与界面上那个开关都写它 */
  editorWordWrap: "mml.editor.wordWrap",

  // ---- 各窗口视图偏好（JSON） ----
  /**
   * 方块窗口：分类 / 搜索词 / 图标尺寸档
   *
   * **资源窗口**的分类顺序 / 上次类别 / 模组展示方式**不在这里** —— 那三项跟着实例走，
   * 存在实例自己的 `gui_setting.json`，见 `windows/resource/composables/useResourceView`。
   */
  blockView: "mml.blockView",

  /** NBT 树键列宽度，单位像素；0 表示按默认比例，见 NbtTree */
  nbtKeyWidth: "mml.nbtKeyWidth",

  // ---- 跨窗口一次性通知 ----
  /** 新增实例 uuid：添加实例 / 整合包窗口写给主窗口，见 AddInstanceWindow */
  addedInstance: "mml.addedInstance",
} as const;

/** 键名类型，只允许 [`KEYS`] 里登记过的键 */
export type StorageKey = (typeof KEYS)[keyof typeof KEYS];

/** 取 localStorage；隐私模式等拿不到时返回 null，调用方据此把读当空、把写当无事发生 */
function store(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

/**
 * 读原始字符串
 *
 * 想区分"没存过"和"存了空串"时必须用这个，否则直接用下面那几个带默认值的便捷函数。
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 *
 * # 返回值
 *
 * 没有存过或存储不可用时为 `null`
 */
export function readRaw(key: StorageKey): string | null {
  try {
    return store()?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

/**
 * 写原始字符串
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `value`: 要写入的字符串
 */
export function writeRaw(key: StorageKey, value: string): void {
  try {
    store()?.setItem(key, value);
  } catch {
    /* 写不进去就算了：内存里的状态仍然是对的 */
  }
}

/**
 * 删掉一个键
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 */
export function remove(key: StorageKey): void {
  try {
    store()?.removeItem(key);
  } catch {
    /* 同上 */
  }
}

/**
 * 读字符串
 *
 * 空串是**合法值**，不会被当成"没存过"。
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `dflt`: 没存过时的返回值
 *
 * # 返回值
 *
 * 存过的字符串，没存过时为 `dflt`
 */
export function readString(key: StorageKey, dflt = ""): string {
  return readRaw(key) ?? dflt;
}

/**
 * 写字符串
 *
 * 传空串时按"删掉这个键"处理，与全仓 `if (v) set else remove` 的老口径一致。
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `value`: 要写入的字符串，空串表示删除
 */
export function writeString(key: StorageKey, value: string): void {
  if (value) writeRaw(key, value);
  else remove(key);
}

/**
 * 读布尔
 *
 * `"1"` / `"true"` 为真，其余为假。
 * 注意默认值：`readFlag(key, true)` 表示"**没存过时算开**"，并且 `"0"` 表示关 ——
 * 这正是本仓 `localStorage.getItem(x) !== "0"` 的老口径。
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `dflt`: 没存过时的返回值
 *
 * # 返回值
 *
 * 存过的布尔值，没存过时为 `dflt`
 */
export function readFlag(key: StorageKey, dflt = false): boolean {
  const raw = readRaw(key);
  if (raw === null) return dflt;
  return raw === "1" || raw === "true";
}

/**
 * 写布尔，落盘为 `"1"` / `"0"`
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `value`: 要写入的布尔值
 */
export function writeFlag(key: StorageKey, value: boolean): void {
  writeRaw(key, value ? "1" : "0");
}

/**
 * 读数字
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `dflt`: 没存过或存了非数字时的返回值
 *
 * # 返回值
 *
 * 存过的数字，非法时为 `dflt`
 */
export function readNumber(key: StorageKey, dflt: number): number {
  const raw = readRaw(key);
  if (raw === null) return dflt;
  const v = Number(raw);
  return Number.isFinite(v) ? v : dflt;
}

/**
 * 写数字
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `value`: 要写入的数字
 */
export function writeNumber(key: StorageKey, value: number): void {
  writeRaw(key, String(value));
}

/**
 * 读 JSON 对象
 *
 * 只读"对象形态"的偏好，数组 / 数字之类的偏好用 [`readRaw`] 自己解析。
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 *
 * # 返回值
 *
 * 解析出的对象；解析失败或存的不是对象时为 `{}`
 */
export function readJson<T extends object>(key: StorageKey): Partial<T> {
  const raw = readRaw(key);
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Partial<T>)
      : {};
  } catch {
    return {};
  }
}

/**
 * 写 JSON
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `value`: 要序列化的值
 */
export function writeJson(key: StorageKey, value: unknown): void {
  try {
    writeRaw(key, JSON.stringify(value));
  } catch {
    /* 循环引用之类：不写就是了 */
  }
}

/**
 * 值变更监听，跨窗口同步
 *
 * 无 Tauri（浏览器预览）时退回原生 `storage` 事件；Tauri 下由 [`emitChange`] 派发同名事件。
 * 回调收到 `newValue`，键被删除时为 `null`。
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `handler`: 变更回调
 *
 * # 返回值
 *
 * 退订函数
 */
export function onStorageChange(
  key: StorageKey,
  handler: (newValue: string | null) => void,
): () => void {
  const listener = (e: Event) => {
    const detail = (e as CustomEvent<{ key: string; newValue: string | null }>).detail;
    if (detail) {
      // 自己派发的同源事件
      if (detail.key !== key) return;
      handler(detail.newValue);
      return;
    }
    const se = e as StorageEvent;
    if (se.key !== key) return;
    handler(se.newValue);
  };
  window.addEventListener(CHANGE_EVENT, listener);
  window.addEventListener("storage", listener);
  return () => {
    window.removeEventListener(CHANGE_EVENT, listener);
    window.removeEventListener("storage", listener);
  };
}

/**
 * 广播一次值变更，让**其它窗口**知道
 *
 * 在 [`writeRaw`] 之外单独调：写值的语义由业务决定，有的键只是镜像、不需要同步。
 * 同源（本窗口）不触发 —— 与原生 `storage` 事件口径一致，调用方自己已经更新过内存状态了。
 *
 * - `key`: 存储键，只能取 [`KEYS`] 里登记过的
 * - `newValue`: 变更后的值，键被删除时传 `null`
 */
export function emitChange(key: StorageKey, newValue: string | null): void {
  if (!isTauri()) return; // 浏览器预览有原生 storage 事件，别重复派发
  window.dispatchEvent(new CustomEvent(CHANGE_EVENT, { detail: { key, newValue } }));
}
