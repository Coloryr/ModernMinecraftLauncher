//! jar 读取与扫描
//!
//! 打开模组 jar、读元数据与图标、生成稳定 uuid，并并行扫描整个 mods 目录。

use std::{
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use mml_base::{
    file_item::FileHash,
    hash_helper::{self, HashType},
    serialize_tools::MiniJsonObj,
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData},
    names,
};
use mml_sys::path_helper;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use uuid::Uuid;
use zip::ZipArchive;

use crate::error::archive_read_err;
use crate::{class_scan, gui_hook::ProgressGui, loader::LoaderType};

use super::meta::{
    parse_manifest, read_fabric_json, read_forge_json, read_forge_toml, read_quilt_json,
    sanitize_mcmod_json,
};
use super::types::{ModItemObj, ModObj};

/// 读取 core mod 信息，靠 MANIFEST.MF 识别
///
/// - `archive`: 模组压缩包
/// - `mod_info`: 模组信息，识别出 core mod 时追加一条
///
/// # 返回值
///
/// 成功返回 `Ok(())`；读取失败返回对应错误，没有 MANIFEST.MF 时直接成功
pub(super) fn read_core_mod(
    archive: &mut ZipArchive<impl Read + Seek>,
    mod_info: &mut ModObj,
) -> CoreResult<()> {
    // 读取 META-INF/MANIFEST.MF
    let mut manifest_file = match archive.by_name("META-INF/MANIFEST.MF") {
        Ok(file) => file,
        Err(_) => return Ok(()),
    };

    let mut info = ModItemObj::default();

    let mut content = String::new();
    manifest_file
        .read_to_string(&mut content)
        .map_err(archive_read_err)?;

    let manifest = parse_manifest(&content);

    // 只有真的识别出 core mod 标记才算 core mod
    let mut is_core = false;

    // 检查 FMLCorePlugin，即 Forge core mod 主类
    if let Some(core_plugin) = manifest.get("FMLCorePlugin") {
        info.mod_id = core_plugin.clone();
        info.name = core_plugin
            .rsplit('.')
            .next()
            .unwrap_or(core_plugin)
            .to_string();
        info.loaders = LoaderType::Forge;
        is_core = true;
    }

    // 检查 TweakClass，即 LaunchWrapper 注入类
    if let Some(tweak_class) = manifest.get("TweakClass") {
        if info.mod_id.is_empty() {
            info.mod_id = tweak_class.clone();
            info.name = tweak_class
                .rsplit('.')
                .next()
                .unwrap_or(tweak_class)
                .to_string();
        }
        info.loaders = LoaderType::Forge;
        is_core = true;
    }

    // **没识别出 core mod 就不要 push 这条 info**：`META-INF/MANIFEST.MF` 几乎每个 jar 都有，
    // 无条件 push 会多出一条加载器为默认值 `Normal` 的空条目，界面上每个模组都会多出「原版」
    if is_core {
        mod_info.info.push(info);
    }
    // 原先这里无条件置 true，而 MANIFEST.MF 几乎每个 Forge / NeoForge jar 都有 ——
    // 结果界面上**每个**模组都挂着"核心"徽标。core 只表示"用老式 core mod 机制加载"，
    // 没有上面两个标记就不是
    mod_info.core = is_core;

    Ok(())
}

/// 扫描内置的模组，即 jar-in-jar
///
/// **会递归**：内置 jar 里还能再套内置 jar，`parse_mod_archive` 又会调回这里，层数不限。
///
/// - `archive`: 当前这一层的压缩包
/// - `mod_info`: 解析结果追加到其中
/// - `base`: **当前层在父包里的路径链**，顶层为空；内置 jar 没有独立文件路径，
///   得靠它拼出唯一身份
///
/// # 返回值
///
/// 成功返回 `Ok(())`；读取失败返回对应错误
pub(super) fn read_jar_in_jar(
    archive: &mut ZipArchive<impl Read + Seek>,
    mod_info: &mut ModObj,
    base: &Path,
) -> CoreResult<()> {
    // 收集所有 META-INF/jarjar/ 目录下的 .jar 文件
    let jar_entries: Vec<usize> = (0..archive.len())
        .filter_map(|i| {
            archive.by_index(i).ok().and_then(|entry| {
                let name = entry.name().unwrap_or_default();
                if name.ends_with(names::JAR_DOT_EXT)
                    && (name.starts_with(names::MOD_JAR_JAR_DIR)
                        || name.starts_with(names::MOD_JARS_DIR))
                {
                    Some(i)
                } else {
                    None
                }
            })
        })
        .collect();

    for idx in jar_entries {
        let mut entry = archive.by_index(idx).map_err(archive_read_err)?;
        // 条目名要留着当身份用，先把它的借用结束掉再读内容
        let entry_name = entry.name().unwrap_or_default().to_string();

        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(archive_read_err)?;

        let cursor = Cursor::new(bytes);
        let mut inner_zip = ZipArchive::new(cursor).map_err(|err| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: PathBuf::new(),
                error: err.to_string(),
            })
        })?;

        // 这一层在父包里的路径链：顶层 base 为空 → 就是条目名本身；
        // 再往下一层就是"父条目名/子条目名"，层层累加
        let rel = base.join(&entry_name);

        match parse_mod_archive(&mut inner_zip, &rel) {
            Ok(mut inmod) => {
                // 内置 jar 没有独立文件路径，`file` 存条目名、`uuid` 由路径链生成；
                // 留默认值会让同层内置 jar 的渲染 key 全一样，展开一个就把同层全展开
                inmod.file = PathBuf::from(&entry_name);
                inmod.uuid = gen_mod_uuid(&rel);
                mod_info.jar_in_jar.push(inmod);
            }
            Err(err) => {
                mml_log::error_type(err);
            }
        }
    }

    Ok(())
}

/// 从任意可读的 ZIP 归档中解析模组信息，核心解析逻辑
///
/// - `archive`: 压缩包
/// - `base`: 这个包在**父包里的路径链**，顶层传文件路径，内置 jar 传"父条目名/子条目名"；
///   只用来给内置 jar 生成唯一身份，见 [`read_jar_in_jar`]
///
/// # 返回值
///
/// 返回解析出的模组信息；读取或解析失败返回对应错误
pub(super) fn parse_mod_archive(
    archive: &mut ZipArchive<impl Read + Seek>,
    base: &Path,
) -> CoreResult<ModObj> {
    let mut mod_info = ModObj::default();

    // mcmod.info 与下面的 mods.toml 同一口径：坏文件只跳过它自己，不牵连整包；
    // 老包的 mcmod.info 手改坏、编码不对的情况并不少见
    if let Ok(item) = archive.by_name(names::MC_MOD_INFO_FILE)
        && let Err(err) = read_forge_json(item, &mut mod_info)
    {
        mml_log::error_type(err);
    }

    // **单个元数据文件坏了不该让整包失败**：真实案例 `mekanism_lasers-1.1.10.3-a.jar`
    // 里两个 toml 坏了一个，旧实现用 `?` 抛出后整包变成"读取失败"、连模组名都看不到
    macro_rules! try_read_file {
        ($archive:expr, $name:expr, $func:expr, $loader:expr) => {
            if let Ok(item) = $archive.by_name($name) {
                if let Err(err) = $func(item, $loader, &mut mod_info) {
                    mml_log::error_type(err);
                }
            }
        };
    }

    try_read_file!(
        archive,
        names::MC_MOD_TOML_FILE,
        read_forge_toml,
        LoaderType::Forge
    );
    try_read_file!(
        archive,
        names::NEO_TOML_FILE,
        read_forge_toml,
        LoaderType::NeoForge
    );
    try_read_file!(
        archive,
        names::NEO_TOML1_FILE,
        read_forge_toml,
        LoaderType::NeoForge
    );

    // 读取 fabric.mod.json / quilt.mod.json，同样是"坏了只跳过它自己"
    if let Ok(item) = archive.by_name(names::FABRIC_MOD_FILE)
        && let Err(err) = read_fabric_json(item, &mut mod_info)
    {
        mml_log::error_type(err);
    }

    if let Ok(item) = archive.by_name(names::QUILT_MOD_FILE)
        && let Err(err) = read_quilt_json(item, &mut mod_info)
    {
        mml_log::error_type(err);
    }

    // 扫描coremod
    read_core_mod(archive, &mut mod_info)?;

    // 扫描 jar-in-jar，会递归：内置 jar 里还能再套内置 jar
    read_jar_in_jar(archive, &mut mod_info, base)?;

    // 读图标：放在最后是因为图标要挂在已经解析出的第一条 info 上
    read_mod_icon(archive, &mut mod_info);

    Ok(mod_info)
}

/// 图标体积上限，单位字节；图标只是装饰，有人塞过整张高清图，不值得读进内存
const MAX_ICON_SIZE: u64 = 512 * 1024;

/// 读模组图标
///
/// 各加载器把图标路径放在不同字段里，见 [`find_mod_icon_path`]；这里再按条目名把
/// **图片字节**读出来，`ModItemObj::icon` 存的是字节，界面那边转 data URL。
///
/// - `archive`: 模组压缩包
/// - `mod_info`: 模组信息，图标写到第一条 info 上
pub(super) fn read_mod_icon(archive: &mut ZipArchive<impl Read + Seek>, mod_info: &mut ModObj) {
    let Some(path) = find_mod_icon_path(archive) else {
        return;
    };

    let Ok(mut entry) = archive.by_name(&path) else {
        return;
    };
    if entry.size() == 0 || entry.size() > MAX_ICON_SIZE {
        return;
    }

    let mut data = Vec::with_capacity(entry.size() as usize);
    if entry.read_to_end(&mut data).is_err() || data.is_empty() {
        return;
    }

    match mod_info.info.first_mut() {
        Some(info) => info.icon = Some(data),
        // 只认得出图标、没有别的元数据的包：补一条，别把图标丢了
        None => mod_info.info.push(ModItemObj {
            icon: Some(data),
            ..Default::default()
        }),
    }
}

/// 找出图标在包里的条目名
///
/// 支持：Forge / NeoForge 的 `mods.toml` `logoFile`、1.12 及更早 Forge 老包的
/// `mcmod.info` `logoFile`、Fabric / Quilt 的 `icon`，后者是字符串或按尺寸分的
/// 对象如 `{"64": "…png"}`。
///
/// - `archive`: 模组压缩包
///
/// # 返回值
///
/// 返回包内条目名；元数据缺失或没写图标时返回 `None`
pub(super) fn find_mod_icon_path(archive: &mut ZipArchive<impl Read + Seek>) -> Option<String> {
    // Forge / NeoForge：mods.toml 的 logoFile
    for name in [
        names::MC_MOD_TOML_FILE,
        names::NEO_TOML_FILE,
        names::NEO_TOML1_FILE,
    ] {
        let Ok(mut file) = archive.by_name(name) else {
            continue;
        };
        let mut text = String::new();
        if file.read_to_string(&mut text).is_err() {
            continue;
        }
        if let Some(path) = forge_logo_file(&text) {
            return Some(path);
        }
    }

    // 1.12 及更早的 Forge 老包：mcmod.info 的 logoFile，这类包没有 mods.toml
    if let Some(path) = mcmod_logo_file(archive) {
        return Some(path);
    }

    // Fabric / Quilt：JSON 的 icon，quilt 在 quilt_loader.metadata.icon 下
    for name in [names::FABRIC_MOD_FILE, names::QUILT_MOD_FILE] {
        let Ok(file) = archive.by_name(name) else {
            continue;
        };
        let Ok(obj) = MiniJsonObj::from_stream(file) else {
            continue;
        };
        let Some(root) = obj.as_object() else {
            continue;
        };
        let holder = if name == names::QUILT_MOD_FILE {
            root.get_object("quilt_loader")
                .and_then(|loader| loader.get_object("metadata"))
        } else {
            Some(root)
        };
        let Some(holder) = holder else {
            continue;
        };

        // 字符串形式
        if let Some(value) = holder.get_opt_string("icon") {
            return Some(value);
        }
        // 对象形式按尺寸分档，取第一个非空值够用
        if let Some(map) = holder.get_object("icon") {
            for (_, value) in map.iter() {
                if let Some(value) = value.as_string() {
                    return Some(value);
                }
            }
        }
    }

    None
}

/// 从 mods.toml 文本里取未被注释的 `logoFile`
///
/// Forge 的 mods.toml 模板里 `logoFile` 基本都注释掉了，如 `#logoFile="examplemod.png"`；
/// 照读会去开不存在的条目，所以跳过注释行并去掉行尾注释。
pub(super) fn forge_logo_file(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("logoFile") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        // 去掉引号与行内注释，如 `logoFile="a.png" # 图标`
        let value = rest.split('#').next().unwrap_or(rest).trim();
        let value = value.trim_matches(|c| c == '"' || c == '\'').trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

/// 从 `mcmod.info` 里取 `logoFile`，只认 1.12 及更早的 Forge 老包
///
/// - `archive`: 模组压缩包
///
/// # 返回值
///
/// 返回包内条目名；没有 `mcmod.info`、读取失败或谁都没写 `logoFile` 时返回 `None`
pub(super) fn mcmod_logo_file(archive: &mut ZipArchive<impl Read + Seek>) -> Option<String> {
    let mut file = archive.by_name(names::MC_MOD_INFO_FILE).ok()?;
    let mut json = String::new();
    file.read_to_string(&mut json).ok()?;

    mcmod_logo_file_in(&json)
}

/// 从 `mcmod.info` 的**文本**里取 `logoFile`，即 [`mcmod_logo_file`] 的纯文本部分
///
/// 形态与 [`read_forge_json`] 一致：顶层是数组，或带 `modList` 数组的对象。
/// 逐条看 `logoFile`，取第一个非空的 —— 图标是装饰性的，多条目包认第一个就够。
///
/// 容错也照 [`read_forge_json`] 那条路：先直接解析，失败再用 [`sanitize_mcmod_json`]
/// 修一遍单引号 / 数组里未加引号的裸标识符。
pub(super) fn mcmod_logo_file_in(json: &str) -> Option<String> {
    let obj = match MiniJsonObj::from_str(json) {
        Ok(obj) => obj,
        Err(_) => MiniJsonObj::from_str(&sanitize_mcmod_json(json)).ok()?,
    };

    let values = if obj.is_list() {
        obj.as_list()
    } else {
        obj.as_object().and_then(|map| map.get_list("modList"))
    }?;

    for value in values.iter() {
        let Some(map) = value.as_object() else {
            continue;
        };
        let Some(path) = map.get_opt_string("logoFile") else {
            continue;
        };
        let path = path.trim();
        if !path.is_empty() {
            return Some(path.to_string());
        }
    }

    None
}

/// 读取模组信息
///
/// - `path`: 模组文件路径
///
/// # 返回值
///
/// 返回解析出的模组信息；打开或解析失败返回对应错误
pub fn read_mod_info<P: AsRef<Path>>(path: P) -> CoreResult<ModObj> {
    let file = path_helper::open_read(&path)?;
    let mut zip = ZipArchive::new(file).map_err(|err| {
        ErrorType::ArchiveOpenError(FileSystemErrorData {
            path: path.as_ref().to_path_buf(),
            error: err.to_string(),
        })
    })?;

    // base 传文件路径本身：内置 jar 的路径链就是"文件路径/父条目名/子条目名"，
    // 这样不同模组里的同名内置 jar 也能区分开
    let mut mod_info = parse_mod_archive(&mut zip, path.as_ref())?;
    mod_info.file = path.as_ref().to_path_buf();

    // 从注解扫描 side，仅文件类模组可用
    if let Ok(scan_result) = class_scan::scan_jar(path.as_ref()) {
        for scan_mod in &scan_result.mods {
            if let Some(info) = mod_info
                .info
                .iter_mut()
                .find(|info| info.mod_id == scan_mod.modid)
            {
                info.side = scan_mod.side;
            }
        }
    }

    Ok(mod_info)
}

/// 读模组
///
/// - `path`: 模组文件路径
/// - `sha256`: 是否连 SHA256 一起算，否则只算 SHA1
///
/// # 返回值
///
/// 返回模组信息，含哈希；读取失败返回对应错误
pub(super) fn read_mod<P: AsRef<Path>>(path: P, sha256: bool) -> CoreResult<ModObj> {
    let sha1 = hash_helper::gen_hash_from_file(HashType::Sha1, path.as_ref())?;

    let hash = if sha256 {
        let sha256 = hash_helper::gen_hash_from_file(HashType::Sha256, path.as_ref())?;
        FileHash::Sha1Sha256(sha1, sha256)
    } else {
        FileHash::Sha1(sha1)
    };

    let mut mod_info = read_mod_info(path)?;
    mod_info.hash = hash;

    Ok(mod_info)
}

/// 模组 uuid 的命名空间，固定值，保证不同启动器数据目录下生成的 uuid 一致
const MOD_UUID_NAMESPACE: Uuid = Uuid::from_u128(0x9d1a_2f3e_4c5b_6a70_8192_a3b4_c5d6_e7f8);

/// 模组的稳定标识：文件完整路径的 uuid v5
///
/// 同一文件每次扫描得到同一个 uuid，启用 / 禁用 / 删除按它定位
///
/// - `path`: 模组文件完整路径
///
/// # 返回值
///
/// 返回模组的稳定标识
///
/// **注意文件名一变 uuid 就变**，它是路径的哈希；启用 / 禁用只是给文件名加减后缀，
/// 所以那两个操作之后 uuid 会变成另一个值，需要同步身份的地方都靠这个函数重算。
pub fn gen_mod_uuid(path: &Path) -> Uuid {
    Uuid::new_v5(&MOD_UUID_NAMESPACE, path.to_string_lossy().as_bytes())
}

/// 扫描文件列表
///
/// - `files`: 文件列表
/// - `process_fn`: 单个文件的处理函数
/// - `gui`: 进度回调，可传 `None`，见 [`ProgressGui`]；每完成一个文件报一次 `(已完成, 总数)`
///
/// # 返回值
///
/// 返回扫描出的模组列表，读取失败的文件标记 `fail`
pub(super) fn scan_mod_files<F>(files: Vec<PathBuf>, process_fn: F, gui: ProgressGui) -> Vec<ModObj>
where
    F: Fn(&PathBuf) -> CoreResult<ModObj> + Send + Sync,
{
    let list = Mutex::new(Vec::new());

    // 总数只算**真正要处理的** .jar / .disable / .disabled：
    // 目录里可能有 .txt、.DS_Store 之类，拿文件总数当中会导致进度永远到不了 x/x
    let total = files
        .iter()
        .filter(|item| {
            item.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case(names::JAR_EXT)
                    || ext.eq_ignore_ascii_case(names::DISABLE_EXT)
                    || ext.eq_ignore_ascii_case(names::DISABLED_EXT)
            })
        })
        .count();
    // 并行完成数，rayon 里各线程都会加
    let done = AtomicUsize::new(0);

    files.par_iter().for_each(|item| {
        if let Some(ext) = item.extension() {
            let is_jar = ext.eq_ignore_ascii_case(names::JAR_EXT);
            let is_disabled = ext.eq_ignore_ascii_case(names::DISABLE_EXT)
                || ext.eq_ignore_ascii_case(names::DISABLED_EXT);

            if is_jar || is_disabled {
                let disable = is_disabled;
                let result = process_fn(item);

                let mut entry = match result {
                    Ok(mut item) => {
                        item.disable = disable;
                        item
                    }
                    Err(err) => {
                        mml_log::error_type(err);
                        ModObj {
                            fail: true,
                            file: item.clone(),
                            ..Default::default()
                        }
                    }
                };
                entry.uuid = gen_mod_uuid(item);

                list.lock().unwrap().push(entry);

                if let Some(gui) = &gui {
                    // 完成一个报一次，顺序不定，界面只关心计数
                    gui.set_progress_now(done.fetch_add(1, Ordering::Relaxed) + 1, Some(total));
                }
            }
        }
    });

    // 收尾对齐到 x/x：总数只统计了要处理的文件，正常已经在循环里报满；
    // 万一过滤与实际处理有出入，这里保证进度不会停在 x-1/x
    if let Some(gui) = &gui {
        gui.set_progress_now(total, Some(total));
    }

    list.into_inner().unwrap()
}
