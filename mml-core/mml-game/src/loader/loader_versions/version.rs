//! 版本号比较，以及从加载器版本号反推所属游戏版本

/// 版本号比较：a >= b，按 `.` 分段做数值比较
///
/// - `a`: 左侧版本号
/// - `b`: 右侧版本号
///
/// # 返回值
///
/// 返回 `a` 是否大于等于 `b`
pub(crate) fn version_ge(a: &str, b: &str) -> bool {
    /// 把版本号按 `.` 拆成数值段，非数字段按 0 计
    fn parts(v: &str) -> Vec<u64> {
        v.split('.')
            .map(|n| n.parse::<u64>().unwrap_or(0))
            .collect()
    }
    let (pa, pb) = (parts(a), parts(b));
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    true
}

/// 从 Forge 全量版本列表中筛出指定游戏版本的构建号
///
/// 元数据条目形如 `1.20.1-47.1.0`，旧版本可能带分支后缀，去掉 `{mc}-` 前缀。
///
/// - `all`: 全量版本条目
/// - `mc`: 游戏版本号
///
/// # 返回值
///
/// 返回该游戏版本的构建号列表
pub(crate) fn forge_of_mc(all: &[String], mc: &str) -> Vec<String> {
    let prefix = format!("{mc}-");
    all.iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|v| v[prefix.len()..].to_string())
        .collect()
}

/// 从 NeoForge 版本号推断所属的游戏版本
///
/// - 旧方案：`21.1.77` → `1.21.1`，即 `1.` 加前两段
/// - 新方案，主版本 ≥ 26：`26.2.0.0-beta` → `26.2`，`26.1.1.x` → `26.1.1`，
///   带 `+snapshot-x` 的映射到对应快照版本
/// - 以 `0` 开头的愚人节版本返回 `None`，如 `0.25w14craftmine.3-beta`
///
/// - `version`: NeoForge 版本号
///
/// # 返回值
///
/// 返回所属的游戏版本号；无法推断返回 `None`
pub(crate) fn neoforge_mc_version(version: &str) -> Option<String> {
    if version.starts_with('0') {
        return None;
    }

    let mut spl = version.split('.');
    let major: u64 = spl.next()?.parse().ok()?;
    let second = spl.next()?;

    if major >= 26 {
        let mut mc = format!("{major}.{second}");
        // 26.1.0.x → 26.1；26.1.1.x → 26.1.1
        let third = spl.next().unwrap_or("0");
        if third != "0" {
            mc.push('.');
            mc.push_str(third);
        }
        // 26.1.0.0-alpha+snapshot-1 → 26.1-snapshot-1
        if let Some((_, suffix)) = version.split_once('+') {
            mc.push('-');
            mc.push_str(suffix);
        }
        Some(mc)
    } else {
        Some(format!("1.{major}.{second}"))
    }
}

/// 从 JSON 值提取版本号列表：元素为对象取 `version` 字段，为字符串直接取
///
/// - `value`: JSON 数组值
///
/// # 返回值
///
/// 返回版本号列表
pub(crate) fn value_versions(value: &serde_json::Value) -> Vec<String> {
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| match item {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Object(map) => map
                .get("version")
                .and_then(|v| v.as_str())
                .map(String::from),
            _ => None,
        })
        .collect()
}
