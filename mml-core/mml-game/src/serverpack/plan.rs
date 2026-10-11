//! 服务器包升级的**纯决策**，无 IO、无网络
//!
//! 从 `upgrade_serverpack` 里抽出来的两类判定：
//!
//! 1. **要删什么** —— [`plan_removals`]：旧有新无的在线文件、同一项目换了路径的旧文件、
//!    旧有新无且 `delete_old` 的配置目录；
//! 2. **要下什么** —— [`plan_downloads`]：每个文件的下载地址与校验哈希，地址自带 `url`
//!    优先，为空时用 `pid`/`fid` 解析出来的那个；
//!
//! 外加两个小纯函数 [`mod_key`] 身份标识、[`make_hash`] 哈希构造，以及判定来源的
//! [`is_curseforge`] 与取地址的 [`modrinth_file_url`]。
//!
//! 抽出来的理由与上一轮 `modpack/diff.rs` 相同：**删错文件 / 下错地址这类判定离线就能测**，
//! 见 `tests/serverpack.rs`；内联在带网络与磁盘操作的 async 函数里时测不到。
//!
//! 判定规则**原样搬运**，未做修改。

use std::collections::HashMap;

use mml_base::file_item::FileHash;
use mml_net::modrinth_api::version_obj::ModrinthVersionObj;

use crate::serverpack::serverpack_obj::{ServerArchiveItemObj, ServerItemObj, ServerPackObj};

/// 升级时要删除的旧内容
#[derive(Debug, Default)]
pub struct ServerPackRemovals<'a> {
    /// 要删除的文件：旧有新无，或同一项目换了路径
    pub files: Vec<&'a ServerItemObj>,
    /// 要删除的目录：旧有新无、`delete_old` 为真且 `dir` 非空
    pub dirs: Vec<&'a ServerArchiveItemObj>,
}

/// 算出升级时要删除的旧文件与旧目录
///
/// 规则与原内联逻辑一致：
///
/// - 在线文件按 [`mod_key`] 配对，即 `pid`，缺失时退回文件名：新包里没有 → 删除旧文件；
///   同一项目但 `file` 路径变了 → 删除旧路径的文件；路径没变 → 不动，下载阶段覆盖。
/// - 配置压缩包按 `file` 配对：新包里没有、且 `delete_old` 为真、`dir` 非空 → 删除该目录。
///
/// 注意：**路径变了就删旧路径**这条只对"同一项目"成立；不同项目但同一路径的文件不会被删，
/// 由下载阶段直接覆盖；这是原实现的取舍，未改动。
pub fn plan_removals<'a>(old: &'a ServerPackObj, new: &ServerPackObj) -> ServerPackRemovals<'a> {
    let mut out = ServerPackRemovals::default();

    for item in &old.online_list {
        match new
            .online_list
            .iter()
            .find(|n| mod_key(n) == mod_key(item))
        {
            // 旧包中已移除的文件
            None => out.files.push(item),
            // 同一文件但路径改变，删除旧文件
            Some(n) if n.file != item.file => out.files.push(item),
            _ => {}
        }
    }

    for item in &old.archive_list {
        let removed = !new.archive_list.iter().any(|n| n.file == item.file);
        if removed && item.delete_old && !item.dir.is_empty() {
            out.dirs.push(item);
        }
    }

    out
}

/// 要下载的内容
#[derive(Debug, Default)]
pub struct ServerPackDownloads<'a> {
    /// 在线文件：(条目, 下载地址)
    pub files: Vec<(&'a ServerItemObj, String)>,
    /// 配置压缩包：(条目, 下载地址)
    pub archives: Vec<(&'a ServerArchiveItemObj, String)>,
}

/// 算出要下载的文件列表与各自地址
///
/// 地址规则与原内联逻辑一致：条目自带 `url` 优先，空串视为没有；为空时用 `resolved`
/// 里的地址，即 `fid → url`，由 `resolve_download_urls` 解析；都拿不到就是空串，
/// 下载阶段会因此失败并报 `DownloadFileFail`。
pub fn plan_downloads<'a>(
    pack: &'a ServerPackObj,
    resolved: &HashMap<String, String>,
) -> ServerPackDownloads<'a> {
    let files = pack
        .online_list
        .iter()
        .map(|item| {
            let url = item
                .url
                .clone()
                .filter(|u| !u.is_empty())
                .or_else(|| item.fid.as_ref().and_then(|f| resolved.get(f).cloned()))
                .unwrap_or_default();
            (item, url)
        })
        .collect();

    let archives = pack
        .archive_list
        .iter()
        .map(|item| (item, item.url.clone()))
        .collect();

    ServerPackDownloads { files, archives }
}

/// 模组身份标识：优先使用项目编号，否则回退到文件名
///
/// - `item`: 在线文件信息
///
/// # 返回值
///
/// 返回身份标识字符串
pub fn mod_key(item: &ServerItemObj) -> String {
    item.pid.clone().unwrap_or_else(|| item.file.clone())
}

/// 根据校验值构建下载哈希
///
/// - `sha1`: SHA1 校验值，没有时为 `None`
/// - `sha256`: SHA256 校验值，没有时为 `None`
///
/// # 返回值
///
/// 返回对应的哈希类型，两个都缺失时为 `FileHash::None`
pub fn make_hash(sha1: &Option<String>, sha256: &Option<String>) -> FileHash {
    match (sha1, sha256) {
        (Some(sha1), Some(sha256)) => FileHash::Sha1Sha256(sha1.clone(), sha256.clone()),
        (Some(sha1), None) => FileHash::Sha1(sha1.clone()),
        (None, Some(sha256)) => FileHash::Sha256(sha256.clone()),
        (None, None) => FileHash::None,
    }
}

/// 判断文件来源：CurseForge 的项目/文件编号是纯数字，Modrinth 是 base62 字符串
///
/// 注意：`pid` 缺失时**只看 `fid`**，数字即 CurseForge；`pid` 与 `fid` 都没有时判为
/// Modrinth —— 原实现如此，未改动。
///
/// - `item`: 在线文件信息
///
/// # 返回值
///
/// 返回是否来自 CurseForge
pub fn is_curseforge(item: &ServerItemObj) -> bool {
    let Some(pid) = &item.pid else {
        return item
            .fid
            .as_deref()
            .map_or(false, |f| f.parse::<u64>().is_ok());
    };
    pid.parse::<u64>().is_ok()
}

/// 取 Modrinth 版本的主文件下载地址，没有 primary 标记时取第一个
///
/// - `version`: Modrinth 版本信息
///
/// # 返回值
///
/// 返回下载地址；版本没有文件返回 `None`
pub fn modrinth_file_url(version: &ModrinthVersionObj) -> Option<String> {
    version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .map(|f| f.url.clone())
}
