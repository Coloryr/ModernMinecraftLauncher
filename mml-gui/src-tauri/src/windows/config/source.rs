//! 文件来源解析与类型判定
//!
//! 来源为空串是实例本身，否则是存档目录名；路径一律按「相对所选来源」的口径传。

use std::path::{Component, Path, PathBuf};

use mml_names::names;

use crate::dtos::{FileItemDto, FileItemType};
use crate::err_keys;

use mml_game::launcher::instance_setting_obj::InstanceSettingObj;

/// 单个配置文件的可读上限，单位字节
///
/// 文本全文与 NBT 树都要整份塞进一个 IPC 返回值；超过它一律拒绝，
/// 免得在列表里误点一个大文件把窗口卡死。
pub(super) const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;

/// 按扩展名认文件类型
///
/// 认不出来的一律按纯文本：这里面对的是人写的配置文件，扩展名没见过也该能打开看看，
/// 别因为认不出来就把它藏了。大小写不敏感，`.JSON` 也算 JSON。
///
/// - `path`: 文件路径，只取扩展名
///
/// # 返回值
///
/// 见 [`FileItemType`]
pub(super) fn get_file_type<P: AsRef<Path>>(path: P) -> FileItemType {
    let Some(ext) = path.as_ref().extension() else {
        return FileItemType::Text;
    };
    let ext = ext.to_string_lossy().to_ascii_lowercase();

    match ext.as_str() {
        names::JSON_EXT | names::JSONC_EXT | names::MCMETA_EXT => FileItemType::Json,
        names::JSON5_EXT => FileItemType::Json5,
        names::TOML_EXT => FileItemType::Toml,
        names::YAML_EXT | names::YML_EXT => FileItemType::Yaml,
        names::PROPERTIES_EXT | names::CFG_EXT | names::CONF_EXT | names::INI_EXT => {
            FileItemType::Properties
        }
        names::XML_EXT => FileItemType::Xml,
        names::JS_EXT | names::MJS_EXT | names::CJS_EXT => FileItemType::JavaScript,
        names::SNBT_EXT => FileItemType::Snbt,
        names::DAT_EXT | names::DAT_OLD_EXT | names::RIO_EXT | names::NBT_EXT => FileItemType::Nbt,
        names::MCA_EXT => FileItemType::Mca,
        names::PNG_EXT | names::JPG_EXT | names::JPEG_EXT | names::GIF_EXT | names::WEBP_EXT => {
            FileItemType::Image
        }
        names::ZIP_EXT
        | names::JAR_EXT
        | names::R7Z_EXT
        | names::TAR_EXT
        | names::GZ_EXT
        | names::TGZ_EXT
        | names::TXZ_EXT
        | names::MRPACK_EXT => FileItemType::Archive,
        _ => FileItemType::Text,
    }
}

/// 解析相对路径，挡掉路径穿越
///
/// 路径是 IPC 入参，前端原样传回来，必须自己挡一次：只接受由普通名字组成的相对路径，
/// `..`、绝对路径、盘符一律拒绝。
///
/// - `base`: 基准目录
/// - `rel`: 相对 `base` 的路径，如 `options.txt`、`config/abc.toml`，或一个存档目录名
///
/// # 返回值
///
/// 非法路径返回 `err.fileName`
///
/// 导出窗口浏览实例目录时也用这一份，见 `windows::export::export_list_dir`
pub(crate) fn resolve_under(base: &Path, rel: &str) -> Result<PathBuf, String> {
    // 前端可能把 Windows 的反斜杠原样传回来，统一成正斜杠再解析
    let rel = rel.trim().replace('\\', "/");
    let path = Path::new(&rel);
    if rel.is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(String::from(err_keys::FILE_NAME));
    }
    Ok(base.join(path))
}

/// 按来源解析配置文件路径
///
/// 来源为空串表示实例本身，路径相对游戏目录；否则是存档目录名，路径相对该存档目录。
/// **列表与读 / 写共用它**：列表给出来的相对路径，原样传回来必须解到同一个文件。
///
/// - `game`: 实例
/// - `source`: 来源，空串为实例本身，否则是 saves 下的一级目录名
/// - `file`: 来源内的相对路径
///
/// # 返回值
///
/// 解析出的绝对路径；来源或路径非法（绝对路径、`..` 穿越）时返回 `err.fileName`
pub(super) fn resolve_source(
    game: &InstanceSettingObj,
    source: &str,
    file: &str,
) -> Result<PathBuf, String> {
    resolve_source_in(&game.get_game_path(), &game.get_saves_path(), source, file)
}

/// [`resolve_source`] 的纯版本：只吃两个基准目录，便于单测
///
/// - `game_path`: 游戏目录，即 `.minecraft`
/// - `saves_path`: 存档根目录，即游戏目录下的 `saves`
/// - `source`: 来源，空串为实例本身，否则是 `saves` 下的一级目录名
/// - `file`: 来源内的相对路径
///
/// # 返回值
///
/// 解析出的绝对路径；来源或路径非法时返回 `err.fileName`
pub(super) fn resolve_source_in(
    game_path: &Path,
    saves_path: &Path,
    source: &str,
    file: &str,
) -> Result<PathBuf, String> {
    // 空来源就是实例本身，直接相对游戏目录解
    if source.is_empty() {
        return resolve_under(game_path, file);
    }

    // 存档来源：先把存档目录解出来，再在它下面解文件
    let dir = resolve_under(saves_path, source)?;
    resolve_under(&dir, file)
}

/// 按「相对该来源的路径」把一个文件收进列表
///
/// 分隔符统一成 `/`：前端拿它拼树、再原样传回来，反斜杠在界面上只会是噪音。
///
/// - `items`: 结果列表
/// - `base`: 来源目录，即实例的游戏目录或某个存档目录
/// - `file`: 文件的绝对路径；不在 `base` 下时跳过
pub(super) fn push_file(items: &mut Vec<FileItemDto>, base: &Path, file: &Path) {
    let Ok(name) = file.strip_prefix(base) else {
        return;
    };
    items.push(FileItemDto {
        name: name.to_string_lossy().replace('\\', "/"),
        file_type: get_file_type(file),
    });
}

/// 校验这个文件能不能当文本编辑
///
/// - `path`: 文件的绝对路径
///
/// # 返回值
///
/// NBT / 区域文件 / 图片 / 压缩包返回 `err.fileBinary`，它们走各自的结构视图或二进制视图
pub(super) fn check_binary(path: &Path) -> Result<(), String> {
    match get_file_type(path) {
        FileItemType::Nbt | FileItemType::Mca | FileItemType::Image | FileItemType::Archive => {
            Err(String::from(err_keys::FILE_BINARY))
        }
        _ => Ok(()),
    }
}

/// 大文件不给整份读：文本全文与 NBT 树都要塞进一个 IPC 返回值
///
/// - `path`: 文件的绝对路径
///
/// # 返回值
///
/// 体积超过 [`MAX_FILE_SIZE`] 返回 `err.fileTooLarge`
pub(super) fn check_size(path: &Path) -> Result<(), String> {
    let size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    if size > MAX_FILE_SIZE {
        return Err(String::from(err_keys::FILE_TOO_LARGE));
    }
    Ok(())
}
