use super::*;

#[test]
fn test_byte_buf_round_trip() {
    let mut buf = BytesMut::new();
    buf.write_string("hello");
    buf.write_string_list(&["a".to_string(), "b".to_string()]);
    buf.put_u8(1);
    buf.put_u8(0);

    assert_eq!(buf.read_string(), "hello");
    assert_eq!(
        buf.read_string_list(),
        vec!["a".to_string(), "b".to_string()]
    );
    assert!(buf.read_bool());
    assert!(!buf.read_bool());
    assert!(buf.is_empty());
}

/// 空 / 中文 / emoji 字符串应按 UTF-8 字节长度正确编解码
#[test]
fn test_byte_buf_unicode_and_empty() {
    let mut buf = BytesMut::new();
    buf.write_string("");
    buf.write_string("中文测试");
    buf.write_string("🎮ollider");

    assert_eq!(buf.read_string(), "");
    assert_eq!(buf.read_string(), "中文测试");
    assert_eq!(buf.read_string(), "🎮ollider");
    assert!(buf.is_empty());
}

/// 空列表与嵌套空字符串列表的往返
#[test]
fn test_byte_buf_string_list_edge() {
    let mut buf = BytesMut::new();
    buf.write_string_list(&[]);
    buf.write_string_list(&[String::new(), String::new()]);

    let empty: Vec<String> = buf.read_string_list();
    assert!(empty.is_empty());
    assert_eq!(buf.read_string_list(), vec![String::new(), String::new()]);
    assert!(buf.is_empty());
}

/// 布尔值只占 1 字节，非 0 值均视为 true
#[test]
fn test_byte_buf_bool_bytes() {
    let mut buf = BytesMut::new();
    buf.put_u8(1);
    buf.put_u8(0);
    buf.put_u8(42);

    assert!(buf.read_bool());
    assert!(!buf.read_bool());
    assert!(buf.read_bool());
}

/// 长度前缀应与写入字节数一致，手动校验编码布局
#[test]
fn test_byte_buf_string_layout() {
    let mut buf = BytesMut::new();
    buf.write_string("abc");

    // [i32 大端长度 3]["abc"]
    assert_eq!(&buf[..4], &3i32.to_be_bytes());
    assert_eq!(&buf[4..], b"abc");
}

