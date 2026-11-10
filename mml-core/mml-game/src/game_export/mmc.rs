//! MultiMC 格式导出
//!
//! 生成 `mmc-pack.json` 与 `instance.cfg`，JVM 参数写进配置行。

use mml_base::{
    archives::{ArchiveType, BaseArchive},
    serialize_tools,
};
use mml_config::config_obj::RunArgObj;
use mml_names::{i18_items::error_type::CoreResult, names};

use super::ExportArg;
use super::archive::{add_game_files, pack_all_files};

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    loader::LoaderType,
    other_launcher::mmc_obj::{ComponentsObj, MMCObj},
};

/// 导出为 MMC 实例包：整包实例，供 MultiMC / Prism 导入
///
/// 包内是"一个实例目录"：`mmc-pack.json` 写组件、`instance.cfg` 写实例设置，
/// 勾选的文件按实例目录结构放入：`.minecraft/` 仍在 `.minecraft/`，实例根上的文件留在根。
/// 组件 uid 与导入侧 `other_launcher` 的
/// `to_instance` 判据一一对应，导出的包能被本启动器原样导回来。
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误。自定义加载器在 MMC 里没有对应组件，
/// 这时只导游戏版本、不写加载器组件
pub(super) fn mmc(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let mut components = vec![ComponentsObj {
        cached_version: game.version.clone(),
        uid: "net.minecraft".to_string(),
        version: game.version.clone(),
    }];

    let uid = match game.loader {
        LoaderType::Forge => Some("net.minecraftforge"),
        LoaderType::NeoForge => Some("net.neoforged"),
        LoaderType::Fabric => Some("net.fabricmc.fabric-loader"),
        LoaderType::Quilt => Some("org.quiltmc.quilt-loader"),
        _ => None,
    };
    if let Some(uid) = uid {
        let version = game.loader_version.clone().unwrap_or_default();
        components.push(ComponentsObj {
            cached_version: version.clone(),
            uid: uid.to_string(),
            version,
        });
    }

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;
    // 顶层套一层实例目录：MMC / Prism 从 zip 导入时按它建实例
    let inst_dir = format!("{}/", game.name);
    let mut files = pack_all_files(&arg);

    if let Some(gui) = &arg.gui {
        gui.start(files.len() + 2);
        gui.update(Some(names::MMCJSON_FILE.to_string()), 0);
    }

    let data = serialize_tools::json_to_bytes(&MMCObj {
        format_version: 1,
        components,
    })?;
    zip.add_data(&format!("{inst_dir}{}", names::MMCJSON_FILE), &data)?;

    // 图标：MMC 的 iconKey 就是图标文件名去掉扩展名，导入时按它在实例根目录找同名图片；
    // 原图直接进包，不再另造一份 icon.png
    let icon = game.get_icon_file_or_default();
    let icon_name = icon
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let icon_key = icon
        .is_file()
        .then(|| {
            icon.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .flatten();

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MMCCFG_FILE.to_string()), 1);
    }

    let jvm_args = mmc_jvm_args(game.jvm_arg.as_ref());
    zip.add_data(
        &format!("{inst_dir}{}", names::MMCCFG_FILE),
        mmc_instance_cfg(
            &game.name,
            icon_key.as_deref(),
            game.jvm_arg.as_ref(),
            &jvm_args,
        )
        .as_bytes(),
    )?;

    if let Some(name) = icon_name.filter(|_| icon.is_file()) {
        // 勾选里本来就带上图标时不要再写一遍：同一路径两条记录会让导入方取到哪条看实现
        files.retain(|file| file != &icon);
        zip.add_file(&format!("{inst_dir}{name}"), &icon)?;
    }

    // 勾选相对实例目录：.minecraft 原样落在实例目录下，实例根上的文件也按原位置带走
    add_game_files(
        &mut zip,
        &game.get_base_path(),
        inst_dir.trim_end_matches('/'),
        &files,
        &arg.gui,
        2,
        &arg.cancel,
    )
}

/// MMC 的 `instance.cfg`：按 MMC 自己的完整格式写一份，不只是一两个键
///
/// Prism / MultiMC 的 cfg 是带 `[General]` 段的 INI；缺了那些 Override / ConfigVersion
/// 键时，导入方会按自己的默认值处理，表现为"设置没带过来"。
///
/// - `name`: 实例名
/// - `icon_key`: 包根图标文件名去掉扩展名，如 `gtnh_icon`；没有图标时传 `None`，写 `default`
/// - `jvm`: 源实例的 JVM 设置，内存上下限写进 cfg
/// - `jvm_args`: 已转成 MMC 变量的 JVM 参数，空表时不写 `JvmArgs`
///
/// # 返回值
///
/// INI 文本，行尾是 `\n`
pub fn mmc_instance_cfg(
    name: &str,
    icon_key: Option<&str>,
    jvm: Option<&RunArgObj>,
    jvm_args: &[String],
) -> String {
    let max = jvm.and_then(|jvm| jvm.max_memory);
    let min = jvm.and_then(|jvm| jvm.min_memory);
    let mut text = String::from("[General]\n");
    text.push_str("InstanceType=OneSix\n");
    text.push_str("JoinServerOnLaunch=false\n");
    text.push_str("OverrideCommands=false\n");
    text.push_str("OverrideConsole=false\n");
    text.push_str("OverrideGameTime=false\n");
    text.push_str(&format!("OverrideJavaArgs={}\n", !jvm_args.is_empty()));
    text.push_str("OverrideJavaLocation=false\n");
    text.push_str(&format!(
        "OverrideMemory={}\n",
        max.is_some() || min.is_some()
    ));
    text.push_str("OverrideNativeWorkarounds=false\n");
    text.push_str("OverrideWindow=false\n");
    text.push_str(&format!("iconKey={}\n", icon_key.unwrap_or("default")));
    text.push_str(&format!("name={name}\n"));
    text.push_str("notes=\n");
    text.push_str("ConfigVersion=1.2\n");
    text.push_str("AutomaticJava=false\n");
    text.push_str("ExportAuthor=\n");
    text.push_str("ExportName=\n");
    text.push_str("ExportOptionalFiles=true\n");
    text.push_str("ExportSummary=\n");
    text.push_str("ExportVersion=1.0.0\n");
    text.push_str("LogPrePostOutput=true\n");
    text.push_str("ManagedPack=false\n");
    text.push_str("ManagedPackID=\n");
    text.push_str("ManagedPackName=\n");
    text.push_str("ManagedPackType=\n");
    text.push_str("ManagedPackVersionID=\n");
    text.push_str("ManagedPackVersionName=\n");
    text.push_str("OverrideEnv=false\n");
    text.push_str("OverrideLegacySettings=false\n");
    text.push_str("OverrideMiscellaneous=false\n");
    text.push_str("OverridePerformance=false\n");
    text.push_str("Profiler=\n");
    text.push_str("UseAccountForInstance=false\n");
    text.push_str("lastLaunchTime=0\n");
    text.push_str("lastTimePlayed=0\n");
    text.push_str("linkedInstances=[]\n");
    text.push_str("totalTimePlayed=0\n");
    if let Some(max) = max {
        text.push_str(&format!("MaxMemAlloc={max}\n"));
    }
    if let Some(min) = min {
        text.push_str(&format!("MinMemAlloc={min}\n"));
    }
    if !jvm_args.is_empty() {
        text.push_str(&format!("JvmArgs={}\n", jvm_args.join(" ")));
    }
    text
}

/// 把源实例的自定义 JVM 参数转成 MMC 写法
///
/// - `jvm`: 源实例的 JVM 设置
///
/// # 返回值
///
/// 逐条替换占位符后的参数；没有自定义参数时为空表
pub(super) fn mmc_jvm_args(jvm: Option<&RunArgObj>) -> Vec<String> {
    jvm.and_then(|jvm| jvm.jvm_args.as_deref())
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(mmc_vars)
        .collect()
}

/// 把 MML 路径占位符换成 MMC 变量
///
/// MMC 的变量是单个 $ 前缀、没有结尾 $，如 $INST_DIR；由启动器在启动前做文本替换。
///
/// - `text`: 待替换的 JVM 参数 / 命令文本
///
/// # 返回值
///
/// 替换后的文本；没有对应 MMC 变量的占位符（如 %LAUNCH_DIR%）原样保留
pub fn mmc_vars(text: &str) -> String {
    text.replace(names::ARG_GAME_BASE_DIR, "$INST_DIR")
        .replace(names::ARG_GAME_DIR, "$INST_MC_DIR")
        .replace(names::ARG_JAVA_LOCAL, "$INST_JAVA")
        .replace(names::ARG_JAVA_ARG, "$INST_JAVA_ARGS")
        .replace(names::ARG_GAME_NAME, "$INST_NAME")
        .replace(names::ARG_GAME_UUID, "$INST_ID")
}
