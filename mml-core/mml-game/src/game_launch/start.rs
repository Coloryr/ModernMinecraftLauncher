//! 启动流程
//!
//! 登录、检查、拼命令、起进程，串成一次完整的启动。

use std::time::Instant;

use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use tokio_util::sync::CancellationToken;

use crate::{
    cmd_shell,
    game_log::GameLog,
    gui_hook::{LaunchGui, LaunchState, ProcessRunType},
    launcher::instance_setting_obj::InstanceSettingObj,
    loader::LoaderType,
};

use super::{
    handle::InstanceHandle,
    java::{debugger_dir_error, java_major_of, jdwp_arg},
    types::{GameLaunchArg, GameRunObj, LaunchDebugger},
};

// 捕获文件名只给 RenderDoc 用，那个分支在 macOS 上不编译
#[cfg(any(windows, target_os = "linux"))]
use super::java::capture_stem;

impl InstanceSettingObj {
    /// 启动游戏实例
    ///
    /// - `arg`: 启动参数
    /// - `gui`: 启动界面回调
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；登录失败、文件检查失败或启动失败返回对应错误
    pub async fn start_game(
        &self,
        arg: &mut GameLaunchArg,
        gui: LaunchGui,
        cancel: CancellationToken,
    ) -> CoreResult<()> {
        // 清理之前的日志
        crate::clear_game_log(&self.uuid);

        // 检查版本号是否合理
        if self.version.is_empty() {
            return Err(ErrorType::InstanceGameVersionMissing);
        }

        // 有加载器的实例必须填写加载器版本
        if self.loader != LoaderType::Normal && self.loader != LoaderType::Custom {
            if self
                .loader_version
                .as_deref()
                .is_none_or(|data| data.is_empty())
            {
                return Err(ErrorType::InstanceLoaderVersionMissing);
            }
        }

        if self.loader == LoaderType::Custom && !self.get_loader_file().exists() {
            return Err(ErrorType::InstanceLoaderFileMissing);
        }

        // 登陆账户
        self.auth_login(arg, gui.clone(), cancel.clone()).await?;

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        // 服务器包更新：用户在升级弹窗里选了「直接启动」时这次不比对清单
        if !arg.skip_server_pack {
            self.server_pack_update(
                gui.clone(),
                cancel.clone(),
                arg.server_pack_cancel.clone(),
                &arg.server_pack_skip,
            )
            .await?;
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(gui) = &gui {
            gui.update_state(self, LaunchState::Check);
        }

        // 检查游戏文件
        let mut obj = self
            .check_game_file(arg, gui.clone(), cancel.clone())
            .await?;

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        // 获取启动用的JAVA
        let jvm = self.get_java(&mut obj, gui.clone());

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if jvm.is_none() {
            return Err(ErrorType::JavaNotFound);
        }

        let jvm = jvm.unwrap();
        if let Some(major) = java_major_of(&jvm) {
            crate::add_game_log_item(&self.uuid, GameLog::JavaVersion(major as u32));
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(gui) = &gui {
            gui.update_state(self, LaunchState::Jvm);
        }

        InstanceSettingObj::allow_security_manager(&mut obj, &jvm);
        InstanceSettingObj::fix_zgc_generational(&mut obj, &jvm);

        // 创建启动参数
        // 完整命令行 = JVM 参数 + 主类 + 游戏参数；`check` 为真时跳过仍缺失的库文件。
        // 这里原来只取了 make_game_arg 的**游戏参数**，java 会把第一个 `--width`
        // 当成 JVM 选项，直接报"Unrecognized option"起不来
        let mut jvm_arg = self.make_run_arg(&jvm, &arg, &mut obj, true);
        if let Some(port) = arg.jvm_debug {
            jvm_arg.insert(0, jdwp_arg(&jvm, port));
        }

        crate::add_game_log_item(&self.uuid, GameLog::JavaPath(jvm.clone()));
        crate::add_game_log_item(&self.uuid, GameLog::LaunchArgsStart);

        let mut hide = false;

        for item in jvm_arg.iter() {
            if hide {
                hide = false;
                crate::add_game_log_item(
                    &self.uuid,
                    GameLog::LaunchArgs("********************".to_string()),
                );
            } else {
                crate::add_game_log_item(&self.uuid, GameLog::LaunchArgs(item.clone()));
            }

            let low = item.to_lowercase();
            if low.starts_with("--uuid") || low.starts_with("--accesstoken") {
                hide = true;
            }
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let mut env = self.make_env_arg();
        self.add_launcher_env(&mut env, &jvm, &obj.jvm_args);
        self.log_launch_env(&env);

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let default = &mml_config::read_config().jvm_arg;

        let run_arg = self.jvm_arg.as_ref().unwrap_or(default).clone();

        let pre_run = run_arg
            .launch_pre_run
            .unwrap_or(default.launch_pre_run.clone().unwrap_or_default());
        let pre_run_cmd = run_arg
            .pre_run_arg
            .unwrap_or(default.pre_run_arg.clone().unwrap_or_default());

        if pre_run && !pre_run_cmd.is_empty() {
            let prerun = run_arg
                .pre_run_with_game
                .unwrap_or(default.pre_run_with_game.unwrap_or_default());

            let mut can_run = true;
            if let Some(gui) = &gui {
                can_run = gui.launch_process(ProcessRunType::PreLaunch);
            }

            if can_run {
                if let Some(gui) = &gui {
                    gui.update_state(self, LaunchState::Pre);
                }
                let start = Instant::now();
                let temp = self.replace_arg(&jvm, &jvm_arg, &pre_run_cmd);
                cmd_shell::ensure_runner(&temp, &self.get_base_path(), &self.get_game_path())
                    .await?;
                self.cmd_run(&temp, &env, prerun, arg.admin, &cancel)
                    .await?;
                let time = start.elapsed();
                crate::add_game_log_item(&self.uuid, GameLog::CmdPreTime(time));
            }
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        // RenderDoc：先把工具准备好，再用 renderdoccmd 拉起 Java。
        // 放在启动前命令之后：那条命令可能还在补文件，准备工具不必抢在它前面。
        let (run_java, run_args) = match &arg.debugger {
            LaunchDebugger::None => (jvm.clone(), jvm_arg.clone()),
            LaunchDebugger::RenderDoc => {
                #[cfg(any(windows, target_os = "linux"))]
                {
                    if let Some(gui) = &gui {
                        gui.update_state(self, LaunchState::RenderDoc);
                    }
                    let cmd = mml_tools::renderdoc::ensure().await?;
                    let captures = mml_tools::renderdoc::captures_dir();
                    std::fs::create_dir_all(&captures).map_err(debugger_dir_error)?;
                    let stem = format!(
                        "{}_{}_{}",
                        capture_stem(&self.name),
                        self.uuid,
                        chrono::Local::now().format("%Y%m%d_%H%M%S")
                    );
                    let args = mml_tools::renderdoc::capture_args(
                        &jvm,
                        &jvm_arg,
                        &self.get_game_path(),
                        &captures,
                        &stem,
                    );
                    crate::add_game_log_item(
                        &self.uuid,
                        GameLog::RenderDoc {
                            path: cmd.clone(),
                            captures: captures.clone(),
                        },
                    );
                    (cmd, args)
                }
                #[cfg(not(any(windows, target_os = "linux")))]
                {
                    return Err(ErrorType::RenderDocUnsupported);
                }
            }
            LaunchDebugger::Nsight(cli) => {
                let captures = mml_tools::nsight::captures_dir();
                std::fs::create_dir_all(&captures).map_err(debugger_dir_error)?;
                let args = mml_tools::nsight::capture_args(
                    &jvm,
                    &jvm_arg,
                    &self.get_game_path(),
                    &captures,
                    &env,
                );
                crate::add_game_log_item(
                    &self.uuid,
                    GameLog::Nsight {
                        path: cli.clone(),
                        captures: captures.clone(),
                    },
                );
                (cli.clone(), args)
            }
        };

        // RenderDoc 首次下载可能很久，下完再看一眼用户是不是已经取消
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let start = Instant::now();
        let run = GameRunObj {
            uuid: self.uuid,
            encoding: self.encoding,
            auth: arg.auth.clone(),
            path: self.get_game_path(),
            java: run_java,
            args: run_args,
            env: env.clone(),
            admin: arg.admin,
            debugger: arg.debugger.clone(),
        };
        let handel = InstanceHandle::new(run)?;
        let time = start.elapsed();

        crate::add_run_game(handel);
        crate::add_game_log_item(&self.uuid, GameLog::LaunchTime(time));

        // 进程已经起来了：推一次 End，前端据此把"启动中"切成"运行中"（换成停止键等）。
        // ColorMC 在同一位置推 LaunchState.End，见 GameLaunch.cs:555 —— 之前漏了这一条，
        // 日志窗口那颗按钮因此永远收不到"已启动"，停在播放图标上。
        if let Some(gui) = &gui {
            gui.update_state(self, LaunchState::End);
        }

        let post_run = run_arg
            .launch_post_run
            .unwrap_or(default.launch_post_run.clone().unwrap_or_default());
        let post_run_cmd = run_arg
            .post_run_arg
            .unwrap_or(default.post_run_arg.clone().unwrap_or_default());

        if post_run && !post_run_cmd.is_empty() {
            let mut can_run = true;
            if let Some(gui) = &gui {
                can_run = gui.launch_process(ProcessRunType::PostLaunch);
            }

            if can_run {
                if let Some(gui) = &gui {
                    gui.update_state(self, LaunchState::Post);
                }
                let start = Instant::now();
                let temp = self.replace_arg(&jvm, &jvm_arg, &post_run_cmd);
                cmd_shell::ensure_runner(&temp, &self.get_base_path(), &self.get_game_path())
                    .await?;
                self.cmd_run(&temp, &env, false, arg.admin, &cancel).await?;
                let time = start.elapsed();
                crate::add_game_log_item(&self.uuid, GameLog::CmdPostTime(time));
            }
        }

        Ok(())
    }
}
