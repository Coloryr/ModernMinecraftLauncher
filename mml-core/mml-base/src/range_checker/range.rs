//! 版本范围类型、区间边界与命中判断

use super::version::is_plain_version;
use super::*;

/// 版本约束条件，写法沿用 Maven 版本范围
#[derive(Debug, Clone, PartialEq)]
pub enum VersionRange {
    /// 任意版本（`*`）
    Any,
    /// 精确版本（`1.0.0`）
    Exact(String),
    /// 单区间（`[1.0.0,2.0.0)`）
    Interval(Interval),
    /// 多区间联合（`1.0 || 2.0` 或 `(,1.0),[1.1,)`）
    Union(Vec<VersionRange>),
    /// AND：所有子条件都必须满足（新增，用于 JSON 数组）
    All(Vec<VersionRange>),
}

/// 单区间，上下界各自可缺省
#[derive(Debug, Clone, PartialEq)]
pub struct Interval {
    /// 下界版本号，`None` 表示不限
    pub lower: Option<String>,
    /// 下界是否闭合
    pub lower_inclusive: bool,
    /// 上界版本号，`None` 表示不限
    pub upper: Option<String>,
    /// 上界是否闭合
    pub upper_inclusive: bool,
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionRange::Any => write!(f, "*"),
            VersionRange::Exact(v) => write!(f, "{}", v),
            VersionRange::Interval(i) => write!(f, "{}", i),
            VersionRange::Union(rs) => {
                let parts: Vec<String> = rs.iter().map(|r| r.to_string()).collect();
                write!(f, "{}", parts.join(" || "))
            }
            VersionRange::All(rs) => {
                let parts: Vec<String> = rs.iter().map(|r| r.to_string()).collect();
                write!(f, "{}", parts.join(" && "))
            }
        }
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lb = if self.lower_inclusive { '[' } else { '(' };
        let ub = if self.upper_inclusive { ']' } else { ')' };
        let lower = self.lower.as_deref().unwrap_or("");
        let upper = self.upper.as_deref().unwrap_or("");
        write!(f, "{}{},{}{}", lb, lower, upper, ub)
    }
}

/// 区间的一侧边界
pub(super) struct Bound {
    /// 版本号
    pub(super) version: String,
    /// 是否闭合
    pub(super) inclusive: bool,
}

/// 一个记号换算出的区间边界
pub(super) struct Bounds {
    /// 下界；`None` 表示不限
    pub(super) lower: Option<Bound>,
    /// 上界；`None` 表示不限
    pub(super) upper: Option<Bound>,
}

impl Bounds {
    /// 两侧都不限
    ///
    /// # 返回值
    ///
    /// 任意版本都命中的边界
    pub(super) fn unbounded() -> Self {
        Bounds {
            lower: None,
            upper: None,
        }
    }

    /// 只给下界
    ///
    /// - `version`: 下界版本号
    /// - `inclusive`: 下界是否闭合
    ///
    /// # 返回值
    ///
    /// 上界不限的边界
    pub(super) fn lower_only(version: String, inclusive: bool) -> Self {
        Bounds {
            lower: Some(Bound { version, inclusive }),
            upper: None,
        }
    }

    /// 只给上界
    ///
    /// - `version`: 上界版本号
    /// - `inclusive`: 上界是否闭合
    ///
    /// # 返回值
    ///
    /// 下界不限的边界
    pub(super) fn upper_only(version: String, inclusive: bool) -> Self {
        Bounds {
            lower: None,
            upper: Some(Bound { version, inclusive }),
        }
    }

    /// 上下界都给
    ///
    /// - `lower`: 下界版本号
    /// - `lower_inclusive`: 下界是否闭合
    /// - `upper`: 上界版本号
    /// - `upper_inclusive`: 上界是否闭合
    ///
    /// # 返回值
    ///
    /// 双侧都限的边界
    pub(super) fn both(
        lower: String,
        lower_inclusive: bool,
        upper: String,
        upper_inclusive: bool,
    ) -> Self {
        Bounds {
            lower: Some(Bound {
                version: lower,
                inclusive: lower_inclusive,
            }),
            upper: Some(Bound {
                version: upper,
                inclusive: upper_inclusive,
            }),
        }
    }
}

impl From<Bounds> for Interval {
    fn from(bounds: Bounds) -> Self {
        Interval {
            lower: bounds.lower.as_ref().map(|b| b.version.clone()),
            lower_inclusive: bounds.lower.as_ref().map(|b| b.inclusive).unwrap_or(false),
            upper: bounds.upper.as_ref().map(|b| b.version.clone()),
            upper_inclusive: bounds.upper.as_ref().map(|b| b.inclusive).unwrap_or(false),
        }
    }
}

impl Interval {
    /// 判断版本号是否落在本区间内
    ///
    /// - `version`: 待判断的版本号
    ///
    /// # 返回值
    ///
    /// 在区间内返回 `true`；边界不是干净版本号时一律返回 `true`
    pub fn contains(&self, version: &str) -> bool {
        // 边界不是干净的版本号时比不准：`[1.21,1.21z)`、`[1.21-3.1.0c,)` 这类写法
        // 各家实现给出的结论都不一样，而上游加载器是放行的 —— 这里也一律当"符合"，
        // 免得报出一堆没有意义的版本不符
        if self.lower.as_deref().is_some_and(|b| !is_plain_version(b))
            || self.upper.as_deref().is_some_and(|b| !is_plain_version(b))
        {
            return true;
        }

        if let Some(lower) = &self.lower {
            let ord = compare_versions(version, lower);
            let ok = if self.lower_inclusive {
                ord != Ordering::Less
            } else {
                ord == Ordering::Greater
            };
            if !ok {
                return false;
            }
        }
        if let Some(upper) = &self.upper {
            let ord = compare_versions(version, upper);
            let ok = if self.upper_inclusive {
                ord != Ordering::Greater
            } else {
                ord == Ordering::Less
            };
            if !ok {
                return false;
            }
        }
        true
    }
}

impl VersionRange {
    /// 判断版本号是否落在本范围内
    ///
    /// - `version`: 待判断的版本号
    ///
    /// # 返回值
    ///
    /// 在范围内返回 `true`
    pub fn contains(&self, version: &str) -> bool {
        match self {
            VersionRange::Any => true,
            VersionRange::Exact(v) => compare_versions(version, v) == Ordering::Equal,
            VersionRange::Interval(i) => i.contains(version),
            VersionRange::Union(rs) => rs.iter().any(|r| r.contains(version)),
            VersionRange::All(rs) => rs.iter().all(|r| r.contains(version)),
        }
    }
}
