//! URL 构建辅助模块
//!
//! 本模块按当前下载源自动构建对应的 API 端点 URL，下载源为官方或 BMCLAPI；
//! 所有需要区分镜像源的 API 调用都通过此模块取地址。
//!
//! # 镜像源支持
//!
//! - `Offical` — 使用 Mojang/CurseForge/Fabric 等官方地址
//! - `Bmclapi` — 使用 BMCLAPI 国内镜像地址

use mml_config::config_obj::SourceLocal;

use crate::{
    authlib_api::{ArtifactsObj, AuthlibInjectorObj},
    optifine_api::OptifineListObj,
    urls,
};

/// 获取当前配置的下载源
///
/// # 返回值
///
/// 返回当前配置的下载源
pub fn get_source() -> SourceLocal {
    let config = mml_config::read_config();

    config.http.source
}

/// 获取游戏版本清单地址
///
/// - `source`: 指定下载源；`None` 时取当前配置
///
/// # 返回值
///
/// 返回版本清单 URL
pub fn game_version(source: Option<SourceLocal>) -> String {
    let source = match source {
        None => get_source(),
        Some(data) => data,
    };

    format!(
        "{}mc/game/version_manifest_v2.json",
        match source {
            SourceLocal::Bmclapi => urls::BMCLAPI,
            SourceLocal::Offical => urls::MOJANG_META,
        }
    )
}

/// 获取 minecraft 资源下载地址
///
/// - `hash`: 资源文件哈希，前两位作目录名
///
/// # 返回值
///
/// 返回资源文件下载 URL
pub fn get_download_assets(hash: &str) -> String {
    let prefix: String = hash.chars().take(2).collect();

    match get_source() {
        SourceLocal::Offical => format!("{}{prefix}/{hash}", urls::MINECRAFT_RESOURCES),
        SourceLocal::Bmclapi => format!("{}assets/{prefix}/{hash}", urls::BMCLAPI),
    }
}

/// 获取其他下载源的 Minecraft 客户端地址
///
/// - `url`: 官方下载地址
/// - `version`: 游戏版本
///
/// # 返回值
///
/// 返回客户端 JAR 下载 URL；镜像源下换成 BMCLAPI 地址
pub fn get_minecraft_client(url: &str, version: &str) -> String {
    if get_source() == SourceLocal::Bmclapi {
        format!("{}version/{}/client", urls::BMCLAPI, version)
    } else {
        String::from(url)
    }
}

/// 把下载地址转换到当前下载源
///
/// - `url`: 原始下载地址，就地修改；官方源下原样不动
pub fn change_source(url: &mut String) {
    if get_source() != SourceLocal::Bmclapi {
        return;
    }

    for item in urls::MOJANG {
        *url = url.replace(item, urls::BMCLAPI);
    }
}

/// 获取 Forge 版本信息地址
///
/// - `version`: 游戏版本
///
/// # 返回值
///
/// 返回 Forge 版本列表 URL
pub fn get_forge_versions(version: &str) -> String {
    match get_source() {
        SourceLocal::Offical => {
            format!("{}net/minecraftforge/forge/maven-metadata.xml", urls::FORGE)
        }
        SourceLocal::Bmclapi => format!("{}forge/minecraft/{version}", urls::BMCLAPI),
    }
}

/// 获取 NeoForge 版本信息地址
///
/// - `version`: 游戏版本
///
/// # 返回值
///
/// 返回 NeoForge 版本列表 URL
pub fn get_neoforge_meta(version: &str) -> String {
    match get_source() {
        SourceLocal::Offical => {
            format!(
                "{}api/maven/versions/releases/net%2Fneoforged%2Fneoforge",
                urls::NEOFORGE
            )
        }
        SourceLocal::Bmclapi => format!("{}neoforge/list/{version}", urls::BMCLAPI),
    }
}

/// 获取 Fabric 版本信息地址
///
/// # 返回值
///
/// 返回 Fabric 版本列表 URL
pub fn get_fabric_meta() -> String {
    match get_source() {
        SourceLocal::Offical => format!("{}v2/versions", urls::FABRIC_META),
        SourceLocal::Bmclapi => format!("{}fabric-meta/v2/versions", urls::BMCLAPI),
    }
}

/// 获取 Quilt 版本信息地址
///
/// # 返回值
///
/// 返回 Quilt 版本列表 URL
pub fn get_quilt_meta() -> String {
    // Quilt 没有镜像源，固定走官方 QUILT_META

    format!("{}v3/versions", urls::QUILT_META)
}

/// 获取外置登录信息地址
///
/// # 返回值
///
/// 返回 Authlib-Injector 元数据 URL
pub fn get_authlib_injector_meta() -> String {
    match get_source() {
        SourceLocal::Offical => format!("{}artifacts.json", urls::AUTHLIB),
        SourceLocal::Bmclapi => format!("{}mirrors/authlib-injector/artifacts.json", urls::BMCLAPI),
    }
}

/// 获取高清修复信息地址
///
/// # 返回值
///
/// 返回 OptiFine 版本列表 URL
pub fn get_optifine_meta() -> String {
    match get_source() {
        SourceLocal::Offical => format!("{}downloads", urls::OPTIFINE),
        SourceLocal::Bmclapi => format!("{}optifine/versionList", urls::BMCLAPI),
    }
}

/// 获取Forge下载地址
///
/// - `mc`: 游戏版本
/// - `version`: forge版本
///
/// # 返回值
///
/// 返回 Forge 安装文件所在目录 URL
pub fn get_forge_jar(mc: &str, version: &str) -> String {
    match get_source() {
        SourceLocal::Offical => format!("{}net/minecraftforge/forge/{mc}-{version}/", urls::FORGE),
        SourceLocal::Bmclapi => format!(
            "{}maven/net/minecraftforge/forge/{mc}-{version}/",
            urls::BMCLAPI
        ),
    }
}

/// 获取 NeoForge 下载地址
///
/// - `v2222`: 是否为 1.20.2 及以上版本，决定 URL 里用 `neoforge` 还是 `forge` 段
/// - `mc`: 游戏版本
/// - `version`: NeoForge 版本
///
/// # 返回值
///
/// 返回 NeoForge 安装文件所在目录 URL
pub fn get_neoforge_jar(v2222: bool, mc: &str, version: &str) -> String {
    // 内层路径不带结尾斜杠，外层拼接时会补上，避免出现双斜杠
    let url = if v2222 {
        format!("neoforge/{version}")
    } else {
        format!("forge/{mc}-{version}")
    };

    match get_source() {
        SourceLocal::Offical => format!("{}releases/net/neoforged/{url}/", urls::NEOFORGE),
        SourceLocal::Bmclapi => format!("{}maven/net/neoforged/{url}/", urls::BMCLAPI),
    }
}

/// 获取 Authlib-Injector 构建产物的元数据地址
///
/// - `obj`: Authlib-Injector 构建产物信息，用其中的 `build_number`
///
/// # 返回值
///
/// 返回对应构建产物的元数据 JSON URL
pub fn get_authlib_injector(obj: &ArtifactsObj) -> String {
    match get_source() {
        SourceLocal::Offical => format!("{}artifact/{}.json", urls::AUTHLIB, obj.build_number),
        SourceLocal::Bmclapi => format!(
            "{}mirrors/authlib-injector/artifact/{}.json",
            urls::BMCLAPI,
            obj.build_number
        ),
    }
}

/// 获取 Authlib-Injector JAR 下载地址
///
/// - `obj`: Authlib-Injector 构建产物信息，用其中的 `build_number` 与 `version`
///
/// # 返回值
///
/// 返回 Authlib-Injector JAR 下载 URL
pub fn get_authlib_injector_jar(obj: &AuthlibInjectorObj) -> String {
    match get_source() {
        SourceLocal::Offical => format!(
            "{}artifact/{}/authlib-injector-{}.jar",
            urls::AUTHLIB,
            obj.build_number,
            obj.version
        ),
        SourceLocal::Bmclapi => format!(
            "{}mirrors/authlib-injector/artifact/{}/authlib-injector-{}.jar",
            urls::BMCLAPI,
            obj.build_number,
            obj.version
        ),
    }
}

/// 获取高清修复下载地址
///
/// - `obj`: 高清修复信息
///
/// # 返回值
///
/// 返回 OptiFine JAR 下载 URL，固定走 BMCLAPI
pub fn get_optifine_jar(obj: &OptifineListObj) -> String {
    format!(
        "{}optifine/{}/{}/{}",
        urls::BMCLAPI,
        obj.mcversion,
        obj.rtype,
        obj.patch
    )
}

/// 替换 Minecraft 运行库下载地址
///
/// - `url`: 运行库地址
///
/// # 返回值
///
/// 返回替换后的地址；官方源原样返回
pub fn replace_minecraft_libraries(url: &str) -> String {
    match get_source() {
        SourceLocal::Offical => String::from(url),
        SourceLocal::Bmclapi => url.replace(
            urls::MINECRAFT_LIBRARIES,
            &format!("{}maven/", urls::BMCLAPI),
        ),
    }
}

/// 替换 Forge 运行库下载地址
///
/// - `url`: 运行库地址
///
/// # 返回值
///
/// 返回替换后的地址；官方源原样返回
pub fn replace_forge_libraries(url: &str) -> String {
    match get_source() {
        SourceLocal::Offical => String::from(url),
        SourceLocal::Bmclapi => url.replace(urls::FORGE, &format!("{}maven/", urls::BMCLAPI)),
    }
}

/// 替换 NeoForge 运行库下载地址
///
/// - `url`: 运行库地址
///
/// # 返回值
///
/// 返回替换后的地址；官方源原样返回
pub fn replace_neoforge_libraries(url: &str) -> String {
    match get_source() {
        SourceLocal::Offical => String::from(url),
        SourceLocal::Bmclapi => url.replace(urls::NEOFORGE, &format!("{}maven/", urls::BMCLAPI)),
    }
}

/// 替换 Fabric 库下载地址
///
/// - `url`: 库文件地址
///
/// # 返回值
///
/// 返回替换后的地址；官方源原样返回
pub fn replace_fabric_libraries(url: &str) -> String {
    match get_source() {
        SourceLocal::Offical => String::from(url),
        SourceLocal::Bmclapi => url.replace(urls::FABRIC, &format!("{}maven/", urls::BMCLAPI)),
    }
}

/// 按游戏版本给出 Forge 下载地址里要用的版本后缀
///
/// - `version`: 游戏版本
///
/// # 返回值
///
/// 返回拼接下载地址时使用的版本后缀，无特殊规则的版本原样返回
pub fn forge_url_fix(version: &str) -> String {
    if version.eq_ignore_ascii_case("1.7.2") {
        String::from("-mc172")
    } else if version.eq_ignore_ascii_case("1.7.10") {
        String::from("-1.7.10")
    } else if version.eq_ignore_ascii_case("1.8.9") {
        String::from("-1.8.9")
    } else if version.eq_ignore_ascii_case("1.9") {
        String::from("-1.9.0")
    } else if version.eq_ignore_ascii_case("1.9.4") {
        String::from("-1.9.4")
    } else if version.eq_ignore_ascii_case("1.10") {
        String::from("-1.10.0")
    } else {
        String::from(version)
    }
}
