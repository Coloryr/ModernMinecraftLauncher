//! 序列化工具模块
//!
//! 提供 JSON 和 TOML 格式的解析、序列化工具函数，
//! 以及简化的值访问封装 `MiniJsonObj`、`MiniTomlMap` 和
//! 自定义反序列化器 `deserialize_number_or_min` / `deserialize_number_or_max`。
//!
//! # 简化访问封装
//!
//! - [`MiniJsonObj`] / [`MiniJsonMap`] — JSON 值的类型安全访问
//! - [`MiniTomlObj`] / [`MiniTomlMap`] — TOML 值的类型安全访问

use std::{
    cmp,
    collections::{HashMap, hash_map},
    io::Read,
    path::Path,
};

use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};
use mml_sys::path_helper;
use serde::{Serialize, de};
use serde_json::Number;

/// 出错原因转 SerializerError
fn serializer_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::SerializerError(ErrorData::from_error(err))
}

/// 包装一个 JSON 值，提供类型安全的访问接口
pub struct MiniJsonObj {
    /// 被包装的原始 JSON 值
    value: serde_json::Value,
}

/// JSON 对象的键值对视图，键为字段名
#[derive(Default)]
pub struct MiniJsonMap {
    /// 字段名到包装值的映射
    map: HashMap<String, MiniJsonObj>,
}

impl MiniJsonObj {
    /// 直接包装一个 JSON 值
    ///
    /// - `value`: 被包装的 JSON 值
    fn from_value(value: serde_json::Value) -> Self {
        Self { value }
    }

    /// 从字符串反序列化
    ///
    /// - `str`: 需要反序列化的 JSON 文本
    ///
    /// # 返回值
    ///
    /// JSON 非法时返回序列化错误
    pub fn from_str(str: &str) -> CoreResult<Self> {
        let value = serde_json::from_str::<serde_json::Value>(str).map_err(serializer_err)?;

        Ok(Self { value })
    }

    /// 从流中反序列化
    ///
    /// - `stream`: 需要反序列化的数据流
    ///
    /// # 返回值
    ///
    /// JSON 非法时返回序列化错误
    pub fn from_stream<R: Read>(stream: R) -> CoreResult<Self> {
        let value =
            serde_json::from_reader::<_, serde_json::Value>(stream).map_err(serializer_err)?;

        Ok(Self { value })
    }

    /// 判断底层值是否为 JSON 数组
    ///
    /// # 返回值
    ///
    /// 是数组时为 `true`，其余类型为 `false`
    pub fn is_list(&self) -> bool {
        self.value.is_array()
    }

    /// 判断底层值是否为 JSON 对象
    ///
    /// # 返回值
    ///
    /// 是对象时为 `true`，其余类型为 `false`
    pub fn is_obj(&self) -> bool {
        self.value.is_object()
    }

    /// 判断底层值是否为 JSON 字符串
    ///
    /// # 返回值
    ///
    /// 是字符串时为 `true`，其余类型为 `false`
    pub fn is_str(&self) -> bool {
        self.value.is_string()
    }

    /// 转换成列表
    ///
    /// # 返回值
    ///
    /// 是数组时返回逐项包装后的列表，其余类型返回 `None`
    pub fn as_list(&self) -> Option<Vec<MiniJsonObj>> {
        match &self.value {
            serde_json::Value::Array(arr) => Some(
                arr.into_iter()
                    .map(|item| MiniJsonObj::from_value(item.clone()))
                    .collect(),
            ),
            _ => None,
        }
    }

    /// 转换为键值对
    ///
    /// # 返回值
    ///
    /// 是对象时返回包装后的映射，其余类型返回 `None`
    pub fn as_object(&self) -> Option<MiniJsonMap> {
        match &self.value {
            serde_json::Value::Object(map) => Some(MiniJsonMap {
                map: map
                    .into_iter()
                    .map(|(key, value)| (key.clone(), MiniJsonObj::from_value(value.clone())))
                    .collect(),
            }),
            _ => None,
        }
    }

    /// 转换为字符串
    ///
    /// # 返回值
    ///
    /// 是字符串时返回其内容，其余类型返回 `None`
    pub fn as_string(&self) -> Option<String> {
        self.value.as_str().map(|value| value.to_string())
    }

    /// 转换为 64 位整数
    ///
    /// # 返回值
    ///
    /// 是整数时返回其值，其余类型返回 `None`
    pub fn as_i64(&self) -> Option<i64> {
        self.value.as_i64()
    }
}

impl MiniJsonMap {
    /// 取出某个键的数组，只保留其中的字符串项
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是数组时返回空列表
    pub fn extract_strings(&self, key: &str) -> Vec<String> {
        self.map
            .get(key)
            .and_then(|v| v.as_list())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_string().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 获取可空字符串
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是字符串时返回 `None`
    pub fn get_opt_string(&self, key: &str) -> Option<String> {
        self.map.get(key).and_then(|v| v.as_string())
    }

    /// 获取不可空字符串
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是字符串时返回空串
    pub fn get_string(&self, key: &str) -> String {
        self.map
            .get(key)
            .and_then(|v| v.as_string())
            .unwrap_or_default()
    }

    /// 获取数字
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是整数时返回 `None`
    pub fn get_opt_i64(&self, key: &str) -> Option<i64> {
        self.map.get(key).and_then(|item| item.as_i64())
    }

    /// 取出某个键的**原始值**，类型事先不知道时用它，由调用方自己判断形状
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在时返回 `None`
    pub fn get(&self, key: &str) -> Option<&MiniJsonObj> {
        self.map.get(key)
    }

    /// 获取键值对
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是对象时返回 `None`
    pub fn get_object(&self, key: &str) -> Option<MiniJsonMap> {
        self.map.get(key).and_then(|item| item.as_object())
    }

    /// 获取列表
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是数组时返回 `None`
    pub fn get_list(&self, key: &str) -> Option<Vec<MiniJsonObj>> {
        self.map.get(key).and_then(|item| item.as_list())
    }

    /// 是否存在键
    ///
    /// - `key`: 需要查找的键
    ///
    /// # 返回值
    ///
    /// 存在时为 `true`
    pub fn have_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// 遍历所有键值对
    ///
    /// # 返回值
    ///
    /// 返回顺序不定的键值对迭代器
    pub fn iter(&self) -> hash_map::Iter<'_, String, MiniJsonObj> {
        self.map.iter()
    }
}

/// 包装一个 TOML 值，提供类型安全的访问接口
pub struct MiniTomlObj {
    /// 被包装的原始 TOML 值
    value: toml::Value,
}

impl MiniTomlObj {
    /// 从 TOML 值创建
    ///
    /// - `value`: 被包装的 TOML 值
    ///
    /// # 返回值
    ///
    /// 返回包装后的对象，不做类型校验
    pub fn from_value(value: toml::Value) -> Self {
        Self { value }
    }

    /// 获取可空字符串
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 当前值不是表、键不存在或不是字符串时返回 `None`
    pub fn get_opt_string(&self, key: &str) -> Option<String> {
        self.value
            .as_table()
            .and_then(|item| item.get(key))
            .and_then(|item| item.as_str().map(|item| item.to_string()))
    }

    /// 转为列表
    ///
    /// # 返回值
    ///
    /// 是数组时返回逐项包装后的列表，其余类型返回 `None`
    pub fn as_list(&self) -> Option<Vec<MiniTomlObj>> {
        match &self.value {
            toml::Value::Array(list) => Some(
                list.into_iter()
                    .map(|item| MiniTomlObj::from_value(item.clone()))
                    .collect(),
            ),
            _ => None,
        }
    }

    /// 转为键值对
    ///
    /// # 返回值
    ///
    /// 是表时返回包装后的映射，其余类型返回 `None`
    pub fn as_object(&self) -> Option<MiniTomlMap> {
        match &self.value {
            toml::Value::Table(table) => Some(MiniTomlMap::from_table(table.clone())),
            _ => None,
        }
    }

    /// 转为布尔
    ///
    /// # 返回值
    ///
    /// 是布尔时返回其值，其余类型返回 `None`
    pub fn as_bool(&self) -> Option<bool> {
        match &self.value {
            toml::Value::Boolean(bool) => Some(*bool),
            _ => None,
        }
    }

    /// 转为字符串
    ///
    /// # 返回值
    ///
    /// 是字符串时返回其内容，其余类型返回 `None`
    pub fn as_string(&self) -> Option<String> {
        match &self.value {
            toml::Value::String(str) => Some(str.to_string()),
            _ => None,
        }
    }
}

/// TOML 表的键值对视图，键为配置项名
pub struct MiniTomlMap {
    /// 配置项名到包装值的映射
    table: HashMap<String, MiniTomlObj>,
}

impl MiniTomlMap {
    /// 从键值对中创建
    ///
    /// - `table`: TOML 表
    ///
    /// # 返回值
    ///
    /// 返回包装后的映射，不做类型校验
    pub fn from_table(table: toml::Table) -> Self {
        MiniTomlMap {
            table: table
                .into_iter()
                .map(|(key, value)| (key.clone(), MiniTomlObj::from_value(value.clone())))
                .collect(),
        }
    }

    /// 从流中读取并解析
    ///
    /// - `stream`: 输入数据流
    ///
    /// # 返回值
    ///
    /// 读取失败或 TOML 非法时返回相应错误
    pub fn from_stream<R: Read>(stream: &mut R) -> CoreResult<Self> {
        let mut toml = String::new();
        stream
            .read_to_string(&mut toml)
            .map_err(|err| ErrorType::ArchiveReadError(ErrorData::from_error(err)))?;

        let obj = toml::from_str::<toml::Table>(&toml).map_err(serializer_err)?;

        Ok(Self::from_table(obj))
    }

    /// 获取队列
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是数组时返回 `None`
    pub fn get_list(&self, key: &str) -> Option<Vec<MiniTomlObj>> {
        let value = self.table.get(key)?;
        value.as_list().map(|item| item.into_iter().collect())
    }

    /// 获取键值对
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是表时返回 `None`
    pub fn get_object(&self, key: &str) -> Option<MiniTomlMap> {
        self.table.get(key).and_then(|item| item.as_object())
    }

    /// 获取字符串
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 键不存在或不是字符串时返回 `None`
    pub fn get_opt_string(&self, key: &str) -> Option<String> {
        self.table
            .get(key)
            .and_then(|item| item.value.as_str().map(|item| item.to_string()))
    }

    /// 获取布尔值
    ///
    /// - `key`: 需要取出的键
    ///
    /// # 返回值
    ///
    /// 布尔值直接返回，字符串按忽略大小写与 `true` 比较；键不存在时返回 `false`
    pub fn get_bool(&self, key: &str) -> bool {
        self.table
            .get(key)
            .and_then(|v| v.as_bool())
            .or_else(|| {
                self.table
                    .get(key)
                    .and_then(|v| v.as_string())
                    .map(|s| s.eq_ignore_ascii_case("true"))
            })
            .unwrap_or(false)
    }

    /// 遍历
    ///
    /// # 返回值
    ///
    /// 返回顺序不定的键值对迭代器
    pub fn iter(&self) -> hash_map::Iter<'_, String, MiniTomlObj> {
        self.table.iter()
    }
}

/// 从 JSON 文件反序列化
///
/// - `file`: 需要读取的文件
///
/// # 返回值
///
/// 文件无法打开或 JSON 非法时返回相应错误
pub fn json_from_file<T: de::DeserializeOwned>(file: impl AsRef<Path>) -> CoreResult<T> {
    let temp = path_helper::open_read(&file)?;
    Ok(serde_json::from_reader::<_, T>(temp).map_err(serializer_err)?)
}

/// 从字节反序列化
///
/// - `data`: 输入数据
///
/// # 返回值
///
/// JSON 非法时返回序列化错误
pub fn json_from_bytes<T: de::DeserializeOwned>(data: &[u8]) -> CoreResult<T> {
    Ok(serde_json::from_slice::<T>(&data).map_err(serializer_err)?)
}

/// 从流中反序列化
///
/// - `stream`: 输入数据流
///
/// # 返回值
///
/// JSON 非法时返回序列化错误
pub fn json_from_stream<T: de::DeserializeOwned>(stream: impl Read) -> CoreResult<T> {
    Ok(serde_json::from_reader::<_, T>(stream).map_err(serializer_err)?)
}

/// 从字符串反序列化
///
/// - `str`: 输入字符串
///
/// # 返回值
///
/// JSON 非法时返回序列化错误
pub fn json_from_str<T: de::DeserializeOwned>(str: &str) -> CoreResult<T> {
    Ok(serde_json::from_str::<T>(str).map_err(serializer_err)?)
}

/// 将对象序列化为 JSON 并写入文件
///
/// - `obj`: 需要序列化的内容
/// - `file`: 需要写入的文件
///
/// # 返回值
///
/// 文件无法创建或序列化失败时返回相应错误
pub fn json_to_file<T: Serialize>(obj: &T, file: impl AsRef<Path>) -> CoreResult<()> {
    let stream = path_helper::open_write(file)?;
    serde_json::to_writer(stream, obj).map_err(serializer_err)?;

    Ok(())
}

/// 转换为格式化后的 JSON 字符串
///
/// - `obj`: 需要序列化的内容
///
/// # 返回值
///
/// 序列化失败时返回相应错误
pub fn json_to_string<T: Serialize>(obj: &T) -> CoreResult<String> {
    serde_json::to_string_pretty(obj).map_err(serializer_err)
}

/// 转换为**紧凑**的 JSON 字符串（不缩进）
///
/// 给条目多、没人手读的数据文件用；给人看的配置文件仍走 [`json_to_string`]。
///
/// - `obj`: 需要序列化的内容
///
/// # 返回值
///
/// 序列化失败时返回相应错误
pub fn json_to_string_compact<T: Serialize>(obj: &T) -> CoreResult<String> {
    serde_json::to_string(obj).map_err(serializer_err)
}

/// 转换为 JSON 字节
///
/// - `obj`: 需要序列化的内容
///
/// # 返回值
///
/// 序列化失败时返回相应错误
pub fn json_to_bytes<T: Serialize>(obj: &T) -> CoreResult<Vec<u8>> {
    serde_json::to_vec(obj).map_err(serializer_err)
}

/// 反序列化一个可以是数字或数字数组的 JSON 值，数组取最小值
///
/// 数字直接返回，空数组返回 0
///
/// - `deserializer`: serde 传入的反序列化器
///
/// # 返回值
///
/// 返回取到的整数；类型不符时返回 serde 错误
pub fn deserialize_number_or_min<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = i64;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a number or array of numbers")
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<i64, E> {
            Ok(v)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<i64, E> {
            Ok(v as i64)
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<i64, E> {
            Ok(v as i64)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<i64, A::Error> {
            let mut min = i64::MAX;
            let mut found = false;
            while let Some(v) = seq.next_element::<Number>()? {
                if let Some(n) = v.as_i64() {
                    min = cmp::min(min, n);
                    found = true;
                }
            }
            Ok(if found { min } else { 0 })
        }
    }
    deserializer.deserialize_any(V)
}

/// 反序列化一个可以是数字或数字数组的 JSON 值，数组取最大值
///
/// 数字直接返回，空数组返回 0
///
/// - `deserializer`: serde 传入的反序列化器
///
/// # 返回值
///
/// 返回取到的整数；类型不符时返回 serde 错误
pub fn deserialize_number_or_max<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = i64;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a number or array of numbers")
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<i64, E> {
            Ok(v)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<i64, E> {
            Ok(v as i64)
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<i64, E> {
            Ok(v as i64)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<i64, A::Error> {
            let mut max = i64::MIN;
            let mut found = false;
            while let Some(v) = seq.next_element::<Number>()? {
                if let Some(n) = v.as_i64() {
                    max = cmp::max(max, n);
                    found = true;
                }
            }
            Ok(if found { max } else { 0 })
        }
    }
    deserializer.deserialize_any(V)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    /// MiniJsonObj 类型判断与转换
    #[test]
    fn test_mini_json_obj() {
        let obj = MiniJsonObj::from_str(r#"{"a":1,"b":"x"}"#).unwrap();
        assert!(obj.is_obj());
        assert!(!obj.is_list());
        assert!(!obj.is_str());

        let list = MiniJsonObj::from_str(r#"[1,2,3]"#).unwrap();
        assert!(list.is_list());
        assert!(list.as_list().unwrap().len() == 3);

        let s = MiniJsonObj::from_str(r#""text""#).unwrap();
        assert!(s.is_str());
        assert_eq!(s.as_string().unwrap(), "text");
        assert_eq!(s.as_i64(), None);
    }

    /// MiniJsonObj 解析失败
    #[test]
    fn test_mini_json_obj_invalid() {
        assert!(MiniJsonObj::from_str("{invalid").is_err());
    }

    /// MiniJsonMap 各类取值
    #[test]
    fn test_mini_json_map() {
        let obj = MiniJsonObj::from_str(
            r#"{
                "name": "mml",
                "count": 5,
                "missing_opt": null,
                "tags": ["a", "b", 3],
                "nested": {"key": "value"},
                "list_of_obj": [{"x": 1}, {"x": 2}]
            }"#,
        )
        .unwrap();
        let map = obj.as_object().unwrap();

        assert_eq!(map.get_string("name"), "mml");
        // 不存在的键返回默认空串
        assert_eq!(map.get_string("no_such_key"), "");
        assert_eq!(map.get_opt_string("name").unwrap(), "mml");
        assert_eq!(map.get_opt_string("no_such_key"), None);
        assert_eq!(map.get_opt_i64("count").unwrap(), 5);
        assert_eq!(map.get_opt_i64("no_such_key"), None);
        assert!(map.have_key("name"));
        assert!(!map.have_key("no_such_key"));

        // extract_strings 只保留字符串项
        assert_eq!(map.extract_strings("tags"), vec!["a", "b"]);
        assert_eq!(map.extract_strings("no_such_key"), Vec::<String>::new());

        // 嵌套对象与列表
        let nested = map.get_object("nested").unwrap();
        assert_eq!(nested.get_string("key"), "value");
        let list = map.get_list("list_of_obj").unwrap();
        assert_eq!(list.len(), 2);

        // iter 遍历
        assert_eq!(map.iter().count(), 6);
    }

    /// MiniJsonObj::from_stream
    #[test]
    fn test_mini_json_from_stream() {
        let obj = MiniJsonObj::from_stream(Cursor::new(r#"{"ok":true}"#.as_bytes())).unwrap();
        assert_eq!(
            obj.as_object().unwrap().get_opt_string("ok"),
            None // 是布尔不是字符串
        );
        assert!(MiniJsonObj::from_stream(Cursor::new("bad".as_bytes())).is_err());
    }

    /// MiniTomlObj / MiniTomlMap
    #[test]
    fn test_mini_toml() {
        let toml_str = r#"
title = "demo"
enabled = true
flag_str = "true"
score = 10

[server]
host = "localhost"
port = 25565
"#;
        let map = MiniTomlMap::from_stream(&mut Cursor::new(toml_str.as_bytes())).unwrap();
        assert_eq!(map.get_opt_string("title").unwrap(), "demo");
        // 布尔值与字符串 "true" 都返回真
        assert!(map.get_bool("enabled"));
        assert!(map.get_bool("flag_str"));
        assert!(!map.get_bool("score"));
        assert!(!map.get_bool("no_such_key"));

        let server = map.get_object("server").unwrap();
        assert_eq!(server.get_opt_string("host").unwrap(), "localhost");

        // MiniTomlObj 直接访问
        let obj = MiniTomlObj::from_value(toml::Value::Boolean(true));
        assert!(obj.as_bool().unwrap());
        let obj = MiniTomlObj::from_value(toml::Value::from("str"));
        assert_eq!(obj.as_string().unwrap(), "str");
        assert!(obj.as_bool().is_none());
    }

    /// json 文件读写往返
    #[test]
    fn test_json_file_roundtrip() {
        #[derive(Deserialize, Serialize, PartialEq, Debug)]
        struct Data {
            /// 字符串字段，验证文本往返
            name: String,
            /// 整数字段，验证负数往返
            value: i64,
        }

        let dir = mml_testutil::temp_dir().join(format!(
            "mml_base_ser_test_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("data.json");

        let data = Data {
            name: "test".to_string(),
            value: -42,
        };
        json_to_file(&data, &file).unwrap();
        assert_eq!(json_from_file::<Data>(&file).unwrap(), data);

        // bytes / stream / str
        let bytes = json_to_bytes(&data).unwrap();
        assert_eq!(json_from_bytes::<Data>(&bytes).unwrap(), data);
        assert_eq!(
            json_from_stream::<Data>(Cursor::new(bytes.clone())).unwrap(),
            data
        );
        assert_eq!(
            json_from_str::<Data>(&String::from_utf8(bytes).unwrap()).unwrap(),
            data
        );

        // to_string 输出可再次解析
        let s = json_to_string(&data).unwrap();
        assert!(s.contains("\"name\""));

        // 非法 JSON 报错
        assert!(json_from_str::<Data>("{").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// json_to_string 缩进、json_to_string_compact 不缩进
    #[test]
    fn test_json_to_string_compact() {
        #[derive(Serialize)]
        struct Data {
            /// 字符串字段
            name: String,
            /// 整数字段
            value: i64,
        }

        let data = Data {
            name: String::from("x"),
            value: 1,
        };

        let compact = json_to_string_compact(&data).unwrap();
        assert_eq!(compact, r#"{"name":"x","value":1}"#);

        let pretty = json_to_string(&data).unwrap();
        assert!(pretty.contains('\n'));
        assert_eq!(
            json_from_str::<serde_json::Value>(&compact).unwrap(),
            json_from_str::<serde_json::Value>(&pretty).unwrap()
        );
    }

    /// deserialize_number_or_min / deserialize_number_or_max
    #[test]
    fn test_deserialize_number_or_min_max() {
        #[derive(Deserialize)]
        struct MinStruct {
            /// 数字或数字数组字段，取最小值
            #[serde(deserialize_with = "deserialize_number_or_min")]
            v: i64,
        }
        #[derive(Deserialize)]
        struct MaxStruct {
            /// 数字或数字数组字段，取最大值
            #[serde(deserialize_with = "deserialize_number_or_max")]
            v: i64,
        }

        // 数字直接返回
        assert_eq!(
            serde_json::from_str::<MinStruct>(r#"{"v":5}"#).unwrap().v,
            5
        );
        assert_eq!(
            serde_json::from_str::<MaxStruct>(r#"{"v":5}"#).unwrap().v,
            5
        );
        // 数组取最小 / 最大
        assert_eq!(
            serde_json::from_str::<MinStruct>(r#"{"v":[9,3,7]}"#)
                .unwrap()
                .v,
            3
        );
        assert_eq!(
            serde_json::from_str::<MaxStruct>(r#"{"v":[9,3,7]}"#)
                .unwrap()
                .v,
            9
        );
        // 空数组返回 0
        assert_eq!(
            serde_json::from_str::<MinStruct>(r#"{"v":[]}"#).unwrap().v,
            0
        );
        assert_eq!(
            serde_json::from_str::<MaxStruct>(r#"{"v":[]}"#).unwrap().v,
            0
        );
        // 负数
        assert_eq!(
            serde_json::from_str::<MinStruct>(r#"{"v":[-1,-9]}"#)
                .unwrap()
                .v,
            -9
        );
        assert_eq!(
            serde_json::from_str::<MaxStruct>(r#"{"v":[-1,-9]}"#)
                .unwrap()
                .v,
            -1
        );
    }
}
