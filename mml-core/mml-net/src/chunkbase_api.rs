use mml_base::version_parse;

use crate::urls;

/// 生成 Chunkbase 网址
///
/// - `version`: 游戏版本
/// - `seed`: 世界种子
/// - `islb`: 是否为巨型生物群系
///
/// # 返回值
///
/// 返回 Chunkbase 种子地图页面 URL；版本过低或解析失败时按 1.7 处理
pub fn gen_url(version: &str, seed: i64, islb: bool) -> String {
    let res = version_parse::parse_game_version(version);
    let ver = if let Some(vec) = res {
        if vec >= vec![26, 2] {
            "26_2"
        } else if vec >= vec![26, 1] {
            "26_1"
        } else if vec >= vec![1, 21, 9] {
            "1_21_9"
        } else if vec >= vec![1, 21, 6] {
            "1_21_6"
        } else if vec >= vec![1, 21, 5] {
            "1_21_5"
        } else if vec >= vec![1, 21, 4] {
            "1_21_4"
        } else if vec >= vec![1, 21, 2] {
            "1_21_2"
        } else if vec >= vec![1, 21] {
            "1_21"
        } else if vec >= vec![1, 20] {
            "1_20"
        } else if vec >= vec![1, 19, 3] {
            "1_19_3"
        } else if vec >= vec![1, 19] {
            "1_19"
        } else if vec >= vec![1, 18] {
            "1_18"
        } else if vec >= vec![1, 17] {
            "1_17"
        } else if vec >= vec![1, 16] {
            "1_16"
        } else if vec >= vec![1, 15] {
            "1_15"
        } else if vec >= vec![1, 14] {
            "1_14"
        } else if vec >= vec![1, 13] {
            "1_13"
        } else if vec >= vec![1, 12] {
            "1_12"
        } else if vec >= vec![1, 11] {
            "1_11"
        } else if vec >= vec![1, 10] {
            "1_10"
        } else if vec >= vec![1, 9] {
            "1_9"
        } else if vec >= vec![1, 8] {
            "1_8"
        } else {
            "1_7"
        }
    } else {
        "1_7"
    };

    format!(
        "{}seed={seed}&platform=java_{ver}{}&dimension=overworld&x=0&z=0&zoom=0.5",
        urls::CHUNKBASE,
        if islb { "_lb" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chunkbase 网址基础结构
    #[test]
    fn url_structure() {
        assert_eq!(
            gen_url("1.20.4", 123456789, false),
            "https://www.chunkbase.com/apps/seed-map#seed=123456789&platform=java_1_20&dimension=overworld&x=0&z=0&zoom=0.5"
        );
    }

    /// 巨型生物群系后缀 _lb
    #[test]
    fn large_biomes_suffix() {
        let url = gen_url("1.20.4", 1, true);
        assert!(url.contains("platform=java_1_20_lb"));
    }

    /// 旧版本回落到 1_7
    #[test]
    fn old_version_fallback() {
        let url = gen_url("1.7.10", 1, false);
        assert!(url.contains("platform=java_1_7"));
    }

    /// 逐级版本映射
    #[test]
    fn version_mapping() {
        for (version, platform) in [
            ("1.8.9", "java_1_8"),
            ("1.12.2", "java_1_12"),
            ("1.16.5", "java_1_16"),
            ("1.21", "java_1_21"),
            ("1.21.4", "java_1_21_4"),
            ("1.21.9", "java_1_21_9"),
            ("26.2", "java_26_2"),
        ] {
            let url = gen_url(version, 1, false);
            assert!(
                url.contains(&format!("platform={platform}&")),
                "版本 {version} 应映射到 {platform}，实际为 {url}"
            );
        }
    }

    /// 负数种子应原样输出
    #[test]
    fn negative_seed() {
        let url = gen_url("1.20.4", -123, false);
        assert!(url.contains("seed=-123&"));
    }
}
