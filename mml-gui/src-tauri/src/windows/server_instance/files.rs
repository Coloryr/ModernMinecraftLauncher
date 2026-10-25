//! 勾选解析与写盘
//!
//! 前端给的是"勾了哪些模组 / 哪些资源包 / 哪些配置目录"，这里负责展开成文件、算 SHA1，
//! 再按格式打成压缩包或原样复制。游戏目录由调用方传入，测试可以不启动内核单例。

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use mml_base::hash_helper::{HashType, gen_hash_from_file};

use crate::{
    dtos::server_instance_dto::ServerInstancePickDto,
    err_keys,
    windows::{config::resolve_under, export::archive},
};

/// 一个要同步的模组
pub(super) struct ModPick {
    /// 相对游戏目录的路径
    pub(super) path: PathBuf,
    /// 手动填的下载地址，空串表示用实例记录的
    pub(super) url: String,
}

/// 按勾选展开模组文件，路径相对游戏目录
///
/// 勾选的可能是文件，也可能是整个目录；取消勾选的条目按"最近的祖先说话"覆盖。
/// 手填的下载信息只跟着"单个文件"那条走，目录展开出来的文件不带。
///
/// - `base`: 游戏目录
/// - `picks`: 模组页的勾选
///
/// # 返回值
///
/// 见 [`ModPick`]，按路径排序；越界或目录读不了时返回错误文案
pub(super) fn mods(base: &Path, picks: &[ServerInstancePickDto]) -> Result<Vec<ModPick>, String> {
    let mut decisions: HashSet<PathBuf> = HashSet::new();
    for pick in picks.iter().filter(|pick| pick.exclude) {
        if let Ok(rel) = relative(base, &pick.path) {
            decisions.insert(rel);
        }
    }

    // 同一个文件可能既在目录里被展开、又被单独勾了一次，后者带着手填信息，覆盖前面的
    let mut selected: BTreeMap<PathBuf, ModPick> = BTreeMap::new();
    for pick in picks.iter().filter(|pick| !pick.exclude) {
        let Ok(path) = resolve_under(base, &pick.path) else {
            continue;
        };
        let single = path.is_file();
        for rel in expand(base, &path)? {
            if is_excluded(&decisions, &rel) {
                continue;
            }
            let entry = ModPick {
                path: rel.clone(),
                url: if single {
                    pick.url.trim().to_string()
                } else {
                    String::new()
                },
            };
            selected.insert(rel, entry);
        }
    }

    Ok(selected.into_values().collect())
}

/// 列一个目录下的全部文件，路径相对游戏目录
///
/// - `base`: 游戏目录
/// - `dir`: 目录名，相对游戏目录
///
/// # 返回值
///
/// 相对路径列表，按路径排序；目录不存在或读不了时返回错误文案
pub(super) fn dir_files(base: &Path, dir: &str) -> Result<Vec<PathBuf>, String> {
    let path = relative(base, dir)?;
    let root = base.join(&path);
    if !root.is_dir() {
        return Err(err_keys::SERVER_INSTANCE_DIR.to_string());
    }
    let mut list = Vec::new();
    for file in archive::files_under(&root)? {
        let rel = file
            .strip_prefix(base)
            .map_err(|_| err_keys::FILE_NAME.to_string())?;
        list.push(rel.to_path_buf());
    }
    list.sort();
    Ok(list)
}

/// 校验一个单个文件并转成相对路径
///
/// - `base`: 游戏目录
/// - `name`: 前端给的相对路径
///
/// # 返回值
///
/// 相对路径；越界、不存在或不是文件时返回错误文案
pub(super) fn file(base: &Path, name: &str) -> Result<PathBuf, String> {
    let rel = relative(base, name)?;
    if !base.join(&rel).is_file() {
        return Err(err_keys::SERVER_INSTANCE_DIR.to_string());
    }
    Ok(rel)
}

/// 把一个路径展开成文件；目录递归展开
///
/// - `base`: 游戏目录
/// - `path`: 绝对路径
///
/// # 返回值
///
/// 相对游戏目录的文件路径；条目不存在时为空表
fn expand(base: &Path, path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![
            path.strip_prefix(base)
                .map_err(|_| err_keys::FILE_NAME.to_string())?
                .to_path_buf(),
        ]);
    }
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let mut list = Vec::new();
    for file in archive::files_under(path)? {
        let rel = file
            .strip_prefix(base)
            .map_err(|_| err_keys::FILE_NAME.to_string())?;
        list.push(rel.to_path_buf());
    }
    Ok(list)
}

/// 路径本身或最近的上级被取消勾选时算排除
///
/// - `decisions`: 取消勾选的路径集合
/// - `path`: 相对游戏目录的路径
///
/// # 返回值
///
/// 是否排除
fn is_excluded(decisions: &HashSet<PathBuf>, path: &Path) -> bool {
    path.ancestors()
        .any(|ancestor| decisions.contains(ancestor))
}

/// 校验并转成相对游戏目录的路径
///
/// - `base`: 游戏目录
/// - `name`: 前端给的相对路径
///
/// # 返回值
///
/// 相对路径；越界或非法时返回错误文案
fn relative(base: &Path, name: &str) -> Result<PathBuf, String> {
    let path = resolve_under(base, name)?;
    path.strip_prefix(base)
        .map(Path::to_path_buf)
        .map_err(|_| err_keys::FILE_NAME.to_string())
}

/// 算文件的 SHA1，两份清单校验的都是它
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 小写十六进制校验串；读不到文件时返回错误文案
pub(super) fn sha1(path: &Path) -> Result<String, String> {
    gen_hash_from_file(HashType::Sha1, path).map_err(|e| e.to_string())
}

/// 把一个目录打成压缩包，条目相对该目录
///
/// - `base`: 游戏目录
/// - `dir`: 目录名，相对游戏目录
/// - `files`: 该目录下的文件，路径相对 `base`
/// - `file`: 输出压缩包
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`；路径对不上或压缩失败返回错误文案
pub(super) fn zip_dir(
    base: &Path,
    dir: &str,
    files: &[PathBuf],
    file: &Path,
) -> Result<(), String> {
    let mut entries = Vec::with_capacity(files.len());
    for rel in files {
        let name = rel
            .strip_prefix(dir)
            .map_err(|_| String::from(err_keys::FILE_NAME))?;
        entries.push((wire_path(name), base.join(rel)));
    }
    // 生成是一次性动作，没有取消入口
    let cancel = AtomicBool::new(false);
    archive::write_zip(file, &entries, &[], &None, &cancel)
}

/// 把文件按相对路径复制到输出文件夹，父目录不存在就建
///
/// - `base`: 游戏目录
/// - `out`: 输出文件夹
/// - `rel`: 相对游戏目录的路径
///
/// # 返回值
///
/// 复制成功返回 `Ok(())`；建目录或复制失败返回错误文案
pub(super) fn copy_into(base: &Path, out: &Path, rel: &Path) -> Result<(), String> {
    let target = out.join(rel);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(base.join(rel), &target).map_err(|e| e.to_string())?;
    Ok(())
}

/// 相对路径转清单里的正斜杠写法
///
/// - `rel`: 相对路径
///
/// # 返回值
///
/// 形如 `mods/foo.jar`
pub(super) fn wire_path(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

/// 把相对路径转成能拼进 URL 的路径
///
/// 只保留 RFC 3986 的非保留字符与 `/`，其余按 UTF-8 逐字节转义：
/// 配置目录里带空格与中文的路径很常见，原样拼进 URL 客户端下不到。
///
/// - `path`: 已经转成正斜杠的相对路径
///
/// # 返回值
///
/// 转义后的路径，形如 `config/%E4%B8%AD%E6%96%87.toml`
pub(super) fn encode_url_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(*byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
