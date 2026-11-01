use super::*;

/// "hello world" 的常用哈希值
const HELLO_MD5: &str = "5eb63bbbe01eeed093cb22bb8f5acdc3";
const HELLO_SHA1: &str = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
const HELLO_SHA256: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

/// FileHash::eq 匹配逻辑
#[test]
fn test_file_hash_eq() {
    assert!(FileHash::None.eq("anything"));
    assert!(FileHash::Md5(HELLO_MD5.into()).eq(HELLO_MD5));
    assert!(!FileHash::Md5(HELLO_MD5.into()).eq("wrong"));
    assert!(FileHash::Sha1(HELLO_SHA1.into()).eq(HELLO_SHA1));
    assert!(FileHash::Sha256(HELLO_SHA256.into()).eq(HELLO_SHA256));
    assert!(FileHash::Sha512("abc".into()).eq("abc"));
    // 组合哈希任一匹配即可
    assert!(FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into()).eq(HELLO_SHA1));
    assert!(FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into()).eq(HELLO_SHA256));
    assert!(!FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into()).eq("wrong"));
    assert!(FileHash::Sha1Sha512(HELLO_SHA1.into(), "x".into()).eq(HELLO_SHA1));
}

/// get_sha1 只在包含 SHA1 的变体上返回
#[test]
fn test_get_sha1() {
    assert_eq!(FileHash::None.get_sha1(), None);
    assert_eq!(FileHash::Md5(HELLO_MD5.into()).get_sha1(), None);
    assert_eq!(FileHash::Sha256(HELLO_SHA256.into()).get_sha1(), None);
    assert_eq!(
        FileHash::Sha1(HELLO_SHA1.into()).get_sha1(),
        Some(HELLO_SHA1.to_string())
    );
    assert_eq!(
        FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into()).get_sha1(),
        Some(HELLO_SHA1.to_string())
    );
    assert_eq!(
        FileHash::Sha1Sha512(HELLO_SHA1.into(), "x".into()).get_sha1(),
        Some(HELLO_SHA1.to_string())
    );
}

/// 在临时目录创建内容为 "hello world" 的文件
fn make_file() -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml_base_file_item_test_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("hello.txt");
    std::fs::write(&file, b"hello world").unwrap();
    file
}

/// 构造文件项
fn make_item(file: PathBuf, hash: FileHash) -> FileItemObj {
    FileItemObj {
        name: "hello.txt".to_string(),
        file,
        url: String::new(),
        hash,
        later: LaterRun::None,
    }
}

/// check_hash 校验逻辑
#[test]
fn test_check_hash() {
    let file = make_file();

    // 不校验
    assert!(make_item(file.clone(), FileHash::None).check_hash());
    // MD5 正确，大小写不敏感
    assert!(make_item(file.clone(), FileHash::Md5(HELLO_MD5.into())).check_hash());
    assert!(make_item(file.clone(), FileHash::Md5(HELLO_MD5.to_uppercase())).check_hash());
    // MD5 错误
    assert!(!make_item(file.clone(), FileHash::Md5("deadbeef".into())).check_hash());
    // SHA1 / SHA256 正确
    assert!(make_item(file.clone(), FileHash::Sha1(HELLO_SHA1.into())).check_hash());
    assert!(make_item(file.clone(), FileHash::Sha256(HELLO_SHA256.into())).check_hash());
    // 组合哈希两者都对
    assert!(
        make_item(
            file.clone(),
            FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into())
        )
        .check_hash()
    );
    // 组合哈希 SHA1 对但 SHA256 错
    assert!(
        !make_item(
            file.clone(),
            FileHash::Sha1Sha256(HELLO_SHA1.into(), "bad".into())
        )
        .check_hash()
    );
    // 组合哈希 SHA1 错
    assert!(
        !make_item(file.clone(), FileHash::Sha1Sha512("bad".into(), "x".into())).check_hash()
    );

    // 文件不存在
    assert!(!make_item(file.join("no_such_file"), FileHash::None).check_hash());

    let _ = std::fs::remove_dir_all(file.parent().unwrap());
}

/// LaterRun / FileHash 的 Default 实现
#[test]
fn test_defaults() {
    assert!(matches!(FileHash::default(), FileHash::None));
    assert!(matches!(LaterRun::default(), LaterRun::None));
}

