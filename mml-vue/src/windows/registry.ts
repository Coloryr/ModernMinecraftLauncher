// 窗口注册表：定义启动器的所有窗口
// 多窗口模式下每个窗口是独立的 Tauri WebviewWindow / 浏览器标签页，
// 单窗口模式下这些窗口在应用内切换展示。

export type WindowKind =
  | "lobby"
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
  | "export"
  | "config"
  | "server_pack"
  | "server_instance";

export interface WindowInfo {
  /** 窗口标识，与后端注册表、URL 上的 `window` 参数一致 */
  kind: WindowKind;
  /** 窗口标题的 i18n 键，取 `winTitle` 系列；多窗口模式下也是系统窗口标题 */
  titleKey: string;
}

// 窗口宽高不在前端维护：唯一来源是后端 WINDOWS_INFO，由 window_get_window_sizes
// 命令下发。真实窗口与 JS 回退路径都用后端的默认尺寸 + 历史几何
export const WINDOW_REGISTRY: WindowInfo[] = [
  { kind: "lobby", titleKey: "winTitle.lobby" },
  { kind: "main", titleKey: "winTitle.main" },
  { kind: "settings", titleKey: "winTitle.settings" },
  { kind: "stats", titleKey: "winTitle.stats" },
  { kind: "help", titleKey: "winTitle.help" },
  { kind: "resource", titleKey: "winTitle.resource" },
  { kind: "account", titleKey: "winTitle.account" },
  { kind: "add", titleKey: "winTitle.add" },
  { kind: "add_modpack", titleKey: "winTitle.addModpack" },
  { kind: "add_resource", titleKey: "winTitle.addResource" },
  { kind: "collect", titleKey: "winTitle.collect" },
  { kind: "download", titleKey: "winTitle.download" },
  { kind: "java_download", titleKey: "winTitle.javaDownload" },
  { kind: "block", titleKey: "winTitle.block" },
  { kind: "log", titleKey: "winTitle.log" },
  { kind: "export", titleKey: "winTitle.export" },
  { kind: "config", titleKey: "winTitle.config" },
  { kind: "server_pack", titleKey: "winTitle.serverPack" },
  { kind: "server_instance", titleKey: "winTitle.serverInstance" },
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
