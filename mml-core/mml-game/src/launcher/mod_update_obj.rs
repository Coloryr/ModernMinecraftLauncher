//! 实例的模组更新记录，保存于实例目录

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

/// 模组更新记录表，键是项目编号
///
/// 落盘在实例目录的 `mod_update.json`；按项目分开，界面上一个项目就是一组
pub type ModUpdateList = HashMap<String, ModUpdateProjectObj>;

/// 一个项目的全部更新记录
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "PascalCase")]
pub struct ModUpdateProjectObj {
    /// 模组名字，取界面上显示的那个；取不到时为空串，界面回退项目编号
    pub name: String,
    /// 这个项目的更新历史，按时间先后排列
    pub update: Vec<ModUpdateRecordObj>,
}

/// 一条模组更新记录
///
/// 「下载更新」与「替换版本」每换成功一个模组就追加一条；项目编号不在这里，
/// 它是外层表的键
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "PascalCase")]
pub struct ModUpdateRecordObj {
    /// 换上去的文件名，即平台上的文件名；禁用状态下不带 `.disabled` 后缀
    pub file: String,
    /// 换之前的版本号，取界面上显示的那个；取不到时为空串
    pub old_version: String,
    /// 换之后的版本号，即平台上的版本号 / 显示名
    pub version: String,
    /// 换之前的文件号，即平台上那一份的编号；取不到时为空串
    pub old_file_id: String,
    /// 换之后的文件号
    pub file_id: String,
    /// 更新时间，带时区偏移
    pub time: DateTime<FixedOffset>,
    /// 换上去那份文件的 SHA1
    pub sha1: String,
}
