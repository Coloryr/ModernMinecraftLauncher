//! 下载源切换的补充测试，覆盖官方源与 BMCLAPI 镜像源。
//!
//! 与 `tests/url_helper.rs` 互补：这里覆盖 `change_source` 对 Mojang 全部
//! 四个域名的替换，以及各类库地址替换函数在镜像源下的行为。
//!
//! 全局配置只有一份，且测试默认并行执行，因此全部断言集中在单个测试内
//! 顺序完成，避免并行测试互相踩踏。

use mml_config::config_obj::SourceLocal;
use mml_net::{url_helper, urls};

/// 初始化全局配置到临时目录
///
/// # 返回值
///
/// 返回配置目录路径供测后清理；与其它测试文件互不冲突，每个测试二进制独立进程
fn init_config() -> std::path::PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml-net-source-switch-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("创建配置临时目录失败");
    let _ = mml_config::init(&dir);
    dir
}

/// 设置下载源并同步落盘
///
/// - `source`: 目标下载源
///
/// 用 `save_now()` 而非 `save()`：后者依赖 GUI 启动的后台保存线程，
/// 测试进程中没有该线程会 panic。
fn set_source(source: SourceLocal) {
    mml_config::write_config().http.source = source;
    mml_config::save_now();
}

/// 按官方源 → 镜像源 → 官方源的顺序完成全部源切换断言
#[test]
fn source_switch_behaviour() {
    let dir = init_config();

    // ---- 官方源 ----
    set_source(SourceLocal::Offical);

    // change_source：官方源下地址不应被改动
    let mut url = String::from("https://launchermeta.mojang.com/v1/packages/x.json");
    url_helper::change_source(&mut url);
    assert_eq!(
        url,
        "https://launchermeta.mojang.com/v1/packages/x.json",
        "官方源下地址不应被改动"
    );

    // 各库地址替换函数应原样返回
    let minecraft = "https://piston-data.mojang.com/net/minecraft/a.jar";
    assert_eq!(url_helper::replace_minecraft_libraries(minecraft), minecraft);
    let forge = "https://maven.minecraftforge.net/net/forge/a.jar";
    assert_eq!(url_helper::replace_forge_libraries(forge), forge);
    let neoforge = "https://maven.neoforged.net/net/neoforged/a.jar";
    assert_eq!(url_helper::replace_neoforge_libraries(neoforge), neoforge);
    let fabric = "https://maven.fabricmc.net/net/fabric/a.jar";
    assert_eq!(url_helper::replace_fabric_libraries(fabric), fabric);

    // get_source 与写入配置一致
    assert_eq!(url_helper::get_source(), SourceLocal::Offical);

    // ---- BMCLAPI 镜像源 ----
    set_source(SourceLocal::Bmclapi);
    assert_eq!(url_helper::get_source(), SourceLocal::Bmclapi);

    // change_source 应替换 Mojang 的全部四个域名前缀
    for mojang_url in urls::MOJANG {
        let mut url = format!("{}some/path.json", mojang_url);
        url_helper::change_source(&mut url);
        assert_eq!(
            url,
            format!("{}some/path.json", urls::BMCLAPI),
            "Mojang 域名 {mojang_url} 应被替换为 BMCLAPI"
        );
    }

    // 已是镜像地址时不应重复替换
    let mut url = String::from("https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json");
    url_helper::change_source(&mut url);
    assert_eq!(
        url,
        "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json"
    );

    // 非 Mojang 域名不应被改动
    let mut url = String::from("https://example.com/client.jar");
    url_helper::change_source(&mut url);
    assert_eq!(url, "https://example.com/client.jar");

    // 库地址替换为镜像 maven 前缀
    assert_eq!(
        url_helper::replace_minecraft_libraries(
            "https://libraries.minecraft.net/com/google/guava/guava.jar"
        ),
        "https://bmclapi2.bangbang93.com/maven/com/google/guava/guava.jar"
    );
    assert_eq!(
        url_helper::replace_forge_libraries(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.4.jar"
        ),
        "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/1.20.4.jar"
    );
    assert_eq!(
        url_helper::replace_fabric_libraries(
            "https://maven.fabricmc.net/net/fabricmc/fabric-api/1.0.jar"
        ),
        "https://bmclapi2.bangbang93.com/maven/net/fabricmc/fabric-api/1.0.jar"
    );

    // ---- 恢复官方源 ----
    set_source(SourceLocal::Offical);
    assert_eq!(url_helper::get_source(), SourceLocal::Offical);

    // 清理配置临时目录
    let _ = std::fs::remove_dir_all(&dir);
}
