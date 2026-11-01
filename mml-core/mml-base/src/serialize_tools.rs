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
#[path = "../tests/unit/serialize_tools_tests.rs"]
mod tests;
