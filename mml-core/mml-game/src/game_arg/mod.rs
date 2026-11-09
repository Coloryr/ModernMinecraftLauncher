//! 游戏实例启动参数相关生成
//!
//! 入口留在本文件（`make_run_arg` / `make_game_launch_obj`），其余按职责拆开：
//! `args` 拼游戏参数、`jvm` 拼 JVM 参数、`replace` 替换占位符、
//! `normal` / `custom` 是两种启动流程、`libs` 负责运行库落盘与 MMC 补丁的组件规则。
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    path::{Path, PathBuf},
};

use mml_auth::{AuthType, LoginObj};
use mml_base::{
    file_item::{FileHash, FileItemObj, LaterRun},
    hash_helper, serialize_tools,
};
use mml_config::config_obj::GCType;
use mml_names::i18_items::error_type::{
    CoreResult, DataNotFoundData, ErrorType, PathNotExistsData,
};
use mml_names::names;
use mml_net::{mojang_api, urls};
use mml_sys::{Os, path_helper};

use crate::{
    game_launch::{AutoJoinType, GameLaunchArg, MixinArg},
    game_log::GameLog,
    launcher::{custom_game_arg_obj::CustomGameArgObj, instance_setting_obj::InstanceSettingObj},
    launcher_path::{self, assets_path, libraries_path, version_path},
    loader::{LoaderType, liteloader::find_loader_obj},
    mojang::{
        self,
        assets_obj::AssetsObj,
        game_arg_obj::{ArgValue, Argument, GameArgObj, GameAssetIndexObj, LoggingObj},
    },
};

mod args;
mod custom;
mod jvm;
mod libs;
mod normal;
mod replace;
#[cfg(test)]
#[path = "../../tests/unit/game_arg_mod_tests.rs"]
mod tests;

use libs::*;

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

    /// 创建启动参数
    ///
    /// - `jvm`: 实际使用的 Java 路径，自定义参数里的 `%JAVA_LOCAL%` 用它
    /// - `arg`: 启动配置，含账户与语言
    /// - `obj`: 已生成的启动内容
    /// - `check`: 是否校验运行库文件存在，缺失的将被跳过
    ///
    /// # 返回值
    ///
    /// 返回完整命令行参数，依次为Jvm参数、主类与游戏参数。
    pub fn make_run_arg(
        &self,
        jvm: &Path,
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
        let mut extra_class_path: Vec<PathBuf> = Vec::new();
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
                        extra_class_path.push(path.clone());
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
                let text = item
                    .to_string_lossy()
                    .replace('/', std::path::MAIN_SEPARATOR_STR);
                crate::add_game_log_item(&self.uuid, GameLog::RuntimeLib(PathBuf::from(&text)));
                classpath_parts.push(text);
            }
        }
        let classpath = classpath_parts.join(&sep.to_string());

        for path in &extra_class_path {
            crate::add_game_log_item(
                &self.uuid,
                GameLog::ExtraClassPath(path.display().to_string()),
            );
        }

        let mut args = Vec::new();
        self.replace_all_value(
            &arg.auth,
            &mut obj.jvm_args,
            &classpath,
            &obj.native_dir,
            &Some(obj.assets.id.clone()),
        );
        self.replace_all_value(
            &arg.auth,
            &mut obj.game_args,
            &classpath,
            &obj.native_dir,
            &Some(obj.assets.id.clone()),
        );

        // 用户填的 JVM / 游戏参数里也能写 %GAME_BASE_DIR% 这类占位符，
        // 到这里统一换成真实路径，否则原样交给 JVM
        for item in obj.jvm_args.iter_mut().chain(obj.game_args.iter_mut()) {
            *item = self.replace_custom_arg(jvm, item);
        }

        // 附加参数按替换后的值记录，日志里看到的就是真正交给 JVM 的那几条
        self.log_extra_args(jvm);

        args.extend(obj.jvm_args.clone());
        args.push(obj.main_class.clone());
        args.extend(obj.game_args.clone());

        args
    }

    /// 输出实例设置里的附加 JVM 参数与游戏参数
    ///
    /// 只记用户填的那几条，版本 JSON 与加载器生成的不算；占位符已替换。
    ///
    /// - `jvm`: 实际使用的 Java 路径
    fn log_extra_args(&self, jvm: &Path) {
        let config = mml_config::read_config();
        let jvm_args = self
            .jvm_arg
            .as_ref()
            .and_then(|arg| arg.jvm_args.as_deref())
            .or(config.jvm_arg.jvm_args.as_deref())
            .unwrap_or("");
        for line in jvm_args
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            crate::add_game_log_item(
                &self.uuid,
                GameLog::ExtraJvmArg(self.replace_custom_arg(jvm, line)),
            );
        }

        let game_args = self
            .jvm_arg
            .as_ref()
            .and_then(|arg| arg.game_args.as_deref())
            .unwrap_or("");
        for line in game_args
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            crate::add_game_log_item(
                &self.uuid,
                GameLog::ExtraGameArg(self.replace_custom_arg(jvm, line)),
            );
        }
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
