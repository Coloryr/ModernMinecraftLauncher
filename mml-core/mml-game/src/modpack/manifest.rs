//! 整合包 manifest 的读写，供升级比对
//!
//! 两个 worker 里原本各写了一份同样的代码：读旧清单时"任何失败都当作没有旧清单"，
//! 写回时用 `serialize_tools` 落 JSON。抽到这里有两个目的：
//!
//! 1. **口径统一** —— 缺失 / 打不开 / 空文件 / 非法 JSON 一律 `None`，调用方据此落到
//!    "无旧 manifest"分支，按 SHA1 或 `mod_id` 比对，而不是让整次升级失败；
//! 2. **可离线测** —— "覆盖写回""损坏清单不 panic""比对过程不碰文件"这些性质能直接测，
//!    见 `tests/modpack_upgrade.rs`。
//!
//! 写回在调用方是**最后一步**，所有 `?` 都可能提前返回，所以取消 / 比对失败时旧清单
//! 不会被写坏 —— 那是调用方的顺序保证，本模块只负责读写。

use std::path::Path;

use mml_base::serialize_tools;
use mml_names::i18_items::error_type::CoreResult;
use serde::{Serialize, de::DeserializeOwned};

/// 读旧 manifest，**任何失败都当作"没有旧清单"**，不 panic
///
/// - `path`: manifest 文件路径
///
/// # 返回值
///
/// 没有旧清单或读取失败时返回 `None`
///
/// 覆盖：文件不存在、无权限、空文件、内容不是合法 JSON、JSON 结构与 `T` 不符。
pub fn read_manifest<T: DeserializeOwned>(path: impl AsRef<Path>) -> Option<T> {
    serialize_tools::json_from_file(path).ok()
}

/// 写回 manifest，**覆盖**已有文件，供下一次升级比对
///
/// - `path`: manifest 文件路径
/// - `value`: 要落盘的内容
///
/// # 返回值
///
/// 成功返回 `Ok(())`；写失败返回对应错误
///
/// 与读一样走 `serialize_tools`：不存在的父目录由 `path_helper::open_write` 负责报错。
pub fn write_manifest<T: Serialize>(path: impl AsRef<Path>, value: &T) -> CoreResult<()> {
    serialize_tools::json_to_file(value, path)
}
