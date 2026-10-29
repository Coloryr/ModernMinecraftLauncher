//! 在线新文件：成批取回
//!
//! 文件号按平台接口的上限切批，取回的是「替换旧文件」这一步要用的数据。

use std::collections::HashMap;
use std::path::PathBuf;

use mml_base::file_item::FileItemObj;
use mml_game::launcher::ModPackType;
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher::mod_update_obj::ModUpdateRecordObj;
use mml_net::curseforge_api::{self, file_obj::CurseForgeFileDataObj};
use mml_net::modrinth_api::{self, version_obj::ModrinthVersionObj};

use crate::dtos::ModUpdateApplyDto;
use crate::err_keys;
use crate::log_text::LogText;

/// 一批更新里的一条：待下载的新文件 + 下完之后收尾要用的信息
///
/// 下载是**整批**一个任务，所以准备与收尾分成两步，中间隔着那一次下载，
/// 见 [`resource_mods_update_apply`]。
pub(super) struct PendingUpdate {
    /// 待下载的新文件
    pub(super) obj: FileItemObj,
    /// 要删掉的旧文件
    pub(super) old: PathBuf,
    /// 旧文件是不是禁用着；是的话新文件也补上禁用后缀
    pub(super) disable: bool,
    /// 项目编号，写在线信息与更新记录都用它
    pub(super) project_id: String,
    /// 模组名字，记进更新记录
    pub(super) name: String,
    /// 要写回的在线信息
    pub(super) info: OnlineInfoObj,
    /// 要追加的更新记录
    pub(super) record: ModUpdateRecordObj,
}

/// 批量取回来的新文件数据，按来源分家
///
/// 两份结构不一样，但下一步都只是"造下载项 + 造在线信息"，在这里收一下，
/// 逐条准备时就不用再管来源的差别
pub(super) enum NewFileData {
    /// CurseForge 的文件信息
    CurseForge(CurseForgeFileDataObj),
    /// Modrinth 的版本信息
    Modrinth(ModrinthVersionObj),
}

/// 新文件号 → 平台数据
pub(super) type NewFileMap = HashMap<String, NewFileData>;

/// 这一批要问的文件号，按来源分好
pub(super) struct NewFileIds {
    /// CurseForge 的数字文件号，去重升序
    pub(super) curseforge: Vec<u64>,
    /// Modrinth 的版本号，去重升序
    pub(super) modrinth: Vec<String>,
}

/// 一次批量问多少个文件号
///
/// Modrinth 的版本号拼在 URL 上，太多会让请求行过长；CurseForge 虽然走 POST 体，
/// 但接口对 `fileIds` 的长度也有上限，两边都按这个值分批，别赌上限
pub(super) const FILE_IDS_PER_REQUEST: usize = 50;

/// 把这一批条目按来源分成两组文件号，各自去重
///
/// - `items`: 这一批要更新的条目
///
/// # 返回值
///
/// 见 [`NewFileIds`]；来源不认识的条目直接忽略
pub(super) fn split_new_file_ids(items: &[ModUpdateApplyDto]) -> NewFileIds {
    let mut curseforge = Vec::new();
    let mut modrinth = Vec::new();
    for item in items {
        match ModPackType::from_string(&item.source) {
            // 数字号解析不了的先不收：逐条准备时会报"文件号不合法"
            ModPackType::CurseForge => {
                if let Ok(id) = item.new_file_id.parse::<u64>() {
                    curseforge.push(id);
                }
            }
            ModPackType::Modrinth => modrinth.push(item.new_file_id.clone()),
            _ => {}
        }
    }
    curseforge.sort_unstable();
    curseforge.dedup();
    modrinth.sort();
    modrinth.dedup();
    NewFileIds {
        curseforge,
        modrinth,
    }
}

/// 一次问回这一批的新文件
///
/// 下载更新动辄几十上百个模组，逐个问平台就是几十上百次往返，实测每次 1~2 秒；
/// 两个接口都支持按文件号批量取，所以按来源各发一次请求。
///
/// - `items`: 这一批要更新的条目
///
/// # 返回值
///
/// 新文件号 → 平台数据；某一类请求失败时只记日志、返回这一类的空表，
/// 由逐条准备报"文件找不到"，不连累另一类
pub(super) async fn fetch_new_files(items: &[ModUpdateApplyDto]) -> NewFileMap {
    let NewFileIds {
        curseforge,
        modrinth,
    } = split_new_file_ids(items);
    let mut map = NewFileMap::new();

    for chunk in curseforge.chunks(FILE_IDS_PER_REQUEST) {
        match curseforge_api::get_files(chunk.to_vec()).await {
            Ok(list) => {
                for data in list {
                    map.insert(data.id.to_string(), NewFileData::CurseForge(data));
                }
            }
            Err(err) => mml_log::error(
                LogText::FetchCurseForgeFailed {
                    error: err.to_string(),
                }
                .text(),
            ),
        }
    }

    for chunk in modrinth.chunks(FILE_IDS_PER_REQUEST) {
        match modrinth_api::get_versions(chunk.to_vec()).await {
            Ok(list) => {
                for data in list {
                    map.insert(data.id.clone(), NewFileData::Modrinth(data));
                }
            }
            Err(err) => mml_log::error(
                LogText::FetchModrinthFailed {
                    error: err.to_string(),
                }
                .text(),
            ),
        }
    }

    map
}

/// 单独问一个文件：批量取没拿到时的退路
///
/// - `source`: 下载源
/// - `item`: 要更新的条目
///
/// # 返回值
///
/// 见 [`NewFileData`]；文件号非法或平台没有这个文件时返回错误串
pub(super) async fn fetch_one(
    source: ModPackType,
    item: &ModUpdateApplyDto,
) -> Result<NewFileData, String> {
    match source {
        ModPackType::CurseForge => {
            let fid: u64 = item
                .new_file_id
                .parse()
                .map_err(|_| err_keys::BAD_FILE_ID.to_string())?;
            let mut list = curseforge_api::get_files(vec![fid])
                .await
                .map_err(|err| err.to_string())?;
            let data = list
                .pop()
                .ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())?;
            Ok(NewFileData::CurseForge(data))
        }
        ModPackType::Modrinth => {
            let data = modrinth_api::get_version(&item.project_id, &item.new_file_id)
                .await
                .map_err(|err| err.to_string())?;
            Ok(NewFileData::Modrinth(data))
        }
        _ => Err(err_keys::SOURCE_TYPE.to_string()),
    }
}
