//! 启动器使用的路径、文件名与参数常量
//!
//! 常量按用途分组：
//! - 目录名 / 文件名常量 —— 磁盘相对路径片段，用 [`Path::join`](std::path::Path::join) 拼接
//! - 扩展名常量 —— `ZIP_EXT` 不带点、`ZIP_DOT_EXT` 带点，两种写法一一对应
//! - jar 内部路径 —— 如 `META-INF/mods.toml`，使用 `/` 分隔，不是磁盘路径
//! - JVM 参数、命令行参数、启动参数占位符、GC 参数等

pub const DOWNLOAD_DIR: &str = "download";
pub const OVERRIDE_DIR: &str = "overrides";
pub const LIBRARIES_DIR: &str = "libraries";
pub const INSTANCE_DIR: &str = "instances";
pub const REMOVE_DIR: &str = "remove";
pub const BACKUP_DIR: &str = "backup";
pub const TEMP_DIR: &str = "temp";
pub const CACHE_DIR: &str = "cache";
pub const WEBVIEW_DIR: &str = "webview";
pub const VERSION_DIR: &str = "versions";
pub const GAME_DIR: &str = ".minecraft";
pub const GAME_LOGS_DIR: &str = "logs";
pub const GAME_CRASH_DIR: &str = "crash-reports";
pub const GAME_DATAPACK_DIR: &str = "datapacks";
pub const GAME_MODS_DIR: &str = "mods";
pub const GAME_ASSETS_DIR: &str = "assets";
pub const GAME_INDEX_DIR: &str = "indexes";
pub const GAME_OBJECT_DIR: &str = "objects";
pub const GAME_SKIN_DIR: &str = "skins";
pub const GAME_SCREENSHOTS_DIR: &str = "screenshots";
pub const GAME_RESOURCEPACKS_DIR: &str = "resourcepacks";
pub const GAME_SHADERPACKS_DIR: &str = "shaderpacks";
pub const GAME_SAVES_DIR: &str = "saves";
pub const GAME_CONFIG_DIR: &str = "config";
pub const GAME_SCHEMATICS_DIR: &str = "schematics";
pub const JAVA_DIR: &str = "java";
pub const JSON_DIR: &str = "patches";
pub const DEFAULT_DIR: &str = "default";
pub const OPEN_LOADER_DIR: &str = "openloader";
pub const DATA_DIR: &str = "data";
pub const NATIVE_DIR: &str = "native";
pub const MML_DIR: &str = "mml";
pub const MML_INNER_DIR: &str = ".mml";
pub const BLOCK_DIR: &str = "block";
pub const LANG_DIR: &str = "langs";
pub const ITEM_DIR: &str = "items";
pub const MINECRAFT_DIR: &str = "minecraft";
pub const IMAGE_DIR: &str = "image";
pub const KUBEJS_DIR: &str = "kubejs";
pub const LOCAL_DIR: &str = "local";

pub const IPC_PORT_FILE: &str = "temp";
pub const LOG_FILE: &str = "logs.log";
pub const LANG_FILE: &str = "lang.txt";
pub const MOD_INFO_FILE: &str = "modfileinfo.json";
pub const MOD_UPDATE_FILE: &str = "mod_update.json";
pub const GAME_FILE: &str = "game.json";
pub const GUI_SETTING_FILE: &str = "gui_setting.json";
pub const GROUP_FILE: &str = "group_save.json";
pub const LEGACY_GROUP_FILE: &str = "instgroups.json";
pub const MOD_PACK_FILE: &str = "Modpack.json";
pub const CONFIG_FILE: &str = "config.json";
pub const SHA_FILE: &str = "sha1";
pub const COLOR_MC_INFO_FILE: &str = "colormc.info.json";
pub const MMCJSON_FILE: &str = "mmc-pack.json";
pub const MMCCFG_FILE: &str = "instance.cfg";
pub const HMCLFILE: &str = "mcbbs.packmeta";
pub const MANIFEST_FILE: &str = "manifest.json";
pub const MODRINTH_FILE: &str = "modrinth.index.json";
pub const ICON_FILE: &str = "icon.png";
pub const SERVER_FILE: &str = "server.json";
pub const SERVER_OLD_FILE: &str = "server.old.json";
pub const LAUNCH_COUNT_FILE: &str = "launch.json";
pub const LOG4J_FILE: &str = "log4j-rce-patch.xml";
pub const LOADER_FILE: &str = "loader.jar";
pub const LEVEL_FILE: &str = "level.dat";
pub const PACK_META_FILE: &str = "pack.mcmeta";
pub const PACK_ICON_FILE: &str = "pack.png";
pub const OPTION_FILE: &str = "options.txt";
pub const GAME_SERVER_FILE: &str = "servers.dat";
pub const VERSION_FILE: &str = "version.json";
pub const AUTH_FILE: &str = "auth.json";
pub const AUTH_SELECT_FILE: &str = "auth_select.json";
pub const MAVEN_FILE: &str = "maven.json";
pub const JAVA_FILE: &str = "java";
pub const JAVAW_FILE: &str = "javaw.exe";
pub const OPTIFINE_FILE: &str = "optifine.json";
pub const LITELOADER_FILE: &str = "liteloader.json";
pub const MOD_LIST_FILE: &str = "modlist.html";
pub const LATEST_LOG_FILE: &str = "latest.log";
pub const DEBUG_LOG_FILE: &str = "debug.log";
pub const SERVER_MANIFEST_FILE: &str = "server-manifest.json";
pub const LAUNCHER_PROFILES_FILE: &str = "launcher_profiles.json";
pub const FABRIC_MOD_FILE: &str = "fabric.mod.json";
pub const QUILT_MOD_FILE: &str = "quilt.mod.json";
pub const SAVE_BACKUP_FILE: &str = "save_backups.json";
pub const COUNT_DATA_FILE: &str = "count.dat";
pub const WINDOW_SAVE_FILE: &str = "window_save.json";
pub const BLOCK_FILE: &str = "block.json";
pub const ITEM_FILE: &str = "items.json";
pub const GUI_CONFIG_FILE: &str = "gui_config.json";
pub const GAMEPAD_DIR: &str = "gamepad";
pub const GAMEPAD_DEFAULT_MAP_FILE: &str = "default.json";
pub const COLLECT_FILE: &str = "collect.json";
pub const INTEGRATIONS_FILE: &str = "integrations.json";
pub const BLOCK_GROUPS_FILE: &str = "block_groups.json";
pub const COLORMC_MIGRATE_FILE: &str = "colormc_migrate.json";
pub const MINECRAFT_MIGRATE_FILE: &str = "minecraft_migrate.json";

pub const MINECRAFT_KEY: &str = "minecraft";
pub const LANG_KEY1: &str = "minecraft/lang/";
pub const LANG_KEY2: &str = "lang";
pub const LANG_NAME_KEY: &str = "language.name";
pub const FML_KEY: &str = "fmlloader";
pub const FORGE_KEY: &str = "forge";
pub const MINECRAFT_FORGE_KEY: &str = "minecraftforge";
pub const NEOFORGE_KEY: &str = "neoforge";
pub const NEOFORGED_KEY: &str = "neoforged";
pub const FABRIC_KEY: &str = "fabric";
pub const FABRIC_MC_KEY: &str = "fabricmc";
pub const FABRIC_LOADER_KEY: &str = "fabric-loader";
pub const QUILT_KEY: &str = "quilt";
pub const QUILT_MC_KEY: &str = "quiltmc";
pub const QUILT_LOADER_KEY: &str = "quilt-loader";
pub const GAME_KEY: &str = "game";

pub const FILE_INSTALL: &str = "install";
pub const FILE_INSTALLER: &str = "installer";
pub const FILE_UNIVERSAL: &str = "universal";
pub const FILE_CLIENT: &str = "client";
pub const FILE_LAUNCHER: &str = "launcher";
pub const FILE_INSTALL_PROFILE: &str = "install_profile.json";

pub const LOG_EXT: &str = "log";
pub const TXT_EXT: &str = "txt";
pub const ZIP_EXT: &str = "zip";
pub const R7Z_EXT: &str = "7z";
pub const JAR_EXT: &str = "jar";
pub const JSON_EXT: &str = "json";
pub const DISABLE_EXT: &str = "disable";
pub const DISABLED_EXT: &str = "disabled";
pub const LITEMATIC_EXT: &str = "litematic";
pub const SCHEMATIC_EXT: &str = "schematic";
pub const SCHEM_EXT: &str = "schem";
pub const SHA1_EXT: &str = "sha1";
pub const SHA256_EXT: &str = "sha256";
pub const SHA512_EXT: &str = "sha512";
pub const TAR_EXT: &str = "tar";
pub const TGZ_EXT: &str = "tgz";
pub const TXZ_EXT: &str = "txz";
pub const MRPACK_EXT: &str = "mrpack";
pub const DAT_EXT: &str = "dat";
pub const DAT_OLD_EXT: &str = "dat_old";
pub const RIO_EXT: &str = "rio";
pub const MCA_EXT: &str = "mca";
pub const PNG_EXT: &str = "png";
pub const NBT_EXT: &str = "nbt";
pub const TOML_EXT: &str = "toml";
pub const YAML_EXT: &str = "yaml";
pub const JSONC_EXT: &str = "jsonc";
pub const JSON5_EXT: &str = "json5";
pub const MCMETA_EXT: &str = "mcmeta";
pub const YML_EXT: &str = "yml";
pub const PROPERTIES_EXT: &str = "properties";
pub const CFG_EXT: &str = "cfg";
pub const CONF_EXT: &str = "conf";
pub const INI_EXT: &str = "ini";
pub const XML_EXT: &str = "xml";
pub const JS_EXT: &str = "js";
pub const MJS_EXT: &str = "mjs";
pub const CJS_EXT: &str = "cjs";
pub const SNBT_EXT: &str = "snbt";
pub const JPG_EXT: &str = "jpg";
pub const JPEG_EXT: &str = "jpeg";
pub const GIF_EXT: &str = "gif";
pub const WEBP_EXT: &str = "webp";
pub const GZ_EXT: &str = "gz";

pub const LOG_DOT_EXT: &str = ".log";
pub const TXT_DOT_EXT: &str = ".txt";
pub const LOG_GZ_DOT_EXT: &str = ".log.gz";
pub const ZIP_DOT_EXT: &str = ".zip";
pub const R7Z_DOT_EXT: &str = ".7z";
pub const JAR_DOT_EXT: &str = ".jar";
pub const JSON_DOT_EXT: &str = ".json";
pub const DISABLE_DOT_EXT: &str = ".disable";
pub const DISABLED_DOT_EXT: &str = ".disabled";
pub const LITEMATIC_DOT_EXT: &str = ".litematic";
pub const SCHEMATIC_DOT_EXT: &str = ".schematic";
pub const SCHEM_DOT_EXT: &str = ".schem";
pub const SHA1_DOT_EXT: &str = ".sha1";
pub const SHA256_DOT_EXT: &str = ".sha256";
pub const SHA512_DOT_EXT: &str = ".sha512";
pub const TAR_DOT_EXT: &str = ".tar";
pub const TAR_GZ_DOT_EXT: &str = ".tar.gz";
pub const TAR_XZ_DOT_EXT: &str = ".tar.xz";
pub const TGZ_DOT_EXT: &str = ".tgz";
pub const TXZ_DOT_EXT: &str = ".txz";
pub const MRPACK_DOT_EXT: &str = ".mrpack";
pub const DAT_DOT_EXT: &str = ".dat";
pub const DAT_OLD_DOT_EXT: &str = ".dat_old";
pub const RIO_DOT_EXT: &str = ".rio";
pub const MCA_DOT_EXT: &str = ".mca";
pub const PNG_DOT_EXT: &str = ".png";
pub const NBT_DOT_EXT: &str = ".nbt";

pub const ARG_JAVA_LOCAL: &str = "%JAVA_LOCAL%";
pub const ARG_JAVA_ARG: &str = "%JAVA_ARG%";
pub const ARG_LAUNCHER_DIR: &str = "%LAUNCH_DIR%";
pub const ARG_GAME_NAME: &str = "%GAME_NAME%";
pub const ARG_GAME_UUID: &str = "%GAME_UUID%";
pub const ARG_GAME_DIR: &str = "%GAME_DIR%";
pub const ARG_GAME_BASE_DIR: &str = "%GAME_BASE_DIR%";

pub const MC_MOD_INFO_FILE: &str = "mcmod.info";
pub const MC_MOD_TOML_FILE: &str = "META-INF/mods.toml";
pub const NEO_TOML_FILE: &str = "META-INF/neoforge.mods.toml";
pub const NEO_TOML1_FILE: &str = "neoforge.mods.toml";
pub const MOD_JAR_JAR_DIR: &str = "META-INF/jarjar/";
pub const MOD_JARS_DIR: &str = "META-INF/jars/";

pub const MML: &str = "MML";
pub const MINECRAFT: &str = "Minecraft";

/// G1 垃圾回收器的默认 JVM 参数
pub const G1GC: [&str; 8] = [
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+UseG1GC",
    "-XX:MaxGCPauseMillis=200",
    "-XX:G1NewSizePercent=30",
    "-XX:G1MaxNewSizePercent=40",
    "-XX:InitiatingHeapOccupancyPercent=35",
    "-XX:ConcGCThreads=4",
    "-XX:ParallelGCThreads=8",
];

/// ZGC 垃圾回收器的默认 JVM 参数
pub const GCZGC: [&str; 2] = ["-XX:+UseZGC", "-XX:+ZGenerational"];

pub const LANG_ZH_CN: &str = "zh_CN";
pub const LANG_EN_US: &str = "en_US";

pub const NOW_VERSION: &str = "26.3";

/// 安装
pub const COMMAND_INSTALL: &str = "--install";
/// 启动实例
pub const COMMAND_GAME: &str = "--game";
/// 导入
pub const COMMAND_IMPORT: &str = "--import";
/// 重启
pub const COMMAND_RESTART: &str = "--restart";
/// 注册uri
pub const COMMAND_REGISTER: &str = "--register";
/// 覆盖uri
pub const COMMAND_REMAP: &str = "--remap_modrinth";

#[cfg(test)]
mod tests {
    use super::*;

    /// 目录名常量不含路径分隔符，保证跨平台 Path::join 拼接安全
    #[test]
    fn dir_constants_have_no_separators() {
        for name in [
            DOWNLOAD_DIR,
            OVERRIDE_DIR,
            LIBRARIES_DIR,
            INSTANCE_DIR,
            REMOVE_DIR,
            BACKUP_DIR,
            TEMP_DIR,
            CACHE_DIR,
            WEBVIEW_DIR,
            VERSION_DIR,
            GAME_DIR,
            GAME_LOGS_DIR,
            GAME_CRASH_DIR,
            GAME_DATAPACK_DIR,
            GAME_MODS_DIR,
            GAME_ASSETS_DIR,
            GAME_INDEX_DIR,
            GAME_OBJECT_DIR,
            GAME_SKIN_DIR,
            GAME_SCREENSHOTS_DIR,
            GAME_RESOURCEPACKS_DIR,
            GAME_SHADERPACKS_DIR,
            GAME_SAVES_DIR,
            GAME_CONFIG_DIR,
            GAME_SCHEMATICS_DIR,
            JAVA_DIR,
            JSON_DIR,
            DEFAULT_DIR,
            OPEN_LOADER_DIR,
            DATA_DIR,
            NATIVE_DIR,
            MML_DIR,
            MML_INNER_DIR,
            BLOCK_DIR,
            ITEM_DIR,
            MINECRAFT_DIR,
        ] {
            assert!(!name.is_empty(), "目录常量不应为空: {name:?}");
            assert!(!name.contains('/'), "{name:?} 不应包含 '/'");
            assert!(!name.contains('\\'), "{name:?} 不应包含 '\\'");
        }
    }

    /// 单文件名常量不含路径分隔符
    #[test]
    fn file_constants_have_no_separators() {
        for name in [
            LOG_FILE,
            LANG_FILE,
            MOD_INFO_FILE,
            MOD_UPDATE_FILE,
            GAME_FILE,
            GUI_SETTING_FILE,
            GROUP_FILE,
            LEGACY_GROUP_FILE,
            MOD_PACK_FILE,
            CONFIG_FILE,
            SHA_FILE,
            COLOR_MC_INFO_FILE,
            MMCJSON_FILE,
            MMCCFG_FILE,
            HMCLFILE,
            MANIFEST_FILE,
            MODRINTH_FILE,
            ICON_FILE,
            SERVER_FILE,
            SERVER_OLD_FILE,
            LAUNCH_COUNT_FILE,
            LOG4J_FILE,
            LOADER_FILE,
            LEVEL_FILE,
            PACK_META_FILE,
            PACK_ICON_FILE,
            OPTION_FILE,
            GAME_SERVER_FILE,
            VERSION_FILE,
            AUTH_FILE,
            AUTH_SELECT_FILE,
            MAVEN_FILE,
            OPTIFINE_FILE,
            LITELOADER_FILE,
            MOD_LIST_FILE,
            LATEST_LOG_FILE,
            DEBUG_LOG_FILE,
            SERVER_MANIFEST_FILE,
            LAUNCHER_PROFILES_FILE,
            FABRIC_MOD_FILE,
            QUILT_MOD_FILE,
            SAVE_BACKUP_FILE,
            COUNT_DATA_FILE,
            WINDOW_SAVE_FILE,
            BLOCK_FILE,
            ITEM_FILE,
            GUI_CONFIG_FILE,
            INTEGRATIONS_FILE,
            GAMEPAD_DIR,
            GAMEPAD_DEFAULT_MAP_FILE,
            BLOCK_GROUPS_FILE,
            COLORMC_MIGRATE_FILE,
            MINECRAFT_MIGRATE_FILE,
            IPC_PORT_FILE,
        ] {
            assert!(!name.is_empty(), "文件常量不应为空: {name:?}");
            assert!(!name.contains('/'), "{name:?} 不应包含 '/'");
            assert!(!name.contains('\\'), "{name:?} 不应包含 '\\'");
        }
    }

    /// jar 内部路径常量使用 '/' 分隔，不是磁盘路径
    #[test]
    fn jar_inner_path_constants() {
        assert_eq!(MC_MOD_TOML_FILE, "META-INF/mods.toml");
        assert_eq!(NEO_TOML_FILE, "META-INF/neoforge.mods.toml");
        assert_eq!(NEO_TOML1_FILE, "neoforge.mods.toml");
        assert_eq!(MOD_JAR_JAR_DIR, "META-INF/jarjar/");
        assert_eq!(MOD_JARS_DIR, "META-INF/jars/");
    }

    /// 扩展名常量：不带点与带点的写法一一对应
    #[test]
    fn ext_constants_pairs() {
        assert_eq!(format!(".{LOG_EXT}"), LOG_DOT_EXT);
        assert_eq!(format!(".{TXT_EXT}"), TXT_DOT_EXT);
        assert_eq!(format!(".{ZIP_EXT}"), ZIP_DOT_EXT);
        assert_eq!(format!(".{R7Z_EXT}"), R7Z_DOT_EXT);
        assert_eq!(format!(".{JAR_EXT}"), JAR_DOT_EXT);
        assert_eq!(format!(".{JSON_EXT}"), JSON_DOT_EXT);
        assert_eq!(format!(".{SHA1_EXT}"), SHA1_DOT_EXT);
        assert_eq!(format!(".{SHA256_EXT}"), SHA256_DOT_EXT);
        assert_eq!(format!(".{SHA512_EXT}"), SHA512_DOT_EXT);
        assert_eq!(format!(".{TAR_EXT}"), TAR_DOT_EXT);
        assert_eq!(format!(".{TGZ_EXT}"), TGZ_DOT_EXT);
        assert_eq!(format!(".{TXZ_EXT}"), TXZ_DOT_EXT);
        assert_eq!(format!(".{MRPACK_EXT}"), MRPACK_DOT_EXT);
        assert_eq!(format!(".{DAT_EXT}"), DAT_DOT_EXT);
        assert_eq!(format!(".{DAT_OLD_EXT}"), DAT_OLD_DOT_EXT);
        assert_eq!(format!(".{RIO_EXT}"), RIO_DOT_EXT);
        assert_eq!(format!(".{MCA_EXT}"), MCA_DOT_EXT);
        assert_eq!(format!(".{PNG_EXT}"), PNG_DOT_EXT);
        assert_eq!(format!(".{NBT_EXT}"), NBT_DOT_EXT);
        // 复合扩展名
        assert_eq!(LOG_GZ_DOT_EXT, ".log.gz");
        assert_eq!(TAR_GZ_DOT_EXT, ".tar.gz");
        assert_eq!(TAR_XZ_DOT_EXT, ".tar.xz");
    }

    /// JVM 参数常量以 '-' 开头且不为空
    #[test]
    fn jvm_arg_constants() {
        for arg in G1GC.iter().chain(GCZGC.iter()) {
            assert!(!arg.is_empty());
            assert!(arg.starts_with('-'), "JVM 参数应以 '-' 开头: {arg:?}");
        }
        assert!(G1GC.contains(&"-XX:+UseG1GC"));
        assert!(GCZGC.contains(&"-XX:+UseZGC"));
    }

    /// 命令行参数常量以 '--' 开头
    #[test]
    fn command_constants() {
        for arg in [
            COMMAND_INSTALL,
            COMMAND_GAME,
            COMMAND_IMPORT,
            COMMAND_REGISTER,
            COMMAND_REMAP,
        ] {
            assert!(arg.starts_with("--"), "命令常量应以 '--' 开头: {arg:?}");
        }
    }

    /// 占位符常量以 '%' 包裹
    #[test]
    fn arg_placeholder_constants() {
        for arg in [
            ARG_JAVA_LOCAL,
            ARG_JAVA_ARG,
            ARG_LAUNCHER_DIR,
            ARG_GAME_NAME,
            ARG_GAME_UUID,
            ARG_GAME_DIR,
            ARG_GAME_BASE_DIR,
        ] {
            assert!(
                arg.starts_with('%') && arg.ends_with('%'),
                "占位符应以 % 包裹: {arg:?}"
            );
        }
    }
}
