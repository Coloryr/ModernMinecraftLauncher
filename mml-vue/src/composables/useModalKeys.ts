// 弹窗期间的 document 键盘接管
//
// `BaseModal` 自己不处理 Esc（也不做焦点管理），而弹窗内容被 Teleport 到 body，
// 事件不会冒泡到某个组件根上，所以只能在 document 上听。
// 用 `v-if` 控制显隐的弹窗在挂载/卸载时正好等于「可见期」，因此这里直接绑 document，
// 由组件的挂载与销毁负责收尾 —— 不会出现「弹窗关了键盘还在接管」。
//
// ```ts
// useModalKeys((e) => {
//   if (e.key === "Escape") emit("close");
//   else if (e.key === "ArrowLeft") emit("prev");
// });
// ```
import { onMounted, onUnmounted } from "vue";

/** 当前挂载中的弹窗栈，后进先出；只有栈顶能响应 Esc */
const stack: symbol[] = [];

/**
 * 弹窗挂载期间接管 document 的键盘事件
 *
 * 同一个窗口可能叠两个弹窗（如详情弹窗上再开确认框），两个处理器都绑在 document 上
 * 会让一次 Esc 关掉两层。这里按挂载顺序维护栈，只让最后打开的那个响应。
 *
 * - `handler`: `keydown` 处理器；弹窗卸载时自动解绑
 */
export function useModalKeys(handler: (e: KeyboardEvent) => void) {
  const id = Symbol("modal-keys");
  const onKey = (e: KeyboardEvent) => {
    if (stack[stack.length - 1] !== id) return;
    handler(e);
  };

  onMounted(() => {
    stack.push(id);
    document.addEventListener("keydown", onKey);
  });
  onUnmounted(() => {
    document.removeEventListener("keydown", onKey);
    const index = stack.indexOf(id);
    if (index >= 0) stack.splice(index, 1);
  });
}
