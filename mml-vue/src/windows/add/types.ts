// 添加实例窗口的共享类型与常量
//
// 单独放一份是为了让「窗口」与「模式切换组件」共用同一套 ID 与清单，
// 避免把模式列表写进组件后再用字符串转来转去。

/** 四种添加模式 */
export type AddMode = "new" | "archive" | "folder" | "online";

/** 模式切换项的图标名，对应 IconTabs 内置的四个 svg */
export type AddModeIcon = "cube" | "box" | "folder" | "globe";

export interface AddModeTab {
  id: AddMode;
  /** 显示名文案键（走 i18n） */
  labelKey: string;
  icon: AddModeIcon;
}

export const ADD_MODES: AddModeTab[] = [
  { id: "new", labelKey: "add.modeNew", icon: "cube" },
  { id: "archive", labelKey: "add.modeArchive", icon: "box" },
  { id: "folder", labelKey: "add.modeFolder", icon: "folder" },
  { id: "online", labelKey: "add.modeOnline", icon: "globe" },
];

/**
 * 真·整合包：内核里走整合包安装 worker 的那两种 manifest
 *
 * 这两种压缩包导入时走「下载整合包」那条安装任务路径：进度在标题栏的整合包指示器上，
 * 装完自动出现并选中新实例；MMC / HMCL / 直接解压等只是"别人的包格式"，
 * 内核按普通压缩包解，没有在线安装那套元数据，见 add.rs 的 is_modpack。
 */
const MODPACK_PACK_TYPES = new Set(["curseforge", "modrinth"]);

/**
 * 该压缩包类型是否为整合包
 *
 * - `packType`: 压缩包类型 ID，如 `curseforge` / `modrinth` / `mmc`
 *
 * # 返回值
 *
 * 只有走内核整合包安装 worker 的两种 manifest 才为 `true`
 */
export function isModpackPackType(packType: string): boolean {
  return MODPACK_PACK_TYPES.has(packType);
}
