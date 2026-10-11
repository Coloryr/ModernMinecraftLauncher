// 设置窗口 · 统一的保存状态
//
// 这个窗口原来混着四套保存时机：即改即存 / 500ms 防抖 / 显式按钮 / 拖滑杆每次都存，
// 用户看不到"到底存了没"。这里给每个区域一份统一的状态：
// idle → saving → saved 短暂停留后回 idle；error 停留久一点。
//
// 用法：
// ```ts
// const { state, track } = useSaveState();
// async function apply() { await track(() => commands.settings.saveXxx(...)); }
// ```
import { onUnmounted, ref } from "vue";

export type SaveState = "idle" | "saving" | "saved" | "error";

/** saved / error 各自停留多久后回到 idle，单位毫秒 */
const SAVED_HOLD = 1600;
const ERROR_HOLD = 4000;

/**
 * 统一的保存状态机
 *
 * # 返回值
 *
 * - `state`: 当前状态，`idle` / `saving` / `saved` / `error`
 * - `track`: 包一次保存动作，成功转 `saved`、抛错转 `error` 并原样抛出
 * - `markError`: 显式标记失败，用于校验没过、没发请求的场景
 * - `markPending`: 有改动但还在防抖窗口内
 * - `reset`: 清掉定时器并回到 `idle`
 */
export function useSaveState() {
  const state = ref<SaveState>("idle");
  let timer: number | null = null;

  function clearTimer() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  /** 让当前状态停留一会儿再回到 idle */
  function hold(ms: number) {
    clearTimer();
    timer = window.setTimeout(() => {
      timer = null;
      state.value = "idle";
    }, ms);
  }

  /**
   * 包一次保存动作：先把状态置 saving，成功转 saved，抛错转 error 并原样抛出
   */
  async function track<T>(run: () => Promise<T>): Promise<T> {
    state.value = "saving";
    try {
      const result = await run();
      state.value = "saved";
      hold(SAVED_HOLD);
      return result;
    } catch (e) {
      state.value = "error";
      hold(ERROR_HOLD);
      throw e;
    }
  }

  /** 显式标记失败：校验没过、根本没发请求的那种 */
  function markError() {
    state.value = "error";
    hold(ERROR_HOLD);
  }

  /** 有改动但还没到保存时机（防抖窗口内） */
  function markPending() {
    clearTimer();
    state.value = "saving";
  }

  function reset() {
    clearTimer();
    state.value = "idle";
  }

  // 关窗时清掉未触发的状态回落定时器
  onUnmounted(clearTimer);

  return { state, track, markError, markPending, reset };
}
