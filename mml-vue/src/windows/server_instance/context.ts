import type { InjectionKey } from "vue";
import type { useServerInstance } from "./useServerInstance";

/** 服务器实例窗口的上下文，面板组件统一注入，不层层传 props */
export type ServerInstanceContext = ReturnType<typeof useServerInstance>;

/** 上下文注入键 */
export const ServerInstanceKey: InjectionKey<ServerInstanceContext> =
  Symbol("server-instance");
