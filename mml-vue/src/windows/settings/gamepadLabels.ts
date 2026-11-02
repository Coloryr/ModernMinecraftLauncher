// 手柄按钮在界面上的记号：按外观（Xbox / PlayStation）与 A / B 交换取
//
// 映射本身按位置（south / east / north / west）走，这里只决定显示成哪个记号，
// 不影响后端按位置编译出的规则。
import type { GamepadAxisDto, GamepadButtonDto, GamepadLayoutDto } from "../../lib/bindings";

/** Xbox 布局的按钮记号 */
const XBOX: Record<GamepadButtonDto, string> = {
  south: "A",
  east: "B",
  north: "Y",
  west: "X",
  c: "C",
  z: "Z",
  leftTrigger: "LT",
  leftTrigger2: "LB",
  rightTrigger: "RT",
  rightTrigger2: "RB",
  select: "View",
  start: "Menu",
  mode: "Guide",
  leftThumb: "L3",
  rightThumb: "R3",
  dPadUp: "↑",
  dPadDown: "↓",
  dPadLeft: "←",
  dPadRight: "→",
};

/** PlayStation 布局的按钮记号 */
const PLAYSTATION: Record<GamepadButtonDto, string> = {
  south: "✕",
  east: "○",
  north: "△",
  west: "□",
  c: "C",
  z: "Z",
  leftTrigger: "L2",
  leftTrigger2: "L1",
  rightTrigger: "R2",
  rightTrigger2: "R1",
  select: "Share",
  start: "Options",
  mode: "PS",
  leftThumb: "L3",
  rightThumb: "R3",
  dPadUp: "↑",
  dPadDown: "↓",
  dPadLeft: "←",
  dPadRight: "→",
};

/** 轴的记号，两套外观一样 */
const AXES: Record<GamepadAxisDto, string> = {
  leftStickX: "LS X",
  leftStickY: "LS Y",
  leftZ: "LT",
  rightStickX: "RS X",
  rightStickY: "RS Y",
  rightZ: "RT",
  dPadX: "DPad X",
  dPadY: "DPad Y",
};

/** 全部按钮，按配置表的排列顺序 */
export const ALL_BUTTONS = Object.keys(XBOX) as GamepadButtonDto[];

/** 全部轴，按配置表的排列顺序 */
export const ALL_AXES = Object.keys(AXES) as GamepadAxisDto[];

/**
 * 按钮的显示记号
 *
 * - `button`: 手柄按钮
 * - `layout`: 外观，xbox / playstation
 * - `swapAb`: 交换 A / B 两个位置，Nintendo 布局习惯是 A 在右、B 在下
 *
 * # 返回值
 *
 * 如 `A` / `✕`；A / B 交换时 south 与 east 的记号对调
 */
export function buttonLabel(
  button: GamepadButtonDto,
  layout: GamepadLayoutDto,
  swapAb: boolean,
): string {
  const table = layout === "playstation" ? PLAYSTATION : XBOX;
  if (swapAb) {
    if (button === "south") return table.east;
    if (button === "east") return table.south;
  }
  return table[button];
}

/**
 * 轴的显示记号
 *
 * - `axis`: 手柄轴
 *
 * # 返回值
 *
 * 如 `LS X`；没有中英差异，设备标识不翻译
 */
export function axisLabel(axis: GamepadAxisDto): string {
  return AXES[axis];
}