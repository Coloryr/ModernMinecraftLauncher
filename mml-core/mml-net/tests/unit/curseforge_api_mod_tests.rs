use super::*;
use crate::curseforge_api::file_obj::HashesObj;

/// 排序方式编号与 CurseForge API 的 sortField 取值一致：
/// Featured=1，Popularity=2，LastUpdated=3，Name=4，TotalDownloads=6
#[test]
fn sort_type_index() {
    assert_eq!(CurseForgeSortType::Featured.to_id(), 1);
    assert_eq!(CurseForgeSortType::Popularity.to_id(), 2);
    assert_eq!(CurseForgeSortType::LastUpdated.to_id(), 3);
    assert_eq!(CurseForgeSortType::Name.to_id(), 4);
    assert_eq!(CurseForgeSortType::TotalDownloads.to_id(), 6);
}

/// 排序方向编号：Name 为 0，其余为 1
#[test]
fn sort_type_order_index() {
    assert_eq!(CurseForgeSortType::Featured.to_order_index(), 1);
    assert_eq!(CurseForgeSortType::Popularity.to_order_index(), 1);
    assert_eq!(CurseForgeSortType::LastUpdated.to_order_index(), 1);
    assert_eq!(CurseForgeSortType::TotalDownloads.to_order_index(), 1);
    assert_eq!(CurseForgeSortType::Name.to_order_index(), 0);
}

/// 默认排序方式应为流行度
#[test]
fn sort_type_default() {
    assert!(matches!(
        CurseForgeSortType::default(),
        CurseForgeSortType::Popularity
    ));
}

/// 构造一个测试用的文件数据对象
fn make_file(id: u64, file_name: &str, sha1: Option<&str>) -> CurseForgeFileDataObj {
    CurseForgeFileDataObj {
        id,
        file_name: file_name.to_string(),
        hashes: match sha1 {
            Some(value) => vec![HashesObj {
                value: value.to_string(),
                algo: 1,
            }],
            None => Vec::new(),
        },
        ..Default::default()
    }
}

/// fix_download_url：downloadUrl 缺失时按 CF 的 CDN 规则补齐，
/// 形如 `{CDN}files/{id/1000}/{id%1000}/{文件名}`
#[test]
fn fix_download_url_when_missing() {
    let mut obj = make_file(5152805, "jei-1.20.4-forge-15.3.0.4.jar", Some("aaa"));
    assert!(obj.download_url.is_none());

    obj.fix_download_url();

    assert_eq!(
        obj.download_url.as_deref(),
        Some("https://edge.forgecdn.net/files/5152/805/jei-1.20.4-forge-15.3.0.4.jar")
    );
}

/// fix_download_url：已有下载地址时不应覆盖
#[test]
fn fix_download_url_keeps_existing() {
    let mut obj = make_file(1000, "a.jar", None);
    obj.download_url = Some(String::from("https://example.com/a.jar"));

    obj.fix_download_url();

    assert_eq!(
        obj.download_url.as_deref(),
        Some("https://example.com/a.jar")
    );
}

/// sha1_hash：algo == 1 的哈希即 SHA1
#[test]
fn sha1_hash_extraction() {
    let mut obj = make_file(1, "a.jar", None);
    obj.hashes = vec![
        HashesObj {
            value: String::from("md5-value"),
            algo: 2,
        },
        HashesObj {
            value: String::from("sha1-value"),
            algo: 1,
        },
    ];
    assert_eq!(obj.sha1_hash(), "sha1-value");

    // 无哈希时返回空字符串
    let obj = make_file(1, "a.jar", None);
    assert_eq!(obj.sha1_hash(), "");
}

