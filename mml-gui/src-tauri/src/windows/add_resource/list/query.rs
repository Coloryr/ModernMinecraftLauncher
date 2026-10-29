//! 资源站的查询串
//!
//! 前端的筛选条件先规整成一份查询参数，再交给内核；
//! 页码与每页条数在这里兜底，避免内核收到非法值。

use mml_game::launcher::{FileType, ModPackType};
use mml_game::launcher_path::instance_path::OnlineInfoList;
use mml_game::loader::LoaderType;
use uuid::Uuid;

use crate::err_keys;

use crate::windows::add_resource::DOWNLOAD_NOW;

/// 列表与详情命令共用的入参解析结果
pub(super) struct ListQuery {
    /// 实例 uuid，用来查"已下载 / 正在下载"；浏览模式（还没选实例）为 None
    pub(super) uuid: Option<Uuid>,
    /// 实例的在线信息，按项目 ID 查下载状态；没有实例时是空表
    pub(super) info: OnlineInfoList,
    /// 下载源
    pub(super) source: ModPackType,
    /// 资源类型
    pub(super) file_type: FileType,
    /// 模组加载器；非模组一律 `Normal`
    pub(super) mod_loader: LoaderType,
}

/// 解析列表与详情命令共用的入参
///
/// - `game`: 实例 uuid 字符串
/// - `source`: 下载源字符串
/// - `file_type`: 资源类型字符串
/// - `loader`: 模组加载器字符串，仅模组类型有意义，缺省 / 非法值按 `normal`
///
/// # 返回值
///
/// 见 [`ListQuery`]；uuid 非法、实例不存在或资源类型非法时返回错误文案
pub(super) fn parse_query(
    game: &str,
    source: &str,
    file_type: &str,
    loader: Option<&str>,
) -> Result<ListQuery, String> {
    // 空串 = 还没选实例：照常搜索，只是没有"装没装过"的标记。
    // 收藏窗口那个入口就是这么用的 —— 实例在点下载时才选，见 AddResourceWindow。
    let (uuid, info) = if game.is_empty() {
        (None, OnlineInfoList::default())
    } else {
        let Ok(uuid) = Uuid::parse_str(game) else {
            return Err(String::from(err_keys::UUID));
        };
        let Some(instance) = mml_game::get_instance(&uuid) else {
            return Err(String::from(err_keys::GAME_NOT_FOUND));
        };
        let info = instance.read().unwrap().read_online_info();
        (Some(uuid), info)
    };
    let source = ModPackType::from_string(source);
    let Some(file_type) = FileType::from_string(file_type) else {
        return Err(String::from(err_keys::FILE_TYPE_NOT_FOUND));
    };

    // loader 只对模组有意义，其余类型一律 Normal
    let mod_loader = if matches!(file_type, FileType::Mod) {
        LoaderType::from_string(loader.unwrap_or_default()).unwrap_or(LoaderType::Normal)
    } else {
        LoaderType::Normal
    };

    Ok(ListQuery {
        uuid,
        info,
        source,
        file_type,
        mod_loader,
    })
}

/// 项目是否正在下载，列表角标用；判据是该实例任务表里有同 pid 且**还没结束**的条目
///
/// - `game`: 实例 uuid
/// - `pid`: 项目 ID
///
/// # 返回值
///
/// 该项目下有任一文件在下载即为 `true`
pub(super) fn check_download_now(game: Option<&Uuid>, pid: &str) -> bool {
    let Some(game) = game else {
        return false;
    };
    let read = DOWNLOAD_NOW.read().unwrap();
    let Some(list) = read.get(game) else {
        return false;
    };
    // 终态条目会一直留在表里，等用户点「清除已完成」，所以 done / failed / cancelled 都要排掉
    for (key, info) in list.iter() {
        if key.pid == pid && !info.done && !info.failed && !info.cancelled {
            return true;
        }
    }

    false
}

/// 具体文件是否正在下载；判据是该实例任务表里有同 pid 与 fid 且**还没结束**的条目
///
/// - `game`: 实例 uuid
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
///
/// # 返回值
///
/// 该项目下这个文件正在下载即为 `true`
pub(super) fn check_version_download_now(game: Option<&Uuid>, pid: &str, fid: &str) -> bool {
    let Some(game) = game else {
        return false;
    };
    let read = DOWNLOAD_NOW.read().unwrap();
    let Some(list) = read.get(game) else {
        return false;
    };
    for (key, info) in list.iter() {
        if key.fid == fid && key.pid == pid && !info.done && !info.failed && !info.cancelled {
            return true;
        }
    }

    false
}
