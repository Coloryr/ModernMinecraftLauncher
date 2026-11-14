//! 游戏连接握手：挑战应答
//!
//! 启动器给每条连接现生成一个挑战串下发，游戏用启动参数里的密钥对它算 HMAC-SHA256，
//! 把不可逆的结果发回；启动器用同一份密钥再算一遍比对。密钥不过网，挑战串每次都换，
//! 抓到的应答换一条连接就用不了。

use mml_base::hash_helper::gen_hmac_sha256;
use uuid::Uuid;

/// 生成本次连接的挑战串
///
/// # 返回值
///
/// 32 位小写十六进制串，取自随机 UUID，每次调用都不同
pub(crate) fn new_nonce() -> String {
    Uuid::new_v4().simple().to_string()
}

/// 计算挑战应答
///
/// - `key`: 启动时随启动参数下发给游戏的验证密钥
/// - `nonce`: 本次连接的挑战串
///
/// # 返回值
///
/// 64 位小写十六进制串；结果不可反推密钥
pub(crate) fn proof(key: &str, nonce: &str) -> String {
    gen_hmac_sha256(key, nonce)
}

/// 常量时间比较两个应答串
///
/// - `expected`: 启动器自己算出的应答
/// - `got`: 游戏发回的应答
///
/// # 返回值
///
/// 完全一致返回 `true`；长度不同或任一字节不同返回 `false`
pub(crate) fn verify(expected: &str, got: &str) -> bool {
    let (expected, got) = (expected.as_bytes(), got.as_bytes());
    if expected.len() != got.len() {
        return false;
    }
    // 逐字节异或累积后再判零，不做提前返回，避免按响应时间逐位试出应答
    expected
        .iter()
        .zip(got)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
#[path = "../tests/unit/auth_tests.rs"]
mod tests;
