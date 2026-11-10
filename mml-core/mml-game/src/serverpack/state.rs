//! 清单状态
//!
//! 服务器清单的保存、版本判定、旧版搬移与跳过记录。

use std::{collections::HashSet, path::Path};

use mml_base::{
    hash_helper::{HashType, gen_hash_from_file},
    serialize_tools,
};
use mml_config::config_save;
use mml_names::{i18_items::error_type::CoreResult, uuids};
use mml_sys::path_helper;

use crate::{
    launcher::{
        ModPackType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    serverpack::serverpack_obj::ServerPackObj,
};

use super::files::{dir_of, existing_file, file_name_of, local_file, trimmed};
use super::{GUI_SERVER_PACK_KEY, GUI_SKIPPED_KEY, ServerPackSkippedObj, paths};

impl InstanceSettingObj {
    /// 将服务器实例信息标记为旧版
    ///
    /// 第一次同步时本地还没有清单，没有可搬的旧版，直接当成功处理：
    /// 否则新玩家第一次启动必然卡在这里。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；移动失败返回对应错误
    pub fn move_serverpack_to_old(&self) -> CoreResult<()> {
        let file = self.get_server_pack_file();
        if !file.exists() {
            return Ok(());
        }
        path_helper::move_file(file, self.get_server_pack_old_file())
    }

    /// 跳过同步时把旧清单挪回原位
    ///
    /// 同步开始前 `server.json` 已经被挪成 `.old`，中途取消时本地就没有清单了；
    /// 挪回来界面上照样能读到模组与资源包信息，下次启动也会重新比对。
    ///
    /// # 返回值
    ///
    /// 移动成功返回 `Ok(())`；移动失败返回对应错误
    pub fn restore_serverpack_from_old(&self) -> CoreResult<()> {
        let file = self.get_server_pack_file();
        let old = self.get_server_pack_old_file();
        if file.exists() || !old.exists() {
            return Ok(());
        }
        path_helper::move_file(old, file)
    }

    /// 读取旧版服务器实例信息
    ///
    /// # 返回值
    ///
    /// 返回旧版服务器包信息；文件不存在返回 `None`，解析失败返回对应错误
    pub(super) fn get_old_serverpack(&self) -> CoreResult<Option<ServerPackObj>> {
        let file = self.get_server_pack_old_file();
        if !file.exists() || file.is_dir() {
            return Ok(None);
        }
        let obj = serialize_tools::json_from_file::<ServerPackObj>(file)?;

        Ok(Some(obj))
    }

    /// 同步保存服务器包信息
    ///
    /// 导入流程紧接着要读清单时用，异步队列可能还没落盘。
    ///
    /// - `pack`: 服务器包信息
    ///
    /// # 返回值
    ///
    /// 写盘失败时返回对应错误
    pub fn save_serverpack_sync(&self, pack: &ServerPackObj) -> CoreResult<()> {
        serialize_tools::json_to_file(pack, self.get_server_pack_file())
    }

    /// 本地清单的包版本是否与给定版本相同
    ///
    /// - `version`: 远端清单的包版本
    ///
    /// # 返回值
    ///
    /// 本地有清单、版本非空且与参数相同返回 `true`
    pub fn same_pack_version(&self, version: &str) -> bool {
        let version = version.trim();
        if version.is_empty() {
            return false;
        }
        serialize_tools::json_from_file::<ServerPackObj>(self.get_server_pack_file())
            .ok()
            .is_some_and(|old| old.pack_version.trim() == version)
    }

    /// 保存服务器包信息
    ///
    /// - `pack`: 服务器包信息
    pub fn save_serverpack(&self, pack: &ServerPackObj) {
        // 本地清单是给同步流程读的，紧凑存盘
        config_save::save_compact(
            uuids::mix_uuid(self.uuid, uuids::SERVERPACK_FILE_UUID),
            pack,
            self.get_server_pack_file(),
        );
    }

    /// 读实例 `gui_setting.json` 的原始 JSON
    ///
    /// 只拿服务器包那一段，其余字段原样保留；文件不存在或读不懂时返回 `None`。
    ///
    /// # 返回值
    ///
    /// 顶层是对象的 JSON；文件不存在、解析失败时是 `None`
    fn gui_setting_json(&self) -> Option<serde_json::Value> {
        let data = std::fs::read(self.get_gui_setting_file()).ok()?;
        let value = serde_json::from_slice::<serde_json::Value>(&data).ok()?;
        value.is_object().then_some(value)
    }

    /// 读上次跳过更新时留下的旧文件记录
    ///
    /// # 返回值
    ///
    /// `gui_setting.json` 里的跳过记录；没有记录或读不到时是空表
    pub(super) fn skipped_state(&self) -> Vec<ServerPackSkippedObj> {
        let Some(value) = self.gui_setting_json() else {
            return Vec::new();
        };
        let Some(records) = value
            .get(GUI_SERVER_PACK_KEY)
            .and_then(|section| section.get(GUI_SKIPPED_KEY))
            .cloned()
        else {
            return Vec::new();
        };
        serde_json::from_value(records).unwrap_or_default()
    }

    /// 把跳过记录写回实例 `gui_setting.json`
    ///
    /// 只改 `ServerPack.Skipped` 一段：文件里其它设置原样保留。文件内容读不懂时
    /// 直接不写，宁可这次没记，也不能把界面设置整份冲掉。
    ///
    /// - `list`: 本次同步后要留下的跳过记录
    pub(super) fn save_skipped_state(&self, list: &[ServerPackSkippedObj]) {
        let file = self.get_gui_setting_file();
        // 没有记录、也还没有这份文件：不必凭空建一个
        if list.is_empty() && !file.exists() {
            return;
        }
        let mut value = if file.exists() {
            let Some(value) = self.gui_setting_json() else {
                return;
            };
            value
        } else {
            serde_json::json!({})
        };
        let Some(root) = value.as_object_mut() else {
            return;
        };
        let section = root
            .entry(GUI_SERVER_PACK_KEY)
            .or_insert_with(|| serde_json::json!({}));
        if !section.is_object() {
            *section = serde_json::json!({});
        }
        let Some(section) = section.as_object_mut() else {
            return;
        };
        let Ok(records) = serde_json::to_value(list) else {
            return;
        };
        section.insert(GUI_SKIPPED_KEY.to_string(), records);
        let Ok(text) = serde_json::to_string(&value) else {
            return;
        };
        let _ = path_helper::write_text(&file, &text);
    }

    /// 把清单里的游戏版本与加载器写回实例设置
    ///
    /// 写回走注册表：调用方手里的 `self` 是快照，`get_instance` 拿到的才是锁里那份。
    ///
    /// - `pack`: 本次同步的清单
    pub(super) fn apply_pack_setting(&self, pack: &ServerPackObj) {
        let Some(instance) = crate::get_instance(&self.uuid) else {
            return;
        };
        let mut game = instance.write().unwrap();
        if game.modpack_type != ModPackType::ServerPack {
            return;
        }

        let mut changed = false;
        let version = pack.version.trim();
        if !version.is_empty() && game.version != version {
            game.version = version.to_string();
            changed = true;
        }
        if game.loader != pack.loader {
            game.loader = pack.loader;
            changed = true;
        }
        let loader_version = pack
            .loader_version
            .as_deref()
            .map(str::trim)
            .filter(|item| !item.is_empty());
        if let Some(loader_version) = loader_version
            && game.loader_version.as_deref() != Some(loader_version)
        {
            game.loader_version = Some(loader_version.to_string());
            changed = true;
        }

        if changed {
            game.save();
        }
    }

    /// 用清单里的项目号 / 文件号刷新实例的在线信息
    ///
    /// 表里只放**本地真的存在**的文件：清单里有编号但没落盘的不记，用户自己加的模组原样留着。
    /// 只收项目号与文件号都有的条目 —— 这张表的下游（更新检查、依赖检测、项目编号列、
    /// 生成服务器实例）全都按编号查平台，只有下载地址的配置文件记进来只是噪音。
    ///
    /// - `new_pack`: 本次同步的清单
    /// - `game_path`: 游戏目录
    pub(super) fn sync_online_info(&self, new_pack: &ServerPackObj, game_path: &Path) {
        let old = self.read_online_info();
        let mut info = old.clone();

        // 清单接管的文件先按文件名摘掉旧记录，再按本次的编号重建
        let owned: HashSet<String> = new_pack
            .online_list
            .iter()
            .filter_map(|item| file_name_of(&item.file))
            .collect();
        info.retain(|_, item| {
            !owned.contains(&item.file) && existing_file(&local_file(game_path, item)).is_some()
        });

        for item in new_pack.online_list.iter().filter(|item| !item.custom) {
            let (Some(pid), Some(fid)) =
                (trimmed(item.pid.as_deref()), trimmed(item.fid.as_deref()))
            else {
                continue;
            };
            // 本地没有这个文件就没什么可记的；禁用中的也算，玩家随时会再启用
            let Ok(target) = paths::file_path(game_path, &item.file) else {
                continue;
            };
            let Some((local, _)) = existing_file(&target) else {
                continue;
            };

            // 表的主键是 SHA1，按本地文件为准：清单没给校验值就现算
            let sha1 = match trimmed(item.sha1.as_deref()) {
                Some(value) => value.to_string(),
                None => match gen_hash_from_file(HashType::Sha1, &local) {
                    Ok(value) => value,
                    Err(_) => continue,
                },
            };

            let entry = OnlineInfoObj {
                path: dir_of(&item.file),
                name: file_name_of(&item.file).unwrap_or_default(),
                file: file_name_of(&item.file).unwrap_or_default(),
                sha1: sha1.clone(),
                url: item.url.clone().unwrap_or_default(),
                modid: pid.to_string(),
                fileid: fid.to_string(),
            };
            info.insert(sha1, entry);
        }

        // 内容没变就别写盘：每次启动都会同步，白写一遍十几万字节没意义
        if info != old {
            self.save_online_info(&info);
        }
    }
}
