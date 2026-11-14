use super::*;
use std::io::Cursor;
use std::path::PathBuf;

/// 已知标准哈希值
#[test]
fn test_gen_hash_known_values() {
    assert_eq!(
        gen_hash(HashType::Md5, b""),
        "d41d8cd98f00b204e9800998ecf8427e"
    );
    assert_eq!(
        gen_hash(HashType::Md5, b"abc"),
        "900150983cd24fb0d6963f7d28e17f72"
    );
    assert_eq!(
        gen_hash(HashType::Sha1, b"abc"),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    assert_eq!(
        gen_hash(HashType::Sha256, b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        gen_hash(HashType::Sha512, b"abc"),
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
         2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
    );
}

/// 从字符串生成哈希与 gen_hash 一致
#[test]
fn test_gen_hash_from_string() {
    assert_eq!(
        gen_hash_from_string(HashType::Md5, "abc"),
        gen_hash(HashType::Md5, b"abc")
    );
}

/// HMAC-SHA256：RFC 4231 第 1 组用例，密钥 20 个 0x0b
#[test]
fn test_gen_hmac_sha256_rfc4231() {
    let key = "\x0b".repeat(20);
    assert_eq!(
        gen_hmac_sha256(&key, "Hi There"),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );

    // 结果固定长度的小写十六进制，密钥不同结果不同
    let a = gen_hmac_sha256("secret", "nonce");
    assert_eq!(a.len(), 64);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(a, gen_hmac_sha256("secret2", "nonce"));
    assert_ne!(a, gen_hmac_sha256("secret", "nonce2"));
}

/// 从数据流生成哈希，数据跨缓冲区分块
#[test]
fn test_gen_hash_from_reader() {
    let data = vec![7u8; 4096]; // 超过 1024 缓冲区，触发多次读取
    let mut reader = Cursor::new(data.clone());
    let hash = gen_hash_from_reader(HashType::Sha256, &mut reader).unwrap();
    assert_eq!(hash, gen_hash(HashType::Sha256, &data));

    // 空流
    let mut empty = Cursor::new(Vec::new());
    assert_eq!(
        gen_hash_from_reader(HashType::Md5, &mut empty).unwrap(),
        "d41d8cd98f00b204e9800998ecf8427e"
    );
}

/// 异步从数据流生成哈希
#[tokio::test]
async fn test_gen_hash_from_reader_async() {
    let data = vec![1u8; 3000];
    let mut reader = Cursor::new(data.clone());
    let hash = gen_hash_from_reader_async(HashType::Sha1, &mut reader)
        .await
        .unwrap();
    assert_eq!(hash, gen_hash(HashType::Sha1, &data));
}

/// 在临时目录创建唯一文件并返回路径
fn temp_file(tag: &str, content: &[u8]) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml_base_hash_test_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(tag);
    std::fs::write(&file, content).unwrap();
    file
}

/// 清理临时目录
fn cleanup(file: &PathBuf) {
    if let Some(parent) = file.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

/// 从文件生成哈希，同步与异步两条路径
#[test]
fn test_gen_hash_from_file() {
    let file = temp_file("a.txt", b"abc");
    let hash = gen_hash_from_file(HashType::Md5, &file).unwrap();
    assert_eq!(hash, "900150983cd24fb0d6963f7d28e17f72");

    // 不存在的文件报错
    assert!(
        gen_hash_from_file(HashType::Md5, file.parent().unwrap().join("no_such_file")).is_err()
    );

    cleanup(&file);
}

/// 异步从文件生成哈希
#[tokio::test]
async fn test_gen_hash_from_file_async() {
    let file = temp_file("b.txt", b"abc");
    let hash = gen_hash_from_file_async(HashType::Sha1, &file)
        .await
        .unwrap();
    assert_eq!(hash, "a9993e364706816aba3e25717850c26c9cd0d89d");
    cleanup(&file);
}

/// Base64 编解码
#[test]
fn test_base64() {
    assert_eq!(gen_base64("hello"), "aGVsbG8=");
    assert_eq!(de_base64("aGVsbG8=").unwrap(), "hello");

    // 中文往返
    let text = "你好，世界！";
    assert_eq!(de_base64(&gen_base64(text)).unwrap(), text);

    // 非法 base64
    assert!(de_base64("!!not-base64!!").is_err());
    // 合法 base64 但不是合法 UTF-8
    assert!(de_base64("//8=").is_err());
}

