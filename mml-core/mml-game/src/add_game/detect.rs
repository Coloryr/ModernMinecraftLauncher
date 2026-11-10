//! 整合包类型识别
//!
//! 按包内文件判断是 CurseForge、Modrinth、各启动器导出包还是普通压缩包。

use super::MODRINTH_INDEX_FILE;

use std::path::Path;

use mml_base::{archives::BaseArchive, serialize_tools::MiniJsonObj};
use mml_names::{i18_items::error_type::CoreResult, names};

/// 压缩包类型
pub enum PackType {
    /// CurseForge 整合包
    CurseForge,
    /// Modrinth 整合包
    Modrinth,
    /// MMC 导出包
    MMC,
    /// HMCL 导出包
    HMCL,
    /// HMCL 服务器包
    HMCLServer,
    /// 直接解压，包内是本启动器导出的实例
    ArchivePack,
    /// 包含其他启动器数据目录的压缩包
    LauncherPack,
}

impl PackType {
    /// 压缩包独立 ID，与前端 i18n 键对应，不随语言变化
    ///
    /// # 返回值
    ///
    /// 固定的英文 ID，如 `curseforge`
    pub fn id(&self) -> &'static str {
        match self {
            PackType::CurseForge => "curseforge",
            PackType::Modrinth => "modrinth",
            PackType::MMC => "mmc",
            PackType::HMCL => "hmcl",
            PackType::HMCLServer => "hmcl_server",
            PackType::ArchivePack => "archive",
            PackType::LauncherPack => "launcher_pack",
        }
    }

    /// 按 ID 解析压缩包类型
    ///
    /// - `id`: `id()` 给出的英文 ID
    ///
    /// # 返回值
    ///
    /// 对应的类型；未知 ID 返回 `None`
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "curseforge" => Some(PackType::CurseForge),
            "modrinth" => Some(PackType::Modrinth),
            "mmc" => Some(PackType::MMC),
            "hmcl" => Some(PackType::HMCL),
            "hmcl_server" => Some(PackType::HMCLServer),
            "archive" => Some(PackType::ArchivePack),
            "launcher_pack" => Some(PackType::LauncherPack),
            _ => None,
        }
    }

    /// 全部压缩包 ID，下拉列表的数据源
    ///
    /// # 返回值
    ///
    /// 全部类型的 ID，顺序固定
    pub fn ids() -> Vec<&'static str> {
        vec![
            "curseforge",
            "modrinth",
            "mmc",
            "hmcl",
            "hmcl_server",
            "archive",
            "launcher_pack",
        ]
    }
}

/// 压缩包检测结果，添加实例窗口选好压缩包后自动填表用
pub struct DetectedPack {
    /// 检测出的压缩包类型
    pub pack_type: PackType,
    /// 推荐实例名：元数据里的名字，取不到就用文件名去扩展名
    pub name: String,
}

/// 读取压缩包内 JSON 条目的 `name` 字段，读不到返回空串
fn archive_json_name(archive: &BaseArchive, entry: &str) -> String {
    archive
        .read(entry)
        .ok()
        .and_then(|data| MiniJsonObj::from_stream(data.as_slice()).ok())
        .and_then(|json| json.as_object())
        .map(|map| {
            let name = map.get_string("name");
            let version = map
                .get_opt_string("version")
                .or(map.get_opt_string("versionId"));

            match version {
                Some(data) => format!("{name}-{data}"),
                None => name,
            }
        })
        .unwrap_or_default()
}

/// 读取 MMC 包 `instance.cfg` 里的 `name=xxx`，读不到返回空串
fn archive_cfg_name(archive: &BaseArchive) -> String {
    let entry = match archive
        .entries()
        .iter()
        .find(|e| !e.is_dir && e.name.replace('\\', "/").ends_with(names::MMCCFG_FILE))
    {
        Some(entry) => entry.name.clone(),
        None => return String::new(),
    };
    let Ok(data) = archive.read(&entry) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&data);
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("name=") {
            return value.trim().to_string();
        }
    }
    String::new()
}

/// 检测压缩包的整合包类型，并取推荐实例名
///
/// - `file`: 压缩包路径
///
/// 按元数据文件判定类型：**`game.json` 优先**，M²L 导出的实例包只要带它就直接按实例包处理，
/// 不被包内附带的其它启动器元数据抢走；其次 HMCL（包内可能同时带 `mcbbs.packmeta` 与
/// `manifest.json`，所以 CurseForge 放在 HMCL 之后）；无元数据时按 `.minecraft` 目录判
/// 启动器包，否则视为直接解压。名字取元数据里的 `name`，MMC 读 `instance.cfg`，
/// 取不到用文件名去扩展名。
///
/// # 返回值
///
/// 检测结果；压缩包无法打开返回对应错误
pub fn detect_pack<P: AsRef<Path>>(file: P) -> CoreResult<DetectedPack> {
    let file = file.as_ref();
    let archive = BaseArchive::open_readonly(file)?;
    detect_pack_of(&archive, file)
}

/// 在已经打开的压缩包上识别类型，省掉重复解析中央目录
///
/// - `archive`: 已打开的压缩包
/// - `file`: 压缩包路径，只用来兜底取文件名
///
/// # 返回值
///
/// 见 [`DetectedPack`]
pub fn detect_pack_of(archive: &BaseArchive, file: &Path) -> CoreResult<DetectedPack> {
    let find_entry = |suffix: &str| -> Option<String> {
        archive
            .entries()
            .iter()
            .find(|e| !e.is_dir && e.name.replace('\\', "/").ends_with(suffix))
            .map(|e| e.name.clone())
    };

    // 文件名去扩展名兜底
    let fallback = || {
        file.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };

    let mut detected: Option<(PackType, String)> = None;
    for (suffix, pack_type) in [
        // game.json 优先级最大：本启动器导出的实例包一定带它，包内再有别的元数据也按实例包装
        (names::GAME_FILE, PackType::ArchivePack),
        (names::HMCLFILE, PackType::HMCL),
        (names::SERVER_MANIFEST_FILE, PackType::HMCLServer),
        (names::MMCJSON_FILE, PackType::MMC),
        (MODRINTH_INDEX_FILE, PackType::Modrinth),
        (names::MANIFEST_FILE, PackType::CurseForge),
    ] {
        if let Some(entry) = find_entry(suffix) {
            detected = Some((pack_type, archive_json_name(archive, &entry)));
            break;
        }
    }

    let (pack_type, name) = match detected {
        Some((PackType::MMC, json_name)) if json_name.is_empty() => {
            (PackType::MMC, archive_cfg_name(archive))
        }
        Some((pack_type, name)) => (pack_type, name),
        None if super::launcher_pack::find_minecraft_prefix(archive.entries()).is_some() => {
            (PackType::LauncherPack, String::new())
        }
        None => (PackType::ArchivePack, String::new()),
    };

    let name = if name.is_empty() { fallback() } else { name };
    Ok(DetectedPack { pack_type, name })
}
