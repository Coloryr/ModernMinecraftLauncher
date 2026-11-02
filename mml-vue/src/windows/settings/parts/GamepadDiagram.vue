<script setup lang="ts">
// 手柄状态图：按外观画 Xbox / PlayStation 两套外形与记号，按键与摇杆实时高亮
//
// 两套外观共用一张模板，差别都在坐标表里：Xbox 是左上 / 右下错开的两只摇杆，
// PlayStation 是下方对称的两只摇杆，中间多一块触摸板。
// 只画状态，不负责轮询；快照由上层按固定间隔取好传进来。
import type {
  GamepadAxisDto,
  GamepadButtonDto,
  GamepadExtraDto,
  GamepadLayoutDto,
  GamepadStateDto,
} from "../../../lib/bindings";
import { computed } from "vue";
import { t } from "../../../lib/i18n";
import { buttonLabel } from "../gamepadLabels";

const props = defineProps<{
  /** 当前手柄快照；null 表示读取失败或已断开 */
  state: GamepadStateDto | null;
  /** 外观：xbox / playstation */
  layout: GamepadLayoutDto;
  /** 是否交换 A / B 两个位置 */
  swapAb: boolean;
}>();

/** 一个点的坐标，单位与 SVG 的 viewBox 一致 */
interface Point {
  x: number;
  y: number;
}

/** 一套手柄外形的摆位 */
interface PadGeometry {
  /** 机身 */
  body: { x: number; y: number; w: number; h: number; rx: number };
  /** 左右两侧扳机 / 肩键的横条 */
  bumperLeft: { x: number; w: number };
  bumperRight: { x: number; w: number };
  /** 扳机与肩键的纵坐标与高度 */
  triggerY: number;
  triggerH: number;
  shoulderY: number;
  shoulderH: number;
  leftStick: Point;
  rightStick: Point;
  /** 十字键中心 */
  dpad: Point;
  /** 四个面键的中心与到中心的偏移 */
  face: Point;
  faceOffset: number;
  faceRadius: number;
  select: Point;
  start: Point;
  /** 触摸板；Xbox 没有 */
  touch: { x: number; y: number; w: number; h: number } | null;
}

/** Xbox：摇杆左上 / 右下错开，和实体手柄一致 */
const XBOX: PadGeometry = {
  body: { x: 20, y: 54, w: 340, h: 168, rx: 64 },
  bumperLeft: { x: 58, w: 80 },
  bumperRight: { x: 242, w: 80 },
  triggerY: 12,
  triggerH: 20,
  shoulderY: 36,
  shoulderH: 16,
  leftStick: { x: 102, y: 112 },
  rightStick: { x: 282, y: 184 },
  dpad: { x: 102, y: 182 },
  face: { x: 282, y: 104 },
  faceOffset: 30,
  faceRadius: 13,
  select: { x: 158, y: 156 },
  start: { x: 226, y: 156 },
  touch: null,
};

/** PlayStation：两只摇杆并排在下方，中间一块触摸板 */
const PLAYSTATION: PadGeometry = {
  body: { x: 20, y: 54, w: 340, h: 168, rx: 64 },
  bumperLeft: { x: 58, w: 80 },
  bumperRight: { x: 242, w: 80 },
  triggerY: 12,
  triggerH: 20,
  shoulderY: 36,
  shoulderH: 16,
  leftStick: { x: 128, y: 186 },
  rightStick: { x: 252, y: 186 },
  dpad: { x: 92, y: 116 },
  face: { x: 288, y: 112 },
  faceOffset: 28,
  faceRadius: 13,
  select: { x: 146, y: 84 },
  start: { x: 250, y: 84 },
  touch: { x: 160, y: 70, w: 80, h: 36 },
};

/** 当前外观的摆位 */
const geo = computed<PadGeometry>(() => (props.layout === "playstation" ? PLAYSTATION : XBOX));

/** 固定按钮表以外的按键：高级手柄背面的拨片这类，按底层编号显示 */
const extras = computed<GamepadExtraDto[]>(() => props.state?.extras ?? []);

/** 按钮按住的程度，0 到 1 */
function value(button: GamepadButtonDto): number {
  return props.state?.buttons.find((item) => item.button === button)?.value ?? 0;
}

/** 按钮是否算按下 */
function pressed(button: GamepadButtonDto): boolean {
  return value(button) >= 0.5;
}

/** 轴值，-1 到 1，向上为正 */
function axis(name: GamepadAxisDto): number {
  return props.state?.axes.find((item) => item.axis === name)?.value ?? 0;
}

/** 按当前外观与 A / B 交换取记号 */
function label(button: GamepadButtonDto): string {
  return buttonLabel(button, props.layout, props.swapAb);
}

/** 扳机进度条宽度，单位与 SVG 的 viewBox 一致 */
function triggerWidth(button: GamepadButtonDto): number {
  return (geo.value.bumperLeft.w - 8) * Math.min(1, Math.max(0, value(button)));
}

/** 横条的几何中心，给标签定位 */
function midX(item: { x: number; w: number }): number {
  return item.x + item.w / 2;
}

/** 四个面键，按模板里的固定顺序排 */
const FACE_SIDES = ["north", "east", "south", "west"] as const;

/** 面键的四个位置 */
function facePoint(side: (typeof FACE_SIDES)[number]): Point {
  const { face, faceOffset: offset } = geo.value;
  switch (side) {
    case "north":
      return { x: face.x, y: face.y - offset };
    case "east":
      return { x: face.x + offset, y: face.y };
    case "south":
      return { x: face.x, y: face.y + offset };
    default:
      return { x: face.x - offset, y: face.y };
  }
}

/** 十字键的十字底座：以中心为原点，臂长 26、臂宽 18 */
function dpadPath(): string {
  const { x, y } = geo.value.dpad;
  return `M${x - 9} ${y - 26}h18v17h17v18h-17v17h-18v-17h-17v-18h17z`;
}

/** 十字键四个方向的按键块，位置跟着中心走 */
function dpadArm(side: "up" | "down" | "left" | "right"): { x: number; y: number } {
  const { x, y } = geo.value.dpad;
  switch (side) {
    case "up":
      return { x: x - 7, y: y - 24 };
    case "down":
      return { x: x - 7, y: y + 10 };
    case "left":
      return { x: x - 24, y: y - 7 };
    default:
      return { x: x + 10, y: y - 7 };
  }
}
</script>

<template>
  <div class="pad-wrap">
    <svg class="pad" viewBox="0 0 380 260" role="img" :aria-label="t('winGamepad.testTitle')">
      <!-- 机身：一条圆角长条，够放两侧的摇杆与按键 -->
      <rect class="pad-body" :x="geo.body.x" :y="geo.body.y" :width="geo.body.w" :height="geo.body.h"
        :rx="geo.body.rx" />

      <!-- 触摸板：只有 PlayStation 有，纯粹是外形的一部分，不接输入 -->
      <rect v-if="geo.touch" class="pad-touch" :x="geo.touch.x" :y="geo.touch.y" :width="geo.touch.w"
        :height="geo.touch.h" rx="10" />

      <!-- 扳机：键程用底部细条表示，标签不压在填充上 -->
      <g>
        <rect class="pad-key" :class="{ on: pressed('leftTrigger') }" :x="geo.bumperLeft.x"
          :y="geo.triggerY" :width="geo.bumperLeft.w" :height="geo.triggerH" rx="9" />
        <rect class="pad-fill" :x="geo.bumperLeft.x + 2" :y="geo.triggerY + geo.triggerH - 7"
          :width="triggerWidth('leftTrigger')" height="3" rx="1.5" />
        <text class="pad-cap" :class="{ on: pressed('leftTrigger') }" :x="midX(geo.bumperLeft)"
          :y="geo.triggerY + geo.triggerH / 2">{{ label("leftTrigger") }}</text>
        <rect class="pad-key" :class="{ on: pressed('rightTrigger') }" :x="geo.bumperRight.x"
          :y="geo.triggerY" :width="geo.bumperRight.w" :height="geo.triggerH" rx="9" />
        <rect class="pad-fill" :x="geo.bumperRight.x + 2" :y="geo.triggerY + geo.triggerH - 7"
          :width="triggerWidth('rightTrigger')" height="3" rx="1.5" />
        <text class="pad-cap" :class="{ on: pressed('rightTrigger') }" :x="midX(geo.bumperRight)"
          :y="geo.triggerY + geo.triggerH / 2">{{ label("rightTrigger") }}</text>
      </g>

      <!-- 肩键 -->
      <g>
        <rect class="pad-key" :class="{ on: pressed('leftTrigger2') }" :x="geo.bumperLeft.x"
          :y="geo.shoulderY" :width="geo.bumperLeft.w" :height="geo.shoulderH" rx="8" />
        <text class="pad-cap" :class="{ on: pressed('leftTrigger2') }" :x="midX(geo.bumperLeft)"
          :y="geo.shoulderY + geo.shoulderH / 2">{{ label("leftTrigger2") }}</text>
        <rect class="pad-key" :class="{ on: pressed('rightTrigger2') }" :x="geo.bumperRight.x"
          :y="geo.shoulderY" :width="geo.bumperRight.w" :height="geo.shoulderH" rx="8" />
        <text class="pad-cap" :class="{ on: pressed('rightTrigger2') }" :x="midX(geo.bumperRight)"
          :y="geo.shoulderY + geo.shoulderH / 2">{{ label("rightTrigger2") }}</text>
      </g>

      <!-- 左摇杆：外圈按下（L3）时高亮，内点按轴偏移 -->
      <g>
        <circle class="pad-stick" :class="{ on: pressed('leftThumb') }" :cx="geo.leftStick.x"
          :cy="geo.leftStick.y" r="20" />
        <circle class="pad-dot" :cx="geo.leftStick.x + axis('leftStickX') * 9"
          :cy="geo.leftStick.y - axis('leftStickY') * 9" r="7" />
      </g>

      <!-- 十字键：底下一个十字，四个方向的键单独叠上去，按下就点亮 -->
      <g>
        <path class="pad-key" :d="dpadPath()" />
        <rect class="pad-arm" :class="{ on: pressed('dPadUp') }" :x="dpadArm('up').x" :y="dpadArm('up').y"
          width="14" height="14" rx="3" />
        <rect class="pad-arm" :class="{ on: pressed('dPadDown') }" :x="dpadArm('down').x"
          :y="dpadArm('down').y" width="14" height="14" rx="3" />
        <rect class="pad-arm" :class="{ on: pressed('dPadLeft') }" :x="dpadArm('left').x"
          :y="dpadArm('left').y" width="14" height="14" rx="3" />
        <rect class="pad-arm" :class="{ on: pressed('dPadRight') }" :x="dpadArm('right').x"
          :y="dpadArm('right').y" width="14" height="14" rx="3" />
      </g>

      <!-- 功能键只显示 View / Menu -->
      <g>
        <circle class="pad-key" :class="{ on: pressed('select') }" :cx="geo.select.x" :cy="geo.select.y"
          r="10" />
        <circle class="pad-key" :class="{ on: pressed('start') }" :cx="geo.start.x" :cy="geo.start.y"
          r="10" />
        <text class="pad-cap pad-cap-key" :class="{ on: pressed('select') }" :x="geo.select.x"
          :y="geo.select.y">{{ label("select")[0] }}</text>
        <text class="pad-cap pad-cap-key" :class="{ on: pressed('start') }" :x="geo.start.x"
          :y="geo.start.y">{{ label("start")[0] }}</text>
      </g>

      <!-- 右摇杆 -->
      <g>
        <circle class="pad-stick" :class="{ on: pressed('rightThumb') }" :cx="geo.rightStick.x"
          :cy="geo.rightStick.y" r="20" />
        <circle class="pad-dot" :cx="geo.rightStick.x + axis('rightStickX') * 9"
          :cy="geo.rightStick.y - axis('rightStickY') * 9" r="7" />
      </g>

      <!-- 右侧四键：位置由外观决定，记号跟着外观与 A / B 交换走 -->
      <g>
        <circle v-for="side in FACE_SIDES" :key="side" class="pad-key" :class="{ on: pressed(side) }"
          :cx="facePoint(side).x" :cy="facePoint(side).y" :r="geo.faceRadius" />
        <text v-for="side in FACE_SIDES" :key="side" class="pad-label" :class="{ on: pressed(side) }"
          :x="facePoint(side).x" :y="facePoint(side).y">{{ label(side) }}</text>
      </g>
    </svg>

    <!-- 固定按钮表以外的按键：只报编号，按下时点亮，方便对着手柄认是哪一颗 -->
    <div v-if="extras.length" class="pad-extra">
      <span class="pad-extra-title">{{ t("winGamepad.extraButtons") }}</span>
      <span v-for="item in extras" :key="item.code" class="pad-extra-key"
        :class="{ on: item.value >= 0.5 }">{{ item.code }}</span>
    </div>
  </div>
</template>

<style scoped>
/* 手柄图与下面那排额外按键上下排开 */
.pad-wrap {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.pad {
  display: block;
  width: 100%;
  max-width: 460px;
  margin: 0 auto;
}

.pad-body {
  fill: var(--bg-hover);
  stroke: var(--border);
}

/* 触摸板：比机身深一档的底，只用来区分 PS 手柄的外形 */
.pad-touch {
  fill: var(--bg-card);
  stroke: var(--border);
}

.pad-key {
  fill: var(--bg-card);
  stroke: var(--border);
  transition: fill 0.12s, stroke 0.12s;
}

/* 按下：底色与描边一起换成强调色 */
.pad-key.on {
  fill: var(--accent);
  stroke: var(--accent);
}

.pad-fill {
  fill: var(--accent);
}

/* 十字键的方向键平时透明，只借用下面那条十字的底色 */
.pad-arm {
  fill: transparent;
  transition: fill 0.12s;
}

.pad-arm.on {
  fill: var(--accent);
}

.pad-stick {
  fill: var(--bg-card);
  stroke: var(--border);
}

.pad-stick.on {
  stroke: var(--accent);
}

.pad-dot {
  fill: var(--accent);
}

.pad-cap,
.pad-label {
  fill: var(--text-dim);
  font-size: 9px;
  text-anchor: middle;
  dominant-baseline: central;
  pointer-events: none;
}

.pad-label {
  font-size: 12px;
  font-weight: 700;
}

/* View / Menu 两颗比扳机上的小字大一档，缩到设置页里才看得清 */
.pad-cap-key {
  font-size: 11px;
  font-weight: 600;
}

/* 键按下时记号压在强调色上，换成卡片底色才看得清 */
.pad-cap.on,
.pad-label.on {
  fill: var(--bg-card);
}

/* 额外按键：一行编号胶囊，按下时整颗点亮 */
.pad-extra {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.pad-extra-title {
  font-size: 11.5px;
  color: var(--text-dim);
}

.pad-extra-key {
  min-width: 32px;
  padding: 2px 8px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-card);
  color: var(--text);
  font-size: 11.5px;
  text-align: center;
  transition: border-color 0.12s, background 0.12s, color 0.12s;
}

.pad-extra-key.on {
  border-color: var(--accent);
  background: var(--accent);
  color: var(--bg-card);
}
</style>
