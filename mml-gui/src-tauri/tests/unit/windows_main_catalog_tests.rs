use super::{MotdAddr, parse_motd_addr};

/// 造一个期望的解析结果
///
/// - `host`: 期望主机
/// - `port`: 期望端口
fn addr(host: &str, port: u16) -> MotdAddr {
    MotdAddr {
        host: host.into(),
        port,
    }
}

/// 各种地址写法的解析结果，含空串与裸 IPv6
#[test]
fn test_parse_motd_addr() {
    assert_eq!(
        parse_motd_addr("mc.example.com"),
        addr("mc.example.com", 25565)
    );
    assert_eq!(
        parse_motd_addr("mc.example.com:25566"),
        addr("mc.example.com", 25566)
    );
    assert_eq!(
        parse_motd_addr("  mc.example.com:25566 "),
        addr("mc.example.com", 25566)
    );
    assert_eq!(parse_motd_addr("[::1]:25565"), addr("::1", 25565));
    assert_eq!(parse_motd_addr("::1"), addr("::1", 25565));
    assert_eq!(parse_motd_addr(""), addr("", 25565));
}

