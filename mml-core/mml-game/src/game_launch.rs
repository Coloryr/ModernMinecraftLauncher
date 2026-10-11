//! 游戏实例启动相关
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Instant,
};

use encoding_rs_io::DecodeReaderBytesBuilder;
use mml_auth::LoginObj;
use mml_base::{
    file_item::FileHash,
    hash_helper::{self, HashType},
    serialize_tools,
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorData, ErrorType, PathNotExistsData},
    names,
};
use mml_sys::{path_helper, process_helper};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    game_arg::GameLaunchObj,
    game_log::GameLog,
    game_saves::SaveObj,
    gui_hook::{LaunchGui, LaunchState, ProcessRunType},
    launcher::{
        LogEncoding, ModPackType,
        instance_setting_obj::{InstanceSettingObj, ServerObj},
    },
    launcher_path::libraries_path,
    loader::LoaderType,
    serverpack::serverpack_obj::ServerPackObj,
};

/// 启动后自动操作
pub enum AutoJoinType {
    /// 无自动操作
    None,
    /// 进入指定存档，值为存档信息
    Save(Arc<SaveObj>),
    /// 进入指定服务器，值为服务器配置
    Server(Arc<ServerObj>),
}

/// 游戏启动所使用的参数
pub struct GameLaunchArg {
    /// 登录账户，含玩家名与令牌
    pub auth: LoginObj,
    /// 自动进入的操作
    pub auto: AutoJoinType,
    /// 是否以管理员方式启动
    pub admin: bool,
    /// Mixin 调试端口，不调试时为 `None`
    pub mixin: Option<u16>,
    /// 启动时设置语言
    pub lang: Option<String>,
}

/// 游戏实例处理完成后的参数
pub struct GameRunObj {
    /// 游戏实例 uuid
    pub uuid: Uuid,
    /// 日志编码
    pub encoding: LogEncoding,
    /// 登陆的账户
    pub auth: LoginObj,
    /// 工作目录，即游戏根目录
    pub path: PathBuf,
    /// Java 可执行文件路径
    pub java: PathBuf,
    /// 启动参数
    pub args: Vec<String>,
    /// 环境变量
    pub env: HashMap<String, String>,
    /// 是否管理员方式启动
    pub admin: bool,
}

/// 启动命令的参数集合
pub struct LaunchCmd {
    /// Java 可执行文件路径
    pub java: PathBuf,
    /// 工作目录
    pub dir: PathBuf,
    /// 启动参数
    pub arg: Vec<String>,
    /// 环境变量
    pub env: HashMap<String, String>,
}

/// 游戏句柄
pub(crate) struct InstanceHandle {
    /// 游戏进程，共享所有权，用于等待退出和强制结束；提权启动时存的是
    /// 启动器进程，它在目标进程运行期间保持存活，可照样用 try_wait 检测退出
    process: Arc<Mutex<Option<Child>>>,
    /// 游戏实例 uuid
    pub uuid: Uuid,
    /// 进程是否已经退出
    is_exit: Arc<AtomicBool>,
    /// 是否为管理员启动
    pub is_admin: bool,
    /// 退出码
    exit_code: i32,
    /// 提权启动时的目标进程 PID，用于强制结束目标进程
    elevated_pid: Option<u32>,
}

impl InstanceHandle {
    /// 创建游戏句柄并启动游戏进程
    ///
    /// - `run`: 游戏运行参数
    ///
    /// # 返回值
    ///
    /// 返回游戏句柄；启动进程失败返回对应错误
    pub fn new(run: GameRunObj) -> CoreResult<Self> {
        // 创建工作目录
        path_helper::create_dir_all(&run.path)?;

        // 启动进程
        let result = process_helper::launch(run.java, run.args, run.env, run.path, run.admin)?;

        if result.is_admin {
            crate::add_game_log_item(&run.uuid, GameLog::JavaRedirect);
        }

        let process = Arc::new(Mutex::new(Some(result.child)));
        let is_exit = Arc::new(AtomicBool::new(false));

        // 读取 stdout 线程
        if let Some(reader) = result.stdout {
            let exit = is_exit.clone();
            let uuid = run.uuid;
            let encoding = run.encoding;
            let _ = thread::Builder::new()
                .name(format!("Mml Game {} StandardOutput", uuid))
                .spawn(move || {
                    read_process_stream(reader, uuid, encoding, exit);
                });
        }

        // 读取 stderr 线程
        if let Some(reader) = result.stderr {
            let exit = is_exit.clone();
            let uuid = run.uuid;
            let encoding = run.encoding;
            let _ = thread::Builder::new()
                .name(format!("Mml Game {} StandardError", uuid))
                .spawn(move || {
                    read_process_stream(reader, uuid, encoding, exit);
                });
        }

        Ok(InstanceHandle {
            process,
            uuid: run.uuid,
            is_exit,
            is_admin: result.is_admin,
            exit_code: 0,
            elevated_pid: result.pid,
        })
    }

    /// 强制结束游戏进程
    ///
    /// 结束失败时静默忽略。
    pub fn kill(&self) {
        // 提权启动时提权 helper 才是游戏进程的父进程，必须 /T 连整棵进程树一起结束，仅 Windows
        #[cfg(target_os = "windows")]
        if let Some(pid) = self.elevated_pid {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
        // 结束启动器进程，所有平台都走这条；提权启动时启动器退出会连带终止目标进程
        if let Ok(mut guard) = self.process.lock() {
            if let Some(ref mut child) = *guard {
                let _ = child.kill();
                self.is_exit.store(true, Ordering::SeqCst);
            }
        }
    }

    /// 轮询进程状态，退出时标记退出标志
    ///
    /// 已退出或已标记时忽略。
    pub fn tick(&self) {
        // 等待进程退出线程
        let process_clone = self.process.clone();
        let exit_clone = self.is_exit.clone();
        if exit_clone.load(Ordering::Relaxed) {
            return;
        }
        let status = {
            let mut guard = process_clone.lock().unwrap();
            if let Some(ref mut child) = *guard {
                child.try_wait().ok().flatten()
            } else {
                None
            }
        };
        if let Some(exit_status) = status {
            exit_clone.store(true, Ordering::SeqCst);
            let _code = exit_status.code();
            process_clone.lock().unwrap().take();
        }
    }

    /// 进程是否已经退出
    ///
    /// # 返回值
    ///
    /// 返回进程是否已退出
    pub fn is_exit(&self) -> bool {
        self.is_exit.load(Ordering::Relaxed)
    }

    /// 进程退出码
    ///
    /// # 返回值
    ///
    /// 返回进程退出码
    pub fn code(&self) -> i32 {
        self.exit_code
    }
}

/// 读取进程输出流并转发到游戏日志
///
/// - `reader`: 缓冲读取器
/// - `uuid`: 游戏实例 uuid
/// - `encoding`: 日志编码，UTF-8 或 GBK
/// - `exit_flag`: 退出标志，用于停止读取
///
/// 流关闭或收到退出标志后结束。
fn read_process_stream<R: std::io::Read>(
    reader: R,
    uuid: Uuid,
    encoding: LogEncoding,
    exit_flag: Arc<AtomicBool>,
) {
    match encoding {
        LogEncoding::UTF8 => {
            let reader = BufReader::new(reader);
            for line in reader.lines() {
                if exit_flag.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(line) = line {
                    crate::add_game_log(&uuid, &line);
                }
            }
        }
        LogEncoding::GBK => {
            let transcoder = DecodeReaderBytesBuilder::new()
                .encoding(Some(encoding_rs::GBK))
                .build(reader);
            let reader = BufReader::new(transcoder);
            for line in reader.lines() {
                if exit_flag.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(line) = line {
                    crate::add_game_log(&uuid, &line);
                }
            }
        }
    }
}

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
    async fn auth_login(
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
                        access_token: Default::default(),
                        client_token: Default::default(),
                        auth_type: Default::default(),
                        text1: Default::default(),
                        text2: Default::default(),
                        last_login: Default::default(),
                        expire_at: None,
                    };
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    /// 检查缺失的文件，自动下载，完成后返回启动配置
    ///
    /// - `arg`: 启动参数
    /// - `gui`: 启动界面回调
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 返回启动配置；检查或下载失败返回对应错误
    async fn check_game_file(
        &self,
        arg: &mut GameLaunchArg,
        gui: LaunchGui,
        cancel: CancellationToken,
    ) -> CoreResult<GameLaunchObj> {
        if let Some(gui) = &gui {
            gui.update_state(self, LaunchState::Check);
        }

        let start = Instant::now();
        let authlib = libraries_path::check_authlib(&arg.auth.auth_type).await?;
        let obj = self.make_game_launch_obj(arg).await?;
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
                let start = Instant::now();
                let res = mml_downloader::start_download_task(check).await;
                let time = start.elapsed();
                crate::add_game_log_item(&self.uuid, GameLog::DownloadFileTime(time));
                if !res {
                    Err(ErrorType::DownloadFileFail)
                } else {
                    Ok(obj)
                }
            } else {
                Ok(obj)
            }
        }
    }

    /// 创建环境变量
    ///
    /// # 返回值
    ///
    /// 返回启动进程使用的环境变量表
    fn make_env_arg(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
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

    /// 检查服务器包更新
    ///
    /// - `gui`: 启动界面回调
    /// - `cancel`: 取消令牌
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；检查或更新失败返回对应错误
    async fn server_pack_update(
        &self,
        gui: LaunchGui,
        cancel: CancellationToken,
    ) -> CoreResult<()> {
        if self.is_modpack
            && self.modpack_type != ModPackType::ServerPack
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

            if cancel.is_cancelled() {
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
                self.upgrade_serverpack(obj, cancel.clone()).await?;
            }

            let time = start.elapsed();
            crate::add_game_log_item(&self.uuid, GameLog::ServerPackCheckTime(time));
            Ok(())
        } else {
            Ok(())
        }
    }

    /// 获取启动使用的 Java
    ///
    /// - `obj`: 启动配置
    /// - `gui`: 启动界面回调
    ///
    /// # 返回值
    ///
    /// 返回 Java 路径；找不到可用的 Java 返回 `None`
    fn get_java(&self, obj: &GameLaunchObj, gui: LaunchGui) -> Option<PathBuf> {
        if let Some(data) = &self.jvm_local {
            let path = PathBuf::from(data);

            if path.exists() {
                return Some(path);
            } else {
                crate::add_game_log_item(&self.uuid, GameLog::JavaLocalRedirect);
            }
        }

        if mml_jvms::get_all_java().is_empty() {
            mml_jvms::scan_java();
        }

        if let Some(name) = &self.jvm_name {
            if let Some(data) = mml_jvms::get_java_info(&name) {
                if data.path.exists() {
                    return Some(data.path.clone());
                }
            }

            crate::add_game_log_item(&self.uuid, GameLog::JavaLocalRedirect);
        }

        let mut list: Vec<_> = obj.java_versions.clone().into_iter().collect();
        list.sort_unstable_by(|a, b| b.cmp(a));
        for item in list.iter() {
            let jvm = mml_jvms::get_java(*item, true);
            if let Some(jvm) = jvm {
                if jvm.path.exists() {
                    return Some(jvm.path.clone());
                }
            }
        }

        if let Some(gui) = &gui {
            gui.no_java(list[0]);
        }

        None
    }

    /// 执行指令
    ///
    /// - `cmd`: 命令行，按换行分隔
    /// - `env`: 环境变量
    /// - `wait_run`: 是否等待执行完成
    /// - `admin`: 是否以管理员方式启动
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；程序不存在或启动失败返回对应错误
    fn cmd_run(
        &self,
        cmd: &str,
        env: &HashMap<String, String>,
        wait_run: bool,
        admin: bool,
    ) -> CoreResult<()> {
        let args: Vec<&str> = cmd.split('\n').collect();
        let name = args[0].trim();

        let mut path = PathBuf::from(name);
        if !path.exists() {
            path = self.get_base_path().join(name);
        }

        if !path.exists() {
            path = self.get_game_path().join(name);
        }

        if !path.exists() {
            return Err(ErrorType::PathNotExists(PathNotExistsData {
                path: PathBuf::from(name),
            }));
        }

        // 收集参数，跳过第 0 个，那是程序名本身
        let proc_args: Vec<String> = args
            .iter()
            .skip(1)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        match process_helper::launch(&path, proc_args, env.clone(), &self.get_game_path(), admin) {
            Ok(mut result) => {
                // 读取 stdout 线程
                if let Some(stdout) = result.stdout {
                    let uuid1 = self.uuid;
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stdout);
                        for line in reader.lines() {
                            if let Ok(line) = line {
                                crate::add_game_log(&uuid1, &line);
                            }
                        }
                    });
                }

                // 读取 stderr 线程
                if let Some(stderr) = result.stderr {
                    let uuid2 = self.uuid;
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stderr);
                        for line in reader.lines() {
                            if let Ok(line) = line {
                                crate::add_game_log(&uuid2, &line);
                            }
                        }
                    });
                }

                // 是否与游戏同时启动
                if !wait_run {
                    return Ok(());
                }

                result.child.wait().map_err(|e| {
                    ErrorType::StreamError(ErrorData {
                        error: e.to_string(),
                    })
                })?;

                Ok(())
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
            return Err(ErrorType::InstanceVersionError);
        }

        // 有加载器的实例必须填写加载器版本
        if self.loader != LoaderType::Normal && self.loader != LoaderType::Custom {
            if self
                .loader_version
                .as_deref()
                .is_none_or(|data| data.is_empty())
            {
                return Err(ErrorType::InstanceVersionError);
            }
        }

        if self.loader == LoaderType::Custom && !self.get_loader_file().exists() {
            return Err(ErrorType::InstanceVersionError);
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

        let args = self.make_run_arg(&arg, &mut obj, false);
        let env = self.make_env_arg();

        Ok(LaunchCmd {
            java: java.unwrap(),
            dir: self.get_game_path(),
            arg: args,
            env,
        })
    }

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
            return Err(ErrorType::InstanceVersionError);
        }

        // 有加载器的实例必须填写加载器版本
        if self.loader != LoaderType::Normal && self.loader != LoaderType::Custom {
            if self
                .loader_version
                .as_deref()
                .is_none_or(|data| data.is_empty())
            {
                return Err(ErrorType::InstanceVersionError);
            }
        }

        if self.loader == LoaderType::Custom && !self.get_loader_file().exists() {
            return Err(ErrorType::InstanceVersionError);
        }

        // 登陆账户
        self.auth_login(arg, gui.clone(), cancel.clone()).await?;

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        // 服务器包更新
        self.server_pack_update(gui.clone(), cancel.clone()).await?;

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

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(gui) = &gui {
            gui.update_state(self, LaunchState::Jvm);
        }

        // 创建启动参数
        let jvm_arg = self.make_game_arg(&arg.auto);

        crate::add_game_log_item(&self.uuid, GameLog::JavaPath(jvm.clone()));

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

        let env = self.make_env_arg();

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
                let temp = self.replace_arg(&jvm.clone(), &jvm_arg, &pre_run_cmd);
                self.cmd_run(&temp, &env, prerun, arg.admin)?;
                let time = start.elapsed();
                crate::add_game_log_item(&self.uuid, GameLog::CmdPreTime(time));
            }
        }

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let start = Instant::now();
        let run = GameRunObj {
            uuid: self.uuid,
            encoding: self.encoding,
            auth: arg.auth.clone(),
            path: self.get_game_path(),
            java: jvm.clone(),
            args: jvm_arg.clone(),
            env: env.clone(),
            admin: arg.admin,
        };
        let handel = InstanceHandle::new(run)?;
        let time = start.elapsed();

        crate::add_run_game(handel);
        crate::add_game_log_item(&self.uuid, GameLog::LaunchTime(time));

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
                let temp = self.replace_arg(&jvm.clone(), &jvm_arg, &post_run_cmd);
                self.cmd_run(&temp, &env, false, arg.admin)?;
                let time = start.elapsed();
                crate::add_game_log_item(&self.uuid, GameLog::CmdPostTime(time));
            }
        }

        Ok(())
    }
}
