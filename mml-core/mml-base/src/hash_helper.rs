//! 哈希计算和 Base64 编解码模块
//!
//! 提供 MD5、SHA1、SHA256、SHA512 哈希计算、HMAC-SHA256 以及 Base64 编解码功能。
//! 支持多种输入源：字节数组、字符串、文件、读取流，同步与异步两种

use std::{io::Read, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, KeyInit, Mac};
use md5::Md5;
use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};
use mml_sys::path_helper;
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use tokio::io::{AsyncRead, AsyncReadExt};

use digest::{Digest, DynDigest};

use crate::file_item::FileHash;

/// 读流出错转 StreamError
fn stream_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::StreamError(ErrorData::from_error(err))
}

/// base64 出错转 Base64Error
fn base64_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::Base64Error(ErrorData::from_error(err))
}

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

/// 用密钥对文本做 HMAC-SHA256
///
/// 结果不可逆，用于「知道密钥才能算出来」的校验，密钥本身不出现在结果里
///
/// - `key`: 密钥，长度不限
/// - `data`: 参与计算的文本，按 UTF-8 编码
///
/// # 返回值
///
/// 返回小写十六进制串，长度 64
pub fn gen_hmac_sha256(key: &str, data: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC 接受任意长度的密钥");
    mac.update(data.as_bytes());
    bytes_to_hex(&mac.finalize().into_bytes())
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
        let len = reader.read(&mut buffer).map_err(|err| stream_err(err))?;

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
        let len = reader
            .read(&mut buffer)
            .await
            .map_err(|err| stream_err(err))?;

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

/// 读一遍文件，同时算出 SHA1 与 SHA512
///
/// 两种校验各读一遍整份文件是导出时的瓶颈，合在一次读里
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 见 [`FileHash`]，恒为 [`FileHash::Sha1Sha512`]；文件无法打开或读取失败时返回相应错误
pub fn gen_sha1_sha512_from_file<P: AsRef<Path>>(file: P) -> CoreResult<FileHash> {
    let mut file = path_helper::open_read(file)?;
    let mut sha1 = create_hasher(HashType::Sha1);
    let mut sha512 = create_hasher(HashType::Sha512);
    let mut buffer = vec![0u8; 256 * 1024];

    loop {
        let len = file.read(&mut buffer).map_err(stream_err)?;
        if len == 0 {
            break;
        }
        sha1.update(&buffer[..len]);
        sha512.update(&buffer[..len]);
    }

    Ok(FileHash::Sha1Sha512(
        bytes_to_hex(&sha1.finalize()).to_ascii_lowercase(),
        bytes_to_hex(&sha512.finalize()).to_ascii_lowercase(),
    ))
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
    let bytes = BASE64.decode(input).map_err(|err| base64_err(err))?;
    Ok(String::from_utf8(bytes).map_err(|err| base64_err(err))?)
}

#[cfg(test)]
#[path = "../tests/unit/hash_helper_tests.rs"]
mod tests;
