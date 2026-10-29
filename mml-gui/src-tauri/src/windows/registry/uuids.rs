//! 窗口 uuid
//!
//! 一个 kind 一个固定 uuid，按标签查回 kind；
//! 「一实例一窗」的窗口实际 uuid 由窗口族在基准 uuid 上派生。

use super::family::{family_of_label, family_window_id};
use super::spec::WINDOWS_INFO;
use uuid::{Uuid, uuid};

/// 主窗口固定 uuid；其余窗口从 2 号起按顺序分配
pub(crate) const MAIN_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

/// 账户窗口
pub(crate) const ACCOUNT_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

/// 设置窗口
pub(crate) const SETTINGS_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");

/// 统计窗口
pub(crate) const STATS_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000004");

/// 帮助窗口
pub(crate) const HELP_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000006");

/// 资源管理窗口
pub(crate) const RESOURCE_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000007");

/// 添加实例窗口固定 uuid
pub(crate) const ADD_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000008");

/// 下载窗口固定 uuid
pub const DOWNLOAD_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000009");

/// 下载整合包窗口固定 uuid
pub(crate) const ADD_MODPACK_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000a");

/// 添加资源窗口
pub(crate) const ADD_RESOURCE_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000b");

/// 收藏窗口固定 uuid
pub(crate) const COLLECT_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000c");

/// 方块列表窗口固定 uuid
pub(crate) const BLOCK_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000d");

/// Java 下载窗口固定 uuid
pub(crate) const JAVA_DOWNLOAD_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000e");

/// 游戏日志窗口
pub(crate) const LOG_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000f");

/// 实例导出窗口固定 uuid
pub(crate) const EXPORT_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000010");

/// 配置文件窗口
pub(crate) const CONFIG_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000011");

/// 大厅窗口
pub(crate) const LOBBY_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000012");

/// 服务器包导出窗口固定 uuid
pub(crate) const SERVER_PACK_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000013");

/// 服务器实例生成窗口固定 uuid
pub(crate) const SERVER_INSTANCE_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000014");

/// 前端窗口 kind → uuid，标签去 `mml-` 前缀匹配
///
/// 窗口族的 kind 拿到的是它的**基准 uuid**，族内窗口还要按实例派生，见
/// `family::family_window_id`。
///
/// - `kind`: 窗口类型，即标签去掉 `mml-` 前缀后的那一段
///
/// # 返回值
///
/// 未注册的 kind 返回 `None`
pub(crate) fn uuid_for_kind(kind: &str) -> Option<Uuid> {
    WINDOWS_INFO
        .iter()
        .find(|(_, e)| e.label.strip_prefix("mml-") == Some(kind))
        .map(|(uuid, _)| *uuid)
}

/// 按窗口标签求窗口 uuid
///
/// 静态表按标签全等匹配；窗口族不在表里，按 `mml-<kind>-<实例uuid>` 反推。
/// 生命周期里按标签找 uuid 的地方都要走这里，否则族内窗口的事件会被整段跳过。
///
/// - `label`: 窗口标签
///
/// # 返回值
///
/// 窗口 uuid；两套都对不上时为 `None`
pub(crate) fn uuid_of_label(label: &str) -> Option<Uuid> {
    if let Some((uuid, _)) = WINDOWS_INFO.iter().find(|(_, e)| e.label == label) {
        return Some(*uuid);
    }
    family_of_label(label).map(|hit| family_window_id(hit.family, &hit.instance).uuid)
}
