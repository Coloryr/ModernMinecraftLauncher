// 方块列表窗口的共享类型与网格尺寸档
//
// 尺寸档里的 `cell` 是**固定单元格高度**：虚拟滚动按「cell + gap」算行高，
// 所以这里改了数值必须与 BlockGrid 里的实际渲染高度一致（模板用 CSS 变量注入）。

export type BlockSize = "sm" | "md" | "lg";

export interface BlockSizeMetrics {
  /** 网格列最小宽度（自适应列数） */
  minCol: number;
  /** 图标边长 */
  img: number;
  /** 单元格固定高度 */
  cell: number;
  /** 单元格间距 */
  gap: number;
  /** 单元格内边距 */
  pad: number;
  /** 方块名字号 */
  font: number;
}

export const BLOCK_SIZES: Record<BlockSize, BlockSizeMetrics> = {
  sm: { minCol: 78, img: 40, cell: 84, gap: 8, pad: 8, font: 11 },
  md: { minCol: 96, img: 56, cell: 100, gap: 10, pad: 10, font: 11.5 },
  lg: { minCol: 136, img: 88, cell: 136, gap: 12, pad: 12, font: 12.5 },
};

/** 尺寸档顺序（工具条分段控件用） */
export const BLOCK_SIZE_ORDER: readonly BlockSize[] = ["sm", "md", "lg"];

/**
 * 判断任意值是不是合法的尺寸档
 *
 * - `v`: 待判断的值，来自本地存储等不可信来源
 *
 * # 返回值
 *
 * 命中三档之一时为 `true`
 */
export function isBlockSize(v: unknown): v is BlockSize {
  return v === "sm" || v === "md" || v === "lg";
}

/**
 * 玩家头颅的分类 ID
 *
 * 值沿用 crate 里的 `playerSkin`，`block/skin.rs` 已把它写进 `block.json` 的渲染结果，
 * 改名会让旧数据对不上，**只改显示文案**为「玩家头颅」。
 */
export const SKIN_CAT = "playerSkin";

/**
 * 自建分组的分类键前缀
 *
 * 左栏选中的 `cat` 既可能是游戏创造分类名（如 `buildingBlocks`），也可能是自建分组，
 * 用这个前缀区分，见 useBlockGroups 的 `groupKey`。
 */
export const GROUP_PREFIX = "g:";
