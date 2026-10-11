//! 字节缓冲区扩展读写
//!
//! 为 [`BytesMut`] 补充 IPC 消息体所需的字段编解码方法，
//! 布局与消息格式约定一致：大端序，字符串带 i32 长度前缀。

use bytes::{Buf, BufMut, BytesMut};

/// 字节缓冲区的字符串/布尔扩展读写
pub trait ByteBufExt {
    /// 读取 1 字节布尔值，非 0 为 `true`
    ///
    /// # 返回值
    ///
    /// 字节为 0 返回 `false`，其余值返回 `true`
    fn read_bool(&mut self) -> bool;
    /// 读取带 i32 长度前缀的 UTF-8 字符串
    ///
    /// # 返回值
    ///
    /// 返回读出的字符串，非法 UTF-8 时返回空字符串
    fn read_string(&mut self) -> String;
    /// 读取字符串列表，i32 数量前缀后逐个字符串
    ///
    /// # 返回值
    ///
    /// 返回读出的字符串列表，数量为 0 时为空列表
    fn read_string_list(&mut self) -> Vec<String>;
    /// 写入带 i32 长度前缀的字符串
    ///
    /// - `s`: 待写入的字符串
    fn write_string(&mut self, s: &str);
    /// 写入字符串列表，i32 数量前缀后逐个字符串
    ///
    /// - `list`: 待写入的字符串列表
    fn write_string_list(&mut self, list: &[String]);
}

impl ByteBufExt for BytesMut {
    fn read_bool(&mut self) -> bool {
        self.get_u8() != 0
    }

    fn read_string(&mut self) -> String {
        let len = self.get_i32() as usize;
        let bytes = self.split_to(len).to_vec();
        String::from_utf8(bytes).unwrap_or_else(|_| String::new())
    }

    fn read_string_list(&mut self) -> Vec<String> {
        let count = self.get_i32() as usize;
        (0..count).map(|_| self.read_string()).collect()
    }

    fn write_string(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.put_i32(bytes.len() as i32);
        self.put_slice(bytes);
    }

    fn write_string_list(&mut self, list: &[String]) {
        self.put_i32(list.len() as i32);
        for s in list {
            self.write_string(s);
        }
    }
}

#[cfg(test)]
mod tests {
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

    /// 长度前缀应与写入字节数一致（手动校验编码布局）
    #[test]
    fn test_byte_buf_string_layout() {
        let mut buf = BytesMut::new();
        buf.write_string("abc");

        // [i32 大端长度 3]["abc"]
        assert_eq!(&buf[..4], &3i32.to_be_bytes());
        assert_eq!(&buf[4..], b"abc");
    }
}
