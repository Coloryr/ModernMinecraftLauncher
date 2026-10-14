//! 游戏实例配置文件
//!
//! 「修改实例配置」用的文件清单与四种读法：文本全文、NBT 树、区域文件区块表、
//! 任意文件的二进制分段。文件范围是 `.minecraft` 下的 options.txt 与
//! `config` / `kubejs` / `local`，另加某个存档目录（含数据包）；
//! 路径一律按「相对游戏目录」的口径传，读与写共用这一套。

use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use mml_base::hash_helper;
use mml_names::names;
use mml_nbt::NbtType;
use mml_nbt::chunk::ChunkData;
use mml_nbt::nbt_file::NbtFile;
use mml_sys::path_helper;

use crate::dtos::{BytesSliceDto, FileItemDto, FileItemType, McaChunkDto, NbtKindDto, NbtNodeDto};
use crate::err_keys;

use super::instance_snapshot;

/// 单个配置文件的可读上限，单位字节
///
/// 文本全文与 NBT 树都要整份塞进一个 IPC 返回值；超过它一律拒绝，
/// 免得在列表里误点一个大文件把窗口卡死。
const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;

/// 二进制视图一次最多读多少字节
///
/// 一页给 64 KiB 足够铺满屏幕；再大只是让 IPC 白搬数据。
const MAX_SLICE_SIZE: u32 = 64 * 1024;

/// 按扩展名认文件类型
///
/// 认不出来的一律按纯文本：这里面对的是人写的配置文件，扩展名没见过也该能打开看看，
/// 别因为认不出来就把它藏了。大小写不敏感，`.JSON` 也算 JSON。
fn get_file_type<P: AsRef<Path>>(path: P) -> FileItemType {
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
pub(super) fn resolve_under(base: &Path, rel: &str) -> Result<PathBuf, String> {
    // 前端可能把 Windows 的反斜杠原样传回来，统一成正斜杠再解析
    let rel = rel.trim().replace('\\', "/");
    let path = Path::new(&rel);
    if rel.is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(String::from(err_keys::FILE_NAME));
    }
    Ok(base.join(path))
}

/// 按「相对游戏目录的路径」把一个文件收进列表
///
/// 分隔符统一成 `/`：前端拿它拼树、再原样传回来，反斜杠在界面上只会是噪音。
///
/// - `items`: 结果列表
/// - `base`: 游戏目录，即 `.minecraft`
/// - `file`: 文件的绝对路径；不在 `base` 下时跳过
fn push_file(items: &mut Vec<FileItemDto>, base: &Path, file: &Path) {
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
fn check_binary(path: &Path) -> Result<(), String> {
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
fn check_size(path: &Path) -> Result<(), String> {
    let size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    if size > MAX_FILE_SIZE {
        return Err(String::from(err_keys::FILE_TOO_LARGE));
    }
    Ok(())
}

/// 把一个 NBT 标签转成树上的节点
///
/// 数组**只报元素个数**，不展开内容：一个高度图动辄上万项，铺开既慢也没人看，
/// 要看原始字节走二进制视图。
///
/// - `name`: 键名；列表元素传下标字符串，根节点传空串
/// - `nbt`: 标签本体
///
/// # 返回值
///
/// 见 [`NbtNodeDto`]；复合的成员按**键名排序**，HashMap 的顺序不可靠
fn nbt_node(name: String, nbt: &NbtType) -> NbtNodeDto {
    /// 标量节点：只有类型与值
    fn leaf(name: String, kind: NbtKindDto, value: String) -> NbtNodeDto {
        NbtNodeDto {
            name,
            kind,
            value,
            length: 0,
            children: Vec::new(),
            items: Vec::new(),
        }
    }

    /// 数组节点：只有类型与元素个数
    fn array(name: String, kind: NbtKindDto, length: usize) -> NbtNodeDto {
        NbtNodeDto {
            name,
            kind,
            value: String::new(),
            length: length as u32,
            children: Vec::new(),
            items: Vec::new(),
        }
    }

    match nbt {
        NbtType::End(_) => leaf(name, NbtKindDto::End, String::new()),
        NbtType::Byte(v) => leaf(name, NbtKindDto::Byte, v.data.to_string()),
        NbtType::Short(v) => leaf(name, NbtKindDto::Short, v.data.to_string()),
        NbtType::Int(v) => leaf(name, NbtKindDto::Int, v.data.to_string()),
        NbtType::Long(v) => leaf(name, NbtKindDto::Long, v.data.to_string()),
        NbtType::Float(v) => leaf(name, NbtKindDto::Float, v.data.to_string()),
        NbtType::Double(v) => leaf(name, NbtKindDto::Double, v.data.to_string()),
        NbtType::String(v) => leaf(name, NbtKindDto::String, v.data.clone()),
        NbtType::ByteArray(v) => array(name, NbtKindDto::ByteArray, v.data.len()),
        NbtType::IntArray(v) => array(name, NbtKindDto::IntArray, v.data.len()),
        NbtType::LongArray(v) => array(name, NbtKindDto::LongArray, v.data.len()),
        NbtType::List(v) => {
            let items: Vec<NbtNodeDto> = v
                .iter()
                .enumerate()
                .map(|(index, item)| nbt_node(index.to_string(), item))
                .collect();
            let length = items.len() as u32;
            NbtNodeDto {
                name,
                kind: NbtKindDto::List,
                value: String::new(),
                length,
                children: Vec::new(),
                items,
            }
        }
        NbtType::Compound(v) => {
            let mut children: Vec<NbtNodeDto> = v
                .data
                .iter()
                .map(|(key, item)| nbt_node(key.clone(), item))
                .collect();
            children.sort_by(|a, b| a.name.cmp(&b.name));
            NbtNodeDto {
                name,
                kind: NbtKindDto::Compound,
                value: String::new(),
                length: 0,
                children,
                items: Vec::new(),
            }
        }
    }
}

/// 读文件的一段字节
///
/// 偏移与长度都夹到文件范围内，**不报越界错**：翻到最后一页时给不足一页的尾巴、
/// 越界给空段，前端不用为边界单独兜一次。
///
/// - `path`: 文件的绝对路径
/// - `offset`: 起始偏移，单位字节；越界按文件末尾算
/// - `length`: 想要的字节数；超过 [`MAX_SLICE_SIZE`] 按上限截断
///
/// # 返回值
///
/// 见 [`BytesSliceDto`]；文件读不到时返回内核错误串
fn read_slice(path: &Path, offset: u64, length: u32) -> Result<BytesSliceDto, String> {
    let mut stream = path_helper::open_read(path).map_err(|err| err.to_string())?;
    let total = stream.metadata().map_err(|err| err.to_string())?.len();
    let offset = offset.min(total);
    let want = u64::from(length.min(MAX_SLICE_SIZE)).min(total - offset);

    let mut buf = vec![0u8; want as usize];
    if want > 0 {
        stream
            .seek(SeekFrom::Start(offset))
            .map_err(|err| err.to_string())?;
        stream.read_exact(&mut buf).map_err(|err| err.to_string())?;
    }

    Ok(BytesSliceDto {
        offset,
        length: buf.len() as u32,
        total,
        data: hash_helper::gen_base64_bytes(&buf),
    })
}

/// 获取实例可编辑的配置文件列表
///
/// 收 options.txt 与 `config` / `kubejs` / `local` 下的全部文件，
/// 名字都是**相对游戏目录**的路径，与下面几个读 / 写命令同一口径。
///
/// - `game`: 实例 uuid
///
/// # 返回值
///
/// 文件条目列表；uuid 非法或实例不存在时返回错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_files(game: String) -> Result<Vec<FileItemDto>, String> {
    let game = instance_snapshot(&game)?;
    let base = game.get_game_path();
    let mut items = Vec::new();

    let option = game.get_option_file();
    if option.is_file() {
        push_file(&mut items, &base, &option);
    }

    for dir in [
        game.get_config_path(),
        game.get_kubejs_path(),
        game.get_local_path(),
    ] {
        for file in path_helper::get_all_files(dir) {
            push_file(&mut items, &base, &file);
        }
    }

    Ok(items)
}

/// 读取一个文本配置文件的内容
///
/// - `game`: 实例 uuid
/// - `file`: 相对游戏目录的路径，取自 [`config_get_files`] 或 [`config_saves_get_file`]
///
/// # 返回值
///
/// 全文；路径非法返回 `err.fileName`，二进制文件返回 `err.fileBinary`，
/// 体积超过 [`MAX_FILE_SIZE`] 返回 `err.fileTooLarge`，读取失败返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_file(game: String, file: String) -> Result<String, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_under(&game.get_game_path(), &file)?;
    check_binary(&path)?;
    check_size(&path)?;

    path_helper::read_text(&path).map_err(|err| err.to_string())
}

/// 保存文本配置文件，整份覆盖
///
/// - `game`: 实例 uuid
/// - `file`: 相对游戏目录的路径，与 [`config_get_file`] 同一口径
/// - `data`: 新的全文内容
///
/// # 返回值
///
/// 路径非法返回 `err.fileName`，二进制文件返回 `err.fileBinary`，写盘失败返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_save_file(game: String, file: String, data: String) -> Result<(), String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_under(&game.get_game_path(), &file)?;
    check_binary(&path)?;

    path_helper::write_text(&path, &data).map_err(|err| err.to_string())
}

/// 读取一个 NBT 文件，转成可展示的树
///
/// - `game`: 实例 uuid
/// - `file`: 相对游戏目录的路径
///
/// # 返回值
///
/// 见 [`NbtNodeDto`]；路径非法返回 `err.fileName`，体积超过 [`MAX_FILE_SIZE`] 返回
/// `err.fileTooLarge`，不是 NBT 或解不开时返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_nbt(game: String, file: String) -> Result<NbtNodeDto, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_under(&game.get_game_path(), &file)?;
    check_size(&path)?;

    let mut stream = path_helper::open_read(&path).map_err(|err| err.to_string())?;
    let nbt = NbtFile::read(&mut stream).map_err(|err| err.to_string())?;

    Ok(nbt_node(String::new(), &nbt.nbt))
}

/// 读取一个区域文件（`.mca`）的区块表
///
/// 只读 8 KiB 文件头，**不解析任何一个区块**：一个区域文件最多 1024 个区块，
/// 全解析一遍只为列个表不值当。要看某个区块的字节走 [`config_get_bytes_slice`]。
///
/// - `game`: 实例 uuid
/// - `file`: 相对游戏目录的路径
///
/// # 返回值
///
/// 有数据的区块，按表项下标升序；空表项不返回。路径非法返回 `err.fileName`，
/// 扩展名不是 `.mca` 返回 `err.notMcaFile`，文件头读不满返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_mca(game: String, file: String) -> Result<Vec<McaChunkDto>, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_under(&game.get_game_path(), &file)?;
    if get_file_type(&path) != FileItemType::Mca {
        return Err(String::from(err_keys::NOT_MCA_FILE));
    }

    let mut stream = path_helper::open_read(&path).map_err(|err| err.to_string())?;
    let head = ChunkData::read_head(&mut stream).map_err(|err| err.to_string())?;

    Ok(head
        .iter()
        .filter(|item| item.count > 0)
        .map(|item| McaChunkDto {
            index: item.index,
            // 表项下标 = `(x & 31) + (z & 31) * 32`，见 mml_nbt::chunk::chunk_to_head_pos
            x: (item.index % 32) as i32,
            z: (item.index / 32) as i32,
            pos: item.pos,
            sectors: item.count,
            time: item.time,
        })
        .collect())
}

/// 读取任意文件的一段字节，给二进制视图翻页用
///
/// 不过 [`check_binary`]：图片、压缩包就是靠这个看的，NBT 与区域文件也能拿它看原始字节。
///
/// - `game`: 实例 uuid
/// - `file`: 相对游戏目录的路径
/// - `offset`: 起始偏移，单位字节
/// - `length`: 想要的字节数，单位字节；上限 [`MAX_SLICE_SIZE`]
///
/// # 返回值
///
/// 见 [`BytesSliceDto`]；路径非法返回 `err.fileName`，读取失败返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_bytes_slice(
    game: String,
    file: String,
    offset: u64,
    length: u32,
) -> Result<BytesSliceDto, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_under(&game.get_game_path(), &file)?;

    read_slice(&path, offset, length)
}

/// 图片文件扩展名 → MIME
///
/// 与 [`get_file_type`] 的 `Image` 分支同口径；认不出的按二进制流给，让浏览器自己判。
fn image_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// 读取一个图片文件，转成可以直接塞进 `<img src>` 的 data URL
///
/// 配置目录里的图片都是图标 / 缩略图量级，一次 IPC 读完即可；体积上限沿用
/// [`MAX_FILE_SIZE`]，再大就不是"看一眼"的用途了。
///
/// - `game`: 实例 uuid
/// - `file`: 相对游戏目录的路径，取自 [`config_get_files`] 或 [`config_saves_get_file`]
///
/// # 返回值
///
/// `data:<mime>;base64,<...>`；路径非法返回 `err.fileName`，不是图片返回
/// `err.notImageFile`，体积超过上限返回 `err.fileTooLarge`
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_image(game: String, file: String) -> Result<String, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_under(&game.get_game_path(), &file)?;
    if get_file_type(&path) != FileItemType::Image {
        return Err(String::from(err_keys::NOT_IMAGE_FILE));
    }
    check_size(&path)?;

    let mut stream = path_helper::open_read(&path).map_err(|err| err.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|err| err.to_string())?;

    Ok(format!(
        "data:{};base64,{}",
        image_mime(&path),
        hash_helper::gen_base64_bytes(&buf)
    ))
}

/// 获取一个存档目录下的文件列表
///
/// 整个存档目录递归列出，含数据包、`level.dat` 与区域文件；类型标在 `fileType` 上，
/// 前端据此决定打开哪种视图。
///
/// - `game`: 实例 uuid
/// - `save`: 存档目录名，即 saves 下的一级目录
///
/// # 返回值
///
/// 相对游戏目录的路径 + 类型；目录名非法返回 `err.fileName`，目录不存在返回 `err.saveNotFound`
#[gui_macros::ipc_group("config_saves")]
#[tauri::command]
pub fn config_saves_get_file(game: String, save: String) -> Result<Vec<FileItemDto>, String> {
    let game = instance_snapshot(&game)?;
    let base = game.get_game_path();
    let dir = resolve_under(&game.get_saves_path(), &save)?;
    if !dir.is_dir() {
        return Err(String::from(err_keys::SAVE_NOT_FOUND));
    }

    let mut items = Vec::new();
    for file in path_helper::get_all_files(dir) {
        push_file(&mut items, &base, &file);
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use mml_base::hash_helper;
    use uuid::Uuid;

    use crate::dtos::{FileItemType, NbtKindDto};

    use super::{MAX_SLICE_SIZE, check_binary, get_file_type, nbt_node, read_slice, resolve_under};

    /// 相对路径要挡掉穿越：`..`、绝对路径、空串都不行，正反斜杠都认
    #[test]
    fn resolve_under_rejects_escape() {
        let base = Path::new("D:/mml/game/.minecraft");

        assert!(resolve_under(base, "options.txt").is_ok());
        assert!(resolve_under(base, "config/abc.toml").is_ok());
        // 前端可能把 Windows 的反斜杠原样传回来
        assert!(resolve_under(base, "config\\abc.toml").is_ok());

        assert!(resolve_under(base, "../gui_config.json").is_err());
        assert!(resolve_under(base, "config/../../x.txt").is_err());
        assert!(resolve_under(base, "/etc/passwd").is_err());
        assert!(resolve_under(base, "").is_err());
        assert!(resolve_under(base, "   ").is_err());
    }

    /// 二进制那几类要被认出来：前端据此选视图
    #[test]
    fn binary_extensions_are_marked() {
        for name in [
            "level.dat",
            "level.dat_old",
            "advancements.rio",
            "structure.nbt",
            "r.0.0.mca",
            "icon.png",
            "photo.JPEG",
            "datapacks/pack.zip",
            "mod.jar",
        ] {
            assert!(
                matches!(
                    get_file_type(name),
                    FileItemType::Nbt
                        | FileItemType::Mca
                        | FileItemType::Image
                        | FileItemType::Archive
                ),
                "{name} 应认成二进制"
            );
        }
    }

    /// 常见配置文件的类型要认对，认不出来的一律纯文本
    #[test]
    fn common_extensions_are_typed() {
        let cases = [
            ("config/a.json", FileItemType::Json),
            ("pack.mcmeta", FileItemType::Json),
            ("config/a.JSONC", FileItemType::Json),
            ("config/a.json5", FileItemType::Json5),
            ("config/a.toml", FileItemType::Toml),
            ("config/a.yml", FileItemType::Yaml),
            ("config/a.YAML", FileItemType::Yaml),
            ("config/a.properties", FileItemType::Properties),
            ("config/old.cfg", FileItemType::Properties),
            ("config/a.conf", FileItemType::Properties),
            ("config/a.ini", FileItemType::Properties),
            ("config/a.xml", FileItemType::Xml),
            ("kubejs/server.js", FileItemType::JavaScript),
            ("kubejs/server.mjs", FileItemType::JavaScript),
            ("data/x.snbt", FileItemType::Snbt),
            ("data/f.mcfunction", FileItemType::Text),
            ("README", FileItemType::Text),
            ("options.txt", FileItemType::Text),
        ];

        for (name, want) in cases {
            assert_eq!(get_file_type(name), want, "{name} 的类型不对");
        }
    }

    /// 文本编辑只认文本：二进制那几类必须挡掉，它们走结构视图或二进制视图
    #[test]
    fn text_edit_refuses_binary() {
        assert!(check_binary(Path::new("options.txt")).is_ok());
        assert!(check_binary(Path::new("config/a.toml")).is_ok());
        assert!(check_binary(Path::new("data/f.mcfunction")).is_ok());

        for name in ["level.dat", "r.0.0.mca", "icon.png", "datapacks/pack.zip"] {
            assert!(
                check_binary(Path::new(name)).is_err(),
                "{name} 不该给文本编辑"
            );
        }
    }

    /// 二进制分段读取：越界与超长都夹到文件范围内，不报错
    #[test]
    fn slice_is_clamped_to_file() {
        let dir = mml_testutil::temp_dir().join(format!("mml-config-slice-{}", Uuid::new_v4()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("icon.png");
        fs::write(&file, b"0123456789").unwrap();

        let whole = read_slice(&file, 0, 4096).unwrap();
        assert_eq!((whole.offset, whole.length, whole.total), (0, 10, 10));
        assert_eq!(whole.data, hash_helper::gen_base64_bytes(b"0123456789"));

        // 中间一段
        let mid = read_slice(&file, 4, 3).unwrap();
        assert_eq!((mid.offset, mid.length), (4, 3));
        assert_eq!(mid.data, hash_helper::gen_base64_bytes(b"456"));

        // 尾巴不足一页：给剩下的
        let tail = read_slice(&file, 8, 4096).unwrap();
        assert_eq!(tail.length, 2);
        assert_eq!(tail.data, hash_helper::gen_base64_bytes(b"89"));

        // 越界：给空段，不是错误
        let over = read_slice(&file, 100, 16).unwrap();
        assert_eq!((over.offset, over.length), (10, 0));

        // 一次要得再多也只给一页
        fs::write(&file, vec![7u8; MAX_SLICE_SIZE as usize + 16]).unwrap();
        let big = read_slice(&file, 0, u32::MAX).unwrap();
        assert_eq!(big.length, MAX_SLICE_SIZE);
        assert_eq!(big.total, u64::from(MAX_SLICE_SIZE) + 16);

        let _ = fs::remove_dir_all(&dir);
    }

    /// NBT 转树：复合按键名排序、数组只报个数、列表记下标、标量带值
    #[test]
    fn nbt_tree_is_converted() {
        use mml_nbt::nbt_types::{NbtByte, NbtByteArray, NbtCompound, NbtInt, NbtList, NbtString};

        let mut inner = NbtCompound::new();
        inner
            .data
            .insert(String::from("z"), NbtInt::new(2).to_nbt());
        inner.data.insert(
            String::from("a"),
            NbtString::new(String::from("hi")).to_nbt(),
        );

        let mut list = NbtList::new(3);
        assert!(list.add_item(NbtInt::new(7).to_nbt()));
        assert!(list.add_item(NbtInt::new(8).to_nbt()));

        let mut root = NbtCompound::new();
        root.data.insert(String::from("nums"), list.to_nbt());
        root.data.insert(String::from("nested"), inner.to_nbt());
        root.data
            .insert(String::from("b"), NbtByte::new(5).to_nbt());
        root.data.insert(
            String::from("arr"),
            NbtByteArray::new(vec![1, 2, 3]).to_nbt(),
        );

        let node = nbt_node(String::new(), &root.to_nbt());
        assert_eq!(node.kind, NbtKindDto::Compound);
        assert!(node.name.is_empty());

        // 成员按键名排序：arr / b / nested / nums
        let names: Vec<&str> = node.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["arr", "b", "nested", "nums"]);

        assert_eq!(node.children[0].kind, NbtKindDto::ByteArray);
        assert_eq!(node.children[0].length, 3);
        assert!(node.children[0].value.is_empty());

        assert_eq!(node.children[1].kind, NbtKindDto::Byte);
        assert_eq!(node.children[1].value, "5");

        // 内层复合也排序：a / z
        let nested = &node.children[2];
        let inner_names: Vec<&str> = nested.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(inner_names, ["a", "z"]);
        assert_eq!(nested.children[0].value, "hi");

        // 列表元素按下标命名，类型与值都在
        let nums = &node.children[3];
        assert_eq!(nums.kind, NbtKindDto::List);
        assert_eq!(nums.length, 2);
        assert_eq!(nums.items[0].name, "0");
        assert_eq!(nums.items[0].kind, NbtKindDto::Int);
        assert_eq!(nums.items[0].value, "7");
        assert_eq!(nums.items[1].value, "8");
    }
}
