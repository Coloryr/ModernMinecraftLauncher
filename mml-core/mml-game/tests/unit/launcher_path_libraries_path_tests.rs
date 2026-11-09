use super::*;

/// 造一个只有名字与落点的运行库项
fn lib(name: &str, file: &str) -> FileItemObj {
    FileItemObj {
        name: name.to_string(),
        file: PathBuf::from(file),
        ..Default::default()
    }
}

/// 原版库必须进类路径，同键冲突时加载器库胜出
///
/// 原版实例与带加载器的实例都没有 `custom_loader`；过去空加载器那段会把
/// `game_libs` 整个跳过，类路径里只剩加载器库与游戏 jar，guava、netty 这些
/// 原版依赖全不见，真起游戏立刻 NoClassDefFoundError。探针类测试测不出来：
/// 探针不加载游戏类。
#[test]
fn merges_game_libs_and_loader_libs() {
    let instance = InstanceSettingObj {
        loader: LoaderType::Forge,
        ..Default::default()
    };

    let arg = GameLaunchObj {
        game_libs: vec![
            lib("com.google.guava:guava:21.0", "game/guava-21.0.jar"),
            lib("net.minecraft:launchwrapper:1.12", "game/launchwrapper.jar"),
        ],
        loader_libs: vec![
            lib(
                "net.minecraftforge:forge:1.12.2-14.23.5.2864",
                "loader/forge.jar",
            ),
            lib("com.google.guava:guava:32.0", "loader/guava-32.0.jar"),
        ],
        game_jar: lib("net.minecraft:client:1.12.2", "client.jar"),
        ..Default::default()
    };

    let libs = instance.get_libs(&arg);
    let paths: Vec<String> = libs
        .iter()
        .map(|item| item.to_string_lossy().to_string())
        .collect();

    assert!(
        paths.iter().any(|item| item.ends_with("loader/forge.jar")),
        "加载器库要在：{paths:?}"
    );
    assert!(
        paths
            .iter()
            .any(|item| item.ends_with("game/launchwrapper.jar")),
        "原版独有的库不能被丢掉：{paths:?}"
    );
    assert!(
        paths
            .iter()
            .any(|item| item.ends_with("loader/guava-32.0.jar")),
        "同键冲突时应保留加载器那版：{paths:?}"
    );
    assert!(
        !paths
            .iter()
            .any(|item| item.ends_with("game/guava-21.0.jar")),
        "被覆盖的原版库不该有第二份：{paths:?}"
    );
    assert!(
        paths.iter().any(|item| item.ends_with("client.jar")),
        "游戏 jar 要在：{paths:?}"
    );
}

/// NeoForge 1.20.2+ 的 universal 包必须留在类路径外
///
/// FML 会自己把它加成 neoforge 模组；跟着 classpath 走会先成为自动模块，
/// FML 判定已定位后跳过，neoforge 模组缺失，早期窗口崩在 LoadingOverlay。
#[test]
fn excludes_neoforge_universal_from_classpath() {
    let instance = InstanceSettingObj {
        loader: LoaderType::NeoForge,
        version: String::from("1.21.1"),
        ..Default::default()
    };

    // 补齐出来的名字是单段形式，清单里的则是带 classifier 的坐标，两种都要挡
    let arg = GameLaunchObj {
        loader_libs: vec![
            lib(
                "net.neoforged:neoforge-21.1.259-universal",
                "loader/neoforge-21.1.259-universal.jar",
            ),
            lib(
                "net.neoforged:neoforge:21.1.259:universal",
                "loader/neoforge-21.1.259-universal-2.jar",
            ),
            lib(
                "net.neoforged:neoforge:21.1.259",
                "loader/neoforge-21.1.259-universal.jar",
            ),
            lib("net.neoforged:bus:8.0.5", "loader/bus-8.0.5.jar"),
        ],
        ..Default::default()
    };

    let libs = instance.get_libs(&arg);
    let paths: Vec<String> = libs
        .iter()
        .map(|item| item.to_string_lossy().to_string())
        .collect();

    assert!(
        !paths.iter().any(|item| item.contains("universal")),
        "universal 包不该进类路径：{paths:?}"
    );
    assert!(
        paths.iter().any(|item| item.ends_with("bus-8.0.5.jar")),
        "其余加载器库照旧要在：{paths:?}"
    );
}
