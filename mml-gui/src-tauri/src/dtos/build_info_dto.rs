//! 构建信息 DTO —— `app_build_info` 的返回结构

use serde::Serialize;

/// 构建通道
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildChannel {
    /// 正式发布：走 `build/build-release.*`
    Release,
    /// 预发布版：走 `build/build-prerelease.*`，未经完整验证
    Prerelease,
    /// 测试版：直接拉源码启动，`cargo run` / `tauri dev` 都是
    Debug,
}

/// 构建信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoDto {
    /// 版本号，取自 crate 版本，如 `0.1.0`
    pub version: String,
    /// 构建通道，决定主页面是否提示"测试版 / 预发布版"
    pub channel: BuildChannel,
}

/// 启动器可执行文件
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExeInfoDto {
    /// 可执行文件绝对路径
    pub path: String,
    /// 文件内容的 SHA1，小写十六进制，40 位
    pub sha1: String,
}
