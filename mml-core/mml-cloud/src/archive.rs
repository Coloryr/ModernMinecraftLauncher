//! 跨平台路径校验与差异包。先完整验证压缩包，再在隔离目录应用并替换。

use crate::{DELETE_FILE, Error, MAX_ARCHIVE, MAX_FILES, MAX_UNPACKED, Manifest, Result};
use sha1::{Digest, Sha1};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// 校验纯目录名，同时拒绝 Windows 的设备名与备用数据流。
///
/// - `name`: 目录名
///
/// # 返回值
///
/// 名字非法时返回路径错误。
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 240
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:<>\"|?*".contains(c))
        || matches!(name, "." | "..")
    {
        return Err(Error::new("err.cloudPath"));
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(Error::new("err.cloudPath"));
    }
    Ok(())
}

/// 校验包内相对路径，所有平台均使用斜杠分隔。
///
/// - `name`: 清单或压缩包内路径
///
/// # 返回值
///
/// 可直接连接到受控目录的相对路径。
pub fn relative_path(name: &str) -> Result<PathBuf> {
    if name.len() > 2048 {
        return Err(Error::new("err.cloudPath"));
    }
    let mut path = PathBuf::new();
    for component in name.split('/') {
        validate_name(component)?;
        path.push(component);
    }
    Ok(path)
}

fn regular_root(root: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(root)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(Error::new("err.cloudPath"));
    }
    Ok(())
}

/// 扫描普通文件并计算 SHA1，不跟随软链接，不同步 session.lock。
///
/// - `root`: 存档或配置目录
///
/// # 返回值
///
/// 文件清单；目录不存在时返回空表。
pub fn scan(root: &Path) -> Result<Manifest> {
    if !root.try_exists()? {
        return Ok(Manifest::new());
    }
    regular_root(root)?;
    let mut files = Manifest::new();
    let mut total = 0;
    scan_dir(root, root, &mut files, &mut total)?;
    validate_manifest(&files)?;
    Ok(files)
}

fn scan_dir(root: &Path, dir: &Path, files: &mut Manifest, total: &mut u64) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if meta.file_type().is_symlink() {
            return Err(Error::new("err.cloudPath"));
        }
        let entry_path = entry.path();
        let name = entry_path
            .strip_prefix(root)
            .map_err(|_| Error::new("err.cloudPath"))?
            .to_str()
            .ok_or_else(|| Error::new("err.cloudPath"))?
            .replace('\\', "/");
        relative_path(&name)?;
        if meta.is_dir() {
            scan_dir(root, &entry.path(), files, total)?;
        } else if meta.is_file() && name != DELETE_FILE && name != "session.lock" {
            *total = total.saturating_add(meta.len());
            if *total > MAX_UNPACKED || files.len() >= MAX_FILES {
                return Err(Error::new("err.cloudLimit"));
            }
            let mut file = File::open(entry.path())?;
            let mut hash = Sha1::new();
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            files.insert(name, crate::hex(&hash.finalize()));
        }
    }
    Ok(())
}

/// 验证清单中的路径、SHA1 与数量。
///
/// - `manifest`: 来自网络或本地的清单
///
/// # 返回值
///
/// 任一字段非法时返回错误。
pub fn validate_manifest(manifest: &Manifest) -> Result<()> {
    if manifest.len() > MAX_FILES {
        return Err(Error::new("err.cloudLimit"));
    }
    let mut names = HashSet::new();
    for (name, hash) in manifest {
        relative_path(name)?;
        if !names.insert(name.to_lowercase())
            || hash.len() != 40
            || !hash.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(Error::new("err.cloudProtocol"));
        }
    }
    Ok(())
}

/// 文件夹总大小，拒绝链接。
///
/// - `root`: 要统计的目录
///
/// # 返回值
///
/// 普通文件大小之和，单位字节。
pub fn disk_usage(root: &Path) -> Result<u64> {
    if !root.try_exists()? {
        return Ok(0);
    }
    regular_root(root)?;
    let mut total = 0u64;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if meta.file_type().is_symlink() {
            return Err(Error::new("err.cloudPath"));
        }
        total = total.saturating_add(if meta.is_dir() {
            disk_usage(&entry.path())?
        } else {
            meta.len()
        });
    }
    Ok(total)
}

/// 带输出大小限制的 ZIP 写入器。
struct LimitedWriter(Cursor<Vec<u8>>);
impl Write for LimitedWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.0.position().saturating_add(data.len() as u64) > MAX_ARCHIVE as u64 {
            return Err(io::Error::other("err.cloudLimit"));
        }
        self.0.write(data)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for LimitedWriter {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}

/// 比较源目录与远端清单，生成包含删除清单的增量包。
///
/// - `root`: 源目录
/// - `local`: 源目录清单
/// - `remote`: 目标目录当前清单
///
/// # 返回值
///
/// 没有差异时返回 None，否则返回 ZIP 字节。
pub fn delta(root: &Path, local: &Manifest, remote: &Manifest) -> Result<Option<Vec<u8>>> {
    validate_manifest(local)?;
    validate_manifest(remote)?;
    let changed: Vec<_> = local
        .iter()
        .filter(|(name, hash)| remote.get(*name) != Some(*hash))
        .collect();
    let deleted: Vec<_> = remote
        .keys()
        .filter(|name| !local.contains_key(*name))
        .collect();
    if changed.is_empty() && deleted.is_empty() {
        return Ok(None);
    }
    let mut zip = ZipWriter::new(LimitedWriter(Cursor::new(Vec::new())));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, hash) in changed {
        let path = root.join(relative_path(name)?);
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(Error::new("err.cloudPath"));
        }
        zip.start_file(name, options)?;
        let mut file = File::open(path)?;
        let mut hasher = Sha1::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut count = 0u64;
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > MAX_UNPACKED {
                return Err(Error::new("err.cloudLimit"));
            }
            hasher.update(&buffer[..n]);
            zip.write_all(&buffer[..n])?;
        }
        if &crate::hex(&hasher.finalize()) != hash {
            return Err(Error::new("err.cloudConflict"));
        }
    }
    if !deleted.is_empty() {
        zip.start_file(DELETE_FILE, options)?;
        zip.write_all(&serde_json::to_vec(&deleted)?)?;
    }
    Ok(Some(zip.finish()?.0.into_inner()))
}

/// 已验证的差异包，内容在写磁盘前全部读取完成。
struct Patch {
    /// 新增及修改的文件。
    files: BTreeMap<String, Vec<u8>>,
    /// 相对源目录的待删除文件。
    deleted: Vec<String>,
}

fn unpack(bytes: &[u8]) -> Result<Patch> {
    if bytes.len() > MAX_ARCHIVE {
        return Err(Error::new("err.cloudLimit"));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    if zip.len() > MAX_FILES {
        return Err(Error::new("err.cloudLimit"));
    }
    let mut patch = Patch {
        files: BTreeMap::new(),
        deleted: Vec::new(),
    };
    let mut total = 0u64;
    let mut names = HashSet::new();
    for index in 0..zip.len() {
        let mut item = zip.by_index(index)?;
        let name = item.name()?.trim_end_matches('/').to_string();
        relative_path(&name)?;
        if !names.insert(name.to_lowercase())
            || item
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(Error::new("err.cloudPath"));
        }
        if item.is_dir() {
            continue;
        }
        if item.size() > MAX_UNPACKED.saturating_sub(total) {
            return Err(Error::new("err.cloudLimit"));
        }
        let mut data = Vec::new();
        (&mut item)
            .take(MAX_UNPACKED.saturating_sub(total) + 1)
            .read_to_end(&mut data)?;
        total = total.saturating_add(data.len() as u64);
        if total > MAX_UNPACKED {
            return Err(Error::new("err.cloudLimit"));
        }
        if name == DELETE_FILE {
            patch.deleted = serde_json::from_slice(&data)?;
            if patch.deleted.len() > MAX_FILES {
                return Err(Error::new("err.cloudLimit"));
            }
            for deleted in &patch.deleted {
                relative_path(deleted)?;
            }
        } else if name != "session.lock" {
            patch.files.insert(name, data);
        }
    }
    Ok(patch)
}

/// 把一份目录复制到隔离目录，不复制链接。
///
/// - `source`: 原目录
/// - `target`: 新建目录
///
/// # 返回值
///
/// 文件读取失败时返回错误。
pub fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for name in scan(source)?.keys() {
        let relative = relative_path(name)?;
        let destination = target.join(&relative);
        fs::create_dir_all(
            destination
                .parent()
                .ok_or_else(|| Error::new("err.cloudPath"))?,
        )?;
        fs::copy(source.join(relative), destination)?;
    }
    Ok(())
}

/// 在临时副本应用差异包，完整成功后替换；原目录保留为备份。
///
/// - `target`: 目标存档或配置目录
/// - `bytes`: ZIP 差异包
/// - `backup_root`: 原目录备份的父目录
///
/// # 返回值
///
/// 存在旧目录时返回备份路径；任一步失败不覆盖原目录。
pub fn apply(target: &Path, bytes: &[u8], backup_root: &Path) -> Result<Option<PathBuf>> {
    let patch = unpack(bytes)?;
    let parent = target.parent().ok_or_else(|| Error::new("err.cloudPath"))?;
    fs::create_dir_all(parent)?;
    fs::create_dir_all(backup_root)?;
    let staging = parent.join(format!(".cloud-stage-{}", Uuid::new_v4()));
    let result = (|| {
        copy_tree(target, &staging)?;
        for name in patch.deleted {
            let path = staging.join(relative_path(&name)?);
            if path.is_file() {
                fs::remove_file(path)?;
            }
        }
        for (name, data) in patch.files {
            let path = staging.join(relative_path(&name)?);
            fs::create_dir_all(path.parent().ok_or_else(|| Error::new("err.cloudPath"))?)?;
            fs::write(path, data)?;
        }
        scan(&staging)?;
        let backup = if target.try_exists()? {
            regular_root(target)?;
            let backup = backup_root.join(Uuid::new_v4().to_string());
            fs::rename(target, &backup)?;
            Some(backup)
        } else {
            None
        };
        if let Err(err) = fs::rename(&staging, target) {
            if let Some(path) = &backup {
                // 回滚失败时仍保留备份，不删除唯一的旧副本。
                fs::rename(path, target)?;
            }
            return Err(err.into());
        }
        Ok(backup)
    })();
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}
