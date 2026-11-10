//! 依赖与重复检测
//!
//! 按模组声明的依赖与提供的版本判断缺前置、版本不符与重复安装。

use std::collections::HashMap;

use crate::{launcher::instance_setting_obj::InstanceSettingObj, loader::LoaderType};
use mml_base::range_checker::{VersionRange, compare_versions};
use mml_names::i18_items::error_type::CoreResult;

use super::types::{DependantType, ModObj};

/// 收集内置 jar-in-jar 里提供的模组：modid → 各份的版本
///
/// **一个 modid 可能被多个包内置，版本还不同**（各自带一份依赖库），所以收全部而不是
/// 只留一份 —— 只留一份时，能顶上版本要求的那份可能正好被旧的覆盖掉，报出假的"版本不符"。
///
/// - `item`: 顶层模组条目
/// - `deps`: 结果累加到这里
pub(super) fn get_jar_in_jar_list(item: &ModObj, deps: &mut HashMap<String, Vec<String>>) {
    for item in item.jar_in_jar.iter() {
        for item in item.info.iter() {
            if item.mod_id.is_empty() {
                continue;
            }
            deps.entry(item.mod_id.clone())
                .or_default()
                .push(item.version.clone().unwrap_or_default());
        }

        for item in item.jar_in_jar.iter() {
            get_jar_in_jar_list(item, deps);
        }
    }
}

/// 依赖检测发现的一条问题
#[derive(Debug, Clone)]
pub struct DependantProblem {
    /// 出问题的前置 modid
    pub modid: String,
    /// 问题种类
    pub kind: DependantProblemKind,
    /// 要求的版本范围
    pub required: VersionRange,
    /// 已装着的版本，多个用 `, ` 连接；`Missing` 这一类为空串
    pub installed: String,
    /// 有哪些模组要求它，取模组的显示名
    pub required_by: Vec<String>,
}

/// 依赖检测发现的问题种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependantProblemKind {
    /// 缺失：装着的模组里没有这个前置
    Missing,
    /// 版本不符：装着了，但没有一个版本落在要求的范围内
    Mismatch,
    /// 前置被禁用：只有禁用状态的包提供它
    Disabled,
}

/// 依赖检测的结果
#[derive(Debug, Clone)]
pub struct DependantTest {
    /// 检测到的问题，按种类与 modid 排序；没有问题时为空
    pub problems: Vec<DependantProblem>,
}

/// 重复安装的一个 modid
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDuplicate {
    /// 重复的 modid
    pub modid: String,
    /// 提供它的顶层文件，按文件名排序
    pub files: Vec<ModDuplicateFile>,
}

/// 重复安装里涉及的一个文件
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDuplicateFile {
    /// 文件名，不含目录
    pub file: String,
    /// 该文件声明的版本；没写时为空串
    pub version: String,
}

/// 模组的显示名：优先元数据里的名字，退回 modid，再退回文件名
pub(super) fn mod_display_name(item: &ModObj) -> String {
    item.info
        .iter()
        .map(|info| {
            if !info.name.is_empty() {
                info.name.clone()
            } else {
                info.mod_id.clone()
            }
        })
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| mod_file_name(item))
}

/// 模组条目的文件名，不含目录
pub(super) fn mod_file_name(item: &ModObj) -> String {
    item.file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 游戏版本的"档"匹配：`minecraft` 要求 `[1.21]` / `=1.21` 时，装 1.21.1 也算符合
///
/// **只有游戏版本这么放宽** —— 模组作者普遍把"支持 1.21.1"写成 `[1.21]`；
/// 模组之间的 `[19.5.1]` 仍然是严格相等。
pub(super) fn game_version_line(required: &VersionRange, version: &str) -> bool {
    let VersionRange::Exact(line) = required else {
        return false;
    };
    version.starts_with(&format!("{line}."))
}

/// 上界是"下界再补一位补丁号"、且装的正是上界那个版本
///
/// 模组作者常把"支持到 1.21.1"写成 `[1.21,1.21.1)` —— 上界与下界是同一条线
/// （1.21 / 1.21.1），本意是"到 1.21.1 为止"。上游加载器放行这类写法，这里跟它一致，
/// 否则一个整合包的 minecraft 一栏能刷出几十条误报。
///
/// **跨版本的不放宽**：`[1.20,1.21)` 是真的排除了 1.21，照报。
/// 也**只用在游戏 / 加载器自身的前置上**：模组之间的 `[1.0,2.0)` 同样是真的排除 2.0。
pub(super) fn exclusive_upper_is_installed(required: &VersionRange, version: &str) -> bool {
    let VersionRange::Interval(interval) = required else {
        return false;
    };
    if interval.upper_inclusive {
        return false;
    }
    let (Some(lower), Some(upper)) = (interval.lower.as_deref(), interval.upper.as_deref()) else {
        return false;
    };
    upper.starts_with(&format!("{lower}.")) && compare_versions(version, upper).is_eq()
}

/// 这个版本号能不能拿来比版本
///
/// 空串、以及没被替换的占位符都不算 —— Forge 的 mods.toml 常写 `version = "${file.jarVersion}"`，
/// 读出来是字面量，拿去比任何范围都会报"版本不符"。这种版本一律当"任何版本"。
pub(super) fn is_unknown_version(version: &str) -> bool {
    version.is_empty() || version.trim_start().starts_with("${")
}

/// 游戏 / 加载器提供的"虚拟前置"的版本情况
///
/// `minecraft` 由游戏本体提供，`forge` / `fabricloader` 之类由加载器提供 —— 它们不是
/// 模组包，但同样要参与版本检查，否则"给 1.20.1 装的模组放到 1.21 上"这类问题报不出来。
pub enum ProvidedVersion<'a> {
    /// 由游戏 / 加载器提供，版本已知
    Known(&'a str),
    /// 由游戏 / 加载器提供，但版本未知，只当"有"、不比版本
    Unknown,
    /// 与游戏 / 加载器无关，由模组包提供
    FromMods,
}

/// 某个加载器在模组元数据里可能写成哪些 modid
///
/// 一个加载器常有好几种写法（Forge 有 forge / minecraftforge，Quilt 会认 fabricloader）。
/// 依赖检测按它认"由加载器提供"，依赖图按它补"加载器 → minecraft"那条边 —— 同一张表，
/// 两处口径才不会漂。
///
/// - `loader`: 实例的加载器
///
/// # 返回值
///
/// 可能的 modid，小写；原版与自定义加载器没有对应写法，返回空数组
pub fn loader_modids(loader: LoaderType) -> &'static [&'static str] {
    match loader {
        LoaderType::Forge => &["forge", "minecraftforge"],
        LoaderType::NeoForge => &["neoforge", "forge"],
        LoaderType::Cleanroom => &["cleanroom", "forge", "minecraftforge"],
        LoaderType::Fabric => &["fabricloader"],
        LoaderType::Quilt => &["quilt_loader", "quilt_base", "fabricloader"],
        LoaderType::OptiFine => &["optifine"],
        LoaderType::LiteLoader => &["liteloader"],
        LoaderType::Normal | LoaderType::Custom => &[],
    }
}

/// 该前置是否由游戏 / 加载器提供，以及提供它的版本
///
/// `java` / `mcp` / `fml` 由运行环境或老加载器提供，版本口径不在实例设置里，按未知处理。
/// 注意 `fabric` 不算：Fabric API 自己的 modid 就是它，正是最该报出来的那个前置。
///
/// - `game`: 实例设置，取游戏版本与加载器版本
/// - `modid`: 依赖声明的 modid
///
/// # 返回值
///
/// 见 [`ProvidedVersion`]
pub fn provided_version<'a>(game: &'a InstanceSettingObj, modid: &str) -> ProvidedVersion<'a> {
    let id = modid.to_ascii_lowercase();
    if matches!(id.as_str(), "java" | "mcp" | "fml") {
        return ProvidedVersion::Unknown;
    }
    if id == "minecraft" {
        return if game.version.is_empty() {
            ProvidedVersion::Unknown
        } else {
            ProvidedVersion::Known(&game.version)
        };
    }

    if !loader_modids(game.loader).contains(&id.as_str()) {
        return ProvidedVersion::FromMods;
    }

    match game.loader_version.as_deref() {
        // 实例没记加载器版本：当"有"，不比版本，免得整屏误报
        Some(version) if !version.is_empty() => ProvidedVersion::Known(version),
        _ => ProvidedVersion::Unknown,
    }
}

/// 模组依赖检测：按各包声明的强制前置，检查是否齐全、版本是否满足
///
/// 口径：
/// - 只看**启用状态**的顶层模组：禁用包声明的依赖不算，禁用的包也不能顶上依赖；
/// - 内置 jar-in-jar 里的模组算"已提供"，Forge / NeoForge 会把它们加载起来；
/// - `minecraft` 与当前加载器自身的前置按**实例记的版本**检查，见 [`provided_version`]；
///   版本未知的（`java` / `mcp` / 实例没记加载器版本）只当"有"，不比版本；
/// - 版本范围为空或 `*` 时只查有没有，不比版本；装着的包没写版本号时同样不判版本不符；
/// - 同一前置被多个模组要求时合并成一条，版本范围不同则各算一条；
/// - 推荐安装的前置不报。
///
/// 重复安装不是依赖问题，见 [`mods_duplicate_test`]。
///
/// - `game`: 实例设置，取游戏版本 / 加载器与加载器版本
/// - `mods`: 已经扫描出的模组列表，见 `InstanceSettingObj::read_mod`
///
/// # 返回值
///
/// 见 [`DependantTest`]
pub fn mods_dep_test(game: &InstanceSettingObj, mods: &[ModObj]) -> CoreResult<DependantTest> {
    // 装着的：modid → 各条元数据的（版本，是否启用）
    let mut installed: HashMap<String, Vec<(String, bool)>> = HashMap::new();
    // 内置模组：modid → 各份的版本
    let mut bundled: HashMap<String, Vec<String>> = HashMap::new();

    for item in mods.iter().filter(|item| !item.fail) {
        for info in item.info.iter() {
            if info.mod_id.is_empty() {
                continue;
            }
            if info.loaders != game.loader {
                continue;
            }
            installed
                .entry(info.mod_id.clone())
                .or_default()
                .push((info.version.clone().unwrap_or_default(), !item.disable));
        }

        // 禁用的包里那些内置模组不算提供
        if !item.disable {
            get_jar_in_jar_list(item, &mut bundled);
        }
    }

    // 谁要求谁：键取「前置 modid + 版本范围」，同一个前置的不同范围各报一条
    let mut requires: HashMap<(String, String), (VersionRange, Vec<String>)> = HashMap::new();
    for item in mods.iter().filter(|item| !item.fail && !item.disable) {
        let by = mod_display_name(item);
        for info in item.info.iter() {
            if info.loaders != game.loader {
                continue;
            }
            for dep in info.dependants.iter() {
                let DependantType::Required(dep) = dep else {
                    continue;
                };
                if dep.modid.is_empty() {
                    continue;
                }
                let entry = requires
                    .entry((dep.modid.clone(), dep.version.to_string()))
                    .or_insert_with(|| (dep.version.clone(), Vec::new()));
                if !entry.1.contains(&by) {
                    entry.1.push(by.clone());
                }
            }
        }
    }

    let mut problems = Vec::new();

    for ((modid, _), (required, mut required_by)) in requires {
        required_by.sort();

        // 游戏 / 加载器提供的虚拟前置：不查模组列表，直接拿实例记的版本比
        match provided_version(game, &modid) {
            ProvidedVersion::Unknown => continue,
            ProvidedVersion::Known(version) => {
                if required == VersionRange::Any
                    || required.contains(version)
                    || (modid.eq_ignore_ascii_case("minecraft")
                        && game_version_line(&required, version))
                    || exclusive_upper_is_installed(&required, version)
                {
                    continue;
                }
                problems.push(DependantProblem {
                    modid,
                    kind: DependantProblemKind::Mismatch,
                    required,
                    installed: version.to_string(),
                    required_by,
                });
                continue;
            }
            ProvidedVersion::FromMods => {}
        }

        // 能顶上的：启用的顶层条目 + 内置条目
        let mut providers: Vec<String> = installed
            .get(&modid)
            .map(|list| {
                list.iter()
                    .filter(|(_, enable)| *enable)
                    .map(|(version, _)| version.clone())
                    .collect()
            })
            .unwrap_or_default();
        if let Some(versions) = bundled.get(&modid) {
            providers.extend(versions.iter().cloned());
        }

        let kind = if providers.is_empty() {
            match installed.get(&modid).and_then(|list| list.first()) {
                // 只有禁用状态的包提供它：与"缺失"分开报，用户才知道该去启用
                Some((_, false)) => DependantProblemKind::Disabled,
                _ => DependantProblemKind::Missing,
            }
        } else if required == VersionRange::Any
            || providers
                .iter()
                .any(|version| is_unknown_version(version) || required.contains(version))
        {
            continue;
        } else {
            DependantProblemKind::Mismatch
        };

        let installed = if providers.is_empty() {
            installed
                .get(&modid)
                .and_then(|list| list.first().map(|(version, _)| version.clone()))
                .unwrap_or_default()
        } else {
            providers.join(", ")
        };

        problems.push(DependantProblem {
            modid,
            kind,
            required,
            installed,
            required_by,
        });
    }

    // 排序：种类优先（缺失最要紧），同种类按 modid
    problems.sort_by(|a, b| {
        let rank = |kind: &DependantProblemKind| match kind {
            DependantProblemKind::Missing => 0,
            DependantProblemKind::Disabled => 1,
            DependantProblemKind::Mismatch => 2,
        };
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then_with(|| a.modid.cmp(&b.modid))
    });

    Ok(DependantTest { problems })
}

/// 模组重复检测：同一个 modid 由两份及以上**启用**的顶层文件提供
///
/// 与 [`mods_dep_test`] 分开：重复安装不是"前置缺不缺"的问题，属同名文件安装冲突，
/// 在模组列表加载完成后单独检测并上报。
///
/// 口径：
/// - 只看启用状态的顶层文件，禁用包与内置 jar-in-jar 都不算；
/// - 同一个包里多份元数据声明了同一个 modid 不算重复，只有不同文件才算；
/// - 同一个 modid 声明了多个版本时，各文件的版本分别列出。
///
/// - `mods`: 已经扫描出的模组列表，见 `InstanceSettingObj::read_mod`
///
/// # 返回值
///
/// 每个重复的 modid 一条，按 modid 排序；没有重复时为空
pub fn mods_duplicate_test(mods: &[ModObj]) -> Vec<ModDuplicate> {
    // modid → 各顶层文件的（文件名，版本）
    let mut top: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for item in mods.iter().filter(|item| !item.fail && !item.disable) {
        let file = mod_file_name(item);
        for info in item.info.iter() {
            if info.mod_id.is_empty() {
                continue;
            }
            top.entry(info.mod_id.clone())
                .or_default()
                .push((file.clone(), info.version.clone().unwrap_or_default()));
        }
    }

    let mut list: Vec<ModDuplicate> = top
        .into_iter()
        .filter_map(|(modid, entries)| {
            // 同一个包的元数据条目指向同一个文件，按文件名去重后不足两份就不算重复
            let mut files: Vec<ModDuplicateFile> = entries
                .into_iter()
                .map(|(file, version)| ModDuplicateFile { file, version })
                .collect();
            files.sort_by(|a, b| a.file.cmp(&b.file));
            files.dedup_by(|a, b| a.file == b.file);
            if files.len() < 2 {
                return None;
            }
            Some(ModDuplicate { modid, files })
        })
        .collect();

    list.sort_by(|a, b| a.modid.cmp(&b.modid));
    list
}

// 本文件的测试：元数据解析 / 依赖范围与依赖检测 / 图标三组放在一起，
// 共用一个 `use super::*`；元数据那组要起日志，见 `boot_log`。
