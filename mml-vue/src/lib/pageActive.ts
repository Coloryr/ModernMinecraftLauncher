// 「本页面是否在前台」
//
// 单窗口模式下 App.vue 用 `<KeepAlive>` 缓存窗口组件：切走只是 deactivate ——
// 组件不销毁、状态都还在，这正是"回到那一页弹窗还在"的前提，但页面自己那份 DOM
// 会被挪进缓存容器，而 **`Teleport to="body"` 的浮层留在原地照常显示**。
//
// 于是异步流程回来时（整合包装完、后端要重名答复…）会出现两个问题：
// 1. 弹窗盖在当前显示的别的页面上；
// 2. 在那个弹窗上点下去，执行上下文仍是原页面 —— "是否继续添加"的"否"走的是
//    `closeWindow()`，此时当前页是主页面，于是被当成"关掉主窗口"＝退出启动器。
//
// 判据与 Vue 自己那套一致：沿组件父链看有没有祖先处于停用状态。
// `onActivated` / `onDeactivated` 会被 Vue 注入到缓存根上，所以组件嵌多深都收得到；
// 详见 runtime-core 的 `registerKeepAliveHook` / `injectToKeepAliveRoot`。
import {
  getCurrentInstance,
  onActivated,
  onDeactivated,
  ref,
  type ComponentInternalInstance,
  type Ref,
} from "vue";

/** 沿父链找有没有正被 KeepAlive 停用的祖先 */
function isDeactivated(instance: ComponentInternalInstance | null): boolean {
  let current = instance;
  while (current) {
    if (current.isDeactivated) return true;
    current = current.parent;
  }
  return false;
}

/**
 * 所属页面是否在前台，响应式，须在 setup 里调用
 *
 * 初始值按**当下**判定：浮层可能在页面已经切走之后才出现（异步流程回来了），
 * 那它一开始就不该渲染。之后跟着切页变化。
 * 独立窗口不在 KeepAlive 里，恒为 true：多窗口模式、以及 App.vue 直接渲染的全局浮层都算。
 *
 * # 返回值
 *
 * 本页是否在前台，随切页变化的响应式布尔值
 */
export function usePageActive(): Ref<boolean> {
  const instance = getCurrentInstance();
  const active = ref(!isDeactivated(instance));
  if (instance) {
    onActivated(() => {
      active.value = true;
    });
    onDeactivated(() => {
      active.value = false;
    });
  }
  return active;
}
