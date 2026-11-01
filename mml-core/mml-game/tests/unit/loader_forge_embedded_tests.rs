use super::*;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Once;
use uuid::Uuid;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// 起内核系统并初始化运行库目录，每进程一次
///
/// [`crate::test_support::boot`] 不含本 crate 的运行库目录，而用例要往它下面落文件；
/// 该初始化是 `get_or_init`，与别的测试模块重复调用是安全的。
fn setup() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let run = crate::test_support::boot();
        libraries_path::init(&run).unwrap();
    });
}

/// 造一个只含指定条目的安装器压缩包
///
/// - `entries`: 包内路径与内容
///
/// # 返回值
///
/// 返回压缩包路径
fn make_installer(entries: &[(&str, &[u8])]) -> PathBuf {
    let path = mml_testutil::temp_dir().join(format!("mml-forge-{}.zip", Uuid::new_v4()));
    let file = std::fs::File::create(&path).unwrap();
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for (name, data) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
    path
}

/// 造一个 url 为空的运行库条目
///
/// - `path`: 运行库在 Maven 仓库内的相对路径
/// - `sha1`: 期望的 SHA1 值
///
/// # 返回值
///
/// 返回运行库条目
fn embedded_lib(path: &str, sha1: &str) -> ForgeLibrariesObj {
    ForgeLibrariesObj {
        name: format!("test:embedded:{path}"),
        downloads: ForgeDownloadsObj {
            artifact: ArtifactObj {
                path: path.to_string(),
                url: String::new(),
                sha1: sha1.to_string(),
            },
        },
    }
}

/// 打开一个压缩包
fn open_zip(path: &PathBuf) -> ZipArchive<std::fs::File> {
    ZipArchive::new(path_helper::open_read(path).unwrap()).unwrap()
}

/// 运行库在运行库目录下的落点
fn local_of(path: &str) -> PathBuf {
    libraries_path::get_lib_dir().join(path)
}

/// 空 url 的运行库要从安装器 jar 的 `maven/` 下解到运行库目录
#[test]
fn extracts_embedded_lib() {
    setup();
    let data: &[u8] = b"embedded-jar-bytes";
    let rel = format!("test/embedded/{}/lib.jar", Uuid::new_v4().simple());
    let sha1 = hash_helper::gen_hash(HashType::Sha1, data);

    let entry = format!("maven/{rel}");
    let installer = make_installer(&[(&entry, data)]);
    let libs = [embedded_lib(&rel, &sha1)];

    extract_embedded_libs(&mut open_zip(&installer), &libs).unwrap();

    assert_eq!(
        std::fs::read(local_of(&rel)).unwrap(),
        data,
        "内嵌的运行库应被解到运行库目录"
    );
}

/// 安装器 jar 里没有该条目时什么都不落，也不报错
#[test]
fn missing_entry_is_skipped() {
    setup();
    let rel = format!("test/missing/{}/lib.jar", Uuid::new_v4().simple());
    let installer = make_installer(&[("version.json", b"{}")]);
    let libs = [embedded_lib(&rel, "00")];

    extract_embedded_libs(&mut open_zip(&installer), &libs).unwrap();

    assert!(!local_of(&rel).exists(), "jar 里没有的条目不该落盘");
}

/// 本地已有且校验值相符时不再重解一遍
#[test]
fn existing_lib_is_kept() {
    setup();
    let data: &[u8] = b"local-jar-bytes";
    let rel = format!("test/kept/{}/lib.jar", Uuid::new_v4().simple());
    let sha1 = hash_helper::gen_hash(HashType::Sha1, data);
    let local = local_of(&rel);
    std::fs::create_dir_all(local.parent().unwrap()).unwrap();
    std::fs::write(&local, data).unwrap();

    let entry = format!("maven/{rel}");
    let installer = make_installer(&[(&entry, b"other-bytes")]);
    let libs = [embedded_lib(&rel, &sha1)];

    extract_embedded_libs(&mut open_zip(&installer), &libs).unwrap();

    assert_eq!(
        std::fs::read(&local).unwrap(),
        data,
        "校验值相符时不该被 jar 里的内容覆盖"
    );
}

/// 空 url 且本地拿不到的条目不进下载列表，否则会发出一个空地址请求
#[tokio::test]
async fn empty_url_without_local_file_is_not_in_list() {
    setup();
    let rel = format!("test/absent/{}/lib.jar", Uuid::new_v4().simple());
    let info = vec![embedded_lib(&rel, "00")];

    let list =
        build_forge_libs(&info, "1.12.2", "14.23.5.2864", false, false, false, false).await;

    assert!(
        !list.iter().any(|item| item.url.is_empty()),
        "下载列表里不该有空地址的条目：{:?}",
        list.iter()
            .map(|item| item.name.clone())
            .collect::<Vec<_>>()
    );
}

/// 空 url 但本地已解出来的条目要留下，它得进 classpath
#[tokio::test]
async fn empty_url_with_local_file_stays_in_list() {
    setup();
    let data: &[u8] = b"local-jar-bytes";
    let rel = format!("test/present/{}/lib.jar", Uuid::new_v4().simple());
    let sha1 = hash_helper::gen_hash(HashType::Sha1, data);
    let local = local_of(&rel);
    std::fs::create_dir_all(local.parent().unwrap()).unwrap();
    std::fs::write(&local, data).unwrap();

    let info = vec![embedded_lib(&rel, &sha1)];
    let list =
        build_forge_libs(&info, "1.12.2", "14.23.5.2864", false, false, false, false).await;

    assert!(
        list.iter().any(|item| item.file == local),
        "本地已有且校验值相符的条目应留在列表里"
    );
}

