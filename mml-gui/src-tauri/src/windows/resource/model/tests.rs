//! 资源窗口模型相关单元测试

use super::window::*;
use super::cache::*;

use std::collections::HashMap;
use std::sync::{Arc};

use mml_game::game_mods::ModObj;
use mml_game::loader::LoaderType;
use uuid::Uuid;


use crate::windows::lifecycle::{window_model_by_uuid, window_model_or_insert};

use std::path::PathBuf;

/// 造一份缓存：两条模组，第二条是禁用状态
fn cache() -> ModCache {
    let first = ModObj {
        uuid: Uuid::new_v4(),
        file: PathBuf::from("mods/a.jar"),
        ..Default::default()
    };
    let second = ModObj {
        uuid: Uuid::new_v4(),
        file: PathBuf::from("mods/b.jar.disabled"),
        disable: true,
        ..Default::default()
    };
    ModCache {
        list: vec![first, second],
        online: HashMap::new(),
        version: "1.20.1".to_string(),
        loader: LoaderType::Normal,
    }
}

/// 缓存存进去能读出来；没存过时读不到
#[test]
fn cache_round_trip() {
    let model = ResourceWindowModel::new();
    assert!(model.with_cache(|_| ()).is_none());

    model.set_cache(cache());
    assert_eq!(model.with_cache(|cache| cache.list.len()), Some(2));
}

/// 启用 / 禁用之后：文件名、启用状态、uuid（路径的 v5）都跟着换
#[test]
fn rename_mod_updates_entry() {
    let model = ResourceWindowModel::new();
    model.set_cache(cache());
    let old_uuid = model
        .with_cache(|cache| cache.list[0].uuid.to_string())
        .unwrap();
    let new_path = PathBuf::from("mods/a.jar.disabled");

    model.rename_mod(&old_uuid, &new_path, true);

    model
        .with_cache(|cache| {
            let item = cache
                .list
                .iter()
                .find(|item| item.file == new_path)
                .expect("改名后的那一条还在");
            assert!(item.disable);
            assert_eq!(item.uuid, mml_game::game_mods::gen_mod_uuid(&new_path));
            // 旧 uuid 那一条不该同时留着
            let stale = cache
                .list
                .iter()
                .any(|item| item.uuid.to_string() == old_uuid);
            assert!(!stale);
        })
        .unwrap();
}

/// 删除之后那一条不再留在缓存里
#[test]
fn remove_mod_drops_entry() {
    let model = ResourceWindowModel::new();
    model.set_cache(cache());
    let uuid = model
        .with_cache(|cache| cache.list[1].uuid.to_string())
        .unwrap();

    model.remove_mod(&uuid);

    assert_eq!(model.with_cache(|cache| cache.list.len()), Some(1));
}

/// 关窗取消：在跑的都被取消，之后新起的任务不受影响；守卫掉了自己从表里摘掉
#[test]
fn cancel_all_only_hits_running_ops() {
    let model = Arc::new(ResourceWindowModel::new());
    let first = model.begin();
    let second = model.begin();
    assert!(!first.is_cancelled());

    model.cancel_all();
    assert!(first.is_cancelled());
    assert!(second.is_cancelled());

    drop(first);
    let third = model.begin();
    assert_eq!(model.cancels.lock().unwrap().len(), 2);
    assert!(!third.is_cancelled());
}

/// 模型存进窗口表再取出来：存的是模型本体，取的时候类型要对得上
///
/// 存成 `Arc<Mutex<模型>>` 而按 `Arc<模型>` 取，downcast 会失败，
/// 命令侧就报"窗口模型未初始化"。
#[test]
fn model_round_trips_through_window_table() {
    let uuid = Uuid::new_v4();
    let first = window_model_or_insert(&uuid, ResourceWindowModel::new).expect("应当建出模型");
    let second = window_model_by_uuid::<ResourceWindowModel>(&uuid).expect("应当取到模型");
    assert!(Arc::ptr_eq(&first, &second));

    crate::windows::lifecycle::remove_window_model(&uuid);
    assert!(window_model_by_uuid::<ResourceWindowModel>(&uuid).is_none());
}
