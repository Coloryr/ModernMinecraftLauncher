//! 游戏版本判定辅助

use mml_base::version_parse;

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj, launcher_path::version_path,
    mojang::game_arg_obj::GameArgObj,
};

impl InstanceSettingObj {
    /// 是否为V2版本
    ///
    /// # 返回值
    ///
    /// 是 V2 版本时返回 `true`，否则返回 `false`
    pub fn is_game_version_v2(&self) -> bool {
        let version = version_path::get_version(&self.version);
        match version {
            Err(_) => false,
            Ok(data) => data.is_game_version_v2(),
        }
    }
}

impl GameArgObj {
    /// 是否为V2版本
    ///
    /// # 返回值
    ///
    /// 要求的最低启动器版本高于 18 时返回 `true`
    pub fn is_game_version_v2(&self) -> bool {
        self.minimum_launcher_version > 18
    }

    /// 判断是否是 1.17 以上版本
    ///
    /// # 返回值
    ///
    /// 版本不低于 1.17 时返回 `true`
    pub fn is_game_version_117(&self) -> bool {
        version_parse::is_game_version_117(&self.id)
    }

    /// 判断是否是 1.20 以上版本
    ///
    /// # 返回值
    ///
    /// 版本不低于 1.20 时返回 `true`
    pub fn is_game_version_120(&self) -> bool {
        version_parse::is_game_version_120(&self.id)
    }

    /// 判断是否是 1.20.2 以上版本
    ///
    /// # 返回值
    ///
    /// 版本不低于 1.20.2 时返回 `true`
    pub fn is_game_version_1202(&self) -> bool {
        version_parse::is_game_version_1202(&self.id)
    }
}

impl InstanceSettingObj {
    /// 判断是否是 1.17 以上版本
    ///
    /// # 返回值
    ///
    /// 版本不低于 1.17 时返回 `true`
    pub fn is_game_version_117(&self) -> bool {
        version_parse::is_game_version_117(&self.version)
    }

    /// 判断是否是 1.20 以上版本
    ///
    /// # 返回值
    ///
    /// 版本不低于 1.20 时返回 `true`
    pub fn is_game_version_120(&self) -> bool {
        version_parse::is_game_version_120(&self.version)
    }

    /// 判断是否是 1.20.2 以上版本
    ///
    /// # 返回值
    ///
    /// 版本不低于 1.20.2 时返回 `true`
    pub fn is_game_version_1202(&self) -> bool {
        version_parse::is_game_version_1202(&self.version)
    }
}
