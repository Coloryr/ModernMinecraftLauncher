use std::fs;
use std::path::PathBuf;

use mml_base::file_item::FileHash;

use super::*;

/// "hello" 的标准哈希值，公开测试向量
const HELLO_MD5: &str = "5d41402abc4b2a76b9719d911017c592";
const HELLO_SHA1: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
const HELLO_SHA512: &str = "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043";

/// 在临时目录创建内容为 "hello" 的测试文件
///
/// - `name`: 临时子目录的用例特征
///
/// # 返回值
///
/// 返回已写入 `hello` 的文件路径
fn make_hello_file(name: &str) -> PathBuf {
    let dir = crate::test_util::make_temp_dir(name);
    let file = dir.join("hello.txt");
    fs::write(&file, b"hello").unwrap();
    file
}

/// 尝试次数未达上限 3 次时继续重试
#[test]
fn is_need_err_allows_under_threshold() {
    crate::test_util::ensure_env();

    // 日志要文件名与地址，测试里给个空项就够
    let item = mml_base::file_item::FileItemObj {
        name: String::new(),
        file: Default::default(),
        url: String::new(),
        hash: Default::default(),
        later: Default::default(),
    };
    let mut times = 0;
    for _ in 0..2 {
        assert!(!is_need_err(&item, ErrorType::TaskCancel, &mut times));
    }
    assert_eq!(times, 2);
}

/// 尝试次数达到上限 3 次后放弃
#[test]
fn is_need_err_stops_over_threshold() {
    crate::test_util::ensure_env();

    let item = mml_base::file_item::FileItemObj {
        name: String::new(),
        file: Default::default(),
        url: String::new(),
        hash: Default::default(),
        later: Default::default(),
    };
    let mut times = 2;
    assert!(is_need_err(&item, ErrorType::TaskCancel, &mut times));
    assert_eq!(times, 3);
}

/// 无需校验时直接通过
#[test]
fn check_hash_none_passes() {
    let file = make_hello_file("hash-none");
    let mut stream = fs::File::open(&file).unwrap();
    assert!(check_hash(&file, &FileHash::None, &mut stream).is_ok());
}

/// 单哈希 MD5/SHA1/SHA256/SHA512 匹配，且大小写不敏感
#[test]
fn check_hash_single_matches() {
    let file = make_hello_file("hash-single");
    let cases = [
        (FileHash::Md5(HELLO_MD5.to_string()), HELLO_MD5),
        (FileHash::Sha1(HELLO_SHA1.to_string()), HELLO_SHA1),
        (FileHash::Sha256(HELLO_SHA256.to_string()), HELLO_SHA256),
        (FileHash::Sha512(HELLO_SHA512.to_string()), HELLO_SHA512),
    ];

    for (hash, expected) in cases {
        // 小写匹配
        let mut stream = fs::File::open(&file).unwrap();
        assert!(
            check_hash(&file, &hash, &mut stream).is_ok(),
            "{expected} 应校验通过"
        );

        // 大写匹配，校验值大小写不敏感
        let upper = hash_to_upper(&hash);
        let mut stream = fs::File::open(&file).unwrap();
        assert!(
            check_hash(&file, &upper, &mut stream).is_ok(),
            "{expected} 大写应校验通过"
        );
    }
}

/// 单哈希不匹配时返回错误
#[test]
fn check_hash_single_mismatch() {
    let file = make_hello_file("hash-mismatch");
    let wrong = FileHash::Sha256("0".repeat(64));
    let mut stream = fs::File::open(&file).unwrap();
    let result = check_hash(&file, &wrong, &mut stream);
    assert!(result.is_err(), "错误的 SHA256 应校验失败");
}

/// 期望值是空串时按"没公布哈希"处理：不校验、也不判失败
///
/// Fabric 的 loader jar 就是这样：上游 JSON 里声明了 sha256 字段但值是空
#[test]
fn check_hash_empty_expected_passes() {
    let file = make_hello_file("hash-empty");
    let empty = FileHash::Sha256(String::new());
    let mut stream = fs::File::open(&file).unwrap();
    assert!(
        check_hash(&file, &empty, &mut stream).is_ok(),
        "空期望值不该判失败"
    );
}

/// 组合校验 SHA1+SHA256：两者都匹配才通过
#[test]
fn check_hash_sha1_sha256() {
    let file = make_hello_file("hash-combo256");

    // 都匹配
    let mut stream = fs::File::open(&file).unwrap();
    let combo = FileHash::Sha1Sha256(HELLO_SHA1.to_string(), HELLO_SHA256.to_string());
    assert!(check_hash(&file, &combo, &mut stream).is_ok());

    // SHA1 匹配但 SHA256 错误
    let mut stream = fs::File::open(&file).unwrap();
    let combo = FileHash::Sha1Sha256(HELLO_SHA1.to_string(), "0".repeat(64));
    assert!(check_hash(&file, &combo, &mut stream).is_err());

    // SHA1 错误时短路，不再校验第二个
    let mut stream = fs::File::open(&file).unwrap();
    let combo = FileHash::Sha1Sha256("0".repeat(40), HELLO_SHA256.to_string());
    assert!(check_hash(&file, &combo, &mut stream).is_err());
}

/// 组合校验 SHA1+SHA512：两者都匹配才通过
#[test]
fn check_hash_sha1_sha512() {
    let file = make_hello_file("hash-combo512");

    // 都匹配
    let mut stream = fs::File::open(&file).unwrap();
    let combo = FileHash::Sha1Sha512(HELLO_SHA1.to_string(), HELLO_SHA512.to_string());
    assert!(check_hash(&file, &combo, &mut stream).is_ok());

    // SHA1 匹配但 SHA512 错误
    let mut stream = fs::File::open(&file).unwrap();
    let combo = FileHash::Sha1Sha512(HELLO_SHA1.to_string(), "0".repeat(128));
    assert!(check_hash(&file, &combo, &mut stream).is_err());

    // SHA1 错误
    let mut stream = fs::File::open(&file).unwrap();
    let combo = FileHash::Sha1Sha512("0".repeat(40), HELLO_SHA512.to_string());
    assert!(check_hash(&file, &combo, &mut stream).is_err());
}

/// 将校验值转为大写，用于测试大小写不敏感比较
fn hash_to_upper(hash: &FileHash) -> FileHash {
    match hash {
        FileHash::Md5(v) => FileHash::Md5(v.to_uppercase()),
        FileHash::Sha1(v) => FileHash::Sha1(v.to_uppercase()),
        FileHash::Sha256(v) => FileHash::Sha256(v.to_uppercase()),
        FileHash::Sha512(v) => FileHash::Sha512(v.to_uppercase()),
        _ => unreachable!("仅用于单哈希测试"),
    }
}

