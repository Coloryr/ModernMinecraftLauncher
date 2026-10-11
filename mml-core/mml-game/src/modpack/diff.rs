//! 整合包升级的**纯差异计算**：无 IO、无网络，便于离线单测
//!
//! 这些逻辑原本内联在两个 worker 的 `check_upgrade` 里，夹在 API 调用与文件删除之间，
//! 离线测不了；这里按"只搬逻辑、不改判定规则"抽出来。**随后按用户要求改过一轮判定规则**：
//! 四条路径统一成"**根据新旧清单做两向比对**"——
//!
//! - 新有旧无 → 下载，即 `add`
//! - 旧有新无 → 删除，即 `remove`
//! - 同键但版本 / 哈希变了 → 又下载新的又删除旧的
//!
//! 四条路径：
//!
//! | 函数 | 触发条件 | 配对键 | 版本判定 |
//! | --- | --- | --- | --- |
//! | [`diff_curseforge_files`] | 有旧 `manifest.json` | `project_id` | `file_id` |
//! | [`diff_curseforge_online`] | 无旧 manifest，解析失败也算 | `mod_id` | `fileid` / `sha1` |
//! | [`diff_modrinth_files`] | 有旧 `modrinth.index.json` | 包内路径 | `sha1` |
//! | [`diff_modrinth_online`] | 无旧 manifest | `mod_id` | `fileid` / `sha1` |
//!
//! 现行判定规则：
//!
//! 1. **CF 有旧 manifest**：配对时跳过**已配对的旧项**。否则新清单里同一个
//!    `project_id` 出现两次时，第二个新项会重复配到第一个旧项，把一个**仍在新清单里**的
//!    文件推进 `remove` → 升级后实例缺文件。
//! 2. **Modrinth 有旧 manifest**：配对键用**包内路径**而非 `sha1`。按 `sha1` 做双向差集时，
//!    同一个 `sha1` 出现在两个路径，第一个新项被当成"与旧文件相同"而跳过下载、旧项又被
//!    抵消而不删 → 新清单里的那个路径**永远不会被创建**，实例缺文件。按路径配对后路径与
//!    `sha1` 都相同才算不动，改名 = 删旧路径 + 下新路径。
//! 3. **两条"在线信息"路径**，即 CF SHA1 与 Modrinth 无 manifest：补上"旧有新无 → 删除"。
//!    只更新与新增会留下垃圾文件。已确认 `online_info` 只由整合包
//!    路径写入，`update_online` 只清理、`copy_to_other` 只复制，`make_file_online_*` 无调用者，
//!    所以删除它记录的文件不会误伤用户自己装的模组。

use std::collections::HashMap;

use crate::curseforge::pack_obj::FilesObj;
use crate::launcher::file_online_info_obj::OnlineInfoObj;
use crate::launcher_path::instance_path::OnlineInfoList;
use crate::modrinth::pack_obj::ModrinthPackFileObj;

/// 文件级差异：借用新旧清单里的项，下载与删除由调用方决定
#[derive(Debug, Default)]
pub struct FileDiff<'a, T> {
    /// 需要下载的项：新增项与变更后的新版本
    pub add: Vec<&'a T>,
    /// 需要删除的项：仅旧清单有的项与变更前的旧版本
    pub remove: Vec<&'a T>,
}

/// 在线信息级差异：`OnlineInfoObj` 是 `Clone`，直接给出副本
#[derive(Debug, Default)]
pub struct OnlineDiff {
    /// 需要下载的项：新增项与变更后的新版本
    pub add: Vec<OnlineInfoObj>,
    /// 需要删除的项：仅旧有项与变更前的旧版本
    pub remove: Vec<OnlineInfoObj>,
}

/// CurseForge：有旧 manifest 时按 `project_id` 配对，`file_id` 不同视为变更
///
/// - `new_files`: 新整合包清单里的文件
/// - `old_files`: 实例内旧 manifest 里的文件
///
/// 已配对的旧项不再参与配对，否则同一 `project_id` 出现两次时，第二个新项会重复配到
/// 第一个旧项，把仍在新清单里的文件推进 `remove`。
///
/// # 返回值
///
/// 需下载与需删除的借用项，怎么下载 / 删除由调用方决定
pub fn diff_curseforge_files<'a>(
    new_files: &'a [FilesObj],
    old_files: &'a [FilesObj],
) -> FileDiff<'a, FilesObj> {
    let mut add: Vec<&FilesObj> = Vec::new();
    let mut remove: Vec<&FilesObj> = Vec::new();

    let mut old_matched = vec![false; old_files.len()];

    for new_file in new_files {
        let mut found = false;
        for (j, old_file) in old_files.iter().enumerate() {
            // 已配对的旧项不再参与，否则同一 project_id 的第二个新项会重复配到第一个旧项
            if old_matched[j] {
                continue;
            }
            if new_file.project_id == old_file.project_id {
                found = true;
                old_matched[j] = true;
                if new_file.file_id != old_file.file_id {
                    // 同一项目但文件 ID 不同 → 需要更新
                    add.push(new_file);
                    remove.push(old_file);
                }
                break;
            }
        }
        if !found {
            // 仅在新的 manifest 中出现 → 新增
            add.push(new_file);
        }
    }

    // 仅在旧的 manifest 中出现 → 删除
    for (j, old_file) in old_files.iter().enumerate() {
        if !old_matched[j] {
            remove.push(old_file);
        }
    }

    FileDiff { add, remove }
}

/// CurseForge：无旧 manifest 时按 `mod_id` 比对在线信息，`fileid` 或 `sha1` 变化视为变更
///
/// - `new_online`: 新清单解析出来的在线信息，键为 `mod_id`
/// - `old_online`: 实例当前已安装的在线信息
///
/// 两向比对，新有旧无进 `add`，旧有新无进 `remove`；`online_info` 只记整合包管的文件，
/// 所以删除它记录的文件不会误伤用户自装的模组。
///
/// # 返回值
///
/// 需下载与需删除的在线信息副本
pub fn diff_curseforge_online(
    new_online: &OnlineInfoList,
    old_online: &OnlineInfoList,
) -> OnlineDiff {
    let mut add: Vec<OnlineInfoObj> = Vec::new();
    let mut remove: Vec<OnlineInfoObj> = Vec::new();

    for (mod_id, existing_mod) in old_online.iter() {
        match new_online.get(mod_id) {
            Some(new_mod) => {
                // 同 mod_id：检查是否需要更新
                if existing_mod.fileid != new_mod.fileid || existing_mod.sha1 != new_mod.sha1 {
                    add.push(new_mod.clone());
                    remove.push(existing_mod.clone());
                }
            }
            // 新清单里没有 → 删除
            None => remove.push(existing_mod.clone()),
        }
    }

    // 新增：在新列表中但不在现有列表中的模组
    for (mod_id, new_mod) in new_online {
        if !old_online.contains_key(mod_id) {
            add.push(new_mod.clone());
        }
    }

    OnlineDiff { add, remove }
}

/// Modrinth：有旧 manifest 时按包内路径配对，`sha1` 不同视为变更
///
/// - `new_files`: 新整合包索引里的文件
/// - `old_files`: 实例内旧 `modrinth.index.json` 的文件
///
/// 路径与 `sha1` 都一样才不动，改名 = 删旧路径 + 下新路径。
///
/// # 返回值
///
/// 需下载与需删除的借用项，怎么下载 / 删除由调用方决定
pub fn diff_modrinth_files<'a>(
    new_files: &'a [ModrinthPackFileObj],
    old_files: &'a [ModrinthPackFileObj],
) -> FileDiff<'a, ModrinthPackFileObj> {
    let old_by_path: HashMap<&str, &ModrinthPackFileObj> =
        old_files.iter().map(|f| (f.path.as_str(), f)).collect();
    let new_by_path: HashMap<&str, &ModrinthPackFileObj> =
        new_files.iter().map(|f| (f.path.as_str(), f)).collect();

    let add = new_files
        .iter()
        .filter(|f| {
            // 旧清单里没有这个路径 → 下载；有但 sha1 不同 → 下载
            old_by_path
                .get(f.path.as_str())
                .map(|old| old.hashes.sha1 != f.hashes.sha1)
                .unwrap_or(true)
        })
        .collect();

    let remove = old_files
        .iter()
        .filter(|f| {
            new_by_path
                .get(f.path.as_str())
                .map(|new| new.hashes.sha1 != f.hashes.sha1)
                .unwrap_or(true)
        })
        .collect();

    FileDiff { add, remove }
}

/// Modrinth：无旧 manifest 时按 `mod_id` 比对在线信息
///
/// - `new_online`: 新整合包解析出来的在线信息
/// - `old_online`: 实例当前已安装的在线信息
///
/// 两向比对：同 `mod_id` 且 `fileid` 或 `sha1` 不同 → 变更；新有旧无 → 新增；
/// 旧有新无 → 删除。
///
/// # 返回值
///
/// 需下载与需删除的在线信息副本
pub fn diff_modrinth_online(
    new_online: &OnlineInfoList,
    old_online: &OnlineInfoList,
) -> OnlineDiff {
    // temp1 = 当前已安装模组，temp2 = 新整合包模组
    let temp1: Vec<OnlineInfoObj> = old_online.values().cloned().collect();
    let mut temp2: Vec<Option<OnlineInfoObj>> = new_online.values().cloned().map(Some).collect();

    let mut add: Vec<OnlineInfoObj> = Vec::new();
    let mut remove: Vec<OnlineInfoObj> = Vec::new();

    for item in &temp1 {
        let mut matched = false;
        for a in 0..temp2.len() {
            let Some(item1) = &temp2[a] else { continue };
            if item.modid != item1.modid {
                continue;
            }
            // 同 mod_id → 从新列表中取出该模组
            let item1 = temp2[a].take().unwrap();
            matched = true;
            // 同 mod_id 但 fileid/sha1 不同 → 需要更新
            if item.fileid != item1.fileid || item.sha1 != item1.sha1 {
                add.push(item1);
                remove.push(item.clone());
            }
            break;
        }
        // 新整合包里没有 → 删除
        if !matched {
            remove.push(item.clone());
        }
    }

    // 新整合包中有、当前未安装 → 新增
    for item in temp2.iter().flatten() {
        add.push(item.clone());
    }

    OnlineDiff { add, remove }
}
