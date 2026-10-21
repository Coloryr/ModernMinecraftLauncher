//! `url_helper` 的 URL 构建逻辑测试。
//!
//! 大部分构建函数按当前配置的下载源 `SourceLocal` 分支，因此测试先初始化
//! 配置到临时目录，再在单个测试内依次切换官方 / Bmclapi 源断言各 URL 形状。
//! 全局配置只有一份，避免并行测试间互相踩踏。

use mml_config::config_obj::SourceLocal;
use mml_net::{
    authlib_api::{ArtifactsObj, AuthlibInjectorObj},
    optifine_api::OptifineListObj,
    url_helper,
};

/// 初始化全局配置到临时目录。
fn init_config() {
    let dir = mml_testutil::temp_dir().join(format!("mml-net-url-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建配置临时目录失败");
    let _ = mml_config::init(&dir);
}

/// 设置下载源并同步落盘
///
/// - `source`: 目标下载源
///
/// 用 `save_now()` 直接同步写盘：`save()` 依赖 GUI 启动的后台保存线程，
/// 测试进程中没有该线程，会触发空指针 panic。
fn set_source(source: SourceLocal) {
    mml_config::write_config().http.source = source;
    mml_config::save_now();
}

/// 游戏版本清单地址：显式传入源即可，不依赖全局配置。
#[test]
fn game_version_with_explicit_source() {
    assert_eq!(
        url_helper::game_version(Some(SourceLocal::Offical)),
        "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"
    );
    assert_eq!(
        url_helper::game_version(Some(SourceLocal::Bmclapi)),
        "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json"
    );
}

/// Forge 版本地址修正，纯逻辑。
#[test]
fn forge_url_fix_special_versions() {
    assert_eq!(url_helper::forge_url_fix("1.7.2"), "-mc172");
    assert_eq!(url_helper::forge_url_fix("1.7.10"), "-1.7.10");
    assert_eq!(url_helper::forge_url_fix("1.8.9"), "-1.8.9");
    assert_eq!(url_helper::forge_url_fix("1.9"), "-1.9.0");
    assert_eq!(url_helper::forge_url_fix("1.9.4"), "-1.9.4");
    assert_eq!(url_helper::forge_url_fix("1.10"), "-1.10.0");
    // 其余版本原样返回
    assert_eq!(url_helper::forge_url_fix("1.20.4"), "1.20.4");
    assert_eq!(url_helper::forge_url_fix("1.21"), "1.21");
}

/// 切换两种下载源，验证各构建函数随源正确分支。
#[test]
fn url_builders_follow_selected_source() {
    init_config();

    // ---- 官方源 ----
    set_source(SourceLocal::Offical);

    assert_eq!(
        url_helper::get_forge_versions("1.20.4"),
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml"
    );
    assert_eq!(
        url_helper::get_neoforge_meta("1.20.4"),
        "https://maven.neoforged.net/api/maven/versions/releases/net%2Fneoforged%2Fneoforge"
    );
    assert_eq!(
        url_helper::get_fabric_meta(),
        "https://meta.fabricmc.net/v2/versions"
    );
    assert_eq!(
        url_helper::get_quilt_meta(),
        "https://meta.quiltmc.org/v3/versions"
    );
    assert_eq!(
        url_helper::get_authlib_injector_meta(),
        "https://authlib-injector.yushi.moe/artifacts.json"
    );
    assert_eq!(
        url_helper::get_optifine_meta(),
        "https://optifine.net/downloads"
    );

    // 资源下载：官方源走 resources.download.minecraft.net
    let hash = "abcd1234";
    assert_eq!(
        url_helper::get_download_assets(hash),
        format!("https://resources.download.minecraft.net/ab/{hash}")
    );

    // 库替换：官方源原样返回
    let library = "https://libraries.minecraft.net/net/minecraft/1.20.4.jar";
    assert_eq!(url_helper::replace_minecraft_libraries(library), library);
    let forge_lib = "https://maven.minecraftforge.net/foo.jar";
    assert_eq!(url_helper::replace_forge_libraries(forge_lib), forge_lib);
    let fabric_lib = "https://maven.fabricmc.net/bar.jar";
    assert_eq!(url_helper::replace_fabric_libraries(fabric_lib), fabric_lib);

    // 客户端地址：官方源原样返回
    let client = "https://launcher.mojang.com/client.jar";
    assert_eq!(url_helper::get_minecraft_client(client, "1.20.4"), client);

    // Forge/NeoForge jar
    assert_eq!(
        url_helper::get_forge_jar("1.20.4", "49.0.0"),
        "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.4-49.0.0/"
    );
    assert_eq!(
        url_helper::get_neoforge_jar(false, "1.20.4", "50.1.0"),
        "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.4-50.1.0/"
    );
    assert_eq!(
        url_helper::get_neoforge_jar(true, "1.21", "21.0.0"),
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.0.0/"
    );

    // 外置登录 artifact 元数据
    let artifacts = ArtifactsObj { build_number: 183 };
    assert_eq!(
        url_helper::get_authlib_injector(&artifacts),
        "https://authlib-injector.yushi.moe/artifact/183.json"
    );
    let injector = AuthlibInjectorObj {
        build_number: 183,
        version: "1.2.5".into(),
        download_url: String::new(),
        checksums: Default::default(),
    };
    assert_eq!(
        url_helper::get_authlib_injector_jar(&injector),
        "https://authlib-injector.yushi.moe/artifact/183/authlib-injector-1.2.5.jar"
    );

    // ---- Bmclapi 镜像源 ----
    set_source(SourceLocal::Bmclapi);

    assert_eq!(
        url_helper::get_forge_versions("1.20.4"),
        "https://bmclapi2.bangbang93.com/forge/minecraft/1.20.4"
    );
    assert_eq!(
        url_helper::get_neoforge_meta("1.20.4"),
        "https://bmclapi2.bangbang93.com/neoforge/list/1.20.4"
    );
    assert_eq!(
        url_helper::get_fabric_meta(),
        "https://bmclapi2.bangbang93.com/fabric-meta/v2/versions"
    );
    assert_eq!(
        url_helper::get_authlib_injector_meta(),
        "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifacts.json"
    );
    assert_eq!(
        url_helper::get_optifine_meta(),
        "https://bmclapi2.bangbang93.com/optifine/versionList"
    );

    // 资源下载走镜像
    let hash = "abcd1234";
    assert_eq!(
        url_helper::get_download_assets(hash),
        format!("https://bmclapi2.bangbang93.com/assets/ab/{hash}")
    );

    // 库替换走镜像 maven
    assert_eq!(
        url_helper::replace_minecraft_libraries("https://libraries.minecraft.net/net/x.jar"),
        "https://bmclapi2.bangbang93.com/maven/net/x.jar"
    );
    assert_eq!(
        url_helper::replace_forge_libraries("https://maven.minecraftforge.net/y.jar"),
        "https://bmclapi2.bangbang93.com/maven/y.jar"
    );
    // NeoForge 库地址替换的是 forge maven 前缀，旧版 NeoForge 库托管于该域名
    assert_eq!(
        url_helper::replace_neoforge_libraries("https://maven.minecraftforge.net/z.jar"),
        "https://bmclapi2.bangbang93.com/maven/z.jar"
    );
    assert_eq!(
        url_helper::replace_fabric_libraries("https://maven.fabricmc.net/w.jar"),
        "https://bmclapi2.bangbang93.com/maven/w.jar"
    );

    // 客户端地址换成镜像
    assert_eq!(
        url_helper::get_minecraft_client("https://launcher.mojang.com/client.jar", "1.20.4"),
        "https://bmclapi2.bangbang93.com/version/1.20.4/client"
    );

    // change_source：把 Mojang 前缀替换为镜像
    let mut url = String::from("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json");
    url_helper::change_source(&mut url);
    assert_eq!(
        url,
        "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json"
    );

    // Forge/NeoForge jar 镜像
    assert_eq!(
        url_helper::get_forge_jar("1.20.4", "49.0.0"),
        "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/1.20.4-49.0.0/"
    );
    assert_eq!(
        url_helper::get_neoforge_jar(false, "1.20.4", "50.1.0"),
        "https://bmclapi2.bangbang93.com/maven/net/neoforged/forge/1.20.4-50.1.0/"
    );

    // 外置登录 artifact 走镜像
    assert_eq!(
        url_helper::get_authlib_injector(&artifacts),
        "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/183.json"
    );

    // 高清修复 jar 始终走镜像；URL 结构为 {mc}/{rtype}/{patch}
    let optifine = OptifineListObj {
        mcversion: "1.20.4".into(),
        patch: "L5".into(),
        rtype: "HD_U".into(),
        filename: String::new(),
        forge: String::new(),
    };
    assert_eq!(
        url_helper::get_optifine_jar(&optifine),
        "https://bmclapi2.bangbang93.com/optifine/1.20.4/HD_U/L5"
    );

    // 恢复默认源，避免影响其它测试
    set_source(SourceLocal::Offical);
}
