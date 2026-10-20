//! 游戏实例启动参数相关生成
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    path::{Path, PathBuf},
};

use mml_auth::{AuthType, LoginObj};
use mml_base::{
    file_item::{FileHash, FileItemObj, LaterRun},
    hash_helper, serialize_tools, tools,
};
use mml_config::config_obj::GCType;
use mml_names::i18_items::error_type::{
    CoreResult, DataNotFoundData, ErrorType, PathNotExistsData,
};
use mml_names::names;
use mml_net::{mojang_api, urls};
use mml_sys::{Os, path_helper};

use crate::{
    game_launch::{AutoJoinType, GameLaunchArg},
    game_log::GameLog,
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{self, assets_path, libraries_path, version_path},
    loader::{LoaderType, liteloader::find_loader_obj},
    mojang::{
        self,
        assets_obj::AssetsObj,
        game_arg_obj::{ArgValue, Argument, GameArgObj, GameAssetIndexObj, LoggingObj},
    },
};

/// 游戏启动时的配置存储
#[derive(Default)]
pub struct GameLaunchObj {
    /// 游戏运行库
    pub game_libs: Vec<FileItemObj>,
    /// 加载器运行库
    pub loader_libs: Vec<FileItemObj>,
    /// 加载器安装运行库
    pub installer_libs: Vec<FileItemObj>,
    /// Jvm启动参数
    pub jvm_args: Vec<String>,
    /// 游戏启动参数
    pub game_args: Vec<String>,
    /// java版本
    pub java_versions: HashSet<i32>,
    /// 主类
    pub main_class: String,
    /// 本地库路径
    pub native_dir: PathBuf,
    /// 资源文件
    pub assets: GameAssetIndexObj,
    /// 游戏jar
    pub game_jar: FileItemObj,
    /// 安全log4j
    pub log4j_xml: Option<FileItemObj>,
    /// 是否使用ColorASM
    pub use_asm: bool,
}

/// V1启动Jvm参数
const V1_JVM_ARG: [&str; 3] = [
    "-Djava.library.path=${natives_directory}",
    "-cp",
    "${classpath}",
];

/// 创建V1游戏启动参数
///
/// - `game`: 游戏版本数据
///
/// # 返回值
///
/// 返回游戏启动参数列表，无参数时为空列表。
fn make_v1_game_arg(game: &GameArgObj) -> Vec<String> {
    match &game.minecraft_arguments {
        None => Vec::new(),
        Some(arg) => arg.split(' ').map(|s| s.to_string()).collect(),
    }
}

/// 创建V2游戏启动参数
///
/// - `game`: 游戏版本数据
///
/// # 返回值
///
/// 返回游戏启动参数列表，仅取无条件参数，带规则的条件参数被忽略。
fn make_v2_game_arg(game: &GameArgObj) -> Vec<String> {
    game.arguments.as_ref().map_or(Vec::new(), |args| {
        args.game
            .iter()
            .filter_map(|item| match item {
                Argument::Plain(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    })
}

/// 创建V1加载器启动参数
///
/// - `obj`: 游戏实例
/// - `game`: 游戏版本数据，部分加载器会在此基础上追加参数
///
/// # 返回值
///
/// 返回加载器附加的游戏启动参数列表。
fn make_loader_v1_game_arg(obj: &InstanceSettingObj, game: &GameArgObj) -> Vec<String> {
    match obj.loader {
        LoaderType::Forge | LoaderType::NeoForge | LoaderType::Cleanroom => {
            let loader = if obj.loader == LoaderType::NeoForge {
                obj.get_neoforge()
            } else {
                obj.get_forge()
            };

            if let Some(data) = loader
                && let Some(data1) = &data.minecraft_arguments
            {
                data1.split(' ').map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            }
        }
        LoaderType::Normal => Vec::new(),
        LoaderType::Fabric => {
            if let Some(data) = obj.get_fabric() {
                let mut args = make_v1_game_arg(game);
                args.extend(data.arguments.game.iter().cloned());
                args
            } else {
                Vec::new()
            }
        }
        LoaderType::Quilt => {
            if let Some(data) = obj.get_quilt() {
                let mut args = make_v1_game_arg(game);
                args.extend(data.arguments.game.iter().cloned());
                args
            } else {
                Vec::new()
            }
        }
        LoaderType::OptiFine => {
            let mut args = make_v1_game_arg(game);
            args.push("--tweakClass".to_string());
            args.push("optifine.OptiFineTweaker".to_string());
            args
        }
        LoaderType::Custom => obj.get_custom_loader_game_args(),
        LoaderType::LiteLoader => {
            if let Some(data) = version_path::get_liteloader(&obj.version)
                && let Some(loader) =
                    find_loader_obj(&data, obj.loader_version.as_deref().unwrap_or(""))
            {
                let mut args = make_v1_game_arg(game);
                args.push("--tweakClass".to_string());
                args.push(loader.tweak_class.clone());
                args
            } else {
                Vec::new()
            }
        }
    }
}

/// 创建V2加载器启动参数
///
/// - `obj`: 游戏实例
///
/// # 返回值
///
/// 返回加载器附加的游戏启动参数列表。
fn make_loader_v2_game_arg(obj: &InstanceSettingObj) -> Vec<String> {
    match obj.loader {
        LoaderType::Forge | LoaderType::NeoForge | LoaderType::Cleanroom => {
            let loader = if obj.loader == LoaderType::NeoForge {
                obj.get_neoforge()
            } else {
                obj.get_forge()
            };

            loader
                .and_then(|data| data.arguments.as_ref().map(|a| a.game.clone()))
                .unwrap_or_default()
        }
        LoaderType::Normal => Vec::new(),
        LoaderType::Fabric => obj
            .get_fabric()
            .map(|data| data.arguments.game.clone())
            .unwrap_or_default(),
        LoaderType::Quilt => obj
            .get_quilt()
            .map(|data| data.arguments.game.clone())
            .unwrap_or_default(),
        LoaderType::OptiFine => vec![
            "--tweakClass".to_string(),
            "optifine.OptiFineTweaker".to_string(),
        ],
        LoaderType::Custom => obj.get_custom_loader_game_args(),
        // 1.13+ 无 LiteLoader，V2 分支走不到
        LoaderType::LiteLoader => Vec::new(),
    }
}

/// 创建V2游戏Jvm参数
///
/// - `game`: 游戏版本数据
///
/// # 返回值
///
/// 返回Jvm启动参数列表，条件参数按规则过滤后再展开。
fn make_v2_jvm_arg(game: &GameArgObj) -> Vec<String> {
    let Some(data) = &game.arguments else {
        return Vec::new();
    };
    let mut args = Vec::new();
    for item in &data.jvm {
        match item {
            Argument::Plain(str) => args.push(str.clone()),
            Argument::Conditional(obj) => {
                if !mojang::check_allow(&obj.rules) {
                    continue;
                }
                match &obj.value {
                    ArgValue::Single(str) => args.push(str.clone()),
                    ArgValue::Multi(items) => args.extend(items.iter().cloned()),
                }
            }
        }
    }
    args
}

/// 创建加载器Jvm参数
///
/// - `v2`: 是否为V2版本
/// - `obj`: 游戏实例
///
/// # 返回值
///
/// 返回加载器附加的Jvm启动参数列表。
async fn make_loader_jvm_arg(v2: bool, obj: &InstanceSettingObj) -> Vec<String> {
    match obj.loader {
        LoaderType::Normal => Vec::new(),
        LoaderType::Forge | LoaderType::NeoForge => {
            if !v2 {
                return Vec::new();
            }
            let is_neoforge = obj.loader == LoaderType::NeoForge;
            let mut list = vec![
                format!(
                    "-Dforgewrapper.librariesDir={}",
                    libraries_path::get_lib_dir().display()
                ),
                format!(
                    "-Dforgewrapper.installer={}",
                    if is_neoforge {
                        obj.build_neoforge_installer(false).await
                    } else {
                        obj.build_forge_installer(false).await
                    }
                    .file
                    .display()
                ),
                format!(
                    "-Dforgewrapper.minecraft={}",
                    libraries_path::get_game_file(&obj.version).display()
                ),
            ];
            let loader_obj = if is_neoforge {
                obj.get_neoforge()
            } else {
                obj.get_forge()
            };
            if let Some(args) = loader_obj.as_ref().and_then(|o| o.arguments.as_ref()) {
                list.extend(args.jvm.clone());
            }
            list
        }
        LoaderType::Fabric => obj
            .get_fabric()
            .map(|f| f.arguments.jvm.clone())
            .unwrap_or_default(),
        LoaderType::Quilt => Vec::new(),
        // Cleanroom 是 1.12.2 的 V1 加载器，不需要 ForgeWrapper 那套 JVM 参数
        LoaderType::Cleanroom => Vec::new(),
        LoaderType::OptiFine => {
            let mut list = vec![
                format!(
                    "-Dlibdir={}",
                    libraries_path::get_lib_dir().to_string_lossy()
                ),
                format!("-Dgamecore={}", obj.get_game_file().to_string_lossy()),
                format!("-Doptifine={}", obj.get_optifine_file().to_string_lossy()),
            ];
            if v2 {
                list.extend([
                    "--add-opens".to_string(),
                    "java.base/java.lang=ALL-UNNAMED".to_string(),
                    "--add-opens".to_string(),
                    "java.base/java.util=ALL-UNNAMED".to_string(),
                    "--add-opens".to_string(),
                    "java.base/java.net=ALL-UNNAMED".to_string(),
                    "--add-opens".to_string(),
                    "java.base/jdk.internal.loader=ALL-UNNAMED".to_string(),
                ]);
            }
            list
        }
        LoaderType::Custom => obj.get_custom_loader_game_args(),
        LoaderType::LiteLoader => Vec::new(),
    }
}

impl InstanceSettingObj {
    /// 创建游戏启动参数
    ///
    /// - `auto`: 自动加入的目标，存档或服务器
    ///
    /// # 返回值
    ///
    /// 返回游戏启动参数列表，含窗口尺寸、代理、自动加入与自定义参数。
    pub fn make_game_arg(&self, auto: &AutoJoinType) -> Vec<String> {
        let mut args = Vec::new();

        let config = &mml_config::read_config().window;

        let full_screen = self
            .window
            .as_ref()
            .and_then(|w| w.full_screen)
            .or(config.full_screen)
            .unwrap_or(false);
        let width = self
            .window
            .as_ref()
            .and_then(|w| w.width)
            .or(config.width)
            .unwrap_or(0);
        let height = self
            .window
            .as_ref()
            .and_then(|w| w.height)
            .or(config.height)
            .unwrap_or(0);

        if full_screen {
            args.push("--fullscreen".to_string());
        } else {
            if width > 0 {
                args.push("--width".to_string());
                args.push(width.to_string());
            }
            if height > 0 {
                args.push("--height".to_string());
                args.push(height.to_string());
            }
        }

        match auto {
            AutoJoinType::None => {}
            AutoJoinType::Save(save_obj) => {
                // 只有 1.20 起有 quick play；它认的是**存档目录名**（level id），
                // 不是 level.dat 里的显示名。低版本没有等价参数，只能不传
                if self.is_game_version_120()
                    && let Some(dir) = save_obj.path.file_name()
                {
                    args.push("--quickPlaySingleplayer".to_string());
                    args.push(dir.to_string_lossy().to_string());
                }
            }
            AutoJoinType::Server(server_obj) => {
                if let Some(ip) = &server_obj.ip
                    && !ip.is_empty()
                {
                    let port = server_obj.port.unwrap_or(25565);
                    if self.is_game_version_120() {
                        args.push("--quickPlayMultiplayer".to_string());
                        args.push(format!("{ip}:{port}"));
                    } else {
                        args.push("--server".to_string());
                        args.push(ip.clone());
                        args.push("--port".to_string());
                        args.push(port.to_string());
                    }
                }
            }
        }

        if let Some(proxy) = &self.proxy_host {
            if let Some(ip) = &proxy.ip {
                args.push("--proxyHost".to_string());
                args.push(ip.clone());
            }
            if let Some(port) = &proxy.port {
                args.push("--proxyPort".to_string());
                args.push(port.to_string());
            }
            if let Some(user) = &proxy.user {
                args.push("--proxyUser".to_string());
                args.push(user.clone());
            }
            if let Some(password) = &proxy.password {
                args.push("--proxyPass".to_string());
                args.push(password.clone());
            }
        }

        if let Some(arg) = &self.jvm_arg
            && let Some(arg) = &arg.game_args
        {
            args.extend(arg.split('\n').map(|s| s.to_string()));
        }

        args
    }

    /// 创建Jvm参数
    ///
    /// - `auth`: 登陆使用的账户
    /// - `java`: 使用的JAVA主版本号
    /// - `mixin`: 外部注入使用的端口号
    ///
    /// # 返回值
    ///
    /// 返回Jvm启动参数列表，刷新失败且用户拒绝离线模式返回对应错误。
    async fn make_jvm_arg(
        &self,
        auth: &LoginObj,
        java: u8,
        mixin: Option<u16>,
    ) -> CoreResult<Vec<String>> {
        let config = &mml_config::read_config().jvm_arg;

        let jvm_args = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.jvm_args.as_deref())
            .or(config.jvm_args.as_deref())
            .unwrap_or("")
            .to_string();
        let mut gc = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.gc_mode)
            .or(config.gc_mode)
            .unwrap_or(GCType::Auto);
        let max = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.max_memory)
            .or(config.max_memory)
            .unwrap_or(0);
        let min = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.min_memory)
            .or(config.min_memory)
            .unwrap_or(0);
        let colorasm = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.colorasm)
            .or(config.colorasm)
            .unwrap_or(false);

        let mut args = Vec::new();

        if colorasm && let Some(port) = mixin {
            args.push(format!("-Dcolormc.mixin.port={}", port));
            args.push(format!("-Dcolormc.mixin.uuid={}", self.uuid));
            args.push(format!(
                "-javaagent:{}",
                launcher_path::get_colorasm().file.to_string_lossy()
            ));
        }

        if gc == GCType::Auto {
            if java >= 21 {
                gc = GCType::ZGC;
            } else {
                gc = GCType::G1GC;
            }
        }

        if gc == GCType::ZGC {
            args.extend(names::GCZGC.map(|item| item.to_string()));
        } else if gc == GCType::G1GC {
            args.extend(names::G1GC.map(|item| item.to_string()));
        }

        if min > 0 {
            args.push(format!("-Xms{min}m"));
        }

        if max > 0 {
            args.push(format!("-Xmx{max}m"));
        }

        // 空行必须滤掉：空参数会被 java 当成主类名，直接"找不到或无法加载主类"起不来。
        // 自定义参数是从文本框整段贴进来的，结尾带一个换行太常见了
        args.extend(
            jvm_args
                .split('\n')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .map(|item| item.to_string()),
        );

        // 统一处理外置登录认证参数
        let auth_key: Option<String> = match &auth.auth_type {
            AuthType::Nide8 => {
                args.push(format!(
                    "-javaagent:{}={}",
                    libraries_path::get_nide8_file().unwrap().to_string_lossy(),
                    auth.text1.as_ref().unwrap()
                ));
                None
            }
            AuthType::AuthlibInjector => {
                let key = auth.get_authlib_key().await?;
                args.push(format!(
                    "-javaagent:{}={}",
                    libraries_path::get_authlib_file()
                        .unwrap()
                        .to_string_lossy(),
                    auth.text1.as_ref().unwrap(),
                ));
                Some(key)
            }
            AuthType::LittleSkin | AuthType::SelfLittleSkin => {
                let key = auth.get_littleskin_key().await?;
                let endpoint = if matches!(auth.auth_type, AuthType::LittleSkin) {
                    format!("{}api/yggdrasil", urls::LITTLE_SKIN_URL)
                } else {
                    auth.text1.as_ref().unwrap().clone()
                };
                args.push(format!(
                    "-javaagent:{}={}",
                    libraries_path::get_authlib_file()
                        .unwrap()
                        .to_string_lossy(),
                    endpoint,
                ));
                Some(key)
            }
            _ => None,
        };

        if let Some(key) = auth_key {
            args.push(format!(
                "-Dauthlibinjector.yggdrasil.prefetched={}",
                hash_helper::gen_base64(&key)
            ));
            args.push("-Dauthlibinjector.side=client".to_string());
        }

        args.push(format!(
            "-Dcolormc.dir={}",
            mml_base::get_base_dir().to_string_lossy()
        ));
        args.push(format!("-Dcolormc.game.uuid={}", self.uuid.to_string()));
        args.push(format!("-Dcolormc.game.name={}", self.name));
        args.push(format!("-Dcolormc.game.version={}", self.version));
        args.push(format!(
            "-Dcolormc.game.dir={}",
            self.get_game_path().to_string_lossy()
        ));

        Ok(args)
    }

    /// 替换启动参数中的占位符
    ///
    /// - `auth`: 使用的账户
    /// - `args`: 需要替换的参数列表
    /// - `classpath`: 运行库列表
    /// - `native`: 本地库路径
    /// - `lang`: 启动时的语言
    /// - `assets`: 资源文件
    fn replace_all_value(
        &self,
        auth: &LoginObj,
        args: &mut Vec<String>,
        classpath: &str,
        native: &Path,
        lang: &Option<String>,
        assets: &Option<String>,
    ) {
        let assets_path = assets_path::get_assets_dir();
        let game_dir = self.get_game_path();
        let assets_index = assets.clone().unwrap_or_else(|| "legacy".to_string());

        let version_name = match self.loader {
            LoaderType::Normal => self.version.clone(),
            LoaderType::Custom => "custom".to_string(),
            _ => format!(
                "{}-{}-{}",
                self.loader.id(),
                self.version,
                self.loader_version.as_ref().unwrap()
            ),
        };

        let sep = if mml_sys::get_system_info().os == Os::Windows {
            ';'
        } else {
            ':'
        };

        let token = if auth.access_token.is_empty() {
            "0".to_string()
        } else {
            auth.access_token.clone()
        };

        let user_properties = lang
            .as_ref()
            .map(|l| format!("{{\"language\":\"{l}\"}}"))
            .unwrap_or_else(|| "{}".to_string());

        let user_type = if auth.auth_type == AuthType::OAuth {
            "msa"
        } else {
            "mojang"
        };

        let mut map = HashMap::new();
        // 键必须带上 `${}` 里那个 `$`：版本 json 原文就是 `${auth_player_name}`，
        // 少了 `$` 只会替换花括号那一段，替换完留下一个裸 `$`（实测会拼出
        // `-cp $E:\...\client.jar`、`--username $`，游戏直接起不来）
        map.insert("${auth_player_name}", auth.user_name.clone());
        map.insert("${version_name}", version_name);
        map.insert("${game_directory}", game_dir.to_string_lossy().to_string());
        map.insert("${assets_root}", assets_path.to_string_lossy().to_string());
        map.insert("${assets_index_name}", assets_index);
        map.insert("${auth_uuid}", auth.uuid.clone());
        map.insert("${auth_access_token}", token);
        map.insert("${user_properties}", user_properties);
        map.insert("${user_type}", user_type.to_string());
        map.insert("${version_type}", self.get_version_type());
        map.insert("${natives_directory}", native.to_string_lossy().to_string());
        map.insert(
            "${library_directory}",
            libraries_path::get_lib_dir().to_string_lossy().to_string(),
        );
        map.insert("${classpath_separator}", sep.to_string());
        map.insert("${launcher_name}", names::MML.to_string());
        map.insert("${launcher_version}", mml_names::VERSION.clone());
        map.insert("${classpath}", classpath.to_string());
        // 这两个是微软账户的客户端 id 与 xuid，离线 / 外置账户没有对应值，置空即可 ——
        // 留着 `${clientid}` 字面量会被游戏当成真的 id
        map.insert("${clientid}", String::new());
        map.insert("${auth_xuid}", String::new());

        for index in 0..args.len() {
            for (key, value) in map.iter() {
                args[index] = args[index].replace(key, value);
            }
        }
    }

    /// 创建启动参数
    ///
    /// - `arg`: 启动配置，含账户与语言
    /// - `obj`: 已生成的启动内容
    /// - `check`: 是否校验运行库文件存在，缺失的将被跳过
    ///
    /// # 返回值
    ///
    /// 返回完整命令行参数，依次为Jvm参数、主类与游戏参数。
    pub fn make_run_arg(
        &self,
        arg: &GameLaunchArg,
        obj: &mut GameLaunchObj,
        check: bool,
    ) -> Vec<String> {
        let sep = if mml_sys::get_system_info().os == Os::Windows {
            ';'
        } else {
            ':'
        };

        if obj.use_asm {
            obj.game_libs.push(launcher_path::get_colorasm());
        }

        let mut libs = self.get_libs(obj);
        if let Some(jvm) = &self.advance_jvm
            && let Some(classpath) = &jvm.class_path
            && !classpath.is_empty()
        {
            let dir1 = self.get_game_path();
            let dir2 = self.get_base_path();

            for item in classpath.split(';') {
                let path = item
                    .replace(names::ARG_GAME_NAME, &self.name)
                    .replace(names::ARG_GAME_UUID, &self.uuid.to_string())
                    .replace(names::ARG_GAME_DIR, &dir1.to_string_lossy())
                    .replace(names::ARG_GAME_BASE_DIR, &dir2.to_string_lossy())
                    .replace(
                        names::ARG_LAUNCHER_DIR,
                        &mml_base::get_base_dir().to_string_lossy(),
                    );
                if let Ok(path) = soft_canonicalize::soft_canonicalize(path.trim()) {
                    let path = strip_verbatim(path);
                    // 存在即可：这里既可能是 jar，也可能是一个类目录（advance_jvm 的 ClassPath
                    // 常被填成编译输出目录），只认 is_file 会把目录整条丢掉
                    if !check || path.exists() {
                        libs.push(path);
                    }
                }
            }
        }

        let mut classpath_parts: Vec<String> = Vec::with_capacity(libs.len());
        for item in &libs {
            // 存在即可，别只认文件：类路径里既有 jar，也有目录（编译输出目录、资源目录）
            if !check || item.exists() {
                // 统一成平台分隔符：maven 坐标拼出来的那段是 `/`，与 Windows 的 `\` 混在同一条
                // 路径里，classpath 与日志都不好读
                let text = item.to_string_lossy().replace('/', std::path::MAIN_SEPARATOR_STR);
                crate::add_game_log_item(&self.uuid, GameLog::RuntimeLib(PathBuf::from(&text)));
                classpath_parts.push(text);
            }
        }
        let classpath = classpath_parts.join(&sep.to_string());

        let mut args = Vec::new();
        self.replace_all_value(
            &arg.auth,
            &mut obj.jvm_args,
            &classpath,
            &obj.native_dir,
            &arg.lang,
            &Some(obj.assets.id.clone()),
        );
        self.replace_all_value(
            &arg.auth,
            &mut obj.game_args,
            &classpath,
            &obj.native_dir,
            &arg.lang,
            &Some(obj.assets.id.clone()),
        );

        args.extend(obj.jvm_args.clone());
        args.push(obj.main_class.clone());
        args.extend(obj.game_args.clone());

        args
    }

    /// 获取启动主类
    ///
    /// # 返回值
    ///
    /// 返回主类名，游戏版本不存在返回对应错误。
    fn make_mainclass(&self) -> CoreResult<String> {
        if let Some(arg) = &self.advance_jvm
            && let Some(main) = &arg.main_class
            && !main.is_empty()
        {
            return Ok(main.clone());
        }

        let version = version_path::get_version(&self.version)?;
        let v2 = version.is_game_version_v2();

        Ok(match self.loader {
            LoaderType::Normal => version.main_class.clone(),
            LoaderType::LiteLoader => "net.minecraft.launchwrapper.Launch".to_string(),
            LoaderType::Forge | LoaderType::NeoForge => {
                if v2 {
                    "io.github.zekerzhayard.forgewrapper.installer.Main".to_string()
                } else if self.loader == LoaderType::NeoForge {
                    self.get_neoforge().unwrap().main_class.clone()
                } else {
                    self.get_forge().unwrap().main_class.clone()
                }
            }
            LoaderType::Cleanroom => self.get_forge().unwrap().main_class.clone(),
            LoaderType::Fabric => self.get_fabric().unwrap().main_class.clone(),
            LoaderType::Quilt => self.get_quilt().unwrap().main_class.clone(),
            LoaderType::OptiFine => "com.coloryr.optifinewrapper.OptifineWrapper".to_string(),
            LoaderType::Custom => self.get_custom_loader_mainclass(),
        })
    }

    /// 替换参数中的占位符
    ///
    /// - `jvm`: 使用的JAVA路径
    /// - `arg`: JVM参数列表，用于替换 `{java_args}` 类占位符
    /// - `item`: 待替换的参数
    ///
    /// # 返回值
    ///
    /// 返回替换后的参数。
    pub fn replace_arg(&self, jvm: &Path, arg: &Vec<String>, item: &str) -> String {
        item.replace(names::ARG_GAME_NAME, &self.name)
            .replace(names::ARG_GAME_UUID, &self.uuid.to_string())
            .replace(names::ARG_GAME_DIR, &self.get_game_path().to_string_lossy())
            .replace(
                names::ARG_GAME_BASE_DIR,
                &self.get_game_path().to_string_lossy(),
            )
            .replace(names::ARG_JAVA_LOCAL, &jvm.to_string_lossy())
            .replace(names::ARG_JAVA_ARG, &tools::build_vec_string(arg))
    }

    /// 创建常规启动内容
    ///
    /// - `arg`: 启动配置
    /// - `obj`: 待填充的启动内容
    /// - `game_args`: 启动器自定义游戏参数
    ///
    /// # 返回值
    ///
    /// 返回是否成功，游戏版本不存在或运行库路径不存在返回对应错误。
    async fn make_normal_arg(
        &self,
        arg: &GameLaunchArg,
        obj: &mut GameLaunchObj,
        game_args: Vec<String>,
    ) -> CoreResult<()> {
        let game = self.check_version_update().await?;
        let v2 = game.is_game_version_v2();

        // 老版本清单没有 javaVersion 字段（如 1.6.4），什么都不插，
        // 由组件选择处统一兜底到 Java 8
        if let Some(java_ver) = &game.java_version {
            obj.java_versions.insert(java_ver.major_version);
        }

        // 处理运行库
        {
            let native_dir = obj.native_dir.clone();

            // 构建游戏核心jar
            obj.game_jar = game.build_game_item();

            // 处理原版运行库
            let skip_game_libs = self.loader == LoaderType::Custom
                && self.custom_loader.as_ref().is_some_and(|c| c.remove_lib);

            if !skip_game_libs {
                // 传实例：本地库（MMC 的 MMC-hint: local 与实例目录回退）要靠它找到
                let libs = game
                    .build_game_libraries(&native_dir, crate::get_instance(&self.uuid))
                    .await;
                for item in &libs {
                    // Cleanroom 的 MMC 补丁用空 jar 顶掉原版 LWJGL 2，这里直接跳过；
                    // 其余原版库要保留：Cleanroom 清单并不覆盖全部，缺了 ICU / patchy
                    // 之类会连渲染初始化都走不完
                    if self.loader == LoaderType::Cleanroom
                        && (item.name.starts_with("org.lwjgl.lwjgl:")
                            || !matches!(item.later, LaterRun::None))
                    {
                        continue;
                    }
                    do_lib_item(item, &mut obj.game_libs)?;
                }
            }

            // 根据加载器处理
            let mut loader_libs: Option<Vec<FileItemObj>> = None;
            let mut install_libs: Option<Vec<FileItemObj>> = None;

            match self.loader {
                LoaderType::Forge | LoaderType::NeoForge | LoaderType::Cleanroom => {
                    let res = self.get_forge_libs().await?;
                    loader_libs = Some(res.loaders);
                    install_libs = Some(res.installs);

                    // Cleanroom 的清单声明要 Java 25：覆盖原版 1.12.2 的 Java 8，
                    // 否则自动选择会按最低版本挑到 8
                    if self.loader == LoaderType::Cleanroom
                        && let Some(java) = self
                            .get_forge()
                            .and_then(|loader| loader.java_version.as_ref().map(|j| j.major_version))
                    {
                        obj.java_versions.clear();
                        obj.java_versions.insert(java);
                    }

                    if v2 {
                        launcher_path::ready_forge_wrapper()?;
                        if let Some(ref mut loader) = loader_libs {
                            loader.push(launcher_path::get_forge_wrapper());
                        }
                    }
                }
                LoaderType::Fabric => {
                    loader_libs = Some(self.get_fabric_libs().await?);
                }
                LoaderType::Quilt => {
                    loader_libs = Some(self.get_quilt_libs().await?);
                }
                LoaderType::OptiFine => {
                    loader_libs = Some(self.get_optifine_libs().await?);
                    launcher_path::ready_optifine_wrapper()?;
                    if let Some(ref mut loader) = loader_libs {
                        loader.push(launcher_path::get_optifine_wrapper());
                    }
                }
                LoaderType::Custom => {
                    let loader_file = self.get_loader_file();
                    if !loader_file.exists() {
                        return Err(ErrorType::PathNotExists(PathNotExistsData {
                            path: loader_file.clone(),
                        }));
                    }
                    let res = self.decode_loader_jar_with_path(&loader_file).await?;
                    loader_libs = Some(res.libs);
                }
                LoaderType::Normal => {}
                LoaderType::LiteLoader => {
                    loader_libs = Some(self.get_liteloader_libs().await?);
                }
            }

            if let Some(loader) = loader_libs {
                for item in &loader {
                    if !item.file.as_os_str().is_empty() {
                        obj.loader_libs.push(item.clone());
                    }
                }
            }
            if let Some(install) = install_libs {
                for item in &install {
                    if !item.file.as_os_str().is_empty() {
                        obj.installer_libs.push(item.clone());
                    }
                }
            }
        }

        // JVM参数
        let remove_jvm = self
            .jvm_arg
            .as_ref()
            .is_some_and(|j| j.remove_jvm_arg.unwrap_or(false));

        if !remove_jvm {
            if v2 {
                obj.jvm_args.extend(make_v2_jvm_arg(&game));
            } else {
                obj.jvm_args
                    .extend(V1_JVM_ARG.iter().map(|s| s.to_string()));
            }
            obj.jvm_args.extend(make_loader_jvm_arg(v2, &self).await);
        }

        // 创建启动器自定义JVM参数
        let java_version = obj.java_versions.iter().next().copied().unwrap_or(8);
        let jvm_args = self
            .make_jvm_arg(&arg.auth, java_version as u8, arg.mixin)
            .await?;

        obj.jvm_args.extend(jvm_args);

        // 游戏参数
        let remove_game = self
            .jvm_arg
            .as_ref()
            .is_some_and(|j| j.remove_game_arg.unwrap_or(false));

        if !remove_game {
            if v2 {
                obj.game_args.extend(make_v2_game_arg(&game));
                obj.game_args.extend(make_loader_v2_game_arg(&self));
            } else if self.loader != LoaderType::Normal {
                obj.game_args.extend(make_loader_v1_game_arg(&self, &game));
            } else {
                obj.game_args.extend(make_v1_game_arg(&game));
            }
        }

        obj.game_args.extend(game_args);

        // 资源文件
        if let Some(asset_index) = &game.asset_index {
            let assets = assets_path::get_index(asset_index);
            if assets.is_err() {
                let data = mojang_api::get_assets(&asset_index.url).await?;
                let assets_obj: AssetsObj = serialize_tools::json_from_bytes(&data)?;
                if assets_obj.objects.is_empty() {
                    return Err(ErrorType::DataNotFound(DataNotFoundData::Version(
                        asset_index.id.clone(),
                    )));
                }
                assets_path::add_index(&game, &mut Cursor::new(data));
            }
            obj.assets = asset_index.clone();
        }

        obj.main_class = self.make_mainclass()?;

        Ok(())
    }

    /// 创建自定义json启动内容，走MMC整合包格式
    ///
    /// - `arg`: 启动配置
    /// - `obj`: 待填充的启动内容
    /// - `game_args`: 启动器自定义游戏参数
    ///
    /// # 返回值
    ///
    /// 返回是否成功，资源文件缺失返回对应错误。
    async fn make_custom_arg(
        &self,
        arg: &GameLaunchArg,
        obj: &mut GameLaunchObj,
        game_args: Vec<String>,
    ) -> CoreResult<()> {
        // 运行库三层叠加，后面的覆盖前面的：原版 → 加载器 → patch。
        // get_libs 的去重是"后插入者胜"，所以顺序必须就是这个顺序，
        // patch 的库留在下面 custom 循环里最后加。
        // 原版这份先按实例版本号补齐：patch 通常不带客户端下载地址，
        // 不补会缺 net.minecraft.client.main.Main。失败不拦启动，patch 也可能自带
        let vanilla = self.check_version_update().await.ok();
        if let Some(game) = &vanilla
            && obj.game_jar.file.as_os_str().is_empty()
        {
            obj.game_jar = game.build_game_item();
        }


        let mut log4j: Option<&LoggingObj> = None;
        // 按 order 升序叠加：patch 是分层覆写，主类 / 运行库 / tweak / jvm 参数全靠这个顺序定胜负。
        // read_custom_json 返回的是 HashMap，不排的话顺序随哈希走 —— 出现过主类被判成
        // net.minecraft.json（order -2）里那个原版 net.minecraft.client.main.Main 的情况
        let mut custom: Vec<_> = self.read_custom_json().into_iter().collect();
        custom.sort_by(|a, b| a.1.order.cmp(&b.1.order).then_with(|| a.0.cmp(&b.0)));
        for (_, item) in custom.iter() {
            // 安全log4j
            if item.base.logging.is_some() {
                log4j = item.base.logging.as_ref();
            }

            // 下载项
            if !item.base.downloads.client.url.is_empty() {
                if let Some(mc_version) = &item.minecraft_version {
                    let file = libraries_path::get_game_file(mc_version);
                    let source = mml_config::read_config().http.source;
                    obj.game_jar = FileItemObj {
                        name: format!("{mc_version}.jar"),
                        file,
                        url: if source == mml_config::config_obj::SourceLocal::Offical {
                            item.base.downloads.client.url.clone()
                        } else {
                            mml_net::url_helper::get_minecraft_client(
                                &item.base.downloads.client.url,
                                mc_version,
                            )
                        },
                        hash: FileHash::Sha1(item.base.downloads.client.sha1.clone()),
                        later: Default::default(),
                    };
                } else {
                    let file = libraries_path::get_game_file_with_custom(&item.base.id);
                    let mut url = item.base.downloads.client.url.clone();
                    mml_net::url_helper::change_source(&mut url);
                    obj.game_jar = FileItemObj {
                        name: format!("{}.jar", item.base.id),
                        file,
                        url,
                        hash: FileHash::Sha1(item.base.downloads.client.sha1.clone()),
                        later: Default::default(),
                    };
                }
            }

            // 资源文件
            if let Some(asset_index) = &item.base.asset_index {
                let assets = assets_path::get_index(asset_index);
                if assets.is_err() {
                    let data = mml_net::mojang_api::get_assets(&asset_index.url).await?;
                    let assets_obj: AssetsObj = serialize_tools::json_from_bytes(&data)?;
                    if assets_obj.objects.is_empty() {
                        return Err(ErrorType::DataNotFound(DataNotFoundData::Version(
                            asset_index.id.clone(),
                        )));
                    }
                    assets_path::add_index(&item.base, &mut Cursor::new(data));
                }
                obj.assets = asset_index.clone();
            }

            // 运行库
            if item.base.libraries.is_some() {
                let native_dir = obj.native_dir.clone();
                // 同上：MMC 的本地库就在实例目录的 libraries 下，Labymod 的 LabyMod-4.jar 是这种
                let libs = item
                    .base
                    .build_game_libraries(&native_dir, crate::get_instance(&self.uuid))
                    .await;
                for lib_item in &libs {
                    do_lib_item(lib_item, &mut obj.game_libs)?;
                }
            }

            // Java版本
            if let Some(java_ver) = &item.base.java_version {
                obj.java_versions.insert(java_ver.major_version);
            }

            // 主类
            if !item.base.main_class.is_empty() {
                obj.main_class = item.base.main_class.clone();
            }

            // 游戏参数 (V1)
            if item.base.minecraft_arguments.is_some() {
                obj.game_args.extend(make_v1_game_arg(&item.base));
            }

            // 启动参数 (V2)
            if item.base.arguments.is_some() {
                obj.game_args.extend(make_v2_game_arg(&item.base));
                obj.jvm_args.extend(make_v2_jvm_arg(&item.base));
            }

            // MMC特性：mainJar —— 这次使用的客户端 jar，替换语义（自定义 jar 就靠它顶掉原版）
            if !item.main_jar.name.is_empty() {
                let allow = mojang::check_allow(&item.main_jar.rules);
                if allow {
                    let file = if item.main_jar.downloads.artifact.path.is_empty() {
                        mml_net::maven_utils::version_name_to_path(&item.main_jar.name)
                    } else {
                        item.main_jar.downloads.artifact.path.clone()
                    };
                    obj.game_jar = FileItemObj {
                        name: item.main_jar.name.clone(),
                        file: libraries_path::get_lib_dir().join(&file),
                        url: mml_net::url_helper::replace_minecraft_libraries(
                            &item.main_jar.downloads.artifact.url,
                        ),
                        hash: if item.main_jar.downloads.artifact.sha1.is_empty() {
                            Default::default()
                        } else {
                            FileHash::Sha1(item.main_jar.downloads.artifact.sha1.clone())
                        },
                        later: Default::default(),
                    };
                }
            }

            // MMC特性：+mainJar —— 加号是追加语义，把它当一个库加进 classpath，不动客户端 jar
            if let Some(add) = &item.add_main_jar
                && mojang::check_allow(&add.rules)
            {
                let file = if add.downloads.artifact.path.is_empty() {
                    mml_net::maven_utils::version_name_to_path(&add.name)
                } else {
                    add.downloads.artifact.path.clone()
                };
                do_lib_item(
                    &FileItemObj {
                        name: add.name.clone(),
                        file: libraries_path::get_lib_dir().join(&file),
                        url: mml_net::url_helper::replace_minecraft_libraries(
                            &add.downloads.artifact.url,
                        ),
                        hash: if add.downloads.artifact.sha1.is_empty() {
                            Default::default()
                        } else {
                            FileHash::Sha1(add.downloads.artifact.sha1.clone())
                        },
                        later: Default::default(),
                    },
                    &mut obj.game_libs,
                )?;
            }

            // MMC特性：CompatibleJavaMajors
            if let Some(java_majors) = &item.compatible_java_majors {
                for ver in java_majors {
                    obj.java_versions.insert(*ver);
                }
            }

            // MMC特性：AddJvmArgs
            if let Some(add_jvm) = &item.add_jvm_args {
                if obj.jvm_args.is_empty() {
                    obj.jvm_args
                        .extend(V1_JVM_ARG.iter().map(|s| s.to_string()));
                }
                obj.jvm_args.extend(add_jvm.iter().cloned());
            }

            // MMC特性：AddTweakers
            if let Some(tweakers) = &item.add_tweakers {
                obj.game_args.push("--tweakClass".to_string());
                obj.game_args.extend(tweakers.iter().cloned());
            }
        }


        // 加载器层放在 patch 之后、并且只补 patch 完全没有的构件：
        // MMC 实例的加载器本来就由 patches 描述（GTNH 的 Forge 就来自 net.minecraftforge.json），
        // 先塞一遍加载器库会得到重复的 forge，还会把 patch 声明为 "Early Classpath" 的
        // lwjgl3ify-forgePatches.jar 挤到 Forge 后面 —— RFB 的早期类覆盖就失效了
        let loader_libs = match self.loader {
            LoaderType::Forge | LoaderType::NeoForge | LoaderType::Cleanroom => {
                self.get_forge_libs().await.ok().map(|res| res.loaders)
            }
            LoaderType::Fabric => self.get_fabric_libs().await.ok(),
            LoaderType::Quilt => self.get_quilt_libs().await.ok(),
            LoaderType::OptiFine => self.get_optifine_libs().await.ok(),
            _ => None,
        };
        if let Some(libs) = &loader_libs {
            for item in libs {
                let key = libraries_path::LibVersionObj::new(&item.name);
                let exists = obj.game_libs.iter().any(|it| {
                    libraries_path::LibVersionObj::new(&it.name).eq_without_version(&key)
                });
                if !exists {
                    do_lib_item(item, &mut obj.game_libs)?;
                }
            }
        }

        // 原版运行库放最后：patch 与加载器都没给的构件才用原版的。
        // 这里只补空缺、不靠 get_libs 的"后插入者胜"去覆盖 —— 实测那样会漏，
        // 同一个构件多层都带时留下的可能是原版（gson 2.2.4 盖掉了 patch 的 2.10.1）
        if let Some(game) = &vanilla {
            let native_dir = obj.native_dir.clone();
            let libs = game
                .build_game_libraries(&native_dir, crate::get_instance(&self.uuid))
                .await;
            for item in &libs {
                let key = libraries_path::LibVersionObj::new(&item.name);
                let exists = obj.game_libs.iter().any(|it| {
                    libraries_path::LibVersionObj::new(&it.name).eq_without_version(&key)
                });
                if !exists {
                    do_lib_item(item, &mut obj.game_libs)?;
                }
            }
        }

        // 如果没有任何JVM参数，使用V1默认
        if obj.jvm_args.is_empty() {
            obj.jvm_args
                .extend(V1_JVM_ARG.iter().map(|s| s.to_string()));
        }

        // 创建启动器自定义JVM参数
        let java_version = obj.java_versions.iter().next().copied().unwrap_or(8);
        let jvm_args = self
            .make_jvm_arg(&arg.auth, java_version as u8, arg.mixin)
            .await?;

        obj.jvm_args.extend(jvm_args);
        obj.game_args.extend(game_args);

        // log4j安全处理
        if let Some(logging) = log4j {
            let log4j_item = mojang::build_log4j_item(logging);
            let arg_str = logging
                .client
                .argument
                .replace("${path}", &log4j_item.file.to_string_lossy());
            obj.log4j_xml = Some(log4j_item);
            obj.jvm_args.push(arg_str);
        }

        Ok(())
    }

    /// 创建游戏完整启动内容
    ///
    /// - `arg`: 启动配置
    ///
    /// # 返回值
    ///
    /// 返回启动内容，含运行库、参数与主类；游戏版本不存在返回对应错误。
    pub async fn make_game_launch_obj(&self, arg: &GameLaunchArg) -> CoreResult<GameLaunchObj> {
        let mut obj = GameLaunchObj::default();

        // 设置本地库路径
        obj.native_dir = libraries_path::get_native_dir(Some(&self.version));

        let is_custom_json = self.custom_loader.as_ref().is_some_and(|c| c.custom_json);

        // 提前计算两个分支共用的参数
        obj.use_asm = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.colorasm)
            .unwrap_or_else(|| mml_config::read_config().jvm_arg.colorasm.unwrap_or(false));
        let game_args = self.make_game_arg(&arg.auto);

        if !is_custom_json {
            self.make_normal_arg(arg, &mut obj, game_args).await?;
        } else {
            self.make_custom_arg(arg, &mut obj, game_args).await?;
        }


        Ok(obj)
    }
}

/// 去掉 Windows 的 `\\?\` 逐字路径前缀
///
/// - `path`: 规范化后的路径
///
/// # 返回值
///
/// 返回去掉前缀的路径；非 Windows 或本来就没有前缀时原样返回
fn strip_verbatim(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    if let Some(rest) = path.to_string_lossy().strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

/// 处理单个运行库项
///
/// - `item`: 运行库项
/// - `game_libs`: 游戏运行库列表，有效项追加到其中
///
/// # 返回值
///
/// 返回是否成功，本地库解包失败返回对应错误。
fn do_lib_item(item: &FileItemObj, game_libs: &mut Vec<FileItemObj>) -> CoreResult<()> {
    if !item.file.as_os_str().is_empty() {
        game_libs.push(item.clone());
    }
    // 已经在本地的解包项顺手解一次；还没下载的不能在这里读 —— 首次安装时它必然不在，
    // 直接 open_read 会让整个检查以 FileSystemError 中止（老版本 1.7.10 / 1.12.2 都带这种
    // natives 分类库）。缺失的交给下载任务的 later：下载完再按同一份 native 目录解包
    if let LaterRun::UnpackNative(native) = &item.later
        && item.file.exists()
    {
        let reader = path_helper::open_read(&item.file)?;
        mml_downloader::later_tasks::unpack_native(native, reader)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! 启动参数里的 JVM 部分：内存 `-Xms` / `-Xmx`、GC 档与自定义 JVM 参数。
    //!
    //! 放在 crate 内是因为 `make_jvm_arg` 是私有方法，`tests/game_arg.rs` 够不着；
    //! 而公开的 `make_run_arg` 要先过版本 json 的联网校验，没法在测试里用。
    //! 全程离线：账户取默认的离线账户，只有外置登录那几档才会联网。
    //!
    //! 全局配置里的 `jvm_arg` 是实例没给值时的兜底，所以这里先清成全 `None`，
    //! 并让改配置的用例独占执行（同 `tests/game_arg.rs` 的 CONFIG_LOCK）。
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard, Once};
    use std::thread;
    use std::time::{Duration, Instant};

    use mml_auth::LoginObj;
    use mml_config::config_obj::{GCType, RunArgObj};
    use mml_names::names;
    use uuid::Uuid;

    use super::*;
    use crate::game_launch::{AutoJoinType, GameLaunchArg, GameRunObj, InstanceHandle};
    use crate::launcher::LogEncoding;
    use crate::test_support::{boot, run_log_texts};

    /// 取全局配置锁，改配置与读配置的用例串行
    fn lock_config() -> MutexGuard<'static, ()> {
        static CONFIG_LOCK: Mutex<()> = Mutex::new(());
        CONFIG_LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// 起内核各系统，并把全局 JVM 配置清成全 `None`
    ///
    /// `boot()` 不含本 crate 的 `init`，而 `-Dcolormc.*` 那段要读实例路径（`BASE_DIR`），
    /// 所以这里补上；路径都是 `OnceLock::get_or_init`，重复初始化是幂等的。
    ///
    /// # 返回值
    ///
    /// 持有配置锁，用例结束才释放
    fn setup() -> MutexGuard<'static, ()> {
        static INIT: Once = Once::new();
        let guard = lock_config();
        INIT.call_once(|| {
            let run = boot();
            crate::init(&run).unwrap();
            let mut config = mml_config::write_config();
            config.jvm_arg = RunArgObj::default();
        });
        guard
    }

    /// 造一个带指定 JVM 配置的实例
    ///
    /// - `jvm`: 实例自己的 JVM 配置，其余字段取默认值
    ///
    /// # 返回值
    ///
    /// 返回实例
    fn instance(jvm: RunArgObj) -> InstanceSettingObj {
        InstanceSettingObj {
            name: "launch-arg-test".to_string(),
            dir: "launch-arg-test".to_string(),
            version: "1.21.1".to_string(),
            jvm_arg: Some(jvm),
            ..Default::default()
        }
    }

    /// 内存：最小填 `-Xms`、最大填 `-Xmx`，且 `-Xms` 排在前面
    #[tokio::test]
    async fn jvm_arg_memory() {
        let _guard = setup();
        let obj = instance(RunArgObj {
            min_memory: Some(512),
            max_memory: Some(4096),
            // 关掉 GC，这条只盯内存
            gc_mode: Some(GCType::None),
            ..Default::default()
        });

        let args = obj.make_jvm_arg(&LoginObj::default(), 21, None).await.unwrap();

        let min = args.iter().position(|a| a == "-Xms512m").expect("应生成 -Xms512m");
        let max = args.iter().position(|a| a == "-Xmx4096m").expect("应生成 -Xmx4096m");
        assert!(min < max, "-Xms 应排在 -Xmx 之前");
    }

    /// 内存是 0 或没设：该档不生成，0 表示不限而不是 0 MB
    #[tokio::test]
    async fn jvm_arg_memory_zero_skips() {
        let _guard = setup();
        let obj = instance(RunArgObj {
            min_memory: Some(0),
            max_memory: None,
            gc_mode: Some(GCType::None),
            ..Default::default()
        });

        let args = obj.make_jvm_arg(&LoginObj::default(), 21, None).await.unwrap();

        assert!(!args.iter().any(|a| a.starts_with("-Xms")), "0 不该生成 -Xms");
        assert!(!args.iter().any(|a| a.starts_with("-Xmx")), "没设不该生成 -Xmx");
    }

    /// GC 档：自动档按 Java 版本挑，21 及以上走 ZGC、之下走 G1GC；显式指定时听指定的
    #[tokio::test]
    async fn jvm_arg_gc() {
        let _guard = setup();
        let auth = LoginObj::default();
        let auto = instance(RunArgObj {
            gc_mode: Some(GCType::Auto),
            ..Default::default()
        });

        let zgc = auto.make_jvm_arg(&auth, 21, None).await.unwrap();
        assert!(names::GCZGC.iter().all(|item| zgc.contains(&item.to_string())), "Java 21 应走 ZGC");
        assert!(!zgc.contains(&names::G1GC[0].to_string()), "自动档不该同时给 G1GC");

        let g1 = auto.make_jvm_arg(&auth, 17, None).await.unwrap();
        assert!(names::G1GC.iter().all(|item| g1.contains(&item.to_string())), "Java 17 应走 G1GC");
        assert!(!g1.contains(&names::GCZGC[0].to_string()), "低版本不该给 ZGC");

        // 显式档与自动判定不一致时按显式的来
        let forced = instance(RunArgObj {
            gc_mode: Some(GCType::G1GC),
            ..Default::default()
        });
        let forced = forced.make_jvm_arg(&auth, 21, None).await.unwrap();
        assert!(names::G1GC.iter().all(|item| forced.contains(&item.to_string())), "显式 G1GC 应压过自动档");
    }

    /// 自定义 JVM 参数：按行追加，首尾空白与空行都去掉
    #[tokio::test]
    async fn jvm_arg_custom_lines() {
        let _guard = setup();
        let obj = instance(RunArgObj {
            jvm_args: Some("  -Dfoo=1  \n\n-Dbar=2\n".to_string()),
            gc_mode: Some(GCType::None),
            ..Default::default()
        });

        let args = obj.make_jvm_arg(&LoginObj::default(), 17, None).await.unwrap();

        assert!(args.contains(&"-Dfoo=1".to_string()), "应保留自定义参数并去掉首尾空白");
        assert!(args.contains(&"-Dbar=2".to_string()));
        assert!(!args.iter().any(|a| a.is_empty()), "空行不该留成空参数");
    }

    /// 本机能跑的 Java：先 `JAVA_HOME`，再 PATH 上的 `java`
    ///
    /// 探针走源文件模式（`java 探针.java`），那是 JDK 的能力，纯 JRE 跑不了，
    /// 所以这里用 `--list-modules` 认一下有没有 `jdk.compiler`。
    ///
    /// # 返回值
    ///
    /// 可用则返回 java 可执行文件路径，没有可用 JDK 时返回 `None`
    fn find_jdk() -> Option<PathBuf> {
        let mut list = Vec::new();
        if let Ok(home) = std::env::var("JAVA_HOME") {
            let name = if cfg!(windows) { "java.exe" } else { "java" };
            list.push(PathBuf::from(home).join("bin").join(name));
        }
        list.push(PathBuf::from("java"));

        list.into_iter().find(|java| {
            Command::new(java)
                .arg("--list-modules")
                .output()
                .map(|out| {
                    out.status.success() && String::from_utf8_lossy(&out.stdout).contains("jdk.compiler")
                })
                .unwrap_or(false)
        })
    }

    /// 真实启动：命令行由启动器自己拼，进程是真的 Java、跑的是真代码
    ///
    /// 探针用源文件模式跑，打印自定义 JVM 参数与游戏参数，于是"参数是否真的传进进程"
    /// 也能一起断言。本机没有带 `jdk.compiler` 的 JDK 就跳过 —— 那种机器上启动器
    /// 本身也起不了游戏。
    #[tokio::test]
    async fn real_launch_starts_java_process() {
        let _guard = setup();
        let Some(java) = find_jdk() else {
            eprintln!("跳过真实启动用例：本机没有可用的 JDK");
            return;
        };

        // 探针：把自定义 JVM 参数与游戏参数都打出来
        let dir = mml_testutil::temp_path(format!("mml-real-launch-{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let probe = dir.join("MmlLaunchProbe.java");
        std::fs::write(
            &probe,
            r#"public class MmlLaunchProbe {
    public static void main(String[] args) {
        System.out.println("mml-probe-ok prop=" + System.getProperty("mml.probe") + " args=" + String.join(",", args));
    }
}
"#,
        )
        .unwrap();

        // 命令行：JVM 参数走 make_jvm_arg，整条走 make_run_arg，与真实启动同一入口
        let instance = instance(RunArgObj {
            min_memory: Some(256),
            max_memory: Some(512),
            // GC 档不选：本机 Java 版本不确定，GC 的两条用例已经各自断言过参数内容
            gc_mode: Some(GCType::None),
            jvm_args: Some("-Dmml.probe=1".to_string()),
            ..Default::default()
        });
        let jvm = instance.make_jvm_arg(&LoginObj::default(), 21, None).await.unwrap();

        let mut obj = GameLaunchObj {
            jvm_args: jvm,
            main_class: probe.to_string_lossy().to_string(),
            game_args: vec!["--probe-arg".to_string()],
            ..Default::default()
        };
        let launch_arg = GameLaunchArg {
            auth: LoginObj::default(),
            auto: AutoJoinType::None,
            admin: false,
            mixin: None,
            lang: None,
        };
        let args = instance.make_run_arg(&launch_arg, &mut obj, false);
        assert!(args.contains(&"-Xms256m".to_string()), "命令行里应有内存参数");
        assert!(args.contains(&probe.to_string_lossy().to_string()), "命令行里应有主类");

        // 真起进程：句柄负责读流进运行日志、tick 判退出
        let uuid = Uuid::new_v4();
        let handle = InstanceHandle::new(GameRunObj {
            uuid,
            encoding: LogEncoding::UTF8,
            auth: LoginObj::default(),
            path: dir.join("game"),
            java,
            args,
            env: Default::default(),
            admin: false,
        })
        .expect("启动 Java 进程失败");

        // 先等输出：源文件模式要先编译，给足 60 秒
        let deadline = Instant::now() + Duration::from_secs(60);
        while run_log_texts(&uuid).iter().all(|line| !line.contains("mml-probe-ok"))
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(50));
        }

        let texts = run_log_texts(&uuid);
        let line = texts
            .iter()
            .find(|line| line.contains("mml-probe-ok"))
            .unwrap_or_else(|| panic!("运行日志里应出现探针输出，实际：{texts:?}"));

        // 三件事各证明一段链路：进程起来了、自定义 JVM 参数到了 JVM、游戏参数到了进程
        assert!(line.contains("prop=1"), "自定义 JVM 参数应真的传给 JVM：{line}");
        assert!(line.contains("args=--probe-arg"), "游戏参数应真的传给进程：{line}");

        // 再等进程退出
        let deadline = Instant::now() + Duration::from_secs(30);
        while !handle.is_exit() && Instant::now() < deadline {
            handle.tick();
            thread::sleep(Duration::from_millis(50));
        }
        assert!(handle.is_exit(), "Java 进程应在 30 秒内退出");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
