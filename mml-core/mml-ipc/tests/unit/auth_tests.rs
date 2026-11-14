use super::*;

/// 挑战串每次都换，长度固定且只含十六进制字符
#[test]
fn test_new_nonce_unique() {
    let a = new_nonce();
    let b = new_nonce();
    assert_ne!(a, b);
    assert_eq!(a.len(), 32);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

/// 同一密钥与挑战串得到同一应答，换密钥或换挑战串都不同
#[test]
fn test_proof_depends_on_key_and_nonce() {
    let base = proof("key", "nonce");
    assert_eq!(base.len(), 64);
    assert_eq!(base, proof("key", "nonce"));
    assert_ne!(base, proof("key2", "nonce"));
    assert_ne!(base, proof("key", "nonce2"));
}

/// 与 Java 端 HmacSHA256 的实测值一致：两边实现不同，算出来必须逐字符相同
#[test]
fn test_proof_matches_java() {
    assert_eq!(
        proof("secret", "nonce"),
        "042df25f9d1981866dfcab8bce6f4e3721168616b0a02d94a879c29e9f710b58"
    );
    assert_eq!(
        proof("c0ffee", "0123456789abcdef0123456789abcdef"),
        "ad38d1b1ee0fc1fa1edd6234f49f272265174c17ef4b8faf857d192205b1d93c"
    );
}

/// 应答比较按字节全等，长度不同直接不通过
#[test]
fn test_verify() {
    let expected = proof("key", "nonce");
    assert!(verify(&expected, &expected));
    assert!(!verify(&expected, ""));
    assert!(!verify(&expected, &expected[..63]));
    // 只差最后一个字符也要判出来
    let mut last = expected[..63].to_string();
    last.push(if expected.ends_with('0') { '1' } else { '0' });
    assert!(!verify(&expected, &last));
}
