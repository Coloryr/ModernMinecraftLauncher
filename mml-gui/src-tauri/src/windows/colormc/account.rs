//! ColorMC 账户迁移：把 ColorMC 的 `auth.json` 转换成 M²L 的账户
//!
//! 账户文件在工作目录之外（Windows 是 `%LOCALAPPDATA%\ColorMC\auth.json`），不能跟着
//! 顶层条目搬，格式也和 M²L 的 `LoginObj` 有差别：ColorMC 的 `AccessToken` / `ClientToken` /
//! `Text1` / `Text2` 可以是 null，`LastLogin` 可能是没有时区的本地时间。
//!
//! 令牌原样带过来：微软账户两边用同一个 client id，refresh token 能直接刷新；离线账户本来
//! 就没有令牌。合并时**同一个 uuid + 认证类型以 M²L 已有的为准**，只补 ColorMC 独有的，
//! 免得把用户后来刷新过的令牌盖回旧值。
//!
//! 合并走账户模块的内存存储（`mml_auth::auths::get_all` + `LoginObj::save`），不直接
//! 写 `auth.json`：启动时可能还有一次异步保存排在队列里，直接写文件会被它覆盖回去。

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDateTime, TimeZone};
use serde::Deserialize;

use mml_auth::{AuthType, LoginObj, UserKeyObj, normalize_uuid};

/// ColorMC `auth.json` 里的一条账户
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ColorMcAuth {
    /// 玩家用户名，OAuth 账户可能是旧名字
    user_name: String,
    /// 账户 UUID，不带连字符
    #[serde(rename = "UUID")]
    uuid: String,
    /// 访问令牌，离线账户为 null
    access_token: Option<String>,
    /// 客户端标识令牌，OAuth 账户为 null
    client_token: Option<String>,
    /// ColorMC 的认证类型，取值 0..=5，与 M²L 的枚举顺序一致
    auth_type: u8,
    /// 扩展字段，含义随认证类型
    text1: Option<String>,
    /// 扩展字段，M²L 当前未使用，原样保留
    text2: Option<String>,
    /// 最后登录时间，可能是带时区的 RFC 3339，也可能是本地墙上时间
    last_login: Option<String>,
}

impl ColorMcAuth {
    /// 转成 M²L 的账户；认证类型不认识或 UUID 为空时返回 `None`
    fn into_login(self) -> Option<LoginObj> {
        let auth_type = auth_type_of(self.auth_type)?;
        let uuid = normalize_uuid(&self.uuid);
        if uuid.is_empty() {
            return None;
        }

        Some(LoginObj {
            user_name: self.user_name,
            uuid,
            access_token: self.access_token.unwrap_or_default(),
            client_token: self.client_token.unwrap_or_default(),
            auth_type,
            text1: self.text1.filter(|text| !text.is_empty()),
            text2: self.text2.filter(|text| !text.is_empty()),
            last_login: parse_time(self.last_login.as_deref()),
            expire_at: None,
        })
    }
}

/// 读取 ColorMC 的账户文件并转成 M²L 的账户
///
/// - `source`: ColorMC 的 `auth.json`
///
/// # 返回值
///
/// 能识别的账户；文件不存在时返回空表，读不了或解析失败时返回 `Err`
pub(super) fn read(source: &Path) -> Result<Vec<LoginObj>, String> {
    if !source.is_file() {
        return Ok(Vec::new());
    }

    let data = fs::read(source).map_err(|err| err.to_string())?;
    let list: Vec<ColorMcAuth> = serde_json::from_slice(&data).map_err(|err| err.to_string())?;

    Ok(list
        .into_iter()
        .filter_map(ColorMcAuth::into_login)
        .collect())
}

/// 挑出 ColorMC 独有的账户
///
/// - `existing`: M²L 已有的账户
/// - `incoming`: 从 ColorMC 读出来的账户
///
/// # 返回值
///
/// 同键账户不在结果里，返回的每一条都可以直接 `save`
pub(super) fn merge(existing: &[LoginObj], incoming: Vec<LoginObj>) -> Vec<LoginObj> {
    let mut keys: HashSet<UserKeyObj> = existing.iter().map(LoginObj::get_key).collect();

    incoming
        .into_iter()
        .filter(|item| keys.insert(item.get_key()))
        .collect()
}

/// ColorMC 的认证类型数字转成 M²L 的枚举
///
/// 两边的枚举顺序一致：0 离线、1 微软、2 统一通行证、3 外置登录、4 LittleSkin、5 自建皮肤站。
///
/// - `value`: ColorMC 的 `AuthType`
///
/// # 返回值
///
/// 对应的枚举；超出 0..=5 时返回 `None`
fn auth_type_of(value: u8) -> Option<AuthType> {
    Some(match value {
        0 => AuthType::Offline,
        1 => AuthType::OAuth,
        2 => AuthType::Nide8,
        3 => AuthType::AuthlibInjector,
        4 => AuthType::LittleSkin,
        5 => AuthType::SelfLittleSkin,
        _ => return None,
    })
}

/// 解析 ColorMC 的 `LastLogin`
///
/// ColorMC 的离线账户写的是不带时区的本地墙上时间，例如 `0001-01-01T00:00:00`；
/// 带偏移量的那条按 RFC 3339 解析。空值或解析失败时用当前时间。
///
/// - `text`: 原始时间文本
///
/// # 返回值
///
/// 带本地偏移量的时间；`0001` 这种"从未登录"归到 Unix 纪元，避免跑到列表最前面
fn parse_time(text: Option<&str>) -> DateTime<FixedOffset> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return Local::now().fixed_offset();
    };

    if let Ok(time) = DateTime::parse_from_rfc3339(text) {
        return time.fixed_offset();
    }

    if let Ok(naive) = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f") {
        if naive.year() < 1970 {
            return DateTime::UNIX_EPOCH.fixed_offset();
        }
        if let Some(time) = Local.from_local_datetime(&naive).earliest() {
            return time.fixed_offset();
        }
    }

    Local::now().fixed_offset()
}
