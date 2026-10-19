//! 一实例一窗的窗口族
//!
//! 族里窗口的 uuid 是基准 uuid 与实例 uuid 的 UUIDv5，标签是前缀 + 实例 uuid；
//! 按标签反查、按实例派生、最小尺寸都从这里走，族表不另存尺寸。


use uuid::{Uuid};
use super::uuids::{RESOURCE_WINDOW_UUID, ADD_RESOURCE_WINDOW_UUID, LOG_WINDOW_UUID, CONFIG_WINDOW_UUID};
use super::spec::{MinSize, min_size_of};


/// 「一个实例一个窗口」的窗口族
///
/// 与"一个 kind 一个固定窗口"的静态窗口不同，这类窗口的 uuid 由**该窗口自己的基准 uuid**
/// 与实例 uuid 混合而成、标签是 `<前缀><实例uuid>`：同一实例无论打开多少次都落到同一个窗口，
/// 不同实例各开各的。能混合是因为基准 uuid 表里本来就有，不必另立命名空间常量。
///
/// **尺寸不在这里写第二份**：族内窗口的最小 / 默认尺寸仍从基准条目取，见 [`family_min_size`]。
/// 各写一份的话改尺寸常量只有静态条目跟着变，真实窗口纹丝不动。
pub(crate) struct WindowFamily {
    /// 窗口 kind，与前端 `registry.ts` 对应，也是标签前缀中间那一段
    pub(crate) kind: &'static str,
    /// 窗口标签前缀，形如 `mml-log-`
    pub(super) prefix: &'static str,
    /// 该窗口的基准 uuid，注册表静态表里那一条：既是派生窗口 uuid 的命名空间，
    /// 也是族内窗口尺寸的来源
    pub(super) base: Uuid,
}

/// 全部窗口族
///
/// 新加一族只需在这里补一条：按标签反查、按实例派生、最小尺寸三处分支都在下面共用
pub(super) static WINDOW_FAMILIES: &[WindowFamily] = &[
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
pub(crate) fn families() -> &'static [WindowFamily] {
    WINDOW_FAMILIES
}

/// 按 kind 取窗口族
///
/// - `kind`: 窗口类型，即标签去掉 `mml-` 前缀与末尾实例 uuid 后的那一段
///
/// # 返回值
///
/// 不是窗口族时返回 `None`
pub(crate) fn family_of_kind(kind: &str) -> Option<&'static WindowFamily> {
    WINDOW_FAMILIES.iter().find(|item| item.kind == kind)
}

/// 窗口族里一个窗口的身份
pub(crate) struct FamilyWindowId {
    /// 派生出的窗口 uuid；句柄表、几何、模型表都以它登记
    pub(crate) uuid: Uuid,
    /// 窗口标签，形如 `mml-log-<实例uuid>`
    pub(crate) label: String,
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
pub(crate) fn family_window_id(family: &WindowFamily, instance: &Uuid) -> FamilyWindowId {
    FamilyWindowId {
        uuid: Uuid::new_v5(&family.base, instance.as_bytes()),
        label: format!("{}{instance}", family.prefix),
    }
}

/// 从窗口标签解析出的窗口族与实例
pub(crate) struct LabeledFamily {
    /// 命中的窗口族
    pub(crate) family: &'static WindowFamily,
    /// 标签后缀里的实例 uuid
    pub(crate) instance: Uuid,
}

/// 从窗口标签认出它属于哪个窗口族、是哪个实例
///
/// - `label`: 窗口标签
///
/// # 返回值
///
/// 见 [`LabeledFamily`]；不是窗口族的标签、或后缀不是合法 uuid 时返回 `None`
pub(crate) fn family_of_label(label: &str) -> Option<LabeledFamily> {
    for family in WINDOW_FAMILIES {
        if let Some(instance) = label.strip_prefix(family.prefix) {
            return Some(LabeledFamily {
                family,
                instance: Uuid::parse_str(instance).ok()?,
            });
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
pub(crate) fn family_min_size(family: &WindowFamily) -> MinSize {
    min_size_of(&family.base).unwrap_or_default()
}
