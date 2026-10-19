//! M²L GUI 过程宏
//!
//! 三个属性宏都是**源生成器的扫描标记**，本身尽量不改动函数：
//!
//! - [`command`]：IPC 命令，展开成 Tauri 的命令宏；生成器只认这个标记
//! - [`emit`]：事件发射函数，扫描器据此收事件名单，事件名 = 函数名去 `emit_` 前缀、
//!   `_` -> `-`
//! - [`ipc_group`]：命令分组，命令拆进子模块时把 `bindings.ts` 的组键钉回原组
//!
//! 扫描逻辑在 `ipc-gen`，直接按属性字面量做裸文本匹配。

use proc_macro::TokenStream;

/// IPC 命令标记
///
/// 展开成 `#[tauri::command]` + 原函数，行为与直接写 Tauri 的命令属性一致。
/// 生成器只扫描这个标记，不再认 Tauri 的命令属性。
///
/// - `_attr`: 属性参数，本宏不解析
/// - `item`: 被标记的函数原文
///
/// # 返回值
///
/// 带上 Tauri 命令属性的函数
#[proc_macro_attribute]
pub fn command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    quote::quote! { #[tauri::command] #item }.into()
}

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
