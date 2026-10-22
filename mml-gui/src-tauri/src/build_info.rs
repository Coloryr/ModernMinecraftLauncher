//! 构建信息：版本号与构建通道，主页面据此提示"测试版 / 预发布版"
//!
//! 通道来自编译期，不是配置：
//! - `debug_assertions` 打开 → 测试版，直接拉源码启动，`cargo run` / `tauri dev` 都是
//! - 否则开了 `prerelease` feature → 预发布版，见 `build/build-prerelease.*`
//! - 都不是 → 正式发布
//!
//! 判定放在 Rust 侧而不是前端读 `import.meta.env`：预发布与正式发布的前端产物
//! **是同一份**，只有后端的编译 profile 不同。

use crate::dtos::build_info_dto::{BuildChannel, BuildInfoDto};

/// 取构建信息
///
/// # 返回值
///
/// 见 [BuildInfoDto]
#[gui_macros::command]
pub fn app_build_info() -> BuildInfoDto {
    let channel = if cfg!(debug_assertions) {
        BuildChannel::Debug
    } else if cfg!(feature = "prerelease") {
        BuildChannel::Prerelease
    } else {
        BuildChannel::Release
    };
    BuildInfoDto {
        version: env!("CARGO_PKG_VERSION").to_string(),
        channel,
    }
}
