//! 构建信息：版本号与构建通道，主页面据此提示"测试版 / 预发布版"
//!
//! 通道来自编译期，不是配置：
//! - `debug_assertions` 打开 → 测试版，直接拉源码启动，`cargo run` / `tauri dev` 都是
//! - 否则开了 `prerelease` feature → 预发布版，见 `build/build-prerelease.*`
//! - 都不是 → 正式发布
//!
//! 判定放在 Rust 侧而不是前端读 `import.meta.env`：预发布与正式发布的前端产物
//! **是同一份**，只有后端的编译 profile 不同。

use mml_base::hash_helper::{HashType, gen_hash_from_file};

use crate::dtos::build_info_dto::{BuildChannel, BuildInfoDto, ExeInfoDto};

/// 构建日期，yyyymmdd，由 `build.rs` 在编译期注入
const BUILD_DATE: &str = env!("MML_BUILD_DATE");

/// 把版本号补齐成三段
///
/// 清单里少写一段时按 semver 补 0：`1.0` 补齐成 `1.0.0`，否则拼出来是 `1.0.20261028`，
/// 日期会占到大版本位上，与正式版 `1.0.0` 也对不上。
///
/// - `raw`: 清单里的版本号
///
/// # 返回值
///
/// 至少三段的版本号，如 `1.0` → `1.0.0`
fn base_version(raw: &str) -> String {
    let mut parts: Vec<&str> = raw.split('.').collect();
    while parts.len() < 3 {
        parts.push("0");
    }
    parts.join(".")
}

/// 版本号
///
/// 正式版只写清单里的三段，如 `1.0.0`；测试版与预发布版追加构建日期，
/// 如 `1.0.0.20261028`，一眼能看出手里这份是哪个日期的包。
///
/// - `channel`: 构建通道
///
/// # 返回值
///
/// 展示与日志里用的版本号
fn version_of(channel: BuildChannel) -> String {
    let base = base_version(env!("CARGO_PKG_VERSION"));
    match channel {
        BuildChannel::Release => base,
        BuildChannel::Debug | BuildChannel::Prerelease => format!("{base}.{BUILD_DATE}"),
    }
}

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
        version: version_of(channel),
        channel,
    }
}

/// 当前启动器可执行文件的路径与 SHA1
///
/// 帮助页用来核对拿到的 exe 是不是发行页那一份；可执行文件几十 MB，
/// 读盘与哈希放阻塞线程，别占住 async runtime。
///
/// # 返回值
///
/// 见 [ExeInfoDto]；取不到自身路径或读盘失败返回错误文案
#[gui_macros::command]
pub async fn app_build_exe_info() -> Result<ExeInfoDto, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = std::env::current_exe().map_err(|e| e.to_string())?;
        let sha1 = gen_hash_from_file(HashType::Sha1, &path).map_err(|e| e.to_string())?;
        Ok(ExeInfoDto {
            path: path.to_string_lossy().to_string(),
            sha1,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
#[path = "../tests/unit/build_info_tests.rs"]
mod tests;
