/**
 * 命令传输层：把命令名与参数发出去并取回结果
 *
 * Tauri 页面传 `@tauri-apps/api/core` 的 invoke；自定义主页传 `window.mml.invoke` 的包装。
 *
 * - `command`: 命令名，即 Rust 函数名，如 `main_get_instances`
 * - `args`: 参数对象，键名与 Rust 参数一致（camelCase）；无参数时省略
 *
 * # 返回值
 *
 * 命令的返回值；命令不存在或执行失败时 reject
 */
export type Invoke = <T>(command: string, args?: Record<string, unknown> | null) => Promise<T>;
