use super::*;

/// 创建V1游戏启动参数
///
/// - `game`: 游戏版本数据
///
/// # 返回值
///
/// 返回游戏启动参数列表，无参数时为空列表。
pub(super) fn make_v1_game_arg(game: &GameArgObj) -> Vec<String> {
    match &game.minecraft_arguments {
        None => Vec::new(),
        Some(arg) => arg.split(' ').map(|s| s.to_string()).collect(),
    }
}

/// 创建V2游戏启动参数
///
/// - `game`: 游戏版本数据
///
/// # 返回值
///
/// 返回游戏启动参数列表，仅取无条件参数，带规则的条件参数被忽略。
pub(super) fn make_v2_game_arg(game: &GameArgObj) -> Vec<String> {
    game.arguments.as_ref().map_or(Vec::new(), |args| {
        args.game
            .iter()
            .filter_map(|item| match item {
                Argument::Plain(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    })
}

/// 创建V1加载器启动参数
///
/// - `obj`: 游戏实例
/// - `game`: 游戏版本数据，部分加载器会在此基础上追加参数
///
/// # 返回值
///
/// 返回加载器附加的游戏启动参数列表。
pub(super) fn make_loader_v1_game_arg(obj: &InstanceSettingObj, game: &GameArgObj) -> Vec<String> {
    match obj.loader {
        LoaderType::Forge | LoaderType::NeoForge | LoaderType::Cleanroom => {
            let loader = if obj.loader == LoaderType::NeoForge {
                obj.get_neoforge()
            } else {
                obj.get_forge()
            };

            if let Some(data) = loader
                && let Some(data1) = &data.minecraft_arguments
            {
                data1.split(' ').map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            }
        }
        LoaderType::Normal => Vec::new(),
        LoaderType::Fabric => {
            if let Some(data) = obj.get_fabric() {
                let mut args = make_v1_game_arg(game);
                args.extend(data.arguments.game.iter().cloned());
                args
            } else {
                Vec::new()
            }
        }
        LoaderType::Quilt => {
            if let Some(data) = obj.get_quilt() {
                let mut args = make_v1_game_arg(game);
                args.extend(data.arguments.game.iter().cloned());
                args
            } else {
                Vec::new()
            }
        }
        LoaderType::OptiFine => {
            let mut args = make_v1_game_arg(game);
            args.push("--tweakClass".to_string());
            args.push("optifine.OptiFineTweaker".to_string());
            args
        }
        LoaderType::Custom => obj.get_custom_loader_game_args(),
        LoaderType::LiteLoader => {
            if let Some(data) = version_path::get_liteloader(&obj.version)
                && let Some(loader) =
                    find_loader_obj(&data, obj.loader_version.as_deref().unwrap_or(""))
            {
                let mut args = make_v1_game_arg(game);
                args.push("--tweakClass".to_string());
                args.push(loader.tweak_class.clone());
                args
            } else {
                Vec::new()
            }
        }
    }
}

/// 创建V2加载器启动参数
///
/// - `obj`: 游戏实例
///
/// # 返回值
///
/// 返回加载器附加的游戏启动参数列表。
pub(super) fn make_loader_v2_game_arg(obj: &InstanceSettingObj) -> Vec<String> {
    match obj.loader {
        LoaderType::Forge | LoaderType::NeoForge | LoaderType::Cleanroom => {
            let loader = if obj.loader == LoaderType::NeoForge {
                obj.get_neoforge()
            } else {
                obj.get_forge()
            };

            loader
                .and_then(|data| data.arguments.as_ref().map(|a| a.game.clone()))
                .unwrap_or_default()
        }
        LoaderType::Normal => Vec::new(),
        LoaderType::Fabric => obj
            .get_fabric()
            .map(|data| data.arguments.game.clone())
            .unwrap_or_default(),
        LoaderType::Quilt => obj
            .get_quilt()
            .map(|data| data.arguments.game.clone())
            .unwrap_or_default(),
        LoaderType::OptiFine => vec![
            "--tweakClass".to_string(),
            "optifine.OptiFineTweaker".to_string(),
        ],
        LoaderType::Custom => obj.get_custom_loader_game_args(),
        // 1.13+ 无 LiteLoader，V2 分支走不到
        LoaderType::LiteLoader => Vec::new(),
    }
}

/// 创建V2游戏Jvm参数
///
/// - `game`: 游戏版本数据
///
/// # 返回值
///
/// 返回Jvm启动参数列表，条件参数按规则过滤后再展开。
pub(super) fn make_v2_jvm_arg(game: &GameArgObj) -> Vec<String> {
    let Some(data) = &game.arguments else {
        return Vec::new();
    };
    let mut args = Vec::new();
    for item in &data.jvm {
        match item {
            Argument::Plain(str) => args.push(str.clone()),
            Argument::Conditional(obj) => {
                if !mojang::check_allow(&obj.rules) {
                    continue;
                }
                match &obj.value {
                    ArgValue::Single(str) => args.push(str.clone()),
                    ArgValue::Multi(items) => args.extend(items.iter().cloned()),
                }
            }
        }
    }
    args
}

/// 创建加载器Jvm参数
///
/// - `v2`: 是否为V2版本
/// - `obj`: 游戏实例
///
/// # 返回值
///
/// 返回加载器附加的Jvm启动参数列表。
pub(super) async fn make_loader_jvm_arg(v2: bool, obj: &InstanceSettingObj) -> Vec<String> {
    match obj.loader {
        LoaderType::Normal => Vec::new(),
        LoaderType::Forge | LoaderType::NeoForge => {
            if !v2 {
                return Vec::new();
            }
            let is_neoforge = obj.loader == LoaderType::NeoForge;
            let mut list = vec![
                format!(
                    "-Dforgewrapper.librariesDir={}",
                    libraries_path::get_lib_dir().display()
                ),
                format!(
                    "-Dforgewrapper.installer={}",
                    if is_neoforge {
                        obj.build_neoforge_installer(false).await
                    } else {
                        obj.build_forge_installer(false).await
                    }
                    .file
                    .display()
                ),
                format!(
                    "-Dforgewrapper.minecraft={}",
                    libraries_path::get_game_file(&obj.version).display()
                ),
            ];
            let loader_obj = if is_neoforge {
                obj.get_neoforge()
            } else {
                obj.get_forge()
            };
            if let Some(args) = loader_obj.as_ref().and_then(|o| o.arguments.as_ref()) {
                list.extend(args.jvm.clone());
            }
            list
        }
        LoaderType::Fabric => obj
            .get_fabric()
            .map(|f| f.arguments.jvm.clone())
            .unwrap_or_default(),
        LoaderType::Quilt => Vec::new(),
        // Cleanroom 是 1.12.2 的 V1 加载器，不需要 ForgeWrapper 那套 JVM 参数
        LoaderType::Cleanroom => Vec::new(),
        LoaderType::OptiFine => {
            let mut list = vec![
                format!(
                    "-Dlibdir={}",
                    libraries_path::get_lib_dir().to_string_lossy()
                ),
                format!("-Dgamecore={}", obj.get_game_file().to_string_lossy()),
                format!("-Doptifine={}", obj.get_optifine_file().to_string_lossy()),
            ];
            if v2 {
                list.extend([
                    "--add-opens".to_string(),
                    "java.base/java.lang=ALL-UNNAMED".to_string(),
                    "--add-opens".to_string(),
                    "java.base/java.util=ALL-UNNAMED".to_string(),
                    "--add-opens".to_string(),
                    "java.base/java.net=ALL-UNNAMED".to_string(),
                    "--add-opens".to_string(),
                    "java.base/jdk.internal.loader=ALL-UNNAMED".to_string(),
                ]);
            }
            list
        }
        LoaderType::Custom => obj.get_custom_loader_game_args(),
        LoaderType::LiteLoader => Vec::new(),
    }
}
