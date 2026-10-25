// 主窗口共享类型，MainWindow 与其子组件共用
import type { InstanceInfoDto } from "../../lib/bindings";

/** 实例列表显示模式；默认 list，来源见 lib/guiConfig.ts */
export type { ViewMode } from "../../lib/guiConfig";

/** 顶栏功能入口的 id：settings / stats / help / collect */
export type FeatureId = "settings" | "stats" | "help" | "collect";

export interface GroupView {
  /** 分组 uuid，身份字段；改名不影响它 */
  id: string;
  /** 分组显示名；默认分组已翻译成"默认分组" */
  name: string;
  /** 是否默认分组；默认分组的名字是空白，不能靠名字判断 */
  isDefault: boolean;
  /** 组内实例，顺序即显示顺序 */
  items: InstanceInfoDto[];
}

/** 右键菜单状态 */
export interface CtxMenuState {
  /** 菜单左上角位置，单位 px，相对视口 */
  x: number;
  /** 菜单左上角位置，单位 px，相对视口 */
  y: number;
  /** 右键落在哪一类目标上 */
  kind: "group" | "multi" | "instance";
  /** 分组 uuid；仅 kind 为 group 时有值 */
  groupId?: string;
  /** 被右键的实例；仅 kind 为 instance 时有值 */
  instance?: InstanceInfoDto;
}

/** 拖拽候选，实例行与分组标题都会发起 */
export interface DragCandidate {
  /** 这次拖的是实例还是分组标题 */
  kind: "instance" | "group";
  /** 被拖的实例；仅 kind 为 instance 时有值 */
  instance?: InstanceInfoDto;
  /** 分组 uuid；拖动与落点判断都用它 */
  groupId?: string;
  /** 分组显示名，只用于拖拽时那块占位上的文案 */
  groupName?: string;
}

/** 实例右键菜单的动作 id */
export type InstMenuAction =
  | "launch"
  | "viewMods"
  | "openFolder"
  | "viewLog"
  | "changeIcon"
  | "export"
  | "serverInstance"
  | "cloudSave"
  | "shortcut"
  | "copy"
  | "rename"
  | "moveGroup"
  | "delete";
