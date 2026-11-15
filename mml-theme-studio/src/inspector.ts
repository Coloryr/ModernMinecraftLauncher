// 预览页回传给编辑器的元素检查数据。
//
// 预览与编辑器跨源，DOM、样式和规则都只能由注入预览页的脚本读出来、postMessage 回来；
// 这里只定义线上形状，不碰 DOM。

/** 一条样式声明 */
export interface InspectDeclaration {
  /** 属性名，如 color / --accent */
  name: string;
  /** 原始值 */
  value: string;
  /** 是否带 !important */
  important: boolean;
}

/** 一条命中的 CSS 规则 */
export interface InspectRule {
  /** 选择器原文 */
  selector: string;
  /** 这条规则的声明；跨源样式表读不到时为空 */
  declarations: InspectDeclaration[];
}

/** 元素在继承链上的一节，用于面包屑 */
export interface InspectPathItem {
  /** 小写标签名 */
  tag: string;
  /** id，没有时为空串 */
  id: string;
  /** class 列表，最多前四个 */
  classes: string[];
  /** 从 documentElement 起的子节点下标路径，用于选中祖先 */
  path: number[];
}

/** DOM 树里的一个节点 */
export interface InspectTreeNode {
  /** 从 documentElement 起的子节点下标路径 */
  path: number[];
  /** 小写标签名 */
  tag: string;
  /** id，没有时为空串 */
  id: string;
  /** class 列表，最多前四个 */
  classes: string[];
  /** 子元素个数；超过序列化上限时 children 可能不全 */
  childCount: number;
  /** 子节点，已按文档顺序展开 */
  children: InspectTreeNode[];
}

/** 预览页里被选中的元素 */
export interface InspectNode {
  /** 从 documentElement 起的子节点下标路径 */
  path: number[];
  /** 小写标签名 */
  tag: string;
  /** id，没有时为空串 */
  id: string;
  /** 全部 class */
  classes: string[];
  /** 渲染宽度，单位像素 */
  width: number;
  /** 渲染高度，单位像素 */
  height: number;
  /** 唯一选择器，带 nth-of-type */
  selector: string;
  /** 从 html 到该元素的路径 */
  ancestors: InspectPathItem[];
  /** 行内样式 */
  inline: InspectDeclaration[];
  /** 命中的 CSS 规则 */
  rules: InspectRule[];
  /** 常用计算样式；值为默认值的不列 */
  styles: { name: string; value: string }[];
  /** 该元素上可用的 CSS 变量，最多 80 条 */
  variables: { name: string; value: string }[];
}

/** 预览页与编辑器之间的检查协议版本 */
export const INSPECT_PROTOCOL = 1;
