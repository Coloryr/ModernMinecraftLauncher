use std::path::PathBuf;

use mml_base::hash_helper::{self, HashType};

/// 校验 gen_hash 的 MD5 / SHA1 / SHA256 结果与 Base64 往返
#[test]
fn test_hash_functions() {
    let data = b"Hello, World!";

    let md5 = hash_helper::gen_hash(HashType::Md5, data);
    assert_eq!(md5, "65a8e27d8879283831b664bd8b7f0ad4");

    let sha1 = hash_helper::gen_hash(HashType::Sha1, data);
    assert_eq!(sha1, "0a0a9f2a6772942557ab5355d76af442f8f65e01");

    let sha256 = hash_helper::gen_hash(HashType::Sha256, data);
    assert_eq!(
        sha256,
        "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
    );

    let input = "Hello, World!";
    let base64 = hash_helper::gen_base64(input);
    assert_eq!(base64, "SGVsbG8sIFdvcmxkIQ==");

    let decoded = hash_helper::de_base64(&base64).unwrap();
    assert_eq!(decoded, input);
}

/// 校验 gen_hash_from_string 对中文文本的结果
#[test]
fn test_string_input() {
    let text = "测试文本";

    let md5 = hash_helper::gen_hash_from_string(HashType::Md5, text);
    assert_eq!(md5, "d9f09b7badda07e6db79008e3e05a69d");

    let sha1 = hash_helper::gen_hash_from_string(HashType::Sha1, text);
    assert_eq!(sha1, "ae208e73a1e84916fdeb6f30b20e9e8e5baa46f7");

    let sha256 = hash_helper::gen_hash_from_string(HashType::Sha256, text);
    assert_eq!(
        sha256,
        "570ea553d3e66a2c9076c8f51a54d4730359d30c8cfc66a57e3fba8657cc62f4"
    );
}

/// 校验 gen_hash_from_file 对样本文件 1 的结果
#[test]
fn test_file1_input() {
    let file = PathBuf::from("tests").join("hash_test_file1.bin");

    let md5 = hash_helper::gen_hash_from_file(HashType::Md5, &file).unwrap();
    assert_eq!(md5, "c5ac6acdeb625abf8b6566f4cb57f6b4");

    let sha1 = hash_helper::gen_hash_from_file(HashType::Sha1, &file).unwrap();
    assert_eq!(sha1, "d950b265f5f00896f72eacbd848397f362655e24");

    let sha256 = hash_helper::gen_hash_from_file(HashType::Sha256, &file).unwrap();
    assert_eq!(
        sha256,
        "cfb8f10c6cf3dcef0a0ffc33307bed3615a9d1875ddd810c7e504c0fccb74c78"
    );
}

/// 校验 gen_hash_from_file 对样本文件 2 的结果
#[test]
fn test_file2_input() {
    let file = PathBuf::from("tests").join("hash_test_file2.bin");

    let md5 = hash_helper::gen_hash_from_file(HashType::Md5, &file).unwrap();
    assert_eq!(md5, "8dfcfe8d2525ac867dba47e8e6c763d6");

    let sha1 = hash_helper::gen_hash_from_file(HashType::Sha1, &file).unwrap();
    assert_eq!(sha1, "0f8639b8298fa1f461ba25331dfe3fc28cd19bb8");

    let sha256 = hash_helper::gen_hash_from_file(HashType::Sha256, &file).unwrap();
    assert_eq!(
        sha256,
        "426d7ee95476b497b4e54bc55e58a6e24674c53d3bab6cd87405a1acfea06aa8"
    );
}
