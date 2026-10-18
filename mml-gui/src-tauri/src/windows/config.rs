//! 游戏实例配置文件
//!
//! **一个实例一个窗口**：窗口身份由实例 uuid 派生、打开时就锁定，窗口内不再选择实例，
//! 见 `windows/registry` 的窗口族。NBT 会话也按实例归属，关掉某个实例的窗口不会动
//! 别的实例还开着的编辑器，见 [`nbt_session`]。
//!
//! 「修改实例配置」用的文件清单与四种读法：文本全文、NBT 树、区域文件区块表、
//! 任意文件的二进制分段。文件范围是 `.minecraft` 下的 options.txt 与
//! `config` / `kubejs` / `local`，另加某个存档目录（含数据包）。
//!
//! **路径一律按"相对所选来源"的口径传**：来源为空串是实例本身，即相对游戏目录；
//! 否则是存档目录名，相对该存档目录。列表给出来的路径原样回传即可，
//! 解析见 [`resolve_source`]。
//!
//! 文本的保存走**行级差异**而不是整份回传：前端算好"改了哪几行"（见
//! `mml-vue/src/lib/textDiff.ts`），后端按 [`ConfigTextDto::sha1`] 核对磁盘上还是
//! 打开时那一份，再套用差异，见 [`config_save_file`]。
//!
//! NBT 走的是另一条路：打开一个 NBT 文件就是**真的把它打开**（句柄与解析结果留在会话里，
//! 见 [`nbt_session`]），之后的保存是"改值 / 新增 / 删除"三种操作，直接套在会话那份树上、
//! 由同一个句柄写回，不再重新打开与重新读盘。

use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use mml_base::hash_helper::{self, HashType};
use mml_names::names;
use mml_nbt::chunk::ChunkData;
use mml_nbt::nbt_file::NbtFile;
use mml_nbt::nbt_types;
use mml_nbt::{NBT_END_ORDER, NbtType};
use mml_sys::path_helper;
use mml_sys::text_diff::{self, LineHunk};
use uuid::Uuid;

use crate::dtos::{
    BytesSliceDto, ChunkFindDto, ConfigHunkDto, ConfigNbtDto, ConfigTextDto, FileItemDto, FileItemType,
    McaChunkDto, NbtEditDto, NbtKindDto, NbtNodeDto, NbtOpDto,
};
use crate::err_keys;

use super::instance_snapshot;
use super::nbt_session;

use mml_game::launcher::instance_setting_obj::InstanceSettingObj;

/// 单个配置文件的可读上限，单位字节
///
/// 文本全文与 NBT 树都要整份塞进一个 IPC 返回值；超过它一律拒绝，
/// 免得在列表里误点一个大文件把窗口卡死。
const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;

/// 二进制视图一次最多读多少字节
///
/// 一页给 64 KiB 足够铺满屏幕；再大只是让 IPC 白搬数据。
const MAX_SLICE_SIZE: u32 = 64 * 1024;

/// 数组（字节 / 整数 / 长整数）最多展开多少个元素给前端编辑
///
/// 短数组展开成元素行，一行一个下标；比这长的（高度图、位图之类）只报个数，也不接受增删，
/// 免得一次 IPC 白搬几万个数。前端那份常量要与此一致。
const MAX_ARRAY_EDIT: usize = 1024;

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
fn resolve_source(game: &InstanceSettingObj, source: &str, file: &str) -> Result<PathBuf, String> {
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
fn resolve_source_in(
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
/// 数组在 [`MAX_ARRAY_EDIT`] 以内**展开成元素行**（元素不是独立标签，展开只是给前端一个
/// 可编辑的入口），超过就只报个数：一个高度图动辄上万项，铺开既慢也没人看，
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

    /// 数组节点：类型、元素个数，短数组连元素一起展开
    fn array<T: ToString>(
        name: String,
        kind: NbtKindDto,
        item_kind: NbtKindDto,
        data: &[T],
    ) -> NbtNodeDto {
        let items = if data.len() > MAX_ARRAY_EDIT {
            Vec::new()
        } else {
            data.iter()
                .enumerate()
                .map(|(index, item)| leaf(index.to_string(), item_kind, item.to_string()))
                .collect()
        };
        NbtNodeDto {
            name,
            kind,
            value: String::new(),
            length: data.len() as u32,
            children: Vec::new(),
            items,
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
        NbtType::ByteArray(v) => array(name, NbtKindDto::ByteArray, NbtKindDto::Byte, &v.data),
        NbtType::IntArray(v) => array(name, NbtKindDto::IntArray, NbtKindDto::Int, &v.data),
        NbtType::LongArray(v) => array(name, NbtKindDto::LongArray, NbtKindDto::Long, &v.data),
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
/// 列出某个来源下可编辑的配置文件
///
/// 来源为空串时收 options.txt 与 `config` / `kubejs` / `local` 下的全部文件；
/// 否则收该存档目录下的全部文件（含数据包、`level.dat` 与区域文件）。名字都是
/// **相对该来源**的路径，与下面几个读 / 写命令同一口径。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是 saves 下的一级目录名
///
/// # 返回值
///
/// 文件条目列表；uuid 非法、实例不存在或存档目录不存在时返回错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_files(game: String, source: String) -> Result<Vec<FileItemDto>, String> {
    let game = instance_snapshot(&game)?;
    let mut items = Vec::new();

    // 存档来源：树根就是这个存档目录，名字不再带 `saves/<目录>/` 前缀
    if !source.is_empty() {
        let base = resolve_under(&game.get_saves_path(), &source)?;
        if !base.is_dir() {
            return Err(String::from(err_keys::SAVE_NOT_FOUND));
        }
        for file in path_helper::get_all_files(&base) {
            push_file(&mut items, &base, &file);
        }
        return Ok(items);
    }

    let base = game.get_game_path();

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
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径，取自 [`config_get_files`]
///
/// # 返回值
///
/// 见 [`ConfigTextDto`]；路径非法返回 `err.fileName`，二进制文件返回 `err.fileBinary`，
/// 体积超过 [`MAX_FILE_SIZE`] 返回 `err.fileTooLarge`，读取失败返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_file(
    game: String,
    source: String,
    file: String,
) -> Result<ConfigTextDto, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    check_binary(&path)?;
    check_size(&path)?;

    let text = path_helper::read_text(&path).map_err(|err| err.to_string())?;
    Ok(ConfigTextDto {
        sha1: text_sha1(&text),
        text,
    })
}

/// 保存文本配置文件，按前端算好的**行级差异**改盘上那一份
///
/// 差异相对打开时读到的原文，所以先核对磁盘上还是不是那一份；不是就拒绝，
/// 免得把别的窗口（例如资源管理改 options.txt）的改动冲掉。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径，与 [`config_get_file`] 同一口径
/// - `base_sha1`: 打开时那一份全文的 SHA1，即 [`ConfigTextDto::sha1`]
/// - `expected_lines`: 改完后的总行数，按 `\n` 切成多少行；用来交叉校验差异算得对不对
/// - `hunks`: 行替换列表，按 `start` 升序、互不重叠
///
/// # 返回值
///
/// 保存后全文的 SHA1，供前端接着改；路径非法返回 `err.fileName`，二进制文件返回
/// `err.fileBinary`，磁盘上那一份已经变了返回 `err.fileChanged`，写盘失败返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_save_file(
    game: String,
    source: String,
    file: String,
    base_sha1: String,
    expected_lines: u32,
    hunks: Vec<ConfigHunkDto>,
) -> Result<String, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    check_binary(&path)?;

    let now = path_helper::read_text(&path).map_err(|err| err.to_string())?;
    if text_sha1(&now) != base_sha1 {
        return Err(String::from(err_keys::FILE_CHANGED));
    }

    // 差异套不上（越界、交叉）或行数对不上一律按"磁盘上那份已经变了"处理：
    // 两种都只说明这份差异不是对着现在这一份算的。`write_diff` 出错时不落盘
    let hunks: Vec<LineHunk> = hunks
        .iter()
        .map(|hunk| {
            LineHunk::new(
                hunk.start as usize,
                hunk.delete_count as usize,
                hunk.lines.clone(),
            )
        })
        .collect();

    let text = text_diff::write_diff(&path, &now, &hunks, expected_lines as usize)
        .map_err(|_| String::from(err_keys::FILE_CHANGED))?;
    Ok(text_sha1(&text))
}

/// 文本全文的 SHA1，十六进制小写
///
/// 按**解码后文本**的 UTF-8 字节算：读与写都过同一套解码，前后自然同一口径
fn text_sha1(text: &str) -> String {
    hash_helper::gen_hash(HashType::Sha1, text.as_bytes())
}

/// 读取一个 NBT 文件，转成可展示、可编辑的树
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径
///
/// 打开一个 NBT 文件，转成可展示、可编辑的树
///
/// **打开就是真的打开**：文件句柄与解析出的内容都留在会话里（见 [`nbt_session`]），
/// 之后的保存直接在那份状态上改、由同一个句柄写回。会话还在时再打开一次给的是会话里
/// 那棵树，不碰磁盘；要重新读盘就先 [`config_close_nbt`]，再打开。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径
///
/// # 返回值
///
/// 见 [`ConfigNbtDto`]：树 + **文件字节**的 SHA1（保存时按它核对）；
/// 路径非法返回 `err.fileName`，体积超过 [`MAX_FILE_SIZE`] 返回 `err.fileTooLarge`，
/// 不是 NBT 或解不开时返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_open_nbt(
    game: String,
    source: String,
    file: String,
    index: Option<u32>,
) -> Result<ConfigNbtDto, String> {
    // 会话按实例归属，关那个实例的配置窗口时只清它这一份，见 nbt_session
    let owner = Uuid::parse_str(&game).map_err(|_| err_keys::UUID.to_string())?;
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    // 上限是给"整份树要塞进一次 IPC"定的：区域文件里只装一个区块，不该按整份 mca 卡
    if index.is_none() {
        check_size(&path)?;
    }

    let (node, sha1) = nbt_session::open_with(&owner, &path, index, |nbt| {
        nbt_node(String::new(), &nbt.nbt)
    })?;
    Ok(ConfigNbtDto { node, sha1 })
}

/// 关掉一个 NBT 文件的会话，句柄跟着释放
///
/// 前端在切走配置窗口、或不想再保持这个文件打开时调用；下次打开就是重新读盘。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径，与 [`config_open_nbt`] 同一口径
///
/// # 返回值
///
/// 路径非法返回 `err.fileName`；没打开过的文件也当成功
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_close_nbt(
    game: String,
    source: String,
    file: String,
    index: Option<u32>,
) -> Result<(), String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;

    nbt_session::close(&path, index);
    Ok(())
}

/// 配置文件窗口左栏的宽度，单位像素
///
/// # 返回值
///
/// 上次拖出来的宽度；没拖过时是默认的 300
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_tree_width() -> f64 {
    crate::gui_config::get().config_window.tree_width
}

/// 记住左栏宽度，落 `gui_config.json`
///
/// - `width`: 宽度，单位像素；越界夹到 200 到 640
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_set_tree_width(width: f64) {
    let width = width.clamp(200.0, 640.0);
    let mut config = crate::gui_config::get();
    if (config.config_window.tree_width - width).abs() < f64::EPSILON {
        return;
    }
    config.config_window.tree_width = width;
    crate::gui_config::set(config);
}

/// 在文件管理器里打开这个文件所在的目录，并把该文件**选中**
///
/// 只打开目录的话，同一目录下几十个文件还得让用户自己找那一份；
/// 选中它一眼就能看到。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径，与 [`config_get_file`] 同一口径
///
/// # 返回值
///
/// 路径非法返回 `err.fileName`
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_open_file_dir(game: String, source: String, file: String) -> Result<(), String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    mml_sys::open_helper::open_file_with_explorer(&path);
    Ok(())
}

/// 删除一个配置文件，文件进回收站
///
/// 删之前先把这个文件的 NBT 会话关掉，否则句柄还指着已经没了的那一份。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径
///
/// # 返回值
///
/// 路径非法返回 `err.fileName`；删除失败返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_delete_file(game: String, source: String, file: String) -> Result<(), String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    // 普通文件的会话；区块会话跟着区域文件走，这里不去猜下标
    nbt_session::close(&path, None);
    path_helper::move_to_trash(&path).map_err(|err| err.to_string())?;
    Ok(())
}

/// 按前端记下的一批改动改写 NBT 文件
///
/// 改动按顺序套用，见 [`NbtEditDto`]：改值 / 新增 / 删除三种。写回在**打开时那个句柄**上做
/// （见 [`nbt_session::update_with`]），不重新打开也不重新读盘；压缩方式沿用原文件。
/// 改之前核对前端那份基准 SHA1 与文件有没有被别处动过。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径，与 [`config_open_nbt`] 同一口径
/// - `base_sha1`: 打开时那一份文件字节的 SHA1
/// - `edits`: 要套的改动，按发生顺序
///
/// # 返回值
///
/// 写回后文件字节的 SHA1；路径非法返回 `err.fileName`，没打开过、基准对不上或文件被别处
/// 改过返回 `err.fileChanged`，只读文件返回 `err.fileReadOnly`，路径对不上返回 `err.nbtPath`，
/// 值解析不了或数组超上限返回 `err.nbtValue`
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_set_nbt(
    game: String,
    source: String,
    file: String,
    index: Option<u32>,
    base_sha1: String,
    edits: Vec<NbtEditDto>,
) -> Result<String, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    if index.is_none() {
        check_size(&path)?;
    }

    nbt_session::update_with(&path, index, &base_sha1, |nbt| {
        apply_nbt_edits(&mut nbt.nbt, &edits)
    })
}

/// 依次套用一批改动
///
/// - `root`: 根标签
/// - `edits`: 要套的改动，按发生顺序
///
/// # 返回值
///
/// 全部套成功返回 `Ok(())`；路径对不上返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
fn apply_nbt_edits(root: &mut NbtType, edits: &[NbtEditDto]) -> Result<(), String> {
    for edit in edits {
        match edit.op {
            NbtOpDto::Set => set_nbt_at(root, &edit.path, &edit.value)?,
            NbtOpDto::Add => add_nbt_node(root, &edit.path, edit.kind, &edit.value)?,
            NbtOpDto::Rename => rename_nbt_node(root, &edit.path, &edit.value)?,
            NbtOpDto::Remove => remove_nbt_node(root, &edit.path)?,
        }
    }

    Ok(())
}

/// 按路径改一处值
///
/// 复合取成员、列表与数组取下标的元素；路径为空就是根标签自己。数组元素不是独立标签，
/// 只能在这里按下标落到具体的那一格上。
///
/// - `root`: 根标签
/// - `path`: 逐层的键名或下标
/// - `value`: 新值文本
///
/// # 返回值
///
/// 改成功返回 `Ok(())`；路径对不上返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
fn set_nbt_at(root: &mut NbtType, path: &[String], value: &str) -> Result<(), String> {
    let Some((last, parent_path)) = path.split_last() else {
        return set_nbt_scalar(root, value);
    };
    let bad_path = || String::from(err_keys::NBT_PATH);

    match nbt_at_mut(root, parent_path)? {
        NbtType::Compound(compound) => {
            let node = compound.data.get_mut(last).ok_or_else(bad_path)?;
            set_nbt_scalar(node, value)
        }
        NbtType::List(list) => {
            let node = list.get_item_mut(parse_index(last)?).ok_or_else(bad_path)?;
            set_nbt_scalar(node, value)
        }
        NbtType::ByteArray(v) => set_array_item(&mut v.data, last, value),
        NbtType::IntArray(v) => set_array_item(&mut v.data, last, value),
        NbtType::LongArray(v) => set_array_item(&mut v.data, last, value),
        _ => Err(bad_path()),
    }
}

/// 路径里的一段下标：十进制，别的写法一律算路径对不上
fn parse_index(segment: &str) -> Result<usize, String> {
    segment
        .parse::<usize>()
        .map_err(|_| String::from(err_keys::NBT_PATH))
}

/// 改数组里的一个元素，值按元素类型解析
///
/// - `data`: 数组本体
/// - `index`: 元素下标的十进制字符串
/// - `value`: 新值文本
///
/// # 返回值
///
/// 改成功返回 `Ok(())`；下标越界返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
fn set_array_item<T: std::str::FromStr>(data: &mut [T], index: &str, value: &str) -> Result<(), String> {
    let slot = data
        .get_mut(parse_index(index)?)
        .ok_or_else(|| String::from(err_keys::NBT_PATH))?;
    *slot = value
        .trim()
        .parse::<T>()
        .map_err(|_| String::from(err_keys::NBT_VALUE))?;

    Ok(())
}

/// 按路径走到要改的那个标签
///
/// 复合按键名、列表按下标；路径为空就是根标签自己。
///
/// - `root`: 根标签
/// - `path`: 逐层的键名或列表下标
///
/// # 返回值
///
/// 目标标签的可变引用；走到非复合 / 非列表上、索引越界或下标不是整数时返回 `err.nbtPath`
fn nbt_at_mut<'a>(root: &'a mut NbtType, path: &[String]) -> Result<&'a mut NbtType, String> {
    let mut cur = root;
    for segment in path {
        cur = match cur {
            NbtType::Compound(compound) => compound
                .data
                .get_mut(segment)
                .ok_or_else(|| String::from(err_keys::NBT_PATH))?,
            NbtType::List(list) => {
                let index = segment
                    .parse::<usize>()
                    .map_err(|_| String::from(err_keys::NBT_PATH))?;
                list.get_item_mut(index)
                    .ok_or_else(|| String::from(err_keys::NBT_PATH))?
            }
            _ => return Err(String::from(err_keys::NBT_PATH)),
        };
    }

    Ok(cur)
}

/// 按标签类型把文本解析成新值，只认标量
///
/// Byte 在本仓库里是 `u8`（0–255），与 `nbt_node` 的展示口径一致。
///
/// - `nbt`: 目标标签
/// - `value`: 新值文本
///
/// # 返回值
///
/// 解析成功返回 `Ok(())`；不是标量返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
fn set_nbt_scalar(nbt: &mut NbtType, value: &str) -> Result<(), String> {
    let text = value.trim();
    let bad = || String::from(err_keys::NBT_VALUE);

    match nbt {
        NbtType::Byte(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Short(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Int(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Long(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Float(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::Double(v) => v.data = text.parse().map_err(|_| bad())?,
        NbtType::String(v) => v.data = value.to_string(),
        _ => return Err(String::from(err_keys::NBT_PATH)),
    }

    Ok(())
}

/// 解析数组的值文本：逗号或空白分隔的一串数
///
/// 空串是空数组；元素个数超过 [`MAX_ARRAY_EDIT`] 直接拒掉，别让一次 IPC 塞进上万个数。
///
/// - `text`: 值文本，如 `1, 2, 3`
///
/// # 返回值
///
/// 解析出的元素；有解析不了的或超过上限时返回 `err.nbtValue`
fn parse_array<T: std::str::FromStr>(text: &str) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    for item in text.split([',', ' ', '\t', '\r', '\n']) {
        if item.is_empty() {
            continue;
        }
        if out.len() >= MAX_ARRAY_EDIT {
            return Err(String::from(err_keys::NBT_VALUE));
        }
        out.push(item.parse::<T>().map_err(|_| String::from(err_keys::NBT_VALUE))?);
    }

    Ok(out)
}

/// 按路径改一个复合成员的键名
///
/// 只换键名，标签本体（类型、值、子树）原样搬到新键下；列表与数组的元素按序号认，
/// 没有键名可改，根标签自己也没有键名，三种都拒掉。
///
/// - `root`: 根标签
/// - `path`: 目标标签的路径，至少一段
/// - `new_name`: 新键名
///
/// # 返回值
///
/// 改成功返回 `Ok(())`；父容器不是复合、旧键不存在、新键已被占用或路径为空返回 `err.nbtPath`
fn rename_nbt_node(root: &mut NbtType, path: &[String], new_name: &str) -> Result<(), String> {
    let (last, parent_path) = path
        .split_last()
        .ok_or_else(|| String::from(err_keys::NBT_PATH))?;

    match nbt_at_mut(root, parent_path)? {
        NbtType::Compound(compound) => {
            // 改成同名等于没改：别把它当成"新键已被占用"
            if last == new_name {
                return Ok(());
            }
            if compound.data.contains_key(new_name) {
                return Err(String::from(err_keys::NBT_PATH));
            }
            let node = compound
                .data
                .remove(last)
                .ok_or_else(|| String::from(err_keys::NBT_PATH))?;
            compound.data.insert(new_name.to_string(), node);
            Ok(())
        }
        _ => Err(String::from(err_keys::NBT_PATH)),
    }
}

/// 按路径新建一个标签并挂进父容器
///
/// 路径的最后一段在复合里当键名、在列表与数组里当插入下标（等于长度就是追加），前面几段
/// 照常走到父容器；空列表的元素类型跟随新元素，因为列表是同类型容器。数组的元素类型随
/// 数组自己，不看 `kind`。
///
/// - `root`: 根标签
/// - `path`: 新标签的路径，至少一段
/// - `kind`: 新标签的类型；缺省或为 [`NbtKindDto::End`] 都报错，父容器是数组时忽略
/// - `value`: 新标签的值文本
///
/// # 返回值
///
/// 挂上返回 `Ok(())`；父容器不是复合 / 列表 / 数组、键已存在、下标越界或元素类型对不上
/// 返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
fn add_nbt_node(
    root: &mut NbtType,
    path: &[String],
    kind: Option<NbtKindDto>,
    value: &str,
) -> Result<(), String> {
    let (last, parent_path) = path
        .split_last()
        .ok_or_else(|| String::from(err_keys::NBT_PATH))?;
    let kind = kind.ok_or_else(|| String::from(err_keys::NBT_PATH))?;
    let node = make_nbt_node(kind, value)?;

    match nbt_at_mut(root, parent_path)? {
        NbtType::Compound(compound) => {
            if compound.data.contains_key(last) {
                return Err(String::from(err_keys::NBT_PATH));
            }
            compound.data.insert(last.clone(), node);
            Ok(())
        }
        NbtType::List(list) => {
            let index = parse_index(last)?;
            // 空列表还没有元素类型：跟着第一个元素定下来
            if list.len() == 0 {
                list.set_num(node.get_num());
            }
            if list.insert_item(index, node) {
                Ok(())
            } else {
                Err(String::from(err_keys::NBT_PATH))
            }
        }
        NbtType::ByteArray(v) => add_array_item(&mut v.data, last, value),
        NbtType::IntArray(v) => add_array_item(&mut v.data, last, value),
        NbtType::LongArray(v) => add_array_item(&mut v.data, last, value),
        _ => Err(String::from(err_keys::NBT_PATH)),
    }
}

/// 往数组里插一个元素，值按元素类型解析
///
/// 元素个数按 [`MAX_ARRAY_EDIT`] 封顶，别让界面上的数组长到展开不动。
///
/// - `data`: 数组本体
/// - `index`: 插入下标的十进制字符串，等于长度就是追加
/// - `value`: 新元素文本
///
/// # 返回值
///
/// 插上返回 `Ok(())`；下标越界或已到上限返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
fn add_array_item<T: std::str::FromStr>(
    data: &mut Vec<T>,
    index: &str,
    value: &str,
) -> Result<(), String> {
    let index = parse_index(index)?;
    if index > data.len() || data.len() >= MAX_ARRAY_EDIT {
        return Err(String::from(err_keys::NBT_PATH));
    }
    let item = value
        .trim()
        .parse::<T>()
        .map_err(|_| String::from(err_keys::NBT_VALUE))?;
    data.insert(index, item);

    Ok(())
}

/// 按路径删掉一个标签
///
/// 路径的最后一段在复合里是键名、在列表与数组里是下标；根标签删不得，路径为空直接报错。
///
/// - `root`: 根标签
/// - `path`: 目标标签的路径，至少一段
///
/// # 返回值
///
/// 删掉返回 `Ok(())`；父容器不是复合 / 列表 / 数组、键不存在或下标越界返回 `err.nbtPath`
fn remove_nbt_node(root: &mut NbtType, path: &[String]) -> Result<(), String> {
    let (last, parent_path) = path
        .split_last()
        .ok_or_else(|| String::from(err_keys::NBT_PATH))?;

    match nbt_at_mut(root, parent_path)? {
        NbtType::Compound(compound) => {
            if compound.data.remove(last).is_some() {
                Ok(())
            } else {
                Err(String::from(err_keys::NBT_PATH))
            }
        }
        NbtType::List(list) => {
            let index = parse_index(last)?;
            if index >= list.len() {
                return Err(String::from(err_keys::NBT_PATH));
            }
            list.remove(index);
            Ok(())
        }
        NbtType::ByteArray(v) => remove_array_item(&mut v.data, last),
        NbtType::IntArray(v) => remove_array_item(&mut v.data, last),
        NbtType::LongArray(v) => remove_array_item(&mut v.data, last),
        _ => Err(String::from(err_keys::NBT_PATH)),
    }
}

/// 删掉数组里的一个元素
///
/// - `data`: 数组本体
/// - `index`: 元素下标的十进制字符串
///
/// # 返回值
///
/// 删掉返回 `Ok(())`；下标越界返回 `err.nbtPath`
fn remove_array_item<T>(data: &mut Vec<T>, index: &str) -> Result<(), String> {
    let index = parse_index(index)?;
    if index >= data.len() {
        return Err(String::from(err_keys::NBT_PATH));
    }
    data.remove(index);

    Ok(())
}

/// 按类型与值文本造一个新标签
///
/// 复合与列表造空的，值文本忽略，随后可以往里加成员 / 元素。
///
/// - `kind`: 标签类型，[`NbtKindDto::End`] 不可造
/// - `value`: 值文本
///
/// # 返回值
///
/// 新建的标签；类型为 End 返回 `err.nbtPath`，值解析不了返回 `err.nbtValue`
fn make_nbt_node(kind: NbtKindDto, value: &str) -> Result<NbtType, String> {
    let text = value.trim();
    let bad = || String::from(err_keys::NBT_VALUE);

    let node = match kind {
        NbtKindDto::End => return Err(String::from(err_keys::NBT_PATH)),
        NbtKindDto::Byte => nbt_types::byte(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Short => nbt_types::short(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Int => nbt_types::int(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Long => nbt_types::long(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Float => nbt_types::float(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::Double => nbt_types::double(text.parse().map_err(|_| bad())?).to_nbt(),
        NbtKindDto::String => nbt_types::string(value).to_nbt(),
        NbtKindDto::ByteArray => nbt_types::byte_array(parse_array(value)?).to_nbt(),
        NbtKindDto::IntArray => nbt_types::int_array(parse_array(value)?).to_nbt(),
        NbtKindDto::LongArray => nbt_types::long_array(parse_array(value)?).to_nbt(),
        NbtKindDto::List => nbt_types::list(NBT_END_ORDER).to_nbt(),
        NbtKindDto::Compound => nbt_types::compound().to_nbt(),
    };

    Ok(node)
}

/// 读取一个区域文件（`.mca`）的区块表
///
/// 只读 8 KiB 文件头，**不解析任何一个区块**：一个区域文件最多 1024 个区块，
/// 全解析一遍只为列个表不值当。要看某个区块的字节走 [`config_get_bytes_slice`]。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径
///
/// # 返回值
///
/// 有数据的区块，按表项下标升序；空表项不返回。路径非法返回 `err.fileName`，
/// 扩展名不是 `.mca` 返回 `err.notMcaFile`，文件头读不满返回内核错误串
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_mca(
    game: String,
    source: String,
    file: String,
) -> Result<Vec<McaChunkDto>, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    if get_file_type(&path) != FileItemType::Mca {
        return Err(String::from(err_keys::NOT_MCA_FILE));
    }

    let mut stream = path_helper::open_read(&path).map_err(|err| err.to_string())?;
    // 文件头固定 8 KiB：更短的文件连位置表都不完整，按"空区域文件"报
    let len = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    if len < 8192 {
        return Err(String::from(err_keys::MCA_FILE_EMPTY));
    }
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

/// 按方块坐标在存档里找实体或方块实体；不给坐标就整份存档扫一遍
///
/// 文件名与表项下标都由坐标算出来，不需要用户自己挑区域文件：
/// 方块 `(x, z)` → 区块 `(x >> 4, z >> 4)` → 区域 `(cx >> 5, cz >> 5)` 与表项 `(cx & 31) + (cz & 31) * 32`。
/// 实体在 `entities/`、方块实体在 `region/`，两边的区域文件各自一套。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `kind`: 找哪一类，取值 `Entities` / `block_entities`
/// - `x`: 方块坐标 X；与 `z` 都不给表示整份存档扫
/// - `z`: 方块坐标 Z
/// - `name`: id 片段；空串表示只定位区块
///
/// # 返回值
///
/// 命中的区域文件、表项下标与节点路径；没命中时 `file` 与 `path` 都为空。
/// 路径非法返回 `err.fileName`，这一类取错返回 `err.mcaChunkEmpty`
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_find_chunk(
    game: String,
    source: String,
    kind: String,
    x: Option<i32>,
    z: Option<i32>,
    name: String,
) -> Result<ChunkFindDto, String> {
    let game = instance_snapshot(&game)?;
    if kind != "Entities" && kind != "block_entities" {
        return Err(String::from(err_keys::MCA_CHUNK_EMPTY));
    }

    // 实体与方块实体分开放：1.17 起实体在 entities/，方块实体还在 region/
    let dir = if kind == "Entities" { "entities" } else { "region" };
    let base = if source.is_empty() {
        game.get_game_path()
    } else {
        resolve_under(&game.get_saves_path(), &source)?
    };

    let (files, index) = match (x, z) {
        (Some(x), Some(z)) => {
            let chunk_x = x >> 4;
            let chunk_z = z >> 4;
            let region_x = chunk_x >> 5;
            let region_z = chunk_z >> 5;
            let name = format!("{dir}/r.{region_x}.{region_z}.mca");
            let index = ((chunk_x & 31) + (chunk_z & 31) * 32) as u32;
            (vec![name], Some(index))
        }
        // 没给坐标：这一类下面的区域文件全扫
        _ => {
            let mut list: Vec<String> = Vec::new();
            for file in path_helper::get_all_files(&base.join(dir)) {
                if get_file_type(&file) != FileItemType::Mca {
                    continue;
                }
                let Ok(rel) = file.strip_prefix(&base) else {
                    continue;
                };
                list.push(rel.to_string_lossy().replace('\\', "/"));
            }
            list.sort();
            (list, None)
        }
    };

    for rel in files {
        let path = resolve_under(&base, &rel)?;
        if !path.is_file() {
            continue;
        }
        if let Some((index, hit)) = find_in_file(&path, &kind, &name, index)? {
            return Ok(ChunkFindDto {
                file: rel,
                index,
                path: hit.unwrap_or_default(),
            });
        }
    }

    Ok(ChunkFindDto {
        file: String::new(),
        index: 0,
        path: Vec::new(),
    })
}

/// 在一个区域文件里找：给了表项下标就只看那一块，否则扫全部有数据的区块
///
/// 扫描直接读每块的字节、不建会话 —— 扫一遍可能在会话表里堆几百份用不上的句柄。
///
/// - `path`: 区域文件绝对路径
/// - `kind`: `Entities` 或 `block_entities`
/// - `name`: id 片段；空串表示只定位区块
/// - `only`: 只看这一表项；`None` 表示扫全部
///
/// # 返回值
///
/// 命中的表项下标与节点路径（`None` 表示只定位到区块、没挑条目）；没命中返回 `None`
fn find_in_file(
    path: &Path,
    kind: &str,
    name: &str,
    only: Option<u32>,
) -> Result<Option<(u32, Option<Vec<String>>)>, String> {
    let data = std::fs::read(path).map_err(|err| err.to_string())?;
    if data.len() < 8192 {
        return Err(String::from(err_keys::MCA_FILE_EMPTY));
    }
    let mut cursor = Cursor::new(data.as_slice());
    let head = ChunkData::read_head(&mut cursor).map_err(|err| err.to_string())?;

    for item in head.iter().filter(|item| item.count > 0) {
        if let Some(only) = only {
            if item.index != only {
                continue;
            }
        }
        let Ok(payload) = nbt_session::chunk_bytes(&data, item.index) else {
            continue;
        };
        let mut cursor = Cursor::new(payload);
        let Ok(nbt) = NbtFile::read(&mut cursor) else {
            continue;
        };
        let hit = find_in_chunk(&nbt.nbt, kind, name);
        // 只定位区块时不挑条目，命中即返回
        if hit.is_some() || name.is_empty() {
            return Ok(Some((item.index, hit)));
        }
    }

    Ok(None)
}

/// 在区块的某一类列表里按 id 找第一个包含 `name` 的条目
///
/// - `nbt`: 区块根标签
/// - `kind`: `Entities` 或 `block_entities`
/// - `name`: id 片段；空串取第一个条目
///
/// # 返回值
///
/// 命中节点的路径，如 `["block_entities", "7"]`；没有这一列或没命中返回 `None`
fn find_in_chunk(nbt: &NbtType, kind: &str, name: &str) -> Option<Vec<String>> {
    let list = nbt.as_compound()?.data.get(kind)?.as_list()?;
    for (index, item) in list.iter().enumerate() {
        let Some(compound) = item.as_compound() else {
            continue;
        };
        let hit = match compound.data.get("id").and_then(|value| value.as_string()) {
            Some(id) => name.is_empty() || id.data.contains(name),
            // 没有 id 的条目只在"只要区块"时算命中
            None => name.is_empty(),
        };
        if hit {
            return Some(vec![kind.to_string(), index.to_string()]);
        }
    }
    None
}

/// 读取任意文件的一段字节，给二进制视图翻页用
///
/// 不过 [`check_binary`]：图片、压缩包就是靠这个看的，NBT 与区域文件也能拿它看原始字节。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径
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
    source: String,
    file: String,
    offset: u64,
    length: u32,
) -> Result<BytesSliceDto, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;

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
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径，取自 [`config_get_files`]
///
/// # 返回值
///
/// `data:<mime>;base64,<...>`；路径非法返回 `err.fileName`，不是图片返回
/// `err.notImageFile`，体积超过上限返回 `err.fileTooLarge`
#[gui_macros::ipc_group("config")]
#[tauri::command]
pub fn config_get_image(game: String, source: String, file: String) -> Result<String, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use mml_base::hash_helper;
    use uuid::Uuid;

    use crate::dtos::{FileItemType, NbtKindDto};

    use super::{
        MAX_ARRAY_EDIT, MAX_SLICE_SIZE, apply_nbt_edits, check_binary, get_file_type, nbt_at_mut,
        nbt_node, read_slice, resolve_source_in, resolve_under,
    };
    use crate::dtos::{NbtEditDto, NbtOpDto};
    use mml_nbt::nbt_types::{compound, int, list, string};
    use mml_nbt::{NBT_END_ORDER, NbtType};

    /// 造一个复合标签
    fn compound_of(entries: Vec<(&str, NbtType)>) -> NbtType {
        let mut obj = compound();
        for (key, value) in entries {
            obj.data.insert(key.to_string(), value);
        }
        obj.to_nbt()
    }

    /// 造一个列表标签；元素类型序号取第一个元素的，空列表用 END
    fn list_of(items: Vec<NbtType>) -> NbtType {
        let num = items.first().map(|item| item.get_num()).unwrap_or(NBT_END_ORDER);
        let mut obj = list(num);
        for item in items {
            obj.add_item(item);
        }
        obj.to_nbt()
    }

    /// 造一份带复合与列表的样本
    fn sample_nbt() -> NbtType {
        compound_of(vec![
            (
                "Data",
                compound_of(vec![
                    ("GameType", int(1).to_nbt()),
                    ("Name", string("x").to_nbt()),
                ]),
            ),
            ("Items", list_of(vec![int(3).to_nbt(), int(4).to_nbt()])),
        ])
    }

    /// 造一条"改值"的改动
    fn set_at(path: &[&str], value: &str) -> NbtEditDto {
        NbtEditDto {
            op: NbtOpDto::Set,
            path: path.iter().map(|item| item.to_string()).collect(),
            kind: None,
            value: value.to_string(),
        }
    }

    /// 造一条"新增"的改动
    fn add_at(path: &[&str], kind: NbtKindDto, value: &str) -> NbtEditDto {
        NbtEditDto {
            op: NbtOpDto::Add,
            path: path.iter().map(|item| item.to_string()).collect(),
            kind: Some(kind),
            value: value.to_string(),
        }
    }

    /// 造一条"改名"的改动
    fn rename_at(path: &[&str], new_name: &str) -> NbtEditDto {
        NbtEditDto {
            op: NbtOpDto::Rename,
            path: path.iter().map(|item| item.to_string()).collect(),
            kind: None,
            value: new_name.to_string(),
        }
    }

    /// 造一条"删除"的改动
    fn remove_at(path: &[&str]) -> NbtEditDto {
        NbtEditDto {
            op: NbtOpDto::Remove,
            path: path.iter().map(|item| item.to_string()).collect(),
            kind: None,
            value: String::new(),
        }
    }

    /// 按路径取一个标量标签
    fn at<'a>(root: &'a mut NbtType, path: &[&str]) -> &'a mut NbtType {
        let path: Vec<String> = path.iter().map(|item| item.to_string()).collect();
        nbt_at_mut(root, &path).unwrap()
    }

    /// 标量按路径改：复合成员、列表元素都能走到
    #[test]
    fn apply_nbt_edits_changes_scalars() {
        let mut root = sample_nbt();
        apply_nbt_edits(
            &mut root,
            &[
                set_at(&["Data", "GameType"], "2"),
                set_at(&["Items", "1"], "9"),
                set_at(&["Data", "Name"], " 前后有空格也照收 "),
            ],
        )
        .unwrap();

        assert_eq!(at(&mut root, &["Data", "GameType"]).as_int().unwrap().data, 2);
        assert_eq!(at(&mut root, &["Items", "1"]).as_int().unwrap().data, 9);

        // 字符串只 trim 掉解析用的空白，存的是原样（前后空格保留）
        assert_eq!(
            at(&mut root, &["Data", "Name"]).as_string().unwrap().data,
            " 前后有空格也照收 "
        );
    }

    /// 路径对不上、值解析不了都要拒掉：宁可报错，也不能写坏文件
    #[test]
    fn apply_nbt_edits_rejects_bad_edits() {
        let bad = [
            // 键不存在
            set_at(&["Nope"], "1"),
            // 值不是整数
            set_at(&["Data", "GameType"], "abc"),
            // 指到了复合上（改值只认标量与数组）
            set_at(&["Data"], "1"),
            // 列表下标越界
            set_at(&["Items", "9"], "1"),
            // 列表下标不是整数
            set_at(&["Items", "x"], "1"),
        ];

        for edit in bad {
            assert!(
                apply_nbt_edits(&mut sample_nbt(), &[edit]).is_err(),
                "应当拒绝"
            );
        }
    }

    /// 数组元素按下标改：三种元素类型都认，越界与解析不了都拒掉
    #[test]
    fn apply_nbt_edits_edits_array_items() {
        use mml_nbt::nbt_types::{byte_array, int_array, long_array};

        let mut root = compound_of(vec![
            ("Bytes", byte_array(vec![1, 2]).to_nbt()),
            ("Ints", int_array(vec![7]).to_nbt()),
            ("Longs", long_array(vec![9]).to_nbt()),
        ]);

        apply_nbt_edits(
            &mut root,
            &[
                set_at(&["Bytes", "1"], "255"),
                set_at(&["Ints", "0"], "-1"),
                set_at(&["Longs", "0"], "9000000000"),
            ],
        )
        .unwrap();

        match at(&mut root, &["Bytes"]) {
            NbtType::ByteArray(v) => assert_eq!(v.data, vec![1, 255]),
            other => panic!("类型变了：{}", other),
        }
        match at(&mut root, &["Ints"]) {
            NbtType::IntArray(v) => assert_eq!(v.data, vec![-1]),
            other => panic!("类型变了：{}", other),
        }
        match at(&mut root, &["Longs"]) {
            NbtType::LongArray(v) => assert_eq!(v.data, vec![9_000_000_000]),
            other => panic!("类型变了：{}", other),
        }

        // 越界、解析不了、以及想整段替换数组（元素只能一个个改）都要拒
        for edit in [
            set_at(&["Bytes", "9"], "1"),
            set_at(&["Bytes", "0"], "256"),
            set_at(&["Ints", "0"], "x"),
            set_at(&["Bytes", "x"], "1"),
            set_at(&["Bytes"], "1, 2"),
        ] {
            assert!(
                apply_nbt_edits(&mut sample_nbt(), &[edit]).is_err(),
                "应当拒绝"
            );
        }
    }

    /// 数组与列表一样能增删元素：下标等于长度是追加，删一个后面的下标往前挪
    #[test]
    fn apply_nbt_edits_adds_and_removes_array_items() {
        use mml_nbt::nbt_types::int_array;

        let mut root = compound_of(vec![("Ints", int_array(vec![1, 2]).to_nbt())]);

        apply_nbt_edits(
            &mut root,
            &[
                add_at(&["Ints", "2"], NbtKindDto::Int, "3"),
                add_at(&["Ints", "0"], NbtKindDto::Int, "0"),
                remove_at(&["Ints", "1"]),
            ],
        )
        .unwrap();

        match at(&mut root, &["Ints"]) {
            NbtType::IntArray(v) => assert_eq!(v.data, vec![0, 2, 3]),
            other => panic!("类型变了：{}", other),
        }

        // 下标越界、值解析不了、加到上限之外都要拒
        let full: Vec<i32> = (0..MAX_ARRAY_EDIT as i32).collect();
        let mut full_root = compound_of(vec![("Ints", int_array(full).to_nbt())]);
        assert!(
            apply_nbt_edits(&mut full_root, &[add_at(&["Ints", "0"], NbtKindDto::Int, "1")]).is_err(),
            "到上限后不该再加"
        );
        for edit in [
            add_at(&["Ints", "9"], NbtKindDto::Int, "1"),
            add_at(&["Ints", "2"], NbtKindDto::Int, "x"),
            remove_at(&["Ints", "9"]),
        ] {
            let mut root = compound_of(vec![("Ints", int_array(vec![1, 2]).to_nbt())]);
            assert!(apply_nbt_edits(&mut root, &[edit]).is_err(), "应当拒绝");
        }
    }

    /// 增删：复合加成员、列表加元素、两处都能删，根标签删不得
    #[test]
    fn apply_nbt_edits_adds_and_removes() {
        let mut root = sample_nbt();
        apply_nbt_edits(
            &mut root,
            &[
                // 复合加成员
                add_at(&["Data", "New"], NbtKindDto::Int, "7"),
                // 列表追加：下标等于长度
                add_at(&["Items", "2"], NbtKindDto::Int, "5"),
                // 复合删成员、列表删元素
                remove_at(&["Data", "Name"]),
                remove_at(&["Items", "0"]),
            ],
        )
        .unwrap();

        assert_eq!(at(&mut root, &["Data", "New"]).as_int().unwrap().data, 7);
        match at(&mut root, &["Items"]) {
            NbtType::List(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v.get_item(0).unwrap().as_int().unwrap().data, 4);
                assert_eq!(v.get_item(1).unwrap().as_int().unwrap().data, 5);
            }
            other => panic!("类型变了：{}", other),
        }
        assert!(
            nbt_at_mut(&mut root, &["Data".to_string(), "Name".to_string()]).is_err(),
            "成员应当已删掉"
        );

        // 键已存在、下标越界、类型对不上、父容器不是容器、删根标签都要拒
        for edit in [
            add_at(&["Data", "GameType"], NbtKindDto::Int, "1"),
            add_at(&["Items", "9"], NbtKindDto::Int, "1"),
            add_at(&["Items", "0"], NbtKindDto::String, "x"),
            add_at(&["Data", "GameType", "Deep"], NbtKindDto::Int, "1"),
            add_at(&["Data", "Bad"], NbtKindDto::End, "1"),
            remove_at(&["Data", "Nope"]),
            remove_at(&["Items", "9"]),
            remove_at(&[]),
        ] {
            assert!(
                apply_nbt_edits(&mut sample_nbt(), &[edit]).is_err(),
                "应当拒绝"
            );
        }
    }

    /// 改名：只换键名，标签本体连同子树原样搬过去
    #[test]
    fn apply_nbt_edits_renames_members() {
        let mut root = sample_nbt();
        apply_nbt_edits(
            &mut root,
            &[
                // 标量改名：类型与值都不动
                rename_at(&["Data", "Name"], "Title"),
                // 复合改名：整棵子树跟着走
                rename_at(&["Data"], "Info"),
                // 改成同名：不动结构，也不算错
                rename_at(&["Info", "GameType"], "GameType"),
            ],
        )
        .unwrap();

        assert_eq!(at(&mut root, &["Info", "Title"]).as_string().unwrap().data, "x");
        assert_eq!(at(&mut root, &["Info", "GameType"]).as_int().unwrap().data, 1);
        assert!(
            nbt_at_mut(&mut root, &["Data".to_string()]).is_err(),
            "旧键应当已不在"
        );

        // 新键已被占用、旧键不存在、父容器不是复合（列表元素）、改根标签都要拒
        for edit in [
            rename_at(&["Data", "GameType"], "Name"),
            rename_at(&["Data", "Nope"], "X"),
            rename_at(&["Items", "0"], "X"),
            rename_at(&[], "X"),
        ] {
            assert!(
                apply_nbt_edits(&mut sample_nbt(), &[edit]).is_err(),
                "应当拒绝"
            );
        }
    }

    /// 空列表由第一个元素定元素类型：加进去之后读得回来
    #[test]
    fn apply_nbt_edits_fills_empty_list() {
        let mut root = compound_of(vec![("Empty", {
            let mut obj = list(NBT_END_ORDER);
            obj.to_nbt()
        })]);

        apply_nbt_edits(&mut root, &[add_at(&["Empty", "0"], NbtKindDto::String, "hi")]).unwrap();

        match at(&mut root, &["Empty"]) {
            NbtType::List(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v.get_item(0).unwrap().as_string().unwrap().data, "hi");
            }
            other => panic!("类型变了：{}", other),
        }
    }

    /// 改过标量后写回再读出来：值真的落到字节里，压缩方式沿用原文件
    ///
    /// 走的是 [`config_set_nbt`] 的那条链路：读 → 改 → 写；`NbtFile` 的读写往返
    /// 只在 mml-nbt 里各自做过，这里钉住"改过的文件还能读回来"
    #[test]
    fn nbt_edits_survive_round_trip() {
        use std::io::Cursor;

        use mml_nbt::nbt_file::{CompressType, NbtFile};

        let file = NbtFile::new(sample_nbt(), CompressType::GZip);
        let mut bytes = Vec::new();
        file.write(&mut bytes).unwrap();

        let mut read = NbtFile::read(&mut Cursor::new(&bytes)).unwrap();
        apply_nbt_edits(
            &mut read.nbt,
            &[
                set_at(&["Data", "GameType"], "2"),
                add_at(&["Data", "Added"], NbtKindDto::String, "加进来的"),
            ],
        )
        .unwrap();
        let mut out = Vec::new();
        read.write(&mut out).unwrap();

        // 写回的是另一份字节（压缩帧与哈希都会变），但必须读得回来
        assert_ne!(out, bytes);
        let mut back = NbtFile::read(&mut Cursor::new(&out)).unwrap();
        assert_eq!(back.compress, CompressType::GZip);
        assert_eq!(at(&mut back.nbt, &["Data", "GameType"]).as_int().unwrap().data, 2);
        assert_eq!(
            at(&mut back.nbt, &["Data", "Added"]).as_string().unwrap().data,
            "加进来的"
        );
    }

    /// 来源决定基准目录：空串相对游戏目录，存档名相对该存档目录，两处都挡穿越
    #[test]
    fn resolve_source_switches_base() {
        let game_path = Path::new("D:/mml/inst/.minecraft");
        let saves_path = Path::new("D:/mml/inst/.minecraft/saves");

        // 实例来源：相对游戏目录
        assert_eq!(
            resolve_source_in(game_path, saves_path, "", "options.txt").unwrap(),
            game_path.join("options.txt")
        );
        // 存档来源：相对该存档目录，**不再套** `saves/<目录>/` 前缀
        assert_eq!(
            resolve_source_in(game_path, saves_path, "world", "level.dat").unwrap(),
            saves_path.join("world").join("level.dat")
        );
        assert!(resolve_source_in(game_path, saves_path, "world", "../options.txt").is_err());
        assert!(resolve_source_in(game_path, saves_path, "../world", "level.dat").is_err());
    }

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
        // 短数组展开成元素行，元素类型是数组的元素类型、名字是下标
        let items = &node.children[0].items;
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].name, "1");
        assert_eq!(items[1].kind, NbtKindDto::Byte);
        assert_eq!(items[1].value, "2");

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
