//! M²L GUI 过程宏
//!
//! [`emit`]：事件发射函数标记，本身不改动函数，仅作为源生成器的扫描标记。
//! src-tauri 的 build.rs 扫描 emit 标记收集事件名单，事件名 = 函数名去 `emit_` 前缀、
//! `_` -> `-`，如 emit_account_change -> account-change，据此
//! 生成前端 listens.ts 与 Rust `crate::listens` 常量。

use proc_macro::TokenStream;

/// 事件发射函数标记，属性宏，原样返回 item，不改动函数
///
/// - `_attr`: 属性参数，本宏不解析
/// - `item`: 被标记的函数原文
///
/// # 返回值
///
/// 原样返回 `item`，函数本身不变
#[proc_macro_attribute]
pub fn emit(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// IPC 分组标记，属性宏，原样返回 item，不改动函数
///
/// 只作为源生成器的扫描标记：命令拆进子模块时，靠它把 `bindings.ts` 的组键钉回
/// 原来的组，否则组键会跟着 .rs 文件变成子模块名，前端 `commands.<组>.*` 全断。
/// 生成器侧见 `ipc-gen/src/scan.rs` 的 `group_of`；用法是给命令加一行分组标记，
/// 值填原始组名，如 resource。
///
/// - `_attr`: 属性参数，值是**原始组名**，如 `"resource"`；本宏自身不解析它，只留给扫描器读
/// - `item`: 被标记的函数原文
///
/// # 返回值
///
/// 原样返回 `item`，函数本身不变
#[proc_macro_attribute]
pub fn ipc_group(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
