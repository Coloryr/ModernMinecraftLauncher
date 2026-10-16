//! 方块窗口的用户数据：左栏顺序 / 自建分组 / 收藏，配置文件 `block_groups.json`
//!
//! 与 [`crate::collect_utils`] 同一套路：内存一份、改动后异步落盘。
//! 这些是**用户数据**，按仓库约定不进前端 localStorage（见 AGENTS.md「前端本地存储」一节）。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock, RwLock},
};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::{i18_items::error_type::CoreResult, names, uuids};
use serde::{Deserialize, Serialize};

/// 一个自建分组
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct BlockGroupObj {
    /// 分组名
    #[serde(rename = "Name")]
    pub name: String,
}

/// 方块窗口的用户数据
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct BlockGroupsObj {
    /// 左栏顺序：游戏分类名与 `g:<uuid>` 混排
    #[serde(rename = "Order")]
    pub order: Vec<String>,
    /// 自建分组：uuid → 分组
    #[serde(rename = "Groups")]
    pub groups: HashMap<String, BlockGroupObj>,
    /// 分组 uuid → 成员方块 id
    #[serde(rename = "Members")]
    pub members: HashMap<String, Vec<String>>,
    /// 收藏的方块 id
    #[serde(rename = "Favorites")]
    pub favorites: Vec<String>,
}

/// 数据文件路径，init 时设置
static FILE: OnceLock<PathBuf> = OnceLock::new();
/// 内存数据
static DATA: LazyLock<RwLock<BlockGroupsObj>> = LazyLock::new(|| RwLock::new(Default::default()));

/// 记录数据文件路径，启动时调用；实际读取见 [`load`]
///
/// - `path`: 运行路径，文件放它下面
pub fn init<P: AsRef<Path>>(path: P) {
    FILE.get_or_init(|| path.as_ref().join(names::BLOCK_GROUPS_FILE));
}

/// 取当前数据，供命令层转换 DTO
///
/// # 返回值
///
/// 数据的克隆，调用方随便改都不影响内存里那份
pub fn get() -> BlockGroupsObj {
    DATA.read().unwrap().clone()
}

/// 从文件读取；文件不存在时按空数据处理
///
/// # 返回值
///
/// 成功返回 `Ok(())`；文件解析失败返回对应错误
pub fn load() -> CoreResult<()> {
    let file = FILE.get().unwrap();
    let obj: BlockGroupsObj = if file.exists() {
        serialize_tools::json_from_file(file)?
    } else {
        Default::default()
    };

    *DATA.write().unwrap() = obj;

    Ok(())
}

/// 整份替换并异步落盘
///
/// 界面一次只改一处（收藏 / 分组 / 顺序），但整份都小，直接写全量最省事。
///
/// - `obj`: 新的完整数据
pub fn set(obj: BlockGroupsObj) {
    *DATA.write().unwrap() = obj;
    save();
}

/// 异步落盘
fn save() {
    config_save::save(
        uuids::BLOCK_GROUPS_UUID,
        &*DATA.read().unwrap(),
        FILE.get().unwrap(),
    );
}
