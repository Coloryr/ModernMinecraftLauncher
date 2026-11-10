//! 范围检查器
//! 支持maven和fabric的范围检查
//!
//! 按职责拆成几个子模块：
//! - `version`：版本号记号切分与比较
//! - `range`：版本范围类型、区间边界与命中判断
//! - `maven`：Maven 写法（方括号区间）的解析
//! - `syntax`：semver/Fabric 写法的解析，以及统一解析入口

pub(crate) use std::{cmp::Ordering, fmt};

pub(crate) use mml_names::i18_items::error_type::{ArgEmptyData, CoreResult, ErrorType};

pub(crate) use crate::serialize_tools::MiniJsonObj;

mod maven;
mod range;
mod syntax;
mod version;

pub use self::maven::parse_maven_version_range;
pub use self::range::{Interval, VersionRange};
pub use self::syntax::{
    RangeSyntax, parse_fabric_version_range, parse_json_array_range, parse_version_range,
    parse_version_range_with,
};
pub use self::version::compare_versions;

#[cfg(test)]
#[path = "../../tests/unit/range_checker_tests.rs"]
mod tests;
