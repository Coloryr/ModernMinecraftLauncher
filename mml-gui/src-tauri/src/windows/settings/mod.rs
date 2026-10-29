//! 设置窗口：规格 + IPC 命令，含系统字体枚举 / 网络设置 / 启动设置
//!
//! 网络设置落在 core `config.json` 的 `HttpObj`，启动设置落在 `RunArgObj`
//! 与 Java 列表；Java 列表由 mml-jvms 持有，增删即时持久化。

use crate::log_text::LogText;
use mml_config::config_obj::{DnsObj, GameCheckObj, HttpObj, RunArgObj, WindowSettingObj};

use crate::dtos::{
    DnsSettingDto, GameCheckSettingDto, LaunchSettingDto, NetworkSettingDto, RunArgSettingDto,
    SettingsDefaultsDto, WindowSettingDto,
};

// 带命令的子模块必须 `pub(crate)`：生成的 `tauri_commands!` 从 crate 根引用它们
pub(crate) mod bg;
pub(crate) mod java;
pub(crate) mod local_files;

use self::java::java_list;

// ================= 界面字体 =================

/// 枚举系统已安装字体的族名；系统给出的列表已去重，这里再排序一次
///
/// # 返回值
///
/// 按小写字母序排好的字体族名；系统查询失败时返回空列表
#[gui_macros::command]
pub fn settings_get_system_fonts() -> Vec<String> {
    use font_kit::source::SystemSource;

    let mut families = SystemSource::new().all_families().unwrap_or_default();
    families.sort_by_key(|a| a.to_lowercase());
    families
}

// ================= 缓存目录 =================

/// 下载缓存目录的字符串形式，即运行目录下的 `cache`，见 mml-downloader
///
/// 前端把 webview 的持久化数据写在它的 `webview/` 子目录下，即 localStorage 的那份镜像
/// —— 那里是启动器自己的缓存区，与下载临时文件同一个根、由内核 `init` 保证存在。
/// 路径的唯一来源是 `mml_downloader::get_cache_path()`，**不要**在前端另拼一套。
///
/// # 返回值
///
/// 缓存目录的绝对路径，非 UTF-8 字符按替换符处理
#[gui_macros::command]
pub fn settings_get_cache_path() -> String {
    mml_downloader::get_cache_path()
        .to_string_lossy()
        .to_string()
}

// ================= 网络设置 =================

/// 读取网络与下载设置：core config.json 的 HttpObj + DnsObj + GameCheckObj
///
/// # 返回值
///
/// 网络 / 代理、DNS 与游戏文件检查三项当前配置
#[gui_macros::command]
pub fn settings_get_network() -> NetworkSettingDto {
    let config = mml_config::read_config();
    let mut dto = NetworkSettingDto::from(&config.http);
    dto.dns = DnsSettingDto::from(&config.dns);
    dto.check = GameCheckSettingDto::from(&config.check);
    dto
}

/// 保存网络与下载设置：Http / DNS / 游戏文件检查一次落盘
///
/// 保存后**立即生效**：先落盘配置，再依次做两件事：
///
/// - `mml_downloader::cancel_all()`：取消全部下载任务，别让它们在旧代理上继续跑；
/// - `mml_net::rebuild()`：重建两个 HTTP 客户端，并**中断所有在途请求**。
///   旧实现用 `OnceLock` 只建一次，改完代理必须重启才生效；被中断的请求返回
///   "请求已中断"错误，调用方按失败处理、重试即走新代理。
///
/// 这样"卡在加载器列表时改代理"能立刻生效，不用关窗重开。
///
/// - `dto`: 网络 / 代理、DNS 与游戏文件检查三项的新值，整体覆盖
#[gui_macros::command]
pub fn settings_save_network(dto: NetworkSettingDto) {
    // 记一行收到的代理配置，用来判断前端到底有没有把值送进来；代理字段是草稿，点保存才到这里
    mml_log::info(
        LogText::NetworkSettingsSaved {
            work_proxy: dto.work_proxy.to_string(),
            work_proxy_type: dto.work_proxy_type.to_string(),
            proxy_ip: dto.proxy_ip.clone(),
            proxy_port: dto.proxy_port.to_string(),
            login_proxy: dto.login_proxy.to_string(),
            login_proxy_type: dto.login_proxy_type.to_string(),
        }
        .text(),
    );

    let mut config = mml_config::write_config();
    config.http = dto.clone().into();
    config.dns = dto.dns.into();
    config.check = dto.check.into();
    drop(config);
    mml_config::save();
    mml_downloader::cancel_all();
    mml_net::rebuild();
}

// ================= 游戏启动设置 =================

/// 读取启动设置：Java 列表 + 启动参数 + 窗口设置
///
/// # 返回值
///
/// 当前 Java 列表，由 `java_list` 现取；以及启动参数与窗口设置
#[gui_macros::command]
pub fn settings_get_launch() -> LaunchSettingDto {
    let config = mml_config::read_config();
    let mut dto = LaunchSettingDto {
        java_list: Vec::new(),
        run: RunArgSettingDto::from(&config.jvm_arg),
        window: WindowSettingDto::from(&config.window),
    };
    dto.java_list = java_list();
    dto
}

/// 保存启动参数与窗口设置；Java 列表走独立命令增删
///
/// - `run`: 启动参数新值
/// - `window`: 窗口设置新值
#[gui_macros::command]
pub fn settings_save_launch(run: RunArgSettingDto, window: WindowSettingDto) {
    let mut config = mml_config::write_config();
    config.jvm_arg = run.into();
    config.window = window.into();
    drop(config);
    mml_config::save();
}

/// 设置项的出厂默认值，供界面「恢复默认」用
///
/// 形状与 `settings_get_network` / `settings_get_launch` 一致，值取自 core 的默认值：
/// - `HttpObj::default()` / `DnsObj::default()` / `GameCheckObj::default()`
/// - `RunArgObj::new()`：512 / 4096 / GC Auto / 预启动与游戏同时运行
/// - `WindowSettingObj::new()`：窗口 1280×720
///
/// 注意 `GameCheckSettingDto` 的 `derive(Default)` 是**全 false**，与 core 的
/// 「八项全 true」不同，所以这里必须走 `From<&GameCheckObj>` 而不是 DTO 自己的 Default。
///
/// # 返回值
///
/// 网络 / 代理、启动参数与窗口设置三项的出厂默认值
#[gui_macros::command]
pub fn settings_get_defaults() -> SettingsDefaultsDto {
    let mut network = NetworkSettingDto::from(&HttpObj::default());
    network.dns = DnsSettingDto::from(&DnsObj::default());
    network.check = GameCheckSettingDto::from(&GameCheckObj::default());

    SettingsDefaultsDto {
        network,
        run: RunArgSettingDto::from(&RunArgObj::new()),
        window: WindowSettingDto::from(&WindowSettingObj::new()),
    }
}
