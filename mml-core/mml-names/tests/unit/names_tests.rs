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
        COUNT_IMPORT_FILE,
        LINK_INFO_FILE,
        MOD_DOWNLOAD_FILE,
        RESOURCEPACK_DOWNLOAD_FILE,
        DOWNLOAD_INDEX_DIR,
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

