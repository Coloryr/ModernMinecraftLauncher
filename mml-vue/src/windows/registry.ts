// 窗口注册表：定义启动器的所有窗口
// 多窗口模式下每个窗口是独立的 Tauri WebviewWindow / 浏览器标签页，
// 单窗口模式下这些窗口在应用内切换展示。

export type WindowKind =
  | "main"
  | "settings"
  | "stats"
  | "help"
  | "resource"
  | "account"
  | "add"
  | "add_modpack"
  | "add_resource"
  | "collect"
  | "download"
  | "java_download"
  | "block"
  | "log"
  | "export";

export interface WindowInfo {
  /** 窗口标识，与后端注册表、URL 上的 `window` 参数一致 */
  kind: WindowKind;
  /** 窗口标题；多窗口模式下也是系统窗口标题 */
  title: string;
}

// 窗口宽高不在前端维护：唯一来源是后端 WINDOWS_INFO，由 window_get_window_sizes
// 命令下发。真实窗口与 JS 回退路径都用后端的默认尺寸 + 历史几何
export const WINDOW_REGISTRY: WindowInfo[] = [
  { kind: "main", title: "ModernMinecraftLauncher" },
  { kind: "settings", title: "启动器设置" },
  { kind: "stats", title: "游戏统计" },
  { kind: "help", title: "帮助手册" },
  { kind: "resource", title: "资源管理" },
  { kind: "account", title: "账户管理" },
  { kind: "add", title: "添加实例" },
  { kind: "add_modpack", title: "下载整合包" },
  { kind: "add_resource", title: "添加资源" },
  { kind: "collect", title: "资源收藏" },
  { kind: "download", title: "下载管理" },
  { kind: "java_download", title: "下载 Java" },
  { kind: "block", title: "方块列表" },
  { kind: "log", title: "游戏日志" },
  { kind: "export", title: "导出实例" },
];

/**
 * 判断一个字符串是不是已注册的窗口 kind
 *
 * - `v`: 待判定的值，通常来自 URL 查询参数，可能为 null
 *
 * # 返回值
 *
 * 命中注册表时为 true，并把类型收窄成 `WindowKind`
 */
export function isWindowKind(v: string | null): v is WindowKind {
  return !!v && WINDOW_REGISTRY.some((w) => w.kind === v);
}
