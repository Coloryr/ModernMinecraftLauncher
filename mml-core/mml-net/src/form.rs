//! 表单字段
//!
//! 单字段序列化，供 multipart 与 urlencoded 表单复用。

use serde::ser::SerializeTuple;
use serde::{Serialize, Serializer};

/// 表单请求的一个键值对
pub struct FormField<'a> {
    /// 表单字段名，必填，如 `client_id`
    pub key: &'a str,
    /// 表单字段值，按 `application/x-www-form-urlencoded` 编码后进请求体
    pub value: &'a str,
}

impl Serialize for FormField<'_> {
    /// 按 2 元组而不是结构体序列化
    ///
    /// `serde_urlencoded` 只把 2 元组当键值对，具名结构体走 `serialize_struct` 会被它判为
    /// unsupported pair，请求直接失败；按元组序列化后线格式与原先的 `(&str, &str)` 完全一致。
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element(self.key)?;
        tuple.serialize_element(self.value)?;
        tuple.end()
    }
}
