//! 0.1.5 及更早：旧 JSON 下载索引导入 mml-link 的索引库
//!
//! 0.1.5 之前索引是 `mod_download.json` / `resourcepack_download.json`，
//! 内容为 `{ 项目 ID: [记录...] }`，每条记录带 `hash`。
//! 导入后把旧文件改名成 `.bak` 留底，下次启动就不会再导一遍。

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use mml_link::{IndexKind, Store};
use mml_names::{i18_items::info_type::InfoType, names};
use serde_json::Value;

use super::Report;

/// 是否还有待升级的旧索引
///
/// - `root`: 启动器运行目录
///
/// # 返回值
///
/// 任一份旧文件存在返回 `true`
pub fn pending(root: &Path) -> bool {
    [names::MOD_DOWNLOAD_FILE, names::RESOURCEPACK_DOWNLOAD_FILE]
        .into_iter()
        .any(|file| root.join(file).is_file())
}

/// 待导入的一条记录
struct Entry {
    /// 资源类型
    kind: IndexKind,
    /// 项目 ID
    pid: String,
    /// 文件 SHA1，小写
    hash: String,
    /// 记录字节
    payload: Vec<u8>,
}

/// 旧文件解析出来的内容
struct LegacyFile {
    /// 文件路径
    path: std::path::PathBuf,
    /// 文件名，日志与留底用
    name: &'static str,
    /// 解析出的记录
    entries: Vec<Entry>,
}

/// 把旧的 JSON 索引导入索引库
///
/// 先解析出全部记录拿到总数，再逐条写入并回调进度；解析失败的文件原样留下
///
/// - `root`: 启动器运行目录，旧 JSON 就在它下面
/// - `store`: mml-link 的索引库
/// - `on_progress`: 进度回调，参数是已写入条数与总条数；返回 `false` 表示请求停止
///
/// # 返回值
///
/// 见 [`Report`]
pub fn run(
    root: &Path,
    store: &Store,
    on_progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Report {
    let mut report = Report::default();
    let mut files: Vec<LegacyFile> = Vec::new();

    for (kind, name) in [
        (IndexKind::Mod, names::MOD_DOWNLOAD_FILE),
        (IndexKind::ResourcePack, names::RESOURCEPACK_DOWNLOAD_FILE),
    ] {
        let path = root.join(name);
        if !path.is_file() {
            continue;
        }
        match parse_file(&path, kind) {
            Ok((entries, skipped)) => {
                report.failed += skipped;
                files.push(LegacyFile { path, name, entries });
            }
            Err(err) => {
                mml_log::info_type(InfoType::DownloadIndexImportFailed {
                    file: name.to_string(),
                    error: err,
                });
                report.failed += 1;
            }
        }
    }

    let total: usize = files.iter().map(|file| file.entries.len()).sum();
    // 上次被叫停留下的记录不再重写：读一遍已有的键，跳过即可继续
    let (mod_keys, pack_keys) = (
        store.keys(IndexKind::Mod).unwrap_or_default(),
        store.keys(IndexKind::ResourcePack).unwrap_or_default(),
    );
    let (mod_keys, pack_keys): (HashSet<(String, String)>, HashSet<(String, String)>) = (
        mod_keys.into_iter().collect(),
        pack_keys.into_iter().collect(),
    );
    let mut stopped = !on_progress(0, total);

    let mut done = 0usize;
    let mut imported: Vec<&'static str> = Vec::new();
    'files: for file in &files {
        for entry in &file.entries {
            if stopped {
                break 'files;
            }
            let exists = match entry.kind {
                IndexKind::Mod => mod_keys.contains(&(entry.pid.clone(), entry.hash.clone())),
                IndexKind::ResourcePack => pack_keys.contains(&(entry.pid.clone(), entry.hash.clone())),
            };
            if !exists
                && let Err(err) = store.put(entry.kind, &entry.pid, &entry.hash, &entry.payload)
            {
                mml_log::info_type(InfoType::DownloadIndexImportFailed {
                    file: file.name.to_string(),
                    error: err,
                });
                report.failed += 1;
                continue;
            }
            match entry.kind {
                IndexKind::Mod => report.mods += 1,
                IndexKind::ResourcePack => report.resourcepacks += 1,
            }
            done += 1;
            stopped = !on_progress(done, total);
        }
        imported.push(file.name);
    }
    report.cancelled = stopped;

    // 中途停下也要把已写入的落盘：下次启动接着导，已导入的记录按主键覆盖
    if let Err(err) = store.flush() {
        mml_log::info_type(InfoType::DownloadIndexImportFailed {
            file: imported.join(", "),
            error: err,
        });
        report.failed += 1;
        return report;
    }

    // 只有全部导完才动旧文件：中途停下时留着，下次启动继续
    if !report.cancelled {
        for file in &files {
            backup(&file.path);
            report.migrated = true;
        }
    }

    if report.migrated {
        mml_log::info_type(InfoType::DownloadIndexMigrated {
            mods: report.mods,
            resourcepacks: report.resourcepacks,
            failed: report.failed,
        });
    }
    report
}

/// 解析一份旧索引文件
///
/// - `path`: 旧文件
/// - `kind`: 这份索引属于哪类资源
///
/// # 返回值
///
/// 解析出的记录与因缺 hash 跳过的条数；读文件或解析失败时返回错误描述
fn parse_file(path: &Path, kind: IndexKind) -> Result<(Vec<Entry>, usize), String> {
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let map: HashMap<String, Vec<Value>> =
        serde_json::from_str(&text).map_err(|err| err.to_string())?;

    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for (pid, list) in map {
        for mut item in list {
            let Some(hash) = item.get("hash").and_then(Value::as_str) else {
                skipped += 1;
                continue;
            };
            let hash = hash.trim().to_ascii_lowercase();
            if hash.is_empty() {
                skipped += 1;
                continue;
            }
            // 落库的 hash 统一小写：键与记录必须一致，否则按 hash 查不到
            item["hash"] = Value::String(hash.clone());
            entries.push(Entry {
                kind,
                pid: pid.clone(),
                hash,
                payload: item.to_string().into_bytes(),
            });
        }
    }
    Ok((entries, skipped))
}

/// 把旧文件改名成 `.bak` 留底
///
/// - `path`: 旧文件
fn backup(path: &Path) {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    let backup = std::path::PathBuf::from(name);
    // 上次升级留下的备份直接覆盖：它已经没有用了
    let _ = fs::remove_file(&backup);
    let _ = fs::rename(path, &backup);
}
