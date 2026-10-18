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

/// 添加资源窗口
pub(super) const ADD_RESOURCE_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000b");

/// 收藏窗口固定 uuid
pub(super) const COLLECT_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000c");

/// 方块列表窗口固定 uuid
pub(super) const BLOCK_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000d");

/// Java 下载窗口固定 uuid
pub(super) const JAVA_DOWNLOAD_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000e");

/// 游戏日志窗口
pub(super) const LOG_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000f");

/// 实例导出窗口固定 uuid
pub(super) const EXPORT_WINDOW_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000010");

/// 配置文件窗口
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

/// 下载资源窗口最小宽度
pub(super) const ADD_RESOURCE_MIN_WIDTH: f64 = 800.0;
/// 下载资源窗口最小高度
pub(super) const ADD_RESOURCE_MIN_HEIGHT: f64 = 510.0;

/// 设置窗口最小宽度，左侧标签导航布局需要这么宽
pub(super) const SETTINGS_MIN_WIDTH: f64 = 850.0;
/// 设置窗口最小高度
pub(super) const SETTINGS_MIN_HEIGHT: f64 = 600.0;

/// Java 下载窗口最小宽度，四行下拉加下载按钮的小表单窗
pub(super) const JAVA_DOWNLOAD_MIN_WIDTH: f64 = 520.0;
/// Java 下载窗口最小高度
pub(super) const JAVA_DOWNLOAD_MIN_HEIGHT: f64 = 420.0;

/// 游戏日志窗口最小宽度，日志控制台加实例选择条
pub(super) const LOG_MIN_WIDTH: f64 = 800.0;
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

/// 「一个实例一个窗口」的窗口族
///
/// 与"一个 kind 一个固定窗口"的静态窗口不同，这类窗口的 uuid 由**该窗口自己的基准 uuid**
/// 与实例 uuid 混合而成、标签是 `<前缀><实例uuid>`：同一实例无论打开多少次都落到同一个窗口，
/// 不同实例各开各的。能混合是因为基准 uuid 表里本来就有，不必另立命名空间常量。
///
/// **尺寸不在这里写第二份**：族内窗口的最小 / 默认尺寸仍从基准条目取，见 [`family_min_size`]。
/// 各写一份的话改尺寸常量只有静态条目跟着变，真实窗口纹丝不动。
pub(super) struct WindowFamily {
    /// 窗口 kind，与前端 `registry.ts` 对应，也是标签前缀中间那一段
    pub(super) kind: &'static str,
    /// 窗口标签前缀，形如 `mml-log-`
    prefix: &'static str,
    /// 该窗口的基准 uuid，注册表静态表里那一条：既是派生窗口 uuid 的命名空间，
    /// 也是族内窗口尺寸的来源
    base: Uuid,
}

/// 全部窗口族
///
/// 新加一族只需在这里补一条：按标签反查、按实例派生、最小尺寸三处分支都在下面共用
static WINDOW_FAMILIES: &[WindowFamily] = &[
    WindowFamily {
        kind: "log",
        prefix: "mml-log-",
        base: LOG_WINDOW_UUID,
    },
    WindowFamily {
        kind: "resource",
        prefix: "mml-resource-",
        base: RESOURCE_WINDOW_UUID,
    },
    WindowFamily {
        kind: "add_resource",
        prefix: "mml-add_resource-",
        base: ADD_RESOURCE_WINDOW_UUID,
    },
    WindowFamily {
        kind: "config",
        prefix: "mml-config-",
        base: CONFIG_WINDOW_UUID,
    },
];

/// 全部窗口族
///
/// # 返回值
///
/// 静态族表；调用方按实例派生出各族的窗口身份（比如删实例时逐个关窗）
pub(super) fn families() -> &'static [WindowFamily] {
    WINDOW_FAMILIES
}

/// 按 kind 取窗口族
///
/// - `kind`: 窗口类型，即标签去掉 `mml-` 前缀与末尾实例 uuid 后的那一段
///
/// # 返回值
///
/// 不是窗口族时返回 `None`
pub(super) fn family_of_kind(kind: &str) -> Option<&'static WindowFamily> {
    WINDOW_FAMILIES.iter().find(|item| item.kind == kind)
}

/// 窗口族里一个窗口的身份
pub(super) struct FamilyWindowId {
    /// 派生出的窗口 uuid；句柄表、几何、模型表都以它登记
    pub(super) uuid: Uuid,
    /// 窗口标签，形如 `mml-log-<实例uuid>`
    pub(super) label: String,
}

/// 由实例 uuid 求窗口族里那个窗口的身份
///
/// 窗口 uuid 是"该窗口的基准 uuid"与实例 uuid 的 UUIDv5 哈希：基准 uuid 当命名空间，
/// 实例 uuid 作名字。各族的基准 uuid 互不相同，所以同一个实例在不同族里得到不同窗口；
/// 输出带 v5 的版本 / 变体位，也撞不上任何一条基准 uuid。
///
/// - `family`: 目标窗口族
/// - `instance`: 目标实例 uuid
///
/// # 返回值
///
/// 见 [`FamilyWindowId`]；派生是纯函数，同一实例任何时候都得到同一个窗口
pub(super) fn family_window_id(family: &WindowFamily, instance: &Uuid) -> FamilyWindowId {
    FamilyWindowId {
        uuid: Uuid::new_v5(&family.base, instance.as_bytes()),
        label: format!("{}{instance}", family.prefix),
    }
}

/// 从窗口标签认出它属于哪个窗口族、是哪个实例
///
/// - `label`: 窗口标签
///
/// # 返回值
///
/// 窗口族与该实例 uuid；不是窗口族的标签、或后缀不是合法 uuid 时返回 `None`
pub(super) fn family_of_label(label: &str) -> Option<(&'static WindowFamily, Uuid)> {
    for family in WINDOW_FAMILIES {
        if let Some(instance) = label.strip_prefix(family.prefix) {
            return Some((family, Uuid::parse_str(instance).ok()?));
        }
    }
    None
}

/// 窗口族的最小客户区尺寸，注册表口径的逻辑像素
///
/// 取自该族**基准条目**的尺寸，不在族表里另存一份：族内窗口与静态条目同源，
/// 改尺寸常量只改静态表那一处，窗口族的默认尺寸与下限都跟着变。
///
/// - `family`: 目标窗口族
///
/// # 返回值
///
/// 见 [`MinSize`]；基准条目未登记在 [`WINDOWS_INFO`] 里时给零值
pub(super) fn family_min_size(family: &WindowFamily) -> MinSize {
    min_size_of(&family.base).unwrap_or_default()
}

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
    if let Some(uuid) = uuid_for_kind(kind) {
        return min_size_of(&uuid);
    }
    // 窗口族：标签是 `mml-<kind>-<实例uuid>`，尺寸与注册表里那条基准条目同源。
    // 建窗下限与运行时限宽都按标签取，所以这一支必须接上，否则族内窗口没有最小尺寸
    family_of_label(label).map(|(family, _)| family_min_size(family))
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
pub(super) fn uuid_of_label(label: &str) -> Option<Uuid> {
    if let Some((uuid, _)) = WINDOWS_INFO.iter().find(|(_, e)| e.label == label) {
        return Some(*uuid);
    }
    family_of_label(label).map(|(family, instance)| family_window_id(family, &instance).uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个 kind 一条基准 uuid，用于断言"派生 uuid 不撞基准"
    const BASE_OF_KIND: [(&str, Uuid); 4] = [
        ("log", LOG_WINDOW_UUID),
        ("resource", RESOURCE_WINDOW_UUID),
        ("add_resource", ADD_RESOURCE_WINDOW_UUID),
        ("config", CONFIG_WINDOW_UUID),
    ];

    /// 测试里直接取族，省得到处 `family_of_kind(..).unwrap()`
    fn family(kind: &str) -> &'static WindowFamily {
        family_of_kind(kind).unwrap()
    }

    /// 窗口身份：同一实例恒定、不同实例不同，且都不是基准 uuid
    #[test]
    fn family_window_id_is_per_instance() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let id = family_window_id(family("log"), &first);

        assert_eq!(
            id.uuid,
            family_window_id(family("log"), &first).uuid,
            "同一实例应派生同一个窗口"
        );
        assert_ne!(
            id.uuid,
            family_window_id(family("log"), &second).uuid,
            "不同实例应是不同窗口"
        );
        assert_ne!(id.uuid, LOG_WINDOW_UUID, "派生 uuid 不能撞上基准条目");
        assert_eq!(id.label, format!("mml-log-{first}"));
    }

    /// 四族两两不共用 uuid：同一个实例在每族里都应是不同的窗口
    #[test]
    fn families_do_not_share_uuid() {
        let instance = Uuid::new_v4();
        let mut seen: Vec<(Uuid, &str)> = Vec::new();

        for (kind, base) in BASE_OF_KIND {
            let id = family_window_id(family(kind), &instance);
            assert_ne!(id.uuid, base, "{kind} 的派生 uuid 撞上了自己的基准 uuid");
            assert_eq!(id.label, format!("mml-{kind}-{instance}"));
            for (other, other_kind) in &seen {
                assert_ne!(id.uuid, *other, "{kind} 与 {other_kind} 派生出同一个窗口");
            }
            seen.push((id.uuid, kind));
        }
    }

    /// 每族的基准 uuid 都必须是注册表里的静态条目：族内窗口的尺寸与命名空间都从它取，
    /// 写错一个字母就会静默退化成"没有最小尺寸"、而且派生出的窗口与别的族撞不上也认不出
    #[test]
    fn family_bases_are_registered() {
        for (kind, base) in BASE_OF_KIND {
            assert_eq!(
                family(kind).base,
                base,
                "{kind} 的族表基准 uuid 与它的常量对不上"
            );
            let size = min_size_of(&base);
            assert!(size.is_some(), "{kind} 的基准 uuid 没登记进 WINDOWS_INFO");
            assert_eq!(family_min_size(family(kind)), size.unwrap());
            assert!(size.unwrap().width > 0.0 && size.unwrap().height > 0.0);
        }
    }

    /// 族内窗口的尺寸跟着**基准条目的常量**走
    ///
    /// 踩过：族表里另存了一份尺寸，改了 `ADD_RESOURCE_MIN_WIDTH` 而真实窗口纹丝不动
    #[test]
    fn family_size_follows_base_constants() {
        let instance = Uuid::new_v4();
        let id = family_window_id(family("add_resource"), &instance);

        assert_eq!(
            min_size_of_label(&id.label),
            Some(MinSize {
                width: ADD_RESOURCE_MIN_WIDTH,
                height: ADD_RESOURCE_MIN_HEIGHT,
            })
        );
    }

    /// 标签 → 实例 / uuid / 尺寸三条反查都要认得出每一族
    #[test]
    fn family_label_resolves_back() {
        let instance = Uuid::new_v4();

        for (kind, _) in BASE_OF_KIND {
            let family = family(kind);
            let id = family_window_id(family, &instance);
            assert_eq!(
                family_of_label(&id.label).map(|(f, i)| (f.kind, i)),
                Some((kind, instance))
            );
            assert_eq!(uuid_of_label(&id.label), Some(id.uuid));
            assert_eq!(min_size_of_label(&id.label), Some(family_min_size(family)));
        }
    }

    /// 静态窗口的标签解析不受窗口族影响
    #[test]
    fn static_labels_still_resolve() {
        assert_eq!(uuid_of_label("mml-download"), Some(DOWNLOAD_WINDOW_UUID));
        assert_eq!(
            min_size_of_label("mml-download"),
            min_size_of(&DOWNLOAD_WINDOW_UUID)
        );
        for (kind, base) in BASE_OF_KIND {
            // 基准条目本身不是窗口族的标签：后面没有实例 uuid
            let label = format!("mml-{kind}");
            assert!(family_of_label(&label).is_none());
            assert_eq!(uuid_of_label(&label), Some(base));
            assert_eq!(min_size_of_label(&label), Some(family_min_size(family(kind))));
            // 后缀不是 uuid 的同类标签同样不认
            let bad = format!("mml-{kind}-not-a-uuid");
            assert!(family_of_label(&bad).is_none());
            assert_eq!(uuid_of_label(&bad), None);
        }
        // 不是窗口族的 kind 取不到族，不会误把静态窗口当族
        assert!(family_of_kind("download").is_none());
        assert!(family_of_kind("log").is_some());
    }
}
