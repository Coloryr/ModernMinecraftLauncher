//! 加载器 meta：版本列表与支持的游戏版本

use super::*;

/// 按 meta 判断加载器是否支持该游戏版本
///
/// 只看 `game` 数组：加载器版本列表任何游戏版本都非空，拿它当判据会误报。
///
/// - `meta`: 加载器 meta
/// - `mc`: 游戏版本号
///
/// # 返回值
///
/// 该版本是否在支持列表里
pub(crate) fn meta_supports(meta: &LoaderMeta, mc: &str) -> bool {
    meta.games.contains(mc)
}

/// 加载器 meta 里跟"哪个游戏版本能用"有关的两份数据
///
/// Fabric / Quilt 的加载器版本列表不随游戏版本变化，只看它非空就当成"支持所有版本"，
/// 会给 1.12.2 也列出 Fabric。真正能装哪些版本看 meta 的 `game` 数组。
pub(crate) struct LoaderMeta {
    /// 加载器版本号，新版本在前
    pub(crate) loaders: Vec<String>,
    /// 支持的游戏版本
    pub(crate) games: HashSet<String>,
}
