//! 服务器实例不接入 mml-link
//!
//! 服务器实例的文件直接落在实例目录，不建公共目录硬链接、不写映射记录。
//! 这里用真实例句柄覆盖四个入口，并对照普通实例确认开关只对服务器实例生效。
use std::{collections::HashMap, sync::{Arc, RwLock}};

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::*;
use crate::launcher::instance_setting_obj::InstanceSettingObj;
use crate::test_support::boot;

/// 起单例并把实例路径指到测试运行目录
fn boot_paths() {
    let run = boot();
    crate::launcher_path::init(&run).expect("实例路径初始化失败");
}

/// 造一个带在线信息的实例：mods 里一个文件，外加一条在线记录
///
/// - `kind`: 实例类型
///
/// # 返回值
///
/// 已落盘的实例句柄
fn make_instance(kind: ModPackType) -> GameInstance {
    let dir = format!("link-test-{}", Uuid::new_v4().simple());
    let game: GameInstance = Arc::new(RwLock::new(InstanceSettingObj {
        uuid: Uuid::new_v4(),
        name: dir.clone(),
        dir,
        version: "1.21.1".to_string(),
        modpack_type: kind,
        ..Default::default()
    }));
    let game_path = game.read().unwrap().get_game_path();
    std::fs::create_dir_all(game_path.join("mods")).unwrap();
    std::fs::write(game_path.join("mods").join("tiny.jar"), b"jar").unwrap();

    let mut info = HashMap::new();
    info.insert(
        "tiny".to_string(),
        OnlineInfoObj {
            path: "mods".to_string(),
            name: "Tiny".to_string(),
            file: "tiny.jar".to_string(),
            sha1: "0123456789abcdef0123456789abcdef01234567".to_string(),
            url: "https://example.invalid/tiny.jar".to_string(),
            modid: "111".to_string(),
            fileid: "222".to_string(),
        },
    );
    game.read().unwrap().save_online_info_sync(&info).unwrap();
    game
}

/// 服务器实例：四个入口都不碰 mml-link
#[test]
fn server_pack_skips_link_mapping() {
    boot_paths();
    mml_link::set_enabled(true);
    let game = make_instance(ModPackType::ServerPack);
    assert!(is_server_pack(&game));

    assert_eq!(count_instance_resources(&game), 0, "服务器实例没有可映射的资源");
    let count = map_instance_resources(&game);
    assert_eq!(
        (count.mapped, count.failed, count.skipped),
        (0, 0, 0),
        "服务器实例不该执行批量映射"
    );

    let target = game.read().unwrap().get_mods_path().join("tiny.jar");
    let mut items = vec![FileItemObj {
        name: "Tiny".to_string(),
        file: target.clone(),
        url: String::new(),
        hash: FileHash::None,
        later: LaterRun::None,
    }];
    assert!(prepare_downloads(&game, &mut items).is_empty(), "不该生成补链计划");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].file, target,
        "服务器实例的下载目标不该被改到公共目录"
    );

    let base = game.read().unwrap().get_base_path();
    assert!(
        !base.join(mml_names::names::LINK_INFO_FILE).exists(),
        "服务器实例不该写出 link_info.json"
    );
}

/// 服务器实例：启动前的补链检查直接跳过
#[tokio::test]
async fn server_pack_skips_link_check() {
    boot_paths();
    mml_link::set_enabled(true);
    let game = make_instance(ModPackType::ServerPack);
    check_instance_links(&game, CancellationToken::new())
        .await
        .expect("服务器实例的补链检查应当直接返回");

    let base = game.read().unwrap().get_base_path();
    assert!(
        !base.join(mml_names::names::LINK_INFO_FILE).exists(),
        "启动前的补链检查不该给服务器实例写记录"
    );
}

/// 普通实例保持原行为：开启映射后照样统计可处理资源
#[test]
fn normal_instance_still_counts_resources() {
    boot_paths();
    mml_link::set_enabled(true);
    let game = make_instance(ModPackType::None);
    assert!(!is_server_pack(&game));
    assert_eq!(
        count_instance_resources(&game),
        1,
        "普通实例仍按在线信息统计，开关只对服务器实例生效"
    );
}
