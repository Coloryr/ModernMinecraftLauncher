//! 模组扫描缓存
//!
//! 一次扫描的结果留在窗口 model 里，更新检查与依赖检测直接读，不重扫盘。

use std::collections::HashMap;

use mml_game::game_mods::ModObj;
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::loader::LoaderType;

/// 资源窗口在注册表里的 kind，也是窗口族表里的族名
pub(super) const KIND: &str = "resource";

/// 一次模组扫描的结果
///
/// 与列表命令返回给前端的那份是同一趟扫描：列表走 DTO，这里留内核口径的原始数据，
/// 更新检查、依赖检测直接在上面算。
pub(crate) struct ModCache {
    /// 内核扫描出的条目，含 jar-in-jar 内置项
    pub(crate) list: Vec<ModObj>,
    /// 实例的在线信息表，键是项目编号
    pub(crate) online: HashMap<String, OnlineInfoObj>,
    /// 实例的游戏版本，更新检查按它过滤平台上的文件
    pub(crate) version: String,
    /// 实例的加载器
    pub(crate) loader: LoaderType,
}
