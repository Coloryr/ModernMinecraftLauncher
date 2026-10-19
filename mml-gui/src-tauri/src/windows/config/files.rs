//! 文件页：清单、文本、二进制分段与图片
//!
//! 文本按行级差异保存，保存前核对磁盘上还是打开时那一份。

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use mml_base::hash_helper::{self, HashType};
use mml_sys::path_helper;
use mml_sys::text_diff::{self, LineHunk};

use crate::dtos::{BytesSliceDto, ConfigHunkDto, ConfigTextDto, FileItemDto, FileItemType};
use crate::err_keys;

use crate::windows::{instance_snapshot, nbt_session};

use super::source::{
    check_binary, check_size, get_file_type, push_file, resolve_source, resolve_under,
};


/// 二进制视图一次最多读多少字节
///
/// 一页给 64 KiB 足够铺满屏幕；再大只是让 IPC 白搬数据。
pub(super) const MAX_SLICE_SIZE: u32 = 64 * 1024;

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
pub(super) fn read_slice(path: &Path, offset: u64, length: u32) -> Result<BytesSliceDto, String> {
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
pub(super) fn text_sha1(text: &str) -> String {
    hash_helper::gen_hash(HashType::Sha1, text.as_bytes())
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
pub(super) fn image_mime(path: &Path) -> &'static str {
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
    stream
        .read_to_end(&mut buf)
        .map_err(|err| err.to_string())?;

    Ok(format!(
        "data:{};base64,{}",
        image_mime(&path),
        hash_helper::gen_base64_bytes(&buf)
    ))
}
