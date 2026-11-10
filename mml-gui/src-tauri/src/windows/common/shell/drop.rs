//! 拖入路径 / 文本地址命令
//!
//! 判定与地址规范化在 `drop_probe`，这里只做异步包装与 IPC 入口。

use crate::dtos::DropPathDto;

use super::drop_probe;

/// 从拖入的**文本**里取地址，网页卡片那种：`text/uri-list` / `text/plain` / `text/html`
///
/// DOM 的 `drop` 里文本是现成的，但"哪个片段才是地址"这套规则与 [`window_probe_drop`]
/// 解析快捷方式文件时是同一套：去空白 / 去尾斜杠 / 只认 http(s) 且有主机；
/// 所以放在 Rust 侧共用一份，前端别另写一遍。
///
/// - `text`: DOM `drop` 里拿到的文本片段
///
/// # 返回值
///
/// 规范化的 http(s) 地址；取不到返回空串
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub fn window_url_of_text(text: String) -> String {
    drop_probe::url_of_text(&text)
}

/// 拖入路径的探测结果，见 [`DropPathDto`]
///
/// 三个拖拽落点共用：
/// - 主窗口据此决定"开「添加实例」还是开「账户管理」"；
/// - 资源窗口据此认出"拖进来的是目录 / 登录快捷方式"，这两类它都不接；
/// - 「添加实例」窗口自己也拖入时同样先问这里。
///
/// 判定只看文件系统元数据、后缀与文件内容；进了对应窗口后，读条目 / 识别类型
/// 由那边再做一次，见前端 `applyArchivePath` / `pickFolder`。
///
/// 注意：**网页上的"卡片"直接拖进窗口走的是另一条路**：DOM 拖放 + 路径桥，见
/// [`file_drop`] —— 卡片是文本，不会经过这里；本命令只处理**拖进来的文件**。
///
/// - `path`: 拖入文件的绝对路径
///
/// # 返回值
///
/// `kind` 取 `folder` / `archive` / `serverPack` / `shortcut` / `other`；
/// `shortcut` 时 `url` 为解析出的地址，其余情况为空串
#[gui_macros::ipc_group("windows")]
#[gui_macros::command]
pub async fn window_probe_drop(path: String) -> DropPathDto {
    // 认服务器包要打开压缩包读中央目录，整合包动辄 1GB、几万条目；
    // 同步命令会占着主线程，拖进来那一下窗口直接卡住，所以丢到阻塞线程
    tauri::async_runtime::spawn_blocking(move || drop_probe::probe_drop_inner(&path))
        .await
        .unwrap_or_else(|_| DropPathDto {
            kind: "other".to_string(),
            url: String::new(),
        })
}
