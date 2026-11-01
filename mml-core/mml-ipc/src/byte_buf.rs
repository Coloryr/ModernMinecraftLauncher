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
#[path = "../tests/unit/byte_buf_tests.rs"]
mod tests;
