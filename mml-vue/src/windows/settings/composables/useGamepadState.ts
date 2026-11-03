// 手柄实时状态：设置页里需要读数的地方共用（轴区间条这类）
//
// 只做采样，不发输入；和手柄面板一样按 120ms 拉一次，页面不在前台时跳过。
import { onBeforeUnmount, onMounted, ref } from "vue";
import { commands, type GamepadAxisDto, type GamepadDto, type GamepadStateDto } from "../../../lib/bindings";
import { usePageActive } from "../../../lib/pageActive";
import { isTauri } from "../../windowManager";

/** 采样间隔，单位毫秒 */
const TICK = 120;

/**
 * 手柄实时状态
 *
 * # 返回值
 *
 * - `devices`: 当前连接的手柄列表
 * - `state`: 第一只手柄的最新快照，没有手柄时为 null
 * - `axisValue`: 按轴名取当前值，没有数据时按 0
 */
export function useGamepadState() {
  const devices = ref<GamepadDto[]>([]);
  const state = ref<GamepadStateDto | null>(null);
  const active = usePageActive();
  let disposed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  /** 按轴名取当前值 */
  function axisValue(axis: GamepadAxisDto): number {
    return state.value?.axes.find((item) => item.axis === axis)?.value ?? 0;
  }

  /** 拉一帧 */
  async function poll() {
    if (disposed || !isTauri()) return;
    if (active.value) {
      try {
        const list = await commands.gamepad.list();
        const id = list[0]?.id ?? null;
        devices.value = list;
        state.value = id === null ? null : await commands.gamepad.state(id);
      } catch {
        state.value = null;
      }
    }
    if (!disposed) timer = setTimeout(() => void poll(), TICK);
  }

  onMounted(() => {
    disposed = false;
    void poll();
  });

  onBeforeUnmount(() => {
    disposed = true;
    if (timer) clearTimeout(timer);
  });

  return { devices, state, axisValue };
}
