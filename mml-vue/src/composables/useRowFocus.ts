// 逐行表单的"点添加 → 新行自动聚焦"
//
// 行的输入框是 v-for 渲染的，点添加那一刻新的 DOM 还没挂上，不能直接 focus；
// 这里记下"下一行该聚焦哪一组第几行"，等行容器挂载时的 ref 回调再动手。
//
// 实例设置的启动参数面板与设置窗口的游戏启动共用这一份。
import { ref } from "vue";

/** 待聚焦的行：组名 + 行下标 */
interface PendingRow {
  /** 哪一组行（调用方自定义，如 `jvmArgs` / `env`） */
  kind: string;
  /** 行下标 */
  index: number;
}

/**
 * 行焦点辅助
 *
 * # 返回值
 *
 * - `focusNew`: 记下"新加的是哪一组第几行"，等它挂载后聚焦
 * - `setRowRef`: 行容器的 ref 回调，命中待聚焦行时把焦点送进它的第一个 input
 */
export function useRowFocus() {
  const pending = ref<PendingRow | null>(null);

  /**
   * 标记下一行要聚焦
   *
   * - `kind`: 组名，与 `setRowRef` 传入的一致
   * - `index`: 行下标
   */
  function focusNew(kind: string, index: number) {
    pending.value = { kind, index };
  }

  /**
   * 行容器的 ref 回调
   *
   * - `kind`: 组名
   * - `index`: 行下标
   * - `el`: 行元素；卸载时是 `null`
   */
  function setRowRef(kind: string, index: number, el: unknown) {
    const row = el as HTMLElement | null;
    if (!row) return;
    const target = pending.value;
    if (!target || target.kind !== kind || target.index !== index) return;
    pending.value = null;
    row.querySelector("input")?.focus();
  }

  return { focusNew, setRowRef };
}
