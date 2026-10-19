// 兼容层：生成物在 mml-ipc（由 ipc-gen 扫描 Rust 源码产出）
//
// 这里只把 Tauri 的 invoke 绑到命令包装上，既有的 `commands.xxx` 与类型导入路径保持不变。
import { invoke } from "@tauri-apps/api/core";
import { createCommands } from "mml-ipc";

export * from "mml-ipc";

/** 命令，按来源 .rs 模块分组；参数顺序与 Rust 一致，Tauri 注入的 AppHandle / WebviewWindow 已剔除 */
// Tauri 的 invoke 不接受 null 参数（自定义主页桥接接受），这里统一成 undefined
export const commands = createCommands((cmd, args) => invoke(cmd, args ?? undefined));
