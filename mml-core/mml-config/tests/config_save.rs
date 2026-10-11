//! 后台保存任务的写入测试，不涉及全局保存线程

use std::{fs, path::PathBuf};

use mml_config::config_save::ConfigSaveObj;
use uuid::Uuid;

/// ConfigSaveObj::new 序列化 + save 落盘
#[test]
fn save_obj_writes_json() {
    let dir = mml_testutil::temp_dir().join(format!("mml_config_save_test_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file: PathBuf = dir.join("test.json");

    let obj = vec![String::from("a"), String::from("b")];
    let task = ConfigSaveObj::new(&obj, file.clone(), Uuid::new_v4()).unwrap();
    task.save().unwrap();

    let data = fs::read_to_string(&file).unwrap();
    assert_eq!(data, serde_json::to_string_pretty(&obj).unwrap());

    let _ = fs::remove_dir_all(&dir);
}

/// 结构体对象也能正确序列化保存
#[test]
fn save_obj_struct() {
    let dir =
        mml_testutil::temp_dir().join(format!("mml_config_save_test2_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("obj.json");

    #[derive(serde::Serialize)]
    struct Demo {
        /// 名称，序列化为 `name`
        name: String,
        /// 数量，序列化为 `count`
        count: u32,
    }

    let obj = Demo {
        name: String::from("demo"),
        count: 3,
    };
    let task = ConfigSaveObj::new(&obj, file.clone(), Uuid::new_v4()).unwrap();
    task.save().unwrap();

    let back: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(back["name"], "demo");
    assert_eq!(back["count"], 3);

    let _ = fs::remove_dir_all(&dir);
}
