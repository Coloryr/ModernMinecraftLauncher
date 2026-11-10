//! 启动前登录
//!
//! 按实例的登录方式取一次令牌，交给启动参数使用。

use std::time::Instant;

use mml_auth::LoginObj;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use tokio_util::sync::CancellationToken;

use crate::{
    game_log::GameLog, gui_hook::LaunchGui, launcher::instance_setting_obj::InstanceSettingObj,
};

use super::types::GameLaunchArg;

impl InstanceSettingObj {
    /// 尝试刷新账户，若失败则询问是否离线模式
    ///
    /// - `arg`: 启动参数
    /// - `gui`: 启动界面回调
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；刷新失败且用户拒绝离线模式返回对应错误
    pub(super) async fn auth_login(
        &self,
        arg: &mut GameLaunchArg,
        gui: LaunchGui,
        cancel: CancellationToken,
    ) -> CoreResult<()> {
        let start = Instant::now();
        let res = arg.auth.refresh(cancel.clone()).await;
        let time = start.elapsed();
        crate::add_game_log_item(&self.uuid, GameLog::LoginTime(time));

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        match res {
            Ok(_) => {
                arg.auth.save();
                Ok(())
            }
            Err(err) => {
                if let Some(gui) = gui
                    && gui.login_fail(&arg.auth).await
                {
                    let old = &arg.auth;
                    arg.auth = LoginObj {
                        user_name: old.user_name.clone(),
                        uuid: old.uuid.clone(),
                        ..Default::default()
                    };
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }
}
