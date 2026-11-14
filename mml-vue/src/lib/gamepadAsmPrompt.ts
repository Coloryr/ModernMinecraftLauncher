// 游戏内手柄需要 ColorASM 的确认弹窗
//
// 启动流程共用：游戏内手柄开着、这次启动又不会注入 ColorASM 时挂起一次询问，
// 由 App 里常驻的 GamepadAsmModal 回答；选「不开启」照常启动，只是手柄在游戏里不生效。
import { ref } from "vue";

/** 弹窗是否打开 */
export const gamepadAsmOpen = ref(false);

/** 用户的选择 */
export type GamepadAsmAnswer = {
  /** 是否继续启动；false = 取消 */
  ok: boolean;
  /** 是否为实例打开 ColorASM */
  enable: boolean;
};

/** 等待回答的 promise 回调 */
let pending: ((answer: GamepadAsmAnswer) => void) | null = null;

/**
 * 打开 ColorASM 确认弹窗并等待用户选择
 *
 * # 返回值
 *
 * 见 [`GamepadAsmAnswer`]
 */
export function askGamepadAsm(): Promise<GamepadAsmAnswer> {
  // 上一问还没回答就来了新的一问：按「取消」放掉旧的，避免它的 Promise 永远不落地
  pending?.({ ok: false, enable: false });
  pending = null;

  return new Promise((resolve) => {
    pending = resolve;
    gamepadAsmOpen.value = true;
  });
}

/**
 * 关闭弹窗并回答等待方
 *
 * - `answer`: 用户的选择
 */
export function answerGamepadAsm(answer: GamepadAsmAnswer): void {
  gamepadAsmOpen.value = false;
  const resolve = pending;
  pending = null;
  resolve?.(answer);
}
