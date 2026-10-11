//! 哈希计算和 Base64 编解码模块
//!
//! 提供 MD5、SHA1、SHA256、SHA512 哈希计算以及 Base64 编解码功能。
//! 支持多种输入源：字节数组、字符串、文件、读取流，同步与异步两种

use std::{io::Read, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use md5::Md5;
use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};
use mml_sys::path_helper;
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use tokio::io::{AsyncRead, AsyncReadExt};

use digest::{Digest, DynDigest};

/// 哈希算法类型，决定输出校验串的长度
pub enum HashType {
    /// MD5，输出 32 位十六进制串
    Md5,
    /// SHA1，输出 40 位十六进制串
    Sha1,
    /// SHA256，输出 64 位十六进制串
    Sha256,
    /// SHA512，输出 128 位十六进制串
    Sha512,
}

/// 将字节数组格式化为小写十六进制字符串
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 按算法类型创建哈希器
fn create_hasher(hash_type: HashType) -> Box<dyn DynDigest + Send> {
    match hash_type {
        HashType::Md5 => Box::new(Md5::new()),
        HashType::Sha1 => Box::new(Sha1::new()),
        HashType::Sha256 => Box::new(Sha256::new()),
        HashType::Sha512 => Box::new(Sha512::new()),
    }
}

/// 从字节数组生成校验值
///
/// - `hash_type`: 校验算法
/// - `data`: 需要计算的数据
///
/// # 返回值
///
/// 返回小写十六进制校验串
pub fn gen_hash(hash_type: HashType, data: &[u8]) -> String {
    let mut hasher = create_hasher(hash_type);
    hasher.update(data);
    bytes_to_hex(&hasher.finalize()).to_ascii_lowercase()
}

/// 从字符串生成校验值
///
/// - `hash_type`: 校验算法
/// - `data`: 需要计算的数据
///
/// # 返回值
///
/// 返回小写十六进制校验串
pub fn gen_hash_from_string(hash_type: HashType, data: &str) -> String {
    let mut hasher = create_hasher(hash_type);
    hasher.update(data.as_bytes());
    bytes_to_hex(&hasher.finalize()).to_ascii_lowercase()
}

/// 从数据流生成校验值，分块读取直到流结束
///
/// - `hash_type`: 校验算法
/// - `reader`: 需要计算的数据流
///
/// # 返回值
///
/// 返回小写十六进制校验串；读取失败时返回相应错误
pub fn gen_hash_from_reader<R: Read>(hash_type: HashType, reader: &mut R) -> CoreResult<String> {
    let mut hasher = create_hasher(hash_type);
    let mut buffer = [0u8; 1024];

    loop {
        let len = reader.read(&mut buffer).map_err(|err| {
            ErrorType::StreamError(ErrorData {
                error: err.to_string(),
            })
        })?;

        if len == 0 {
            break;
        }
        hasher.update(&buffer[..len]);
    }
    Ok(bytes_to_hex(&hasher.finalize()).to_ascii_lowercase())
}

/// 异步从数据流生成校验值，分块读取直到流结束
///
/// - `hash_type`: 校验算法
/// - `reader`: 需要计算的数据流
///
/// # 返回值
///
/// 返回小写十六进制校验串；读取失败时返回相应错误
pub async fn gen_hash_from_reader_async<R: AsyncRead + Unpin>(
    hash_type: HashType,
    reader: &mut R,
) -> CoreResult<String> {
    let mut hasher = create_hasher(hash_type);
    let mut buffer = [0u8; 1024];

    loop {
        let len = reader.read(&mut buffer).await.map_err(|err| {
            ErrorType::StreamError(ErrorData {
                error: err.to_string(),
            })
        })?;

        if len == 0 {
            break;
        }
        hasher.update(&buffer[..len]);
    }
    Ok(bytes_to_hex(&hasher.finalize()).to_ascii_lowercase())
}

/// 从文件生成校验值
///
/// - `hash_type`: 校验算法
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回小写十六进制校验串；文件无法打开或读取失败时返回相应错误
pub fn gen_hash_from_file<P: AsRef<Path>>(hash_type: HashType, file: P) -> CoreResult<String> {
    let mut file = path_helper::open_read(file)?;
    gen_hash_from_reader(hash_type, &mut file)
}

/// 异步从文件生成校验值
///
/// - `hash_type`: 校验算法
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回小写十六进制校验串；文件无法打开或读取失败时返回相应错误
pub async fn gen_hash_from_file_async<P: AsRef<Path>>(
    hash_type: HashType,
    file: P,
) -> CoreResult<String> {
    let mut file = path_helper::open_read_async(file).await?;
    gen_hash_from_reader_async(hash_type, &mut file).await
}

/// 生成字符串的 Base64
///
/// - `input`: 需要编码的文本
///
/// # 返回值
///
/// 返回标准 Base64 字符串
pub fn gen_base64(input: &str) -> String {
    BASE64.encode(input.as_bytes())
}

/// 生成二进制数据的 Base64
///
/// - `input`: 需要编码的字节，如图片内容
///
/// # 返回值
///
/// 返回标准 Base64 字符串
pub fn gen_base64_bytes(input: &[u8]) -> String {
    BASE64.encode(input)
}

/// 反解 Base64 为文本
///
/// - `input`: Base64 字符串
///
/// # 返回值
///
/// 返回解码后的文本；Base64 非法或结果不是合法 UTF-8 时返回相应错误
pub fn de_base64(input: &str) -> CoreResult<String> {
    let bytes = BASE64.decode(input).map_err(|err| {
        ErrorType::Base64Error(ErrorData {
            error: err.to_string(),
        })
    })?;
    Ok(String::from_utf8(bytes).map_err(|err| {
        ErrorType::Base64Error(ErrorData {
            error: err.to_string(),
        })
    })?)
}

#[cfg(test)]
mod tests {
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
}
