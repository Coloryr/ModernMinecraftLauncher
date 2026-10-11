//! 网络设置 DTO：HTTP 代理 / DNS / 游戏连接检查
//!
//! 从 `settings_dto/mod.rs` 拆出来的。与 `config.json` 的 `HttpObj` / `DnsObj` /
//! `GameCheckObj` 双向转换，枚举走 `super::conv` 的字符串口径。

use serde::{Deserialize, Serialize};

use mml_config::config_obj::{DnsObj, GameCheckObj, HttpObj};

use super::conv::{
    proxy_state_from_str, proxy_state_to_str, proxy_type_from_str, proxy_type_to_str,
    source_from_str, source_to_str,
};

/// 网络设置，前端 wire 为 camelCase
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NetworkSettingDto {
    /// 下载源：Offical / Bmclapi
    pub source: String,
    /// 下载并发线程数
    pub download_thread: u32,
    /// 代理服务器 IP
    pub proxy_ip: String,
    /// 代理服务器端口
    pub proxy_port: u16,
    /// 代理认证用户名
    pub proxy_user: String,
    /// 代理认证密码
    pub proxy_password: String,
    /// 下载等一般请求的代理策略：Auto / None / User
    pub work_proxy: String,
    /// 下载等一般请求的代理类型：Http / Sock4 / Sock5
    pub work_proxy_type: String,
    /// 登录请求的代理策略：Auto / None / User
    pub login_proxy: String,
    /// 登录请求的代理类型：Http / Sock4 / Sock5
    pub login_proxy_type: String,
    /// 是否校验下载文件完整性，按 SHA1
    pub check_file: bool,
    /// 是否自动下载缺失文件
    pub auto_download: bool,
    /// 自定义 DNS，走 DoH
    pub dns: DnsSettingDto,
    /// 游戏文件检查
    pub check: GameCheckSettingDto,
}

impl From<&HttpObj> for NetworkSettingDto {
    /// 由 core 配置构造前端 DTO，下载源 / 代理策略 / 代理类型都转成 wire 字符串
    ///
    /// - `h`: `config.json` 里的网络设置
    ///
    /// # 返回值
    ///
    /// 前端可直接绑定的网络设置；DNS 与文件检查不在 `HttpObj` 里，先给默认值
    fn from(h: &HttpObj) -> Self {
        Self {
            source: source_to_str(h.source),
            download_thread: h.download_thread,
            proxy_ip: h.proxy_ip.clone(),
            proxy_port: h.proxy_port,
            proxy_user: h.proxy_user.clone(),
            proxy_password: h.proxy_password.clone(),
            work_proxy: proxy_state_to_str(h.work_proxy),
            work_proxy_type: proxy_type_to_str(h.work_proxy_type),
            login_proxy: proxy_state_to_str(h.login_proxy),
            login_proxy_type: proxy_type_to_str(h.login_proxy_type),
            check_file: h.check_file,
            auto_download: h.auto_download,
            dns: DnsSettingDto::default(),
            check: GameCheckSettingDto::default(),
        }
    }
}

impl From<&DnsObj> for DnsSettingDto {
    /// 由 core 配置构造前端 DTO
    ///
    /// - `d`: `config.json` 里的自定义 DNS 设置
    ///
    /// # 返回值
    ///
    /// 前端可直接绑定的 DNS 设置
    fn from(d: &DnsObj) -> Self {
        Self {
            enable: d.enable,
            https: d.https.clone(),
            http_proxy: d.http_proxy,
        }
    }
}

impl From<&GameCheckObj> for GameCheckSettingDto {
    /// 由 core 配置构造前端 DTO
    ///
    /// - `c`: `config.json` 里的游戏文件检查设置
    ///
    /// # 返回值
    ///
    /// 前端可直接绑定的文件检查设置
    fn from(c: &GameCheckObj) -> Self {
        Self {
            core: c.core,
            lib: c.lib,
            assets: c.assets,
            game_mod: c.game_mod,
            core_sha1: c.core_sha1,
            lib_sha1: c.lib_sha1,
            assets_sha1: c.assets_sha1,
            mod_sha1: c.mod_sha1,
        }
    }
}

/// 自定义 DNS 设置，前端 wire 为 camelCase，字段即 core `DnsObj`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DnsSettingDto {
    /// 是否启用自定义 DNS
    pub enable: bool,
    /// DNS over HTTPS 服务器地址列表
    pub https: Vec<String>,
    /// 是否对代理连接也启用自定义 DNS
    pub http_proxy: bool,
}

/// 游戏文件检查设置，前端 wire 为 camelCase，字段即 core `GameCheckObj`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GameCheckSettingDto {
    /// 检查游戏核心 jar 是否存在
    pub core: bool,
    /// 检查运行库是否存在
    pub lib: bool,
    /// 检查资源文件是否存在
    pub assets: bool,
    /// 检查模组是否存在
    pub game_mod: bool,
    /// 校验游戏核心 SHA1
    pub core_sha1: bool,
    /// 校验运行库 SHA1
    pub lib_sha1: bool,
    /// 校验资源文件 SHA1
    pub assets_sha1: bool,
    /// 校验模组 SHA1
    pub mod_sha1: bool,
}

impl From<NetworkSettingDto> for HttpObj {
    /// 由前端 DTO 写回 core 配置，wire 字符串还原成 core 枚举
    ///
    /// - `d`: 前端保存上来的网络设置
    ///
    /// # 返回值
    ///
    /// 待写入 `config.json` 的网络设置；DNS / 文件检查不在 `HttpObj` 里，由各自的 DTO 单独转换
    fn from(d: NetworkSettingDto) -> Self {
        Self {
            source: source_from_str(&d.source),
            download_thread: d.download_thread,
            proxy_ip: d.proxy_ip,
            proxy_port: d.proxy_port,
            proxy_user: d.proxy_user,
            proxy_password: d.proxy_password,
            work_proxy: proxy_state_from_str(&d.work_proxy),
            work_proxy_type: proxy_type_from_str(&d.work_proxy_type),
            login_proxy: proxy_state_from_str(&d.login_proxy),
            login_proxy_type: proxy_type_from_str(&d.login_proxy_type),
            check_file: d.check_file,
            auto_download: d.auto_download,
        }
    }
}

impl From<DnsSettingDto> for DnsObj {
    /// 由前端 DTO 写回 core 配置
    ///
    /// - `d`: 前端保存上来的 DNS 设置
    ///
    /// # 返回值
    ///
    /// 待写入 `config.json` 的自定义 DNS 设置
    fn from(d: DnsSettingDto) -> Self {
        Self {
            enable: d.enable,
            https: d.https,
            http_proxy: d.http_proxy,
        }
    }
}

impl From<GameCheckSettingDto> for GameCheckObj {
    /// 由前端 DTO 写回 core 配置
    ///
    /// - `c`: 前端保存上来的文件检查设置
    ///
    /// # 返回值
    ///
    /// 待写入 `config.json` 的游戏文件检查设置
    fn from(c: GameCheckSettingDto) -> Self {
        Self {
            core: c.core,
            lib: c.lib,
            assets: c.assets,
            game_mod: c.game_mod,
            core_sha1: c.core_sha1,
            lib_sha1: c.lib_sha1,
            assets_sha1: c.assets_sha1,
            mod_sha1: c.mod_sha1,
        }
    }
}
