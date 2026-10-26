//! 窗口规格
//!
//! 静态条目是 uuid / 标签 / 最小客户区尺寸的唯一来源，
//! 尺寸只在这里写一份，族表不另存。

use std::collections::HashMap;
use std::sync::LazyLock;

use super::family::{family_min_size, family_of_label};
use super::uuids::{
    ACCOUNT_WINDOW_UUID, ADD_MODPACK_WINDOW_UUID, ADD_RESOURCE_WINDOW_UUID, ADD_WINDOW_UUID,
    BLOCK_WINDOW_UUID, COLLECT_WINDOW_UUID, CONFIG_WINDOW_UUID, DOWNLOAD_WINDOW_UUID,
    EXPORT_WINDOW_UUID, HELP_WINDOW_UUID, JAVA_DOWNLOAD_WINDOW_UUID,
    LOBBY_WINDOW_UUID, LOG_WINDOW_UUID, MAIN_WINDOW_UUID, RESOURCE_WINDOW_UUID,
    SERVER_INSTANCE_WINDOW_UUID, SERVER_PACK_WINDOW_UUID, SETTINGS_WINDOW_UUID, STATS_WINDOW_UUID,
    uuid_for_kind,
};
use uuid::Uuid;

/// 窗口注册表条目
pub(crate) struct WindowEntry {
    /// 窗口标签，形如 `mml-<kind>`，与前端 kind 对应
    pub(crate) label: &'static str,
    /// 最小**客户区**宽度，单位逻辑像素；没有历史几何时就是默认宽度
    pub(crate) min_width: f64,
    /// 最小**客户区**高度，单位逻辑像素；没有历史几何时就是默认高度
    pub(crate) min_height: f64,
}

/// 通用窗口最小尺寸，也就是没有历史几何时的默认尺寸
pub(crate) const MIN_WIDTH: f64 = 640.0;

/// 通用窗口最小高度
pub(crate) const MIN_HEIGHT: f64 = 480.0;

/// 主窗口最小宽度
pub(crate) const MAIN_MIN_WIDTH: f64 = 920.0;

/// 主窗口最小高度
pub(crate) const MAIN_MIN_HEIGHT: f64 = 600.0;

/// 账户窗口最小宽度
pub(crate) const ACCOUNT_MIN_WIDTH: f64 = 800.0;

/// 账户窗口最小高度
pub(crate) const ACCOUNT_MIN_HEIGHT: f64 = 650.0;

/// 资源管理窗口最小宽度，按分类栏加内容区算
pub(crate) const RESOURCE_MIN_WIDTH: f64 = 800.0;

/// 资源管理窗口最小高度
pub(crate) const RESOURCE_MIN_HEIGHT: f64 = 510.0;

/// 添加实例窗口最小宽度
pub(crate) const ADD_MIN_WIDTH: f64 = 700.0;

/// 添加实例窗口最小高度
pub(crate) const ADD_MIN_HEIGHT: f64 = 585.0;

/// 下载窗口最小宽度
pub(crate) const DOWNLOAD_MIN_WIDTH: f64 = 670.0;

/// 下载窗口最小高度
pub(crate) const DOWNLOAD_MIN_HEIGHT: f64 = 470.0;

/// 下载整合包窗口最小宽度
pub(crate) const ADD_MODPACK_MIN_WIDTH: f64 = 920.0;

/// 下载整合包窗口最小高度
pub(crate) const ADD_MODPACK_MIN_HEIGHT: f64 = 600.0;

/// 下载资源窗口最小宽度
pub(crate) const ADD_RESOURCE_MIN_WIDTH: f64 = 800.0;

/// 下载资源窗口最小高度
pub(crate) const ADD_RESOURCE_MIN_HEIGHT: f64 = 510.0;

/// 设置窗口最小宽度，左侧标签导航布局需要这么宽
pub(crate) const SETTINGS_MIN_WIDTH: f64 = 850.0;

/// 设置窗口最小高度
pub(crate) const SETTINGS_MIN_HEIGHT: f64 = 600.0;

/// Java 下载窗口最小宽度，四行下拉加下载按钮的小表单窗
pub(crate) const JAVA_DOWNLOAD_MIN_WIDTH: f64 = 520.0;

/// Java 下载窗口最小高度
pub(crate) const JAVA_DOWNLOAD_MIN_HEIGHT: f64 = 420.0;

/// 游戏日志窗口最小宽度，日志控制台加实例选择条
pub(crate) const LOG_MIN_WIDTH: f64 = 800.0;

/// 游戏日志窗口最小高度
pub(crate) const LOG_MIN_HEIGHT: f64 = 480.0;

/// 实例导出窗口最小宽度，元数据表单加两侧勾选列表
pub(crate) const EXPORT_MIN_WIDTH: f64 = 780.0;

/// 实例导出窗口最小高度
pub(crate) const EXPORT_MIN_HEIGHT: f64 = 640.0;

/// 服务器包导出窗口最小宽度：一张实例下拉加几行说明
pub(crate) const SERVER_PACK_MIN_WIDTH: f64 = 640.0;

/// 服务器包导出窗口最小高度
pub(crate) const SERVER_PACK_MIN_HEIGHT: f64 = 420.0;

/// 服务器实例生成窗口最小宽度：模组页是一张六列表格，窄了要横向滚
pub(crate) const SERVER_INSTANCE_MIN_WIDTH: f64 = 860.0;

/// 服务器实例生成窗口最小高度
pub(crate) const SERVER_INSTANCE_MIN_HEIGHT: f64 = 500.0;

/// 方块列表窗口最小宽度，分类栏加网格；工具条上还有搜索框、图标尺寸档与两个按钮
pub(crate) const BLOCK_MIN_WIDTH: f64 = 770.0;

/// 方块列表窗口最小高度
pub(crate) const BLOCK_MIN_HEIGHT: f64 = 560.0;

/// 配置文件窗口最小宽度，左侧文件列表加右侧编辑器
pub(crate) const CONFIG_MIN_WIDTH: f64 = 820.0;

/// 配置文件窗口最小高度
pub(crate) const CONFIG_MIN_HEIGHT: f64 = 560.0;



/// 游戏统计窗口最小宽度
pub(crate) const STATS_MIN_WIDTH: f64 = 780.0;

/// 游戏统计窗口最小高度
pub(crate) const STATS_MIN_HEIGHT: f64 = 600.0;

/// 窗口注册表：uuid → 窗口信息
///
/// **必须是 `static`，不能是 `const`**：`const` 会在每个使用点各展开一份，
/// 每处各建一张 HashMap、各自初始化一次，`static` 才是全进程一张表。
pub(crate) static WINDOWS_INFO: LazyLock<HashMap<Uuid, WindowEntry>> = LazyLock::new(|| {
    HashMap::from([
        (
            LOBBY_WINDOW_UUID,
            WindowEntry {
                label: "mml-lobby",
                min_width: 780.0,
                min_height: 600.0,
            },
        ),
        (
            MAIN_WINDOW_UUID,
            WindowEntry {
                label: "mml-main",
                min_width: MAIN_MIN_WIDTH,
                min_height: MAIN_MIN_HEIGHT,
            },
        ),
        (
            ACCOUNT_WINDOW_UUID,
            WindowEntry {
                label: "mml-account",
                min_width: ACCOUNT_MIN_WIDTH,
                min_height: ACCOUNT_MIN_HEIGHT,
            },
        ),
        (
            SETTINGS_WINDOW_UUID,
            WindowEntry {
                label: "mml-settings",
                min_width: SETTINGS_MIN_WIDTH,
                min_height: SETTINGS_MIN_HEIGHT,
            },
        ),
        (
            STATS_WINDOW_UUID,
            WindowEntry {
                label: "mml-stats",
                min_width: STATS_MIN_WIDTH,
                min_height: STATS_MIN_HEIGHT,
            },
        ),
        (
            HELP_WINDOW_UUID,
            WindowEntry {
                label: "mml-help",
                min_width: MIN_WIDTH,
                min_height: MIN_HEIGHT,
            },
        ),
        (
            RESOURCE_WINDOW_UUID,
            WindowEntry {
                label: "mml-resource",
                min_width: RESOURCE_MIN_WIDTH,
                min_height: RESOURCE_MIN_HEIGHT,
            },
        ),
        (
            ADD_WINDOW_UUID,
            WindowEntry {
                label: "mml-add",
                min_width: ADD_MIN_WIDTH,
                min_height: ADD_MIN_HEIGHT,
            },
        ),
        (
            DOWNLOAD_WINDOW_UUID,
            WindowEntry {
                label: "mml-download",
                min_width: DOWNLOAD_MIN_WIDTH,
                min_height: DOWNLOAD_MIN_HEIGHT,
            },
        ),
        (
            ADD_MODPACK_WINDOW_UUID,
            WindowEntry {
                label: "mml-add_modpack",
                min_width: ADD_MODPACK_MIN_WIDTH,
                min_height: ADD_MODPACK_MIN_HEIGHT,
            },
        ),
        (
            ADD_RESOURCE_WINDOW_UUID,
            WindowEntry {
                label: "mml-add_resource",
                min_width: ADD_RESOURCE_MIN_WIDTH,
                min_height: ADD_RESOURCE_MIN_HEIGHT,
            },
        ),
        (
            COLLECT_WINDOW_UUID,
            WindowEntry {
                label: "mml-collect",
                min_width: MIN_WIDTH,
                min_height: MIN_HEIGHT,
            },
        ),
        (
            BLOCK_WINDOW_UUID,
            WindowEntry {
                label: "mml-block",
                min_width: BLOCK_MIN_WIDTH,
                min_height: BLOCK_MIN_HEIGHT,
            },
        ),
        (
            JAVA_DOWNLOAD_WINDOW_UUID,
            WindowEntry {
                label: "mml-java_download",
                min_width: JAVA_DOWNLOAD_MIN_WIDTH,
                min_height: JAVA_DOWNLOAD_MIN_HEIGHT,
            },
        ),
        (
            LOG_WINDOW_UUID,
            WindowEntry {
                label: "mml-log",
                min_width: LOG_MIN_WIDTH,
                min_height: LOG_MIN_HEIGHT,
            },
        ),
        (
            EXPORT_WINDOW_UUID,
            WindowEntry {
                label: "mml-export",
                min_width: EXPORT_MIN_WIDTH,
                min_height: EXPORT_MIN_HEIGHT,
            },
        ),
        (
            CONFIG_WINDOW_UUID,
            WindowEntry {
                label: "mml-config",
                min_width: CONFIG_MIN_WIDTH,
                min_height: CONFIG_MIN_HEIGHT,
            },
        ),
        (
            SERVER_PACK_WINDOW_UUID,
            WindowEntry {
                label: "mml-server_pack",
                min_width: SERVER_PACK_MIN_WIDTH,
                min_height: SERVER_PACK_MIN_HEIGHT,
            },
        ),
        (
            SERVER_INSTANCE_WINDOW_UUID,
            WindowEntry {
                label: "mml-server_instance",
                min_width: SERVER_INSTANCE_MIN_WIDTH,
                min_height: SERVER_INSTANCE_MIN_HEIGHT,
            },
        ),
    ])
});

/// 最小客户区尺寸，单位逻辑像素
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct MinSize {
    /// 宽
    pub(crate) width: f64,
    /// 高
    pub(crate) height: f64,
}

/// 取某窗口的最小客户区尺寸，注册表口径的逻辑像素
///
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 见 [`MinSize`]；未注册的 uuid 返回 `None`
pub(crate) fn min_size_of(uuid: &Uuid) -> Option<MinSize> {
    WINDOWS_INFO.get(uuid).map(|e| MinSize {
        width: e.min_width,
        height: e.min_height,
    })
}

/// 按窗口标签取注册表里的最小客户区尺寸，标签形如 `mml-<kind>`
///
/// - `label`: 窗口标签
///
/// # 返回值
///
/// 见 [`MinSize`]；未注册的标签返回 `None`
pub(crate) fn min_size_of_label(label: &str) -> Option<MinSize> {
    let kind = label.strip_prefix("mml-")?;
    if let Some(uuid) = uuid_for_kind(kind) {
        return min_size_of(&uuid);
    }
    // 窗口族：标签是 `mml-<kind>-<实例uuid>`，尺寸与注册表里那条基准条目同源。
    // 建窗下限与运行时限宽都按标签取，所以这一支必须接上，否则族内窗口没有最小尺寸
    family_of_label(label).map(|hit| family_min_size(hit.family))
}
