<script setup lang="ts">
// 手柄状态图：按外观画 Xbox / PlayStation 两套记号，按键与摇杆实时高亮
//
// 只画状态，不负责轮询；快照由上层按固定间隔取好传进来，没有选中设备时调用方不渲染。
import type {
  GamepadAxisDto,
  GamepadButtonDto,
  GamepadLayoutDto,
  GamepadStateDto,
} from "../../../lib/bindings";
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
  return 66 * Math.min(1, Math.max(0, value(button)));
}
</script>

<template>
  <svg class="pad" viewBox="0 0 360 220" role="img" :aria-label="t('winGamepad.testTitle')">
    <!-- 机身：一条圆角长条，够放两侧的摇杆与按键 -->
    <rect class="pad-body" x="30" y="46" width="300" height="144" rx="58" />

    <!-- 扳机：键程用底部细条表示，标签不压在填充上 -->
    <g>
      <rect class="pad-key" :class="{ on: pressed('leftTrigger') }" x="60" y="10" width="70" height="18" rx="8" />
      <rect class="pad-fill" x="62" y="23" :width="triggerWidth('leftTrigger')" height="3" rx="1.5" />
      <text class="pad-cap" :class="{ on: pressed('leftTrigger') }" x="95" y="18">{{ label("leftTrigger") }}</text>
      <rect class="pad-key" :class="{ on: pressed('rightTrigger') }" x="230" y="10" width="70" height="18" rx="8" />
      <rect class="pad-fill" x="232" y="23" :width="triggerWidth('rightTrigger')" height="3" rx="1.5" />
      <text class="pad-cap" :class="{ on: pressed('rightTrigger') }" x="265" y="18">{{ label("rightTrigger") }}</text>
    </g>

    <!-- 肩键 -->
    <g>
      <rect class="pad-key" :class="{ on: pressed('leftTrigger2') }" x="60" y="32" width="70" height="14" rx="7" />
      <text class="pad-cap" :class="{ on: pressed('leftTrigger2') }" x="95" y="39">{{ label("leftTrigger2") }}</text>
      <rect class="pad-key" :class="{ on: pressed('rightTrigger2') }" x="230" y="32" width="70" height="14" rx="7" />
      <text class="pad-cap" :class="{ on: pressed('rightTrigger2') }" x="265" y="39">{{ label("rightTrigger2") }}</text>
    </g>

    <!-- 左摇杆：外圈按下（L3）时高亮，内点按轴偏移 -->
    <g>
      <circle class="pad-stick" :class="{ on: pressed('leftThumb') }" cx="104" cy="96" r="19" />
      <circle class="pad-dot" :cx="104 + axis('leftStickX') * 9" :cy="96 - axis('leftStickY') * 9" r="7" />
    </g>

    <!-- 十字键：底下一个十字，四个方向的键单独叠上去，按下就点亮 -->
    <g>
      <path class="pad-key" d="M85 126h18v17h17v18H103v17H85v-17H68v-18h17z" />
      <rect class="pad-arm" :class="{ on: pressed('dPadUp') }" x="87" y="128" width="14" height="14" rx="3" />
      <rect class="pad-arm" :class="{ on: pressed('dPadDown') }" x="87" y="162" width="14" height="14" rx="3" />
      <rect class="pad-arm" :class="{ on: pressed('dPadLeft') }" x="70" y="145" width="14" height="14" rx="3" />
      <rect class="pad-arm" :class="{ on: pressed('dPadRight') }" x="104" y="145" width="14" height="14" rx="3" />
    </g>

    <!-- 功能键：View / Guide / Menu，外观只换记号 -->
    <g>
      <circle class="pad-key" :class="{ on: pressed('select') }" cx="152" cy="120" r="7" />
      <circle class="pad-key" :class="{ on: pressed('mode') }" cx="180" cy="98" r="8" />
      <circle class="pad-key" :class="{ on: pressed('start') }" cx="208" cy="120" r="7" />
      <text class="pad-cap" :class="{ on: pressed('select') }" x="152" y="120">{{ label("select")[0] }}</text>
      <text class="pad-cap" :class="{ on: pressed('mode') }" x="180" y="98">{{ label("mode")[0] }}</text>
      <text class="pad-cap" :class="{ on: pressed('start') }" x="208" y="120">{{ label("start")[0] }}</text>
    </g>

    <!-- 右摇杆 -->
    <g>
      <circle class="pad-stick" :class="{ on: pressed('rightThumb') }" cx="252" cy="152" r="19" />
      <circle class="pad-dot" :cx="252 + axis('rightStickX') * 9" :cy="152 - axis('rightStickY') * 9" r="7" />
    </g>

    <!-- 右侧四键：位置固定，记号跟着外观与 A / B 交换走 -->
    <g>
      <circle class="pad-key" :class="{ on: pressed('north') }" cx="262" cy="68" r="13" />
      <circle class="pad-key" :class="{ on: pressed('east') }" cx="290" cy="96" r="13" />
      <circle class="pad-key" :class="{ on: pressed('south') }" cx="262" cy="124" r="13" />
      <circle class="pad-key" :class="{ on: pressed('west') }" cx="234" cy="96" r="13" />
      <text class="pad-label" :class="{ on: pressed('north') }" x="262" y="68">{{ label("north") }}</text>
      <text class="pad-label" :class="{ on: pressed('east') }" x="290" y="96">{{ label("east") }}</text>
      <text class="pad-label" :class="{ on: pressed('south') }" x="262" y="124">{{ label("south") }}</text>
      <text class="pad-label" :class="{ on: pressed('west') }" x="234" y="96">{{ label("west") }}</text>
    </g>
  </svg>
</template>

<style scoped>
.pad {
  display: block;
  width: 100%;
  max-width: 420px;
  margin: 0 auto;
}

.pad-body {
  fill: var(--bg-hover);
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

/* 键按下时记号压在强调色上，换成卡片底色才看得清 */
.pad-cap.on,
.pad-label.on {
  fill: var(--bg-card);
}
</style>