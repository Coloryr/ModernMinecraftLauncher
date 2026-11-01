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

