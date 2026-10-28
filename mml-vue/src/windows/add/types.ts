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

