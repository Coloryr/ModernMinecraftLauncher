//! 启动前文件检查
//!
//! 缺什么补什么：游戏核心、运行库、资源索引，顺带同步服务器包。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Instant,
};

use mml_base::{
    file_item::FileHash,
    hash_helper::{self, HashType},
    serialize_tools,
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
use tokio_util::sync::CancellationToken;

use crate::{
    game_arg::GameLaunchObj,
    game_log::GameLog,
    gui_hook::{LaunchGui, LaunchState},
    launcher::{ModPackType, instance_setting_obj::InstanceSettingObj},
    launcher_path::libraries_path,
    serverpack::serverpack_obj::ServerPackObj,
};

use super::types::GameLaunchArg;

impl InstanceSettingObj {
    /// 检查缺失的文件，自动下载，完成后返回启动配置
    ///
    /// 只做"检查 + 补齐"，不起进程；想单独提供"检查游戏文件"入口的调用方可以用它。
    ///
    /// - `arg`: 启动参数
    /// - `gui`: 启动界面回调
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 返回启动配置；检查或下载失败返回对应错误
    pub async fn check_game_file(
        &self,
        arg: &mut GameLaunchArg,
        gui: LaunchGui,
        cancel: CancellationToken,
    ) -> CoreResult<GameLaunchObj> {
        if let Some(gui) = &gui {
            gui.update_state(self, LaunchState::Check);
        }

        // 检查几乎没有可见输出，出了问题只能猜；先把这一步在做什么记下来
        let loader = match &self.loader_version {
            Some(version) if !version.is_empty() => {
                format!("{} {version}", self.loader.id())
            }
            _ => self.loader.id().to_string(),
        };
        crate::add_game_log_item(
            &self.uuid,
            GameLog::CheckStart {
                version: self.version.clone(),
                loader,
            },
        );

        let start = Instant::now();
        let authlib = libraries_path::check_authlib(&arg.auth.auth_type).await?;
        let obj = self.make_game_launch_obj(arg).await?;
        crate::add_game_log_item(&self.uuid, GameLog::MainClass(obj.main_class.clone()));
        // 原生库 jar 已存在时下载线程不会再跑解压，启动前补一遍
        crate::game_check::unpack_existing_natives(&obj);
        let mut check = self.get_lost_game_file(&obj).await?;
        let time = start.elapsed();
        crate::add_game_log_item(&self.uuid, GameLog::CheckGameFileTime(time));

        if let Some(data) = authlib {
            check.push(data);
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        // 有文件需要下载，则开始下载
        if check.is_empty() {
            Ok(obj)
        } else {
            let download = if mml_config::read_config().http.auto_download == false
                && let Some(gui) = &gui
            {
                gui.request_download_file().await
            } else {
                true
            };

            if download {
                if let Some(gui) = &gui {
                    gui.update_state(self, LaunchState::Download);
                }
                crate::add_game_log_item(&self.uuid, GameLog::DownloadStart(check.len()));
                let start = Instant::now();
                let res =
                    mml_downloader::start_download_task_cancellable(check, cancel.clone()).await;
                let time = start.elapsed();
                crate::add_game_log_item(&self.uuid, GameLog::DownloadFileTime(time));
                if res.is_cancelled() || cancel.is_cancelled() {
                    Err(ErrorType::TaskCancel)
                } else if !res.is_done() {
                    Err(ErrorType::DownloadFileFail)
                } else {
                    Ok(obj)
                }
            } else {
                Ok(obj)
            }
        }
    }

    /// 是否把 HOME / APPDATA / XDG 等系统目录隔离到实例目录
    ///
    /// 实例没有单独设置时跟随启动器设置，缺省按启用处理。
    ///
    /// # 返回值
    ///
    /// 启用返回 `true`
    pub(crate) fn isolate_home(&self) -> bool {
        self.jvm_arg
            .as_ref()
            .and_then(|item| item.isolate_home)
            .unwrap_or_else(|| {
                mml_config::read_config()
                    .jvm_arg
                    .isolate_home
                    .unwrap_or(true)
            })
    }

    /// 隔离时使用的用户目录：实例目录本身
    ///
    /// # 返回值
    ///
    /// 实例根目录的绝对路径
    pub(crate) fn isolated_home(&self) -> PathBuf {
        self.get_base_path()
    }

    /// 是否把 MML 环境变量传给游戏进程
    ///
    /// 实例没有单独设置时跟随启动器设置，缺省按启用处理。
    ///
    /// # 返回值
    ///
    /// 启用返回 `true`
    pub(crate) fn mml_env(&self) -> bool {
        self.jvm_arg
            .as_ref()
            .and_then(|item| item.mml_env)
            .unwrap_or_else(|| mml_config::read_config().jvm_arg.mml_env.unwrap_or(true))
    }

    /// 创建环境变量
    ///
    /// # 返回值
    ///
    /// 返回启动进程使用的环境变量表
    pub(crate) fn make_env_arg(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        // 游戏内的 Java agent 靠这两个变量识别 MML 启动，避免重复同步服务器包
        if self.mml_env() {
            env.insert("MML_LAUNCHER".to_string(), "1".to_string());
            env.insert(
                "MML_LAUNCHER_VERSION".to_string(),
                mml_names::VERSION.clone(),
            );
        }
        // 先铺隔离路径，用户自己写的环境变量随后覆盖，手动指定优先
        if self.isolate_home() {
            let home = self.isolated_home();
            let home_str = home.to_string_lossy().into_owned();
            for key in ["HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA"] {
                env.insert(key.to_string(), home_str.clone());
            }
            let config = home.join(".config");
            let data = home.join(".local").join("share");
            let cache = home.join(".cache");
            for dir in [&config, &data, &cache] {
                let _ = std::fs::create_dir_all(dir);
            }
            env.insert(
                "XDG_CONFIG_HOME".to_string(),
                config.to_string_lossy().into_owned(),
            );
            env.insert(
                "XDG_DATA_HOME".to_string(),
                data.to_string_lossy().into_owned(),
            );
            env.insert(
                "XDG_CACHE_HOME".to_string(),
                cache.to_string_lossy().into_owned(),
            );
            #[cfg(windows)]
            if let Some((drive, rest)) = home_str.split_once(':') {
                env.insert("HOMEDRIVE".to_string(), format!("{drive}:"));
                env.insert("HOMEPATH".to_string(), rest.to_string());
            }
        }
        let envstr = self
            .jvm_arg
            .clone()
            .map(|item| item.jvm_env.clone())
            .unwrap_or(mml_config::read_config().jvm_arg.jvm_env.clone())
            .unwrap_or(String::new());

        if !envstr.is_empty() {
            let list: Vec<&str> = envstr.split('\n').collect();
            for item in list {
                let temp = item.trim();
                let index: Vec<&str> = temp.splitn(2, '=').collect();
                if index.len() == 1 {
                    env.insert(index[0].to_string(), String::new());
                } else {
                    env.insert(index[0].to_string(), index[1].to_string());
                }
            }
        }

        env
    }

    /// 写入供游戏内 Java agent 识别的 MML 启动参数环境变量
    ///
    /// 关掉「加入 MML 环境变量」时整组不写。
    ///
    /// - `env`: 目标环境变量表
    /// - `java`: 实际使用的 Java 可执行文件
    /// - `jvm_args`: 已生成的 JVM 参数
    pub(crate) fn add_launcher_env(
        &self,
        env: &mut HashMap<String, String>,
        java: &Path,
        jvm_args: &[String],
    ) {
        if !self.mml_env() {
            return;
        }
        env.insert(
            "ARG_JAVA_LOCAL".to_string(),
            java.to_string_lossy().into_owned(),
        );
        env.insert("ARG_JAVA_ARG".to_string(), jvm_args.join(" "));
        env.insert(
            "ARG_LAUNCHER_DIR".to_string(),
            mml_base::get_base_dir().to_string_lossy().into_owned(),
        );
        env.insert("ARG_GAME_NAME".to_string(), self.name.clone());
        env.insert("ARG_GAME_UUID".to_string(), self.uuid.to_string());
        env.insert(
            "ARG_GAME_DIR".to_string(),
            self.get_game_path().to_string_lossy().into_owned(),
        );
        env.insert(
            "ARG_GAME_BASE_DIR".to_string(),
            self.get_base_path().to_string_lossy().into_owned(),
        );
    }

    /// 输出这次启动传给游戏进程的环境变量，按变量名排序
    ///
    /// - `env`: 已经拼好的环境变量表
    pub(crate) fn log_launch_env(&self, env: &HashMap<String, String>) {
        let mut keys: Vec<&String> = env.keys().collect();
        keys.sort();
        for key in keys {
            crate::add_game_log_item(
                &self.uuid,
                GameLog::EnvVar {
                    key: key.clone(),
                    value: env[key].clone(),
                },
            );
        }
    }

    /// 检查服务器包更新
    ///
    /// - `gui`: 启动界面回调
    /// - `cancel`: 整次启动的取消令牌
    /// - `pack_cancel`: 服务器包同步自己的令牌，取消它只跳过这次同步，启动继续
    /// - `skip`: 这次先不更新的文件，取清单里的相对路径；空表示全部更新
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；检查或更新失败返回对应错误。`pack_cancel` 被取消时按"跳过更新"处理
    pub(super) async fn server_pack_update(
        &self,
        gui: LaunchGui,
        cancel: CancellationToken,
        pack_cancel: Option<CancellationToken>,
        skip: &[String],
    ) -> CoreResult<()> {
        // 界面上点「跳过更新」只停这一份；整次启动的令牌仍然能中止全部
        let sync_cancel = pack_cancel.unwrap_or_else(|| cancel.clone());
        // 只有"在线网络整合包"才按服务器包同步：界面上只有这一类会填服务器实例网址，
        // 别的整合包带一个残留地址不该把启动卡在网络上
        if self.is_modpack
            && self.modpack_type == ModPackType::ServerPack
            && let Some(mut url) = self.server_url.clone()
        {
            if let Some(gui) = &gui {
                gui.update_state(self, LaunchState::CheckServerPack);
            }

            let start = Instant::now();

            let file = self.get_server_pack_file();
            let hash = if file.exists() {
                FileHash::Sha1(hash_helper::gen_hash_from_file(HashType::Sha1, &file)?)
            } else {
                FileHash::None
            };

            if !url.ends_with('/') {
                url.push('/');
            }

            if sync_cancel.is_cancelled() {
                return Err(ErrorType::TaskCancel);
            }

            let client = mml_net::get_work_client();
            let sha1 = client
                .get_text(&format!(
                    "{}{}{}",
                    url,
                    names::SERVER_FILE,
                    names::SHA1_DOT_EXT
                ))
                .await?;

            if matches!(hash, FileHash::None) || hash.get_sha1().unwrap() != sha1 {
                let data = client
                    .get_bytes(&format!("{}{}", url, names::SERVER_FILE))
                    .await?;

                if let Some(gui) = &gui {
                    gui.update_state(self, LaunchState::DownloadServerPack);
                }

                let obj = serialize_tools::json_from_bytes::<ServerPackObj>(&data)?;
                self.move_serverpack_to_old()?;
                if let Err(err) = self
                    .upgrade_serverpack_with(obj, sync_cancel, None, None, skip)
                    .await
                {
                    // 只为这次启动选了「跳过更新」：把旧清单挪回去，接着启动，下次再比对
                    if matches!(err, ErrorType::TaskCancel) && !cancel.is_cancelled() {
                        crate::add_game_log_item(&self.uuid, GameLog::ServerPackSkipped);
                        let _ = self.restore_serverpack_from_old();
                        return Ok(());
                    }
                    return Err(err);
                }
            }

            let time = start.elapsed();
            crate::add_game_log_item(&self.uuid, GameLog::ServerPackCheckTime(time));
            Ok(())
        } else {
            Ok(())
        }
    }
}
