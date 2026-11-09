use super::*;
use crate::loader::forge::build_forge_libs;
use crate::loader::forge_install_obj::ForgeInstallObj;
use crate::loader::forge_launch_obj::ForgeLaunchObj;
use mml_base::serialize_tools;

/// 初始化运行库目录：本用例要算下载项落点
fn setup() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let run = crate::test_support::boot();
        let _ = libraries_path::init(&run);
    });
}

/// Cleanroom 安装器里的 version.json / install_profile.json 能被现有 Forge DTO 解析
#[test]
fn parses_installer_profile() {
    let data = include_str!("../../tests/cleanroom/version.json");
    let version = serialize_tools::json_from_str::<ForgeLaunchObj>(data).unwrap();
    assert_eq!(
        version.main_class,
        "top.outlands.foundation.boot.Foundation"
    );
    assert!(
        version
            .minecraft_arguments
            .as_deref()
            .is_some_and(|args| args.contains("--username"))
    );

    let data = include_str!("../../tests/cleanroom/install_profile.json");
    let profile = serialize_tools::json_from_str::<ForgeInstallObj>(data).unwrap();
    assert_eq!(profile.minecraft, "1.12.2");
    assert!(
        profile
            .libraries
            .iter()
            .any(|lib| lib.name.ends_with(names::FILE_UNIVERSAL)),
        "安装配置里应带 universal 包"
    );
}

/// 原生库要按当前系统与架构取舍：同一坐标只留一条，且都带解压动作
#[test]
fn picks_one_native_per_library() {
    setup();
    let data = include_str!("../../tests/cleanroom/version.json");
    let list = build_cleanroom_natives(data, "1.12.2");

    assert!(!list.is_empty(), "真实清单里应该有当前平台的原生库");

    let mut names = std::collections::HashSet::new();
    for item in &list {
        assert!(
            names.insert(item.name.clone()),
            "同一原生库不该出现多条：{}",
            item.name
        );
        assert!(
            matches!(item.later, LaterRun::UnpackNative(_)),
            "原生库必须带解压动作"
        );
    }
}

/// Cleanroom 的安装器与 universal 包都从 repo.cleanroommc.com/releases 取
#[tokio::test]
async fn builds_cleanroom_artifacts() {
    setup();
    let list = build_forge_libs(
        &Vec::new(),
        "1.12.2",
        "0.7.11-alpha",
        false,
        true,
        false,
        true,
    )
    .await;

    let installer = list
        .iter()
        .find(|item| item.name.ends_with(names::FILE_INSTALLER))
        .expect("应补上 Cleanroom 安装器");
    assert_eq!(
        installer.url,
        "https://repo.cleanroommc.com/releases/com/cleanroommc/cleanroom/0.7.11-alpha/cleanroom-0.7.11-alpha-installer.jar"
    );

    let universal = list
        .iter()
        .find(|item| item.name.ends_with(names::FILE_UNIVERSAL))
        .expect("应补上 Cleanroom universal 包");
    assert_eq!(
        universal.url,
        "https://repo.cleanroommc.com/releases/com/cleanroommc/cleanroom/0.7.11-alpha/cleanroom-0.7.11-alpha-universal.jar"
    );
}
