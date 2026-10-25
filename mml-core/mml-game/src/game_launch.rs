//! 游戏实例启动相关
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI32, Ordering},
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
    i18_items::error_type::{CoreResult, ErrorType, PathNotExistsData},
    names,
};
use mml_sys::{path_helper, process_helper};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::stream_err;
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

/// ColorASM 注入所需的 IPC 连接参数
#[derive(Clone, Debug)]
pub struct MixinArg {
    /// IPC 服务端口
    pub port: u16,
    /// 连接验证密钥，启动时由 IPC 服务生成
    pub key: String,
}

/// 游戏启动所使用的参数
pub struct GameLaunchArg {
    /// 登录账户，含玩家名与令牌
    pub auth: LoginObj,
    /// 自动进入的操作
    pub auto: AutoJoinType,
    /// 是否以管理员方式启动
    pub admin: bool,
    /// ColorASM IPC 连接参数，未启用时为 `None`
    pub mixin: Option<MixinArg>,
    /// 启动时设置语言
    pub lang: Option<String>,
    /// 启动时是否跳过服务器包同步；用户在升级弹窗里选了「直接启动」时为真
    pub skip_server_pack: bool,
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
    /// 退出码；进程还没退出时是 0
    exit_code: AtomicI32,
    /// 通过已提权的辅助进程停止游戏。
    elevation_control: Option<process_helper::ElevationControl>,
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
            exit_code: AtomicI32::new(0),
            elevation_control: result.control,
        })
    }

    /// 强制结束游戏进程
    ///
    /// 结束失败时静默忽略。
    pub fn kill(&self) {
        if let Some(control) = &self.elevation_control {
            let _ = control.request_stop();
            // 保留等待句柄，由 tick 确认游戏及辅助进程真正退出。
            return;
        }
        if let Ok(mut guard) = self.process.lock() {
            if let Some(ref mut child) = *guard {
                if child.kill().is_ok() {
                    self.is_exit.store(true, Ordering::SeqCst);
                }
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
            self.exit_code
                .store(exit_status.code().unwrap_or(-1), Ordering::SeqCst);
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
        self.exit_code.load(Ordering::SeqCst)
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
                        ..Default::default()
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

        let start = Instant::now();
        let authlib = libraries_path::check_authlib(&arg.auth.auth_type).await?;
        let obj = self.make_game_launch_obj(arg).await?;
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

    /// 创建环境变量
    ///
    /// # 返回值
    ///
    /// 返回启动进程使用的环境变量表
    pub(crate) fn make_env_arg(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
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

        // 从小往大取：声明的兼容集合是一个范围，取里面最低的那个最保守。
        // 反过来的话，像 GTNH 声明 [17,21,23,24,25] 的包会挑到 25，
        // 而 Forge 1.7.10 的 FMLTweaker 要设 SecurityManager —— 24 起已被移除，必崩
        let mut list: Vec<_> = obj.java_versions.clone().into_iter().collect();
        list.sort_unstable();
        for item in list.iter() {
            // Java 8 没有跨大版本兼容：launchwrapper 在 9+ 上会
            // ClassCastException（AppClassLoader 不是 URLClassLoader），
            // 所以 8 必须精确命中，不能像 17+ 那样取更高版本
            let jvm = mml_jvms::get_java(*item, *item > 8);
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
    /// 老版本 Forge 的 FMLTweaker 要设 SecurityManager，Java 18 起默认禁止设置，得显式放行
    ///
    /// 只有 Java 24 以下认这个开关：24 起它本身是致命错误（VM 启动阶段直接失败，`System.initPhase3`）。
    /// 所以按**实际选中的** Java 版本决定，取不到版本就不加 —— 宁可让老 Forge 报它自己的错，
    /// 也不能把新 Java 弄得起不来
    ///
    /// - `obj`: 启动内容，`jvm_args` 在此补一项
    /// - `java`: 实际要用的 Java 路径
    fn allow_security_manager(obj: &mut GameLaunchObj, java: &std::path::Path) {
        if obj
            .jvm_args
            .iter()
            .any(|arg| arg.starts_with("-Djava.security.manager"))
        {
            return;
        }

        let major = mml_jvms::get_all_java()
            .iter()
            .find(|item| item.path.as_path() == java)
            .map(|item| item.major_version);

        // 只有 Java 18–23 需要显式放行：8 没有这个开关，24 起 SecurityManager 已被移除
        if major.is_some_and(|major| (18..=23).contains(&major)) {
            obj.jvm_args
                .push("-Djava.security.manager=allow".to_string());
        }
    }

    /// 按命令行文本启动一个进程，并把输出接到实例日志上
    ///
    /// - `cmd`: 命令行文本，首行是程序名，其余按空白拆成参数
    /// - `env`: 额外环境变量
    /// - `wait_run`: 是否等待执行完成
    /// - `admin`: 是否以管理员方式启动
    ///
    /// # 返回值
    ///
    /// 程序找不到或启动失败时返回错误
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

                result.child.wait().map_err(stream_err)?;

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

        InstanceSettingObj::allow_security_manager(&mut obj, java.as_ref().unwrap());
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

        // 服务器包更新：用户在升级弹窗里选了「直接启动」时这次不比对清单
        if !arg.skip_server_pack {
            self.server_pack_update(gui.clone(), cancel.clone()).await?;
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

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(gui) = &gui {
            gui.update_state(self, LaunchState::Jvm);
        }

        InstanceSettingObj::allow_security_manager(&mut obj, &jvm);

        // 创建启动参数
        // 完整命令行 = JVM 参数 + 主类 + 游戏参数；`check` 为真时跳过仍缺失的库文件。
        // 这里原来只取了 make_game_arg 的**游戏参数**，java 会把第一个 `--width`
        // 当成 JVM 选项，直接报"Unrecognized option"起不来
        let jvm_arg = self.make_run_arg(&arg, &mut obj, true);

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
                let temp = self.replace_arg(&jvm, &jvm_arg, &pre_run_cmd);
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
                self.cmd_run(&temp, &env, false, arg.admin)?;
                let time = start.elapsed();
                crate::add_game_log_item(&self.uuid, GameLog::CmdPostTime(time));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! 实例启动的进程层冒烟：真起一个进程，看输出进不进运行日志、`tick` 认不认退出。
    //!
    //! "游戏"由系统命令解释器扮演（Windows 是 `cmd /c echo`，其余是 `sh -c echo`）：
    //! 不下载任何游戏文件、不需要 Java、不联网，只验证启动器这侧的进程与日志链路。
    //!
    //! 顺序上**先等日志、再 tick**：`tick` 判定退出会置退出标志，而读流线程每行都查这个标志，
    //! 先 tick 可能让最后几行没读完就收工。
    use std::time::Duration;

    use uuid::Uuid;

    use super::*;
    use crate::test_support::boot;

    /// 假游戏输出里的标记，用来在运行日志里找那一行
    const MARK: &str = "mml-launch-ok";

    /// 假游戏：打印一行标记后退出 0
    ///
    /// # 返回值
    ///
    /// 返回可执行文件与参数
    fn fake_game() -> (PathBuf, Vec<String>) {
        #[cfg(windows)]
        {
            let exe = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
            (
                PathBuf::from(exe),
                vec!["/c".to_string(), format!("echo {MARK}")],
            )
        }
        #[cfg(not(windows))]
        {
            (
                PathBuf::from("sh"),
                vec!["-c".to_string(), format!("echo {MARK}")],
            )
        }
    }

    /// 运行日志里有没有这一行
    fn log_has(uuid: &Uuid, mark: &str) -> bool {
        crate::test_support::run_log_texts(uuid)
            .iter()
            .any(|line| line.contains(mark))
    }

    /// 起进程 → 输出进运行日志 → tick 判定退出
    #[test]
    fn handle_starts_process_and_forwards_output() {
        boot();
        let uuid = Uuid::new_v4();
        let path = mml_testutil::temp_path(format!("mml-launch-test-{}", uuid.simple()));
        let (java, args) = fake_game();

        let handle = InstanceHandle::new(GameRunObj {
            uuid,
            encoding: LogEncoding::UTF8,
            auth: LoginObj::default(),
            path: path.clone(),
            java,
            args,
            env: HashMap::new(),
            admin: false,
        })
        .expect("启动假游戏进程失败");

        // 先等输出落地：此时还没 tick，读流线程不会被退出标志打断
        let deadline = Instant::now() + Duration::from_secs(10);
        while !log_has(&uuid, MARK) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(log_has(&uuid, MARK), "运行日志里应出现假游戏的输出");

        // 再等进程退出：tick 是轮询式的，调用方得自己按节奏推
        while !handle.is_exit() && Instant::now() < deadline {
            handle.tick();
            thread::sleep(Duration::from_millis(20));
        }
        assert!(handle.is_exit(), "假游戏进程应在 10 秒内退出");

        let _ = std::fs::remove_dir_all(&path);
    }

    /// 每一行输出都要发一次日志事件
    ///
    /// GUI 的实时日志就靠这条事件流。只在"新建日志"那一次发（日志已在表里就只入库、不发），
    /// 实时日志就只剩第一行；而 `start_game` 开头会 `clear_game_log` 先把空日志建好，
    /// 于是真实启动后一行都收不到。
    #[test]
    fn every_output_line_emits_log_event() {
        boot();
        let uuid = Uuid::new_v4();

        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        let id = crate::add_run_log(move |event: &crate::InstanceLog| {
            if let crate::InstanceLogType::AddLog(item) = &event.log
                && let GameLog::Text(text) = &item.log
            {
                sink.lock().unwrap().push(text.clone());
            }
        });

        crate::clear_game_log(&uuid);
        crate::add_game_log(&uuid, "第一行");
        crate::add_game_log(&uuid, "第二行");
        crate::remove_run_log(id);

        let seen = seen.lock().unwrap();
        assert!(
            seen.contains(&"第一行".to_string()),
            "第一行应发事件，实际：{seen:?}"
        );
        assert!(
            seen.contains(&"第二行".to_string()),
            "日志已存在时也要发事件，实际：{seen:?}"
        );
    }
}
