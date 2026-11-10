//! 游戏实例管理模块
//!
//! 本 crate 是启动器的游戏实例层，负责实例的创建、管理与启动，以及
//! 实例内各类资源，包括模组、资源包、光影包、存档、截图、结构文件等的读写。
//!
//! # 核心概念
//!
//! - **实例** — 一个可启动的 Minecraft 游戏环境，由版本、加载器与独立游戏目录组成，
//!   对应 [`launcher::instance_setting_obj::InstanceSettingObj`]，
//!   全局以 [`GameInstance`] 形式共享，即 `Arc<RwLock<...>>`
//! - **分组** — 实例的分组展示，独立存在 `group_save.json`，内存里有一份缓存
//! - **运行句柄** — 启动后的实例由 [`game_launch::InstanceHandle`] 跟踪，
//!   后台线程每秒轮询退出状态并触发 [`InstanceExit`] 事件
//! - **事件** — 实例退出 / 变更 / 运行日志 / 目录外部增删分别通过
//!   [`add_exit`] / [`add_change`] / [`add_run_log`] / [`add_watch_change`] 订阅
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`add_game`] | 添加游戏实例：导入版本、文件夹或压缩包 |
//! | [`curseforge`] | CurseForge 整合包下载安装 |
//! | [`game_arg`] | 启动参数生成 |
//! | [`game_check`] | 实例文件校验 |
//! | [`game_count`] | 启动与游戏时长统计 |
//! | [`game_export`] | 实例导出 |
//! | [`game_group`] | 分组表，独立于实例配置的 `group_save.json` |
//! | [`game_lan`] | 局域网联机 |
//! | [`game_launch`] | 游戏实例启动 |
//! | [`game_libraries`] | 实例运行库处理 |
//! | [`game_log`] | 实例日志，含运行日志与过往日志 |
//! | [`game_options`] | options.txt 配置读写 |
//! | [`game_server`] | 实例服务器管理 |
//! | [`gui_hook`] | GUI 回调钩子，用于启动与安装进度等界面交互 |
//! | `instance_event` | 实例事件：退出 / 变更 / 目录外部增删 / 运行日志 |
//! | `instance_group` | 分组查询、移动与增删的对外 API |
//! | `instance_lang` | 游戏内语言列表与语言代码匹配 |
//! | `instance_runtime` | 运行日志与运行句柄 |
//! | [`launcher`] | 实例设置与运行配置 |
//! | [`launcher_path`] | 各类目录路径管理，覆盖版本、库、资源、实例 |
//! | [`loader`] | 加载器安装，覆盖 Forge、Fabric、Quilt、OptiFine、LiteLoader、自定义 |
//! | [`modpack`] | 整合包安装框架，含 Modrinth 与 CurseForge worker |
//! | [`modrinth`] | Modrinth 整合包下载安装 |
//! | [`mojang`] | Mojang 版本数据处理，含版本清单与下载项 |
//! | [`name_check`] | 实例名 / 分组名的合法性校验 |
//! | [`other_launcher`] | 其他启动器实例导入，覆盖 HMCL、PCL、官方、MMC |
//! | [`path_watch`] | 实例目录文件监视 |
//! | [`player_skin`] | 玩家皮肤获取 |
//! | [`resource`] | 实例内资源，覆盖模组、资源包、光影包、存档、截图、结构文件、数据包与资源链接 |
//! | [`scan_game`] | 扫描游戏版本 |
//! | [`serverpack`] | 服务端整合包生成 |

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, LazyLock, RwLock},
    thread,
    time::Duration,
};

use mml_names::i18_items::error_type::CoreResult;

use uuid::Uuid;

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{
        instance_path::{self},
        version_path,
    },
};

pub mod add_game;
mod cmd_shell;
pub mod curseforge;
mod error;
pub mod game_arg;
pub mod game_check;
pub mod game_count;
pub mod game_export;
pub mod game_group;
pub mod game_lan;
pub mod game_launch;
pub mod game_libraries;
pub mod game_log;
pub mod game_motd;
pub mod game_options;
pub mod game_server;
pub mod gui_hook;
mod instance_event;
mod instance_group;
mod instance_lang;
mod instance_runtime;
pub mod launcher;
pub mod launcher_path;
pub mod loader;
pub mod modpack;
pub mod modrinth;
pub mod mojang;
pub mod name_check;
pub mod other_launcher;
pub mod path_watch;
pub mod player_skin;
pub mod resource;
pub mod scan_game;
pub mod serverpack;

pub use self::instance_event::{
    InstanceChange, InstanceExit, InstanceLog, InstanceLogType, InstanceWatchChange, add_change,
    add_exit, add_run_log, add_watch_change, remove_change, remove_exit, remove_run_log,
    remove_watch_change,
};
pub(crate) use self::instance_event::{invoke_change, invoke_exit, invoke_watch_change};
pub use self::instance_group::{
    add_group, get_group, get_group_list, get_instance_group, get_instance_order, move_group,
    move_instance, remove_group, reorder_groups,
};
pub use self::instance_lang::{InstanceLangObj, game_lang_code, get_instance_langs};
#[cfg(test)]
pub(crate) use self::instance_runtime::RUNTIME_LOGS;
pub(crate) use self::instance_runtime::{
    add_game_log, add_game_log_item, add_run_game, clear_game_log,
};
pub use self::instance_runtime::{get_running_instances, is_running, stop_game};

pub use name_check::check_name;

// 资源模块已下沉到 [`resource`]，这里按原路径再导出，既有引用不必改
pub use self::resource::{
    class_scan, data_res, game_link, game_mods, game_resourcepacks, game_saves, game_schematics,
    game_screenshots, game_shaderpacks,
};

#[cfg(test)]
#[path = "../tests/unit/lib_test_support.rs"]
mod test_support;

/// 实例共享句柄，全局以 `Arc<RwLock>` 形式持有实例设置
pub type GameInstance = Arc<RwLock<InstanceSettingObj>>;

// 分组表已拆到 [`game_group`]：分组不再存在实例配置里，也没有 GROUPS 静态表，
// 而是独立的 group_save.json，这样空分组才能存在、组内顺序也有了落点。
// 对外接口保留在下面几个函数里，委托给该模块。

/// 已登记的实例，按实例 UUID 索引
static INSTANCES: LazyLock<RwLock<HashMap<Uuid, GameInstance>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 初始化
///
/// - `dir`: 运行路径
///
/// # 返回值
///
/// 路径初始化失败时返回对应错误
pub fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    launcher_path::init(dir)
}

/// 开始加载数据
///
/// 加载版本目录、启动目录监视线程与实例退出轮询线程，
/// 并把已存在的实例目录载入实例列表。
///
/// # 返回值
///
/// 实例目录加载失败时返回对应错误
pub fn load() -> CoreResult<()> {
    version_path::load();

    // 分组表 group_save.json 放实例根目录下，必须在实例登记前读，
    // 否则 register_instance 看不到已有归属、会把它们冲回默认分组
    game_group::init(instance_path::get_instance_dir())?;

    path_watch::init_watch()?;

    thread::spawn(|| {
        loop {
            // 句柄的推进与摘除都在 instance_runtime 里，这里只把退出事件发出去
            for (uuid, code) in instance_runtime::poll_exited() {
                invoke_exit(uuid, code);
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    let mut installs = instance_path::load_instance_dir()?;

    // 分组表丢失 / 缺项时，登记次序决定组内位置：按目录创建时间从旧到新，
    // 新实例才会排在末尾，而不是随 `read_dir` 的目录名顺序插到最前面
    installs.sort_by(|a, b| {
        let ta = instance_dir_time(a);
        let tb = instance_dir_time(b);
        ta.cmp(&tb).then_with(|| a.uuid.cmp(&b.uuid))
    });

    // 旧版分组文件 instgroups.json 记的是实例目录名，得等实例读完才拿得到 uuid 映射；
    // 并入分组表后旧文件由 import_legacy 删掉。
    // 必须排在 prune 前面：prune 可能排队一次异步落盘，先并完它写的就是合并后的表
    let legacy: Vec<game_group::LegacyInstance> = installs
        .iter()
        .map(|item| game_group::LegacyInstance {
            dir: item.dir.clone(),
            name: item.name.clone(),
            uuid: item.uuid,
        })
        .collect();
    game_group::import_legacy(&instance_path::get_instance_dir(), &legacy);

    // 再把表里指向已删实例的悬空 uuid 清掉
    let loaded: Vec<Uuid> = installs.iter().map(|i| i.uuid).collect();
    game_group::prune(&loaded);

    for item in installs {
        add_to_group(item, None);
    }

    // 初始实例登记完才开监听：启动时的事件没有对应记录，开了也只会空转
    path_watch::start_watch();

    Ok(())
}

/// 实例目录的创建时间；取不到时退回修改时间，再取不到按最早处理
///
/// - `item`: 实例配置
///
/// # 返回值
///
/// 实例目录的时间戳，用于分组表缺失时的稳定登记顺序
fn instance_dir_time(item: &InstanceSettingObj) -> std::time::SystemTime {
    std::fs::metadata(item.get_base_path())
        .and_then(|meta| meta.created().or_else(|_| meta.modified()))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
}

/// 获取所有实例
///
/// # 返回值
///
/// 返回实例列表
mod instance_edit;
mod instance_registry;
mod instance_setting;

pub(crate) use self::instance_edit::remove_instance_record;
pub use self::instance_edit::{copy_instance, delete_instance, rename_instance};
pub(crate) use self::instance_registry::add_to_group;
pub use self::instance_registry::{
    get_instance, get_instance_by_name, get_instances, get_version_types, have_instance_name,
    have_instance_uuid,
};
