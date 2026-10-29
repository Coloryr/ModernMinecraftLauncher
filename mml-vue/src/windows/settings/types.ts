// 设置窗口的共享类型与标签清单
//
// 标签清单是"唯一真源"：左侧导航、当前页大标题与说明、以及设置项搜索索引都从这里取。

export type SettingsTab =
  | "ui"
  | "java"
  | "network"
  | "launch"
  | "gamepad"
  | "skin"
  | "client"
  | "files"
  | "experimental";

export type SettingsTabIcon =
  | "palette"
  | "coffee"
  | "download"
  | "play"
  | "gamepad"
  | "user"
  | "gear"
  | "folder"
  | "flask";

/** 左侧导航的一项 */
export interface SettingsTabItem {
  /** 标签 id，同时是搜索索引与文案键的后半段 */
  id: SettingsTab;
  /** 导航图标名，取自 GlyphIcon */
  icon: SettingsTabIcon;
}

/** 标签顺序 = 左侧导航顺序 */
export const SETTINGS_TABS: SettingsTabItem[] = [
  { id: "ui", icon: "palette" },
  { id: "java", icon: "coffee" },
  { id: "network", icon: "download" },
  { id: "launch", icon: "play" },
  { id: "gamepad", icon: "gamepad" },
  { id: "skin", icon: "user" },
  { id: "client", icon: "gear" },
  { id: "files", icon: "folder" },
  { id: "experimental", icon: "flask" },
];

/**
 * 判断任意值是不是合法的标签 id
 *
 * - `v`: 待判断的值，来自本地存储等不可信来源
 *
 * # 返回值
 *
 * 命中清单里的标签时为 `true`
 */
export function isSettingsTab(v: unknown): v is SettingsTab {
  return SETTINGS_TABS.some((tab) => tab.id === v);
}
