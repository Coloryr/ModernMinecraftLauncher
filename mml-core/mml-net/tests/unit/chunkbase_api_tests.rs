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

