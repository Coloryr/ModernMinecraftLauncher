//! 窗口注册表：uuid / 标签 / 最小尺寸的**唯一来源**
//!
//! 从 `windows/mod.rs` 拆出来的：这里是一张纯数据表 + 几个反查函数，
//! 原先与"建窗 / 存几何 / 模型生命周期"混在一个 1200 行的文件里，
//! 改个窗口尺寸要翻半个文件。
//!
//! 尺寸口径：注册表里全是**客户区 + 逻辑像素**，与 Windows 外框之间的换算见
//! `super::geometry`。各窗口的默认尺寸就是这张表 —— 前端不再维护第二份。

use std::collections::HashMap;
use std::sync::LazyLock;

use uuid::{Uuid, uuid};

/// 主窗口固定 uuid；其余窗口从 2 号起按顺序分配
pub(super) const MAIN_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

/// 账户窗口
pub(super) const ACCOUNT_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

/// 设置窗口
pub(super) const SETTINGS_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");

/// 统计窗口
pub(super) const STATS_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000004");

/// 帮助窗口
pub(super) const HELP_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000006");

/// 资源管理窗口
pub(super) const RESOURCE_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000007");

/// 添加实例窗口固定 uuid
pub(super) const ADD_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000008");

/// 下载窗口固定 uuid
pub const DOWNLOAD_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000009");

/// 下载整合包窗口固定 uuid
pub(super) const ADD_MODPACK_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000a");

/// 添加资源窗口固定 uuid
pub(super) const ADD_RESOURCE_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000b");

/// 收藏窗口固定 uuid
pub(super) const COLLECT_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000c");

/// 方块列表窗口固定 uuid
pub(super) const BLOCK_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000d");

/// Java 下载窗口固定 uuid
pub(super) const JAVA_DOWNLOAD_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000e");

/// 游戏日志窗口固定 uuid
pub(super) const LOG_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000f");

/// 实例导出窗口固定 uuid
pub(super) const EXPORT_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000010");

/// 配置文件窗口固定 uuid
pub(super) const CONFIG_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000011");

/// 窗口注册表条目
pub(super) struct WindowEntry {
    /// 窗口标签，形如 `mml-<kind>`，与前端 kind 对应
    pub(super) label: &'static str,
    /// 最小**客户区**宽度，单位逻辑像素；没有历史几何时就是默认宽度
    pub(super) min_width: f64,
    /// 最小**客户区**高度，单位逻辑像素；没有历史几何时就是默认高度
    pub(super) min_height: f64,
}

/// 通用窗口最小尺寸，也就是没有历史几何时的默认尺寸
pub(super) const MIN_WIDTH: f64 = 640.0;
/// 通用窗口最小高度
pub(super) const MIN_HEIGHT: f64 = 480.0;

/// 主窗口最小宽度
pub(super) const MAIN_MIN_WIDTH: f64 = 920.0;
/// 主窗口最小高度
pub(super) const MAIN_MIN_HEIGHT: f64 = 600.0;

/// 账户窗口最小宽度
pub(super) const ACCOUNT_MIN_WIDTH: f64 = 800.0;
/// 账户窗口最小高度
pub(super) const ACCOUNT_MIN_HEIGHT: f64 = 650.0;

/// 资源管理窗口最小宽度，按分类栏加内容区算
pub(super) const RESOURCE_MIN_WIDTH: f64 = 800.0;
/// 资源管理窗口最小高度
pub(super) const RESOURCE_MIN_HEIGHT: f64 = 510.0;

/// 添加实例窗口最小宽度
pub(super) const ADD_MIN_WIDTH: f64 = 700.0;
/// 添加实例窗口最小高度
pub(super) const ADD_MIN_HEIGHT: f64 = 585.0;

/// 下载窗口最小宽度
pub(super) const DOWNLOAD_MIN_WIDTH: f64 = 670.0;
/// 下载窗口最小高度
pub(super) const DOWNLOAD_MIN_HEIGHT: f64 = 470.0;

/// 下载整合包窗口最小宽度
pub(super) const ADD_MODPACK_MIN_WIDTH: f64 = 920.0;
/// 下载整合包窗口最小高度
pub(super) const ADD_MODPACK_MIN_HEIGHT: f64 = 600.0;

/// 设置窗口最小宽度，左侧标签导航布局需要这么宽
pub(super) const SETTINGS_MIN_WIDTH: f64 = 850.0;
/// 设置窗口最小高度
pub(super) const SETTINGS_MIN_HEIGHT: f64 = 600.0;

/// Java 下载窗口最小宽度，四行下拉加下载按钮的小表单窗
pub(super) const JAVA_DOWNLOAD_MIN_WIDTH: f64 = 520.0;
/// Java 下载窗口最小高度
pub(super) const JAVA_DOWNLOAD_MIN_HEIGHT: f64 = 420.0;

/// 游戏日志窗口最小宽度，日志控制台加实例选择条
pub(super) const LOG_MIN_WIDTH: f64 = 720.0;
/// 游戏日志窗口最小高度
pub(super) const LOG_MIN_HEIGHT: f64 = 480.0;

/// 实例导出窗口最小宽度，元数据表单加两侧勾选列表
pub(super) const EXPORT_MIN_WIDTH: f64 = 780.0;
/// 实例导出窗口最小高度
pub(super) const EXPORT_MIN_HEIGHT: f64 = 640.0;

/// 方块列表窗口最小宽度，分类栏加网格；工具条上还有搜索框、图标尺寸档与两个按钮
pub(super) const BLOCK_MIN_WIDTH: f64 = 770.0;
/// 方块列表窗口最小高度
pub(super) const BLOCK_MIN_HEIGHT: f64 = 560.0;

/// 配置文件窗口最小宽度，左侧文件列表加右侧编辑器
pub(super) const CONFIG_MIN_WIDTH: f64 = 820.0;
/// 配置文件窗口最小高度
pub(super) const CONFIG_MIN_HEIGHT: f64 = 560.0;

/// 窗口注册表：uuid → 窗口信息
///
/// **必须是 `static`，不能是 `const`**：`const` 会在每个使用点各展开一份，
/// 每处各建一张 HashMap、各自初始化一次，`static` 才是全进程一张表。
pub(super) static WINDOWS_INFO: LazyLock<HashMap<Uuid, WindowEntry>> = LazyLock::new(|| {
    HashMap::from([
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
                min_width: MIN_WIDTH,
                min_height: MIN_HEIGHT,
            },
        ),
        // 帮助窗口是**纯前端静态页**：没有 IPC 命令、没有窗口模型、除了这条注册表项也没有
        // 别的规格 —— 全仓只有这一个窗口是这样，所以 `windows/` 下没有 help.rs。
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
                min_width: MIN_WIDTH,
                min_height: MIN_HEIGHT,
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
    ])
});

/// 前端窗口 kind → uuid，标签去 `mml-` 前缀匹配
///
/// - `kind`: 窗口类型，即标签去掉 `mml-` 前缀后的那一段
///
/// # 返回值
///
/// 未注册的 kind 返回 `None`
pub(super) fn uuid_for_kind(kind: &str) -> Option<Uuid> {
    WINDOWS_INFO
        .iter()
        .find(|(_, e)| e.label.strip_prefix("mml-") == Some(kind))
        .map(|(uuid, _)| *uuid)
}

/// 最小客户区尺寸，单位逻辑像素
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct MinSize {
    /// 宽
    pub(super) width: f64,
    /// 高
    pub(super) height: f64,
}

/// 取某窗口的最小客户区尺寸，注册表口径的逻辑像素
///
/// - `uuid`: 窗口 uuid
///
/// # 返回值
///
/// 见 [`MinSize`]；未注册的 uuid 返回 `None`
pub(super) fn min_size_of(uuid: &Uuid) -> Option<MinSize> {
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
pub(super) fn min_size_of_label(label: &str) -> Option<MinSize> {
    let kind = label.strip_prefix("mml-")?;
    let uuid = uuid_for_kind(kind)?;
    min_size_of(&uuid)
}
