//! 资源收藏

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_game::launcher::{FileType, ModPackType};
use mml_names::{i18_items::error_type::CoreResult, names, uuids};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 收藏项目
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CollectItemObj {
    /// 收藏项 uuid；加载时由 `CollectObj::items` 的 key 回填，不落盘
    #[serde(skip)]
    pub uuid: String,
    /// 下载源
    #[serde(rename = "Source")]
    pub source: ModPackType,
    /// 资源类型
    #[serde(rename = "FileType")]
    pub file_type: FileType,
    /// 资源名字
    #[serde(rename = "Name")]
    pub name: String,
    /// 项目ID
    #[serde(rename = "Pid")]
    pub pid: String,
    /// 图标
    #[serde(rename = "Icon")]
    pub icon: Option<String>,
    /// 网址
    #[serde(rename = "Url")]
    pub url: String,
    /// 收藏时间，Unix 毫秒；早期条目没有这个字段，读到 0
    #[serde(rename = "FavTime")]
    pub fav_time: i64,
    /// 支持的加载器 ID，如 `fabric`；资源包 / 光影包与查不到的老条目为空
    #[serde(rename = "Loaders")]
    pub loaders: Vec<String>,
    /// 支持的游戏版本，如 `1.21.1`；查不到的老条目为空
    #[serde(rename = "GameVersions")]
    pub game_versions: Vec<String>,
}

/// 资源收藏
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CollectObj {
    /// 收藏项目列表
    #[serde(rename = "Items")]
    pub items: HashMap<String, CollectItemObj>,
    /// 收藏分组列表
    #[serde(rename = "Groups")]
    pub groups: HashMap<String, HashSet<String>>,
}

/// 收藏文件路径，init 时设置
static FILE: OnceLock<PathBuf> = OnceLock::new();
/// 内存收藏数据
static COLLECT: LazyLock<RwLock<CollectObj>> = LazyLock::new(|| RwLock::new(Default::default()));

/// 记录收藏文件路径，启动时调用；实际读取见 [`load`]
///
/// - `path`: 运行路径，收藏文件放它下面
pub fn init<P: AsRef<Path>>(path: P) {
    FILE.get_or_init(|| path.as_ref().join(names::COLLECT_FILE));
}

/// 取当前收藏，供命令层转换 DTO
///
/// # 返回值
///
/// 收藏数据的克隆，调用方随便改都不影响内存里那份
pub fn get() -> CollectObj {
    COLLECT.read().unwrap().clone()
}

/// 从文件读取收藏数据
///
/// 回填各条目的 uuid，并剔除分组里指向已不存在条目的悬空引用。
///
/// # 返回值
///
/// 成功返回 `Ok(())`；文件解析失败返回对应错误
pub fn load() -> CoreResult<()> {
    let file = FILE.get().unwrap();
    let mut obj: CollectObj = if file.exists() {
        serialize_tools::json_from_file(file)?
    } else {
        Default::default()
    };

    for (key, value) in obj.items.iter_mut() {
        value.uuid = key.clone();
    }

    for group in obj.groups.values_mut() {
        group.retain(|key1| obj.items.contains_key(key1));
    }

    *COLLECT.write().unwrap() = obj;

    Ok(())
}

/// 保存收藏数据，异步落盘
pub fn save() {
    config_save::save(
        uuids::COLLECT_UUID,
        &*COLLECT.read().unwrap(),
        FILE.get().unwrap(),
    );
}

/// 删除收藏条目，按下载源加项目 ID 匹配，并从所有分组移除
///
/// - `obj`: 待删除的收藏条目，只看 source 与 pid，不看 uuid
pub fn remove_item(obj: CollectItemObj) {
    {
        let mut collect = COLLECT.write().unwrap();

        let key = match collect
            .items
            .iter()
            .find(|(_, value)| value.source == obj.source && value.pid == obj.pid)
        {
            Some((key, _)) => key.clone(),
            None => return,
        };

        for (_, value) in collect.groups.iter_mut() {
            value.remove(&key);
        }

        collect.items.remove(&key);
    }

    save();
}

/// 按 uuid 删除收藏条目，并从所有分组移除
///
/// - `uuid`: 收藏条目 uuid
pub fn remove_uuid(uuid: &str) {
    {
        let mut collect = COLLECT.write().unwrap();

        collect.items.remove(uuid);

        for (_, value) in collect.groups.iter_mut() {
            value.remove(uuid);
        }
    }

    save();
}

/// 添加收藏条目，同下载源同项目 ID 已存在时忽略
///
/// - `obj`: 收藏条目，uuid 由这里生成
///
/// # 返回值
///
/// 新条目的 uuid；已收藏过返回 `None`（收藏后按类型归组要用它）
pub fn add_item(mut obj: CollectItemObj) -> Option<String> {
    let uuid = {
        let mut collect = COLLECT.write().unwrap();

        let have = collect
            .items
            .iter()
            .any(|(_, value)| value.source == obj.source && value.pid == obj.pid);

        if have {
            return None;
        }

        // 收藏时间由这里落，调用方不必各自取时钟；重复收藏已在上面提前返回，不会刷新时间
        obj.fav_time = now_ms();

        let uuid = Uuid::new_v4().to_string();
        collect.items.insert(uuid.clone(), obj);
        uuid
    };

    save();
    Some(uuid)
}

/// 把一条收藏放进分组，分组不存在时顺手建出来
///
/// 「默认分组」不是这里的具名分组，而是"不在任何具名分组里"的那批收藏。一项只能属于
/// 一个具名分组，所以这里先把 uuid 从别的组里摘掉，与 [`set_group_items`] 同一语义。
///
/// - `group`: 分组名，新收藏按资源类型归类时用的就是它
/// - `uuid`: 收藏条目 uuid
pub fn add_to_group(group: &str, uuid: &str) {
    {
        let mut collect = COLLECT.write().unwrap();

        for (name, value) in collect.groups.iter_mut() {
            if name != group {
                value.remove(uuid);
            }
        }

        collect
            .groups
            .entry(group.to_string())
            .or_default()
            .insert(uuid.to_string());
    }

    save();
}

/// 当前时间，Unix 毫秒
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0)
}

/// 添加收藏分组，同名已存在时忽略
///
/// - `name`: 分组名
pub fn add_group(name: &str) {
    {
        let mut collect = COLLECT.write().unwrap();

        if collect.groups.contains_key(name) {
            return;
        }

        collect.groups.insert(name.to_string(), Default::default());
    }

    save();
}

/// 删除收藏分组，分组内条目保留，仅脱离分组
///
/// - `name`: 分组名
pub fn remove_group(name: &str) {
    {
        let mut collect = COLLECT.write().unwrap();
        collect.groups.remove(name);
    }

    save();
}

/// 清空全部收藏，分组保留，只清成员
pub fn clear() {
    {
        let mut collect = COLLECT.write().unwrap();

        collect.items.clear();

        for (_, value) in collect.groups.iter_mut() {
            value.clear();
        }
    }

    save();
}

/// 只清空某个分组的成员，收藏条目本身保留
///
/// - `name`: 分组名，不存在时不做任何事
pub fn clear_group(name: &str) {
    {
        let mut collect = COLLECT.write().unwrap();

        if let Some(value) = collect.groups.get_mut(name) {
            value.clear();
        }
    }

    save();
}

/// 从分组移除若干收藏，条目本身保留，之后会回到默认分组
///
/// - `group`: 分组名，不存在时不做任何事
/// - `uuids`: 要移除的收藏条目 uuid
pub fn remove_group_items(group: &str, uuids: &[String]) {
    {
        let mut collect = COLLECT.write().unwrap();

        if let Some(value) = collect.groups.get_mut(group) {
            for uuid in uuids {
                value.remove(uuid);
            }
        }
    }

    save();
}

/// 把若干收藏移入分组，移动语义：先从其它分组里摘掉
///
/// - `group`: 分组名，不存在时不做任何事
/// - `uuids`: 要移入的收藏条目 uuid
///
/// 界面上的「默认分组」不是这里的具名分组，而是"不在任何具名分组里"的那批收藏，
/// 收藏时不带分组参数，新收藏天然落在那边。既然具名分组之外还有默认分组，
/// 一项就只能属于一个具名分组，否则各组计数相加不等于收藏总数。
pub fn set_group_items(group: &str, uuids: &[String]) {
    {
        let mut collect = COLLECT.write().unwrap();

        if !collect.groups.contains_key(group) {
            return;
        }

        // 先摘出其它分组
        for (name, value) in collect.groups.iter_mut() {
            if name == group {
                continue;
            }
            for uuid in uuids {
                value.remove(uuid);
            }
        }

        if let Some(value) = collect.groups.get_mut(group) {
            for uuid in uuids {
                value.insert(uuid.clone());
            }
        }
    }

    save();
}

/// 补上收藏项的支持信息：加载器与游戏版本
///
/// 收藏那一刻只有列表里现成的字段，支持信息要一次网络请求，所以由调用方在后台补。
/// 条目在这期间被取消收藏就什么都不做，别把信息写回一个不存在的位置。
///
/// - `source`: 下载源
/// - `pid`: 项目 ID
/// - `loaders`: 支持的加载器 ID，见 `LoaderType::id`
/// - `game_versions`: 支持的游戏版本
///
/// # 返回值
///
/// 写入成功返回 `true`；条目已不在收藏里返回 `false`
pub fn set_support(
    source: ModPackType,
    pid: &str,
    loaders: Vec<String>,
    game_versions: Vec<String>,
) -> bool {
    {
        let mut collect = COLLECT.write().unwrap();
        let Some(item) = collect
            .items
            .values_mut()
            .find(|item| item.source == source && item.pid == pid)
        else {
            return false;
        };

        item.loaders = loaders;
        item.game_versions = game_versions;
    }

    save();
    true
}

/// 项目是否已收藏；`items` 以收藏条目 uuid 为键，这里按值里的项目 ID 匹配
///
/// - `pid`: 项目 ID
///
/// # 返回值
///
/// 已收藏返回 `true`
pub fn is_star(pid: &str) -> bool {
    COLLECT
        .read()
        .unwrap()
        .items
        .values()
        .any(|item| item.pid == pid)
}
