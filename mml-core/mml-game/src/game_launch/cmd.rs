//! 拼启动命令并起进程

use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::process_helper;
use tokio_util::sync::CancellationToken;

use crate::error::stream_err;
use crate::{
    cmd_shell,
    gui_hook::{LaunchGui, LaunchState},
    launcher::instance_setting_obj::InstanceSettingObj,
    loader::LoaderType,
};

use super::{
    handle::read_process_stream,
    java::jdwp_arg,
    types::{GameLaunchArg, LaunchCmd},
};

impl InstanceSettingObj {
    /// 按命令行文本启动一个进程，并把输出接到实例日志上
    ///
    /// 命令怎么拆、脚本用哪个解释器见 [`crate::cmd_shell`]。
    ///
    /// - `cmd`: 命令行文本
    /// - `env`: 额外环境变量
    /// - `wait_run`: 是否等待执行完成；等的时候可以取消
    /// - `admin`: 是否以管理员方式启动
    /// - `cancel`: 取消令牌，等命令跑完期间被取消就掐掉它
    ///
    /// # 返回值
    ///
    /// 程序找不到或启动失败时返回错误；等待期间被取消返回 `TaskCancel`
    pub(super) async fn cmd_run(
        &self,
        cmd: &str,
        env: &HashMap<String, String>,
        wait_run: bool,
        admin: bool,
        cancel: &CancellationToken,
    ) -> CoreResult<()> {
        let base = self.get_base_path();
        let command = cmd_shell::resolve(cmd, &base, &self.get_game_path())?;

        let mut run_env = env.clone();
        if command.python {
            // Python 往管道写是块缓冲，不设这个日志会攒到进程退出才一次性出来；
            // 输出编码跟实例的日志编码走，读写两边才对得上
            run_env.insert("PYTHONUNBUFFERED".to_string(), "1".to_string());
            run_env.insert(
                "PYTHONIOENCODING".to_string(),
                self.encoding.codec_name().to_string(),
            );
        }

        // 工作目录是实例根目录，和双击 bat 一致：相对路径照 bat 写，游戏目录用 %GAME_DIR% 取
        match process_helper::launch_hidden(&command.program, command.args, run_env, &base, admin) {
            Ok(mut result) => {
                // 命令输出按实例的日志编码解，和游戏日志同一套；命令不等退出，标志给个不会置位的
                let encoding = self.encoding;
                if let Some(stdout) = result.stdout {
                    let uuid = self.uuid;
                    std::thread::spawn(move || {
                        read_process_stream(
                            stdout,
                            uuid,
                            encoding,
                            Arc::new(AtomicBool::new(false)),
                            false,
                        );
                    });
                }

                if let Some(stderr) = result.stderr {
                    let uuid = self.uuid;
                    std::thread::spawn(move || {
                        read_process_stream(
                            stderr,
                            uuid,
                            encoding,
                            Arc::new(AtomicBool::new(false)),
                            false,
                        );
                    });
                }

                // 是否与游戏同时启动
                if !wait_run {
                    return Ok(());
                }

                // 等它跑完，期间允许取消：点「取消启动」就掐掉这条命令，连带子进程
                loop {
                    if cancel.is_cancelled() {
                        // 提权启动时真正的进程挂在辅助进程下面，先让它去收尾
                        if let Some(control) = &result.control {
                            let _ = control.request_stop();
                        }
                        let _ = process_helper::kill_tree(&mut result.child);
                        return Err(ErrorType::TaskCancel);
                    }

                    match result.child.try_wait() {
                        Ok(Some(_)) => return Ok(()),
                        Ok(None) => tokio::time::sleep(Duration::from_millis(150)).await,
                        Err(err) => return Err(stream_err(err)),
                    }
                }
            }
            Err(err) => Err(err),
        }
    }

    /// 生成游戏启动参数，不用于启动
    ///
    /// - `arg`: 启动参数
    /// - `cancel`: 取消令牌
    /// - `gui`: 启动界面回调
    ///
    /// # 返回值
    ///
    /// 返回启动命令参数；版本号缺失、登录失败或找不到 Java 返回对应错误
    pub async fn create_game_cmd(
        &self,
        arg: &mut GameLaunchArg,
        cancel: &CancellationToken,
        gui: LaunchGui,
    ) -> CoreResult<LaunchCmd> {
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

        if let Some(gui) = &gui {
            gui.update_state(&self, LaunchState::Login);
        }

        self.auth_login(arg, gui.clone(), cancel.clone()).await?;

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(gui) = &gui {
            gui.update_state(&self, LaunchState::ReadInfo);
        }

        let mut obj = self.make_game_launch_obj(arg).await?;

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let java = self.get_java(&obj, gui.clone());

        if java.is_none() {
            return Err(ErrorType::JavaNotFound);
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(gui) = &gui {
            gui.update_state(&self, LaunchState::Jvm);
        }

        let java = java.unwrap();
        InstanceSettingObj::allow_security_manager(&mut obj, &java);
        InstanceSettingObj::fix_zgc_generational(&mut obj, &java);
        let mut args = self.make_run_arg(&java, &arg, &mut obj, false);
        if let Some(port) = arg.jvm_debug {
            args.insert(0, jdwp_arg(&java, port));
        }
        let mut env = self.make_env_arg();
        self.add_launcher_env(&mut env, &java, &obj.jvm_args);

        Ok(LaunchCmd {
            java,
            dir: self.get_game_path(),
            arg: args,
            env,
        })
    }
}
