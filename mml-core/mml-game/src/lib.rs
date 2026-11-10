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
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, RwLock},
    thread,
    time::Duration,
};

use mml_names::{
    i18_items::error_type::{ArgEmptyData, CoreResult, ErrorType, FileSystemErrorData},
    names,
};

use mml_net::{get_work_client, input_file::InputFile};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::{
    gui_hook::{AddInstanceGui, ProgressGui},
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{instance_path::{self}, version_path},
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
pub fn get_instances() -> Vec<GameInstance> {
    let mut list = Vec::new();

    for (_, value) in INSTANCES.read().unwrap().iter() {
        list.push(value.clone());
    }

    list
}

/// 从uuid获取实例
///
/// - `uuid`: 实例 UUID
///
/// # 返回值
///
/// 返回实例；不存在返回 `None`
pub fn get_instance(uuid: &Uuid) -> Option<GameInstance> {
    let list = INSTANCES.read().unwrap();

    Some(list.get(uuid)?.clone())
}


/// 获取游戏版本类型列表，取 Mojang 版本清单的 type 字段
///
/// 返回独立 ID：release / snapshot / old_beta / old_alpha，
/// 显示名由前端 i18n 翻译。
///
/// # 返回值
///
/// 返回版本类型 ID 列表
pub fn get_version_types() -> Vec<&'static str> {
    vec!["release", "snapshot", "old_beta", "old_alpha"]
}

/// 从实例名字获取实例
///
/// - `name`: 实例名字
///
/// # 返回值
///
/// 返回实例；不存在返回 `None`
pub fn get_instance_by_name(name: &str) -> Option<GameInstance> {
    let list = INSTANCES.read().unwrap();
    let temp = list
        .iter()
        .filter(|(_, value)| value.read().unwrap().name.eq_ignore_ascii_case(name))
        .next()?;

    Some(temp.1.clone())
}

/// 是否存在这个名字的实例
///
/// - `name`: 实例名字
///
/// # 返回值
///
/// 存在返回 `true`
pub fn have_instance_name(name: &str) -> bool {
    let list = INSTANCES.read().unwrap();
    let temp = list
        .iter()
        .filter(|(_, value)| value.read().unwrap().name.eq_ignore_ascii_case(name))
        .next();
    !temp.is_none()
}

/// 是否存在这个 uuid 的实例
///
/// - `uuid`: 实例标识
///
/// # 返回值
///
/// 存在返回 `true`
pub fn have_instance_uuid(uuid: &Uuid) -> bool {
    let list = INSTANCES.read().unwrap();
    list.contains_key(uuid)
}

/// 将实例添加到分组中
///
/// UUID 为空或冲突时自动重新生成。
///
/// - `obj`: 实例配置，其中**没有**分组字段
/// - `group`: 要登记到哪个分组，给分组 uuid；`None` 表示默认分组
///
/// # 返回值
///
/// 返回加入列表后的实例句柄
fn add_to_group(obj: InstanceSettingObj, group: Option<Uuid>) -> GameInstance {
    let mut obj = obj;
    while obj.uuid.is_nil() || matches!(get_instance(&obj.uuid), Some(_)) {
        obj.uuid = Uuid::new_v4();
    }

    obj.save();
    let key = obj.uuid.clone();
    let name = obj.name.clone();
    let game: Arc<RwLock<InstanceSettingObj>> = Arc::new(RwLock::new(obj));

    INSTANCES.write().unwrap().insert(key, game.clone());
    // 名字先记进统计快照：就算这个实例马上又被删掉，统计窗口也还叫得出名字
    game_count::remember_name(&key, &name);

    // 登记到分组表；分组已独立存储，不存在的组会被自动创建
    game_group::register_instance(key, group);

    invoke_change(InstanceChange::AddInstance(key));
    game
}

/// 删除实例
///
/// - `uuid`: 实例 UUID
///
/// # 返回值
///
/// 成功返回 `Ok(())`，实例文件移入回收站；实例不存在或删除失败返回对应错误
pub fn delete_instance(uuid: &Uuid) -> CoreResult<()> {
    let instance = get_instance(uuid).ok_or(ErrorType::ArgEmpty(ArgEmptyData::UUID))?;

    // 启动器自己的删除不走外部监听，避免移除记录与删除命令各广播一次
    let _watch = path_watch::WatchPause::new();

    // 先删实例文件并移入回收站，失败则保留实例数据，避免 UI 消失但文件还在
    instance.read().unwrap().delete_files()?;

    remove_instance_record(uuid);
    Ok(())
}

/// 从实例列表移除记录，不删除文件，实例目录监视用
///
/// - `uuid`: 实例 UUID
pub(crate) fn remove_instance_record(uuid: &Uuid) {
    // 删除前把名字留一份，统计窗口里已删除的实例还得靠它显示
    if let Some(game) = get_instance(uuid) {
        let name = game.read().unwrap().name.clone();
        game_count::remember_name(uuid, &name);
    }
    INSTANCES.write().unwrap().remove(uuid);
    // 从分组表里摘掉它，但**不删空分组**：空分组是合法状态。
    // 这里以前有一句"顺手清掉空分组"，正是"空分组留不住"的原因
    game_group::forget_instance(uuid);

    invoke_change(InstanceChange::RemoveInstance(*uuid));
}

/// 给实例目录改名，遇到短暂占用时重试
///
/// 杀毒 / 索引服务会在目录刚建好时短暂持有句柄，第一次改名可能报拒绝访问或共享冲突；
/// 这类错误过几十毫秒就消失，重试几次即可，别把瞬时占用报成改名失败。
///
/// - `from`: 原目录
/// - `to`: 新目录
///
/// # 返回值
///
/// 改名成功返回 `Ok(())`；重试后仍失败返回最后一次的错误
fn rename_instance_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    const RETRY: u32 = 6;

    let mut last = None;
    for attempt in 0..RETRY {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(err) => {
                // 5 = ACCESS_DENIED，32 = SHARING_VIOLATION，33 = LOCK_VIOLATION
                let transient = matches!(err.raw_os_error(), Some(5) | Some(32) | Some(33));
                last = Some(err);
                if !transient || attempt + 1 == RETRY {
                    break;
                }
                thread::sleep(Duration::from_millis(30 * u64::from(attempt + 1)));
            }
        }
    }
    Err(last.unwrap())
}

/// 重命名实例，名字与实例目录一起改
///
/// 新名字与其他实例重复时拒绝；成功后发一次变更事件刷新前端列表。
///
/// - `uuid`: 实例 UUID
/// - `name`: 新名字
///
/// # 返回值
///
/// 成功返回 `Ok(())`；名字为空、不合法、重复或目录改名失败返回对应错误
pub fn rename_instance(uuid: &Uuid, name: &str) -> CoreResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
    }
    name_check::check_name(name)?;

    let instance = get_instance(uuid).ok_or(ErrorType::ArgEmpty(ArgEmptyData::UUID))?;

    // 新名字与别的实例重复时拒绝，自己保持原名除外
    if have_instance_name(name) && instance.read().unwrap().name != name {
        return Err(ErrorType::InstanceNameExists(name.to_string()));
    }

    let old_base = instance.read().unwrap().get_base_path();

    // 实例目录跟着改名，目录名由名字派生，改名成功后再更新名字并保存
    let new_dir = path_helper::replace_path_name(name);
    let new_base = old_base
        .parent()
        .map(|p| p.join(&new_dir))
        .unwrap_or_else(|| old_base.clone());
    if old_base != new_base && old_base.exists() {
        rename_instance_dir(&old_base, &new_base).map_err(|e| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: old_base,
                error: e.to_string(),
            })
        })?;
    }

    let mut obj = instance.write().unwrap();
    obj.name = name.to_string();
    obj.dir = new_dir;
    obj.save();

    drop(obj);
    game_count::remember_name(uuid, name);

    // 重命名不改分组归属，分组表按 uuid 记录、与名字无关，
    // 但要让前端刷新那一行，所以照旧发一次带当前分组的变更事件
    invoke_change(InstanceChange::MoveGroup(*uuid, get_instance_group(uuid)));
    Ok(())
}

/// 复制实例：整份实例目录复制到新名字下，再登记成新实例
///
/// 复制的是目录的完整内容（游戏文件、模组、存档、配置），新实例沿用源实例的分组；
/// 新实例的 uuid 重新生成，目录名由新名字派生。
///
/// - `uuid`: 源实例 UUID
/// - `name`: 新实例名，前后空白会被去掉
///
/// # 返回值
///
/// 返回新实例句柄；名字为空、不合法、重名、源实例不存在或复制失败返回对应错误
pub fn copy_instance(uuid: &Uuid, name: &str) -> CoreResult<GameInstance> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
    }
    name_check::check_name(name)?;
    if have_instance_name(name) {
        return Err(ErrorType::InstanceNameExists(name.to_string()));
    }

    let instance = get_instance(uuid).ok_or(ErrorType::ArgEmpty(ArgEmptyData::UUID))?;
    let (src, mut obj) = {
        let src = instance.read().unwrap();
        (src.get_base_path(), src.clone())
    };

    let dir = path_helper::replace_path_name(name);
    let dest = instance_path::get_instance_dir().join(&dir);
    if dest.exists() {
        return Err(ErrorType::InstanceNameExists(name.to_string()));
    }

    // 监视一直停到新实例登记完成：中途恢复的话，复制出来的目录会被当成外部导入，
    // 带着源实例的 uuid 再登记一遍
    let _watch = path_watch::WatchPause::new();
    let copied = path_helper::copy_dir(&src, &dest);
    if let Err(e) = copied {
        // 失败留下的半份目录要清掉：里面已经有一份 game.json，下次启动会被当成实例导入
        let _ = std::fs::remove_dir_all(&dest);
        return Err(e);
    }

    obj.uuid = Uuid::new_v4();
    obj.name = name.to_string();
    obj.dir = dir;
    obj.save();

    let game = add_to_group(obj, get_instance_group(uuid));
    Ok(game)
}

/// 按图标字节判断扩展名；GIF 原样保留，其余常见格式也给出真实扩展
///
/// - `data`: 图标原始字节
///
/// # 返回值
///
/// `gif` / `png` / `jpg` / `webp`；认不出时按 png 兼容旧行为
fn icon_extension(data: &[u8]) -> &'static str {
    if data.starts_with(b"GIF8") {
        "gif"
    } else if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "webp"
    } else {
        "png"
    }
}

impl InstanceSettingObj {
    /// 创建实例，落在默认分组
    ///
    /// 重名实例经 GUI 询问覆盖或改名；创建实例目录结构并保存设置。
    ///
    /// - `gui`: 添加实例界面回调；无界面时重名实例自动改名
    ///
    /// # 返回值
    ///
    /// 返回新实例句柄；名字为空或用户取消返回对应错误
    pub async fn create_instance(self, gui: AddInstanceGui) -> CoreResult<GameInstance> {
        self.create_instance_in_group(gui, None).await
    }

    /// 创建实例并指定分组
    ///
    /// 与 [`create_instance`](Self::create_instance) 只差分组：分组归属不在实例配置里，
    /// 见 [`crate::game_group`]，只能在登记时一并指定，所以导入或安装整合包这类
    /// "用户已经选了目标分组"的流程走这里。
    ///
    /// - `gui`: 添加实例界面回调；无界面时重名实例自动改名
    /// - `group`: 目标分组 uuid；`None` 表示默认分组，不存在或已删的组也会落到默认分组
    ///
    /// # 返回值
    ///
    /// 返回新实例句柄；名字为空或用户取消返回对应错误
    pub async fn create_instance_in_group(
        mut self,
        gui: AddInstanceGui,
        group: Option<Uuid>,
    ) -> CoreResult<GameInstance> {
        let _watch = path_watch::WatchPause::new();

        // 首尾空白不算名字的一部分，落盘与目录名都用去掉空白后的值
        self.name = self.name.trim().to_string();

        let old = get_instance_by_name(&self.name);
        // 是否覆盖重名实例；无界面或用户拒绝覆盖时改为自动改名，保留原实例
        let mut overwrite = false;
        if let Some(instance) = &old {
            if let Some(gui) = &gui {
                overwrite = gui.overwrite(instance.clone()).await;
                if !overwrite && !gui.name_replace(&self.name).await {
                    return Err(ErrorType::TaskCancel);
                }
            }

            if !overwrite {
                let mut a = 1;
                let mut name = format!("{}{a}", self.name);
                while have_instance_name(&name) {
                    a += 1;
                    name = format!("{}{a}", self.name);
                }

                self.name = name;
            }
        }

        if self.name.is_empty() {
            return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
        }

        // 只在用户选择覆盖时才删除重名的原实例
        if overwrite {
            if let Some(instance) = old {
                let uuid = {
                    let r = instance.read().unwrap();
                    r.uuid
                };

                delete_instance(&uuid)?;
            }
        }

        self.dir = path_helper::replace_path_name(&self.name);

        let dir = self.get_base_path();
        if dir.exists() {
            path_helper::move_to_trash(&dir)?
        }

        path_helper::create_dir_all(dir)?;
        path_helper::create_dir_all(self.get_game_path())?;
        path_helper::create_dir_all(self.get_mods_path())?;
        path_helper::create_dir_all(self.get_config_path())?;
        path_helper::create_dir_all(self.get_logs_path())?;
        path_helper::create_dir_all(self.get_saves_path())?;
        path_helper::create_dir_all(self.get_resourcepacks_path())?;

        self.save_online_info(&HashMap::new());

        self.save();

        // 分组归属不在实例配置里，落在独立的 group_save.json，`None` 即默认分组
        Ok(add_to_group(self, group))
    }

    /// 删除实例文件
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`，实例目录移入回收站；失败返回对应错误
    pub fn delete_files(&self) -> CoreResult<()> {
        path_helper::move_to_trash(self.get_base_path())
    }

    /// 复制实例设置为新的对象
    ///
    /// `InstanceSettingObj` 刻意不实现 `Clone`，避免被随意整对象复制；
    /// 仅在确有需要构建一个独立新对象时显式调用本方法，如复制实例。
    ///
    /// # 返回值
    ///
    /// 返回字段逐项克隆出的新对象，uuid 仍是原实例的
    fn copy_self(&self) -> Self {
        Self {
            uuid: self.uuid,
            name: self.name.clone(),
            dir: self.dir.clone(),
            version: self.version.clone(),
            loader: self.loader,
            loader_version: self.loader_version.clone(),
            jvm_arg: self.jvm_arg.clone(),
            jvm_name: self.jvm_name.clone(),
            jvm_local: self.jvm_local.clone(),
            window: self.window.clone(),
            start_server: self.start_server.clone(),
            auto_save: self.auto_save.clone(),
            proxy_host: self.proxy_host.clone(),
            advance_jvm: self.advance_jvm.clone(),
            is_modpack: self.is_modpack,
            modpack_type: self.modpack_type,
            game_type: self.game_type,
            pid: self.pid.clone(),
            fid: self.fid.clone(),
            icon: self.icon.clone(),
            server_url: self.server_url.clone(),
            custom_loader: self.custom_loader.clone(),
            encoding: self.encoding,
        }
    }

    /// 复制数据到新的实例
    ///
    /// - `name`: 新的实例名字
    /// - `gui`: 添加实例界面回调
    ///
    /// # 返回值
    ///
    /// 返回新实例句柄；新实例与原实例同组
    pub async fn copy_to_other(&self, name: &str, gui: AddInstanceGui) -> CoreResult<GameInstance> {
        // 复制品与原实例同组：分组不在实例配置里，得单独问一次
        let group = get_instance_group(&self.uuid);

        let mut instance = self.copy_self();
        instance.name = name.to_string();
        let instance = instance.create_instance_in_group(gui, group).await?;

        let online = self.read_online_info();
        let custom = self.read_custom_json();

        let read = instance.read().unwrap();
        read.save_custom_json(&custom)?;
        read.save_online_info(&online);

        Ok(instance.clone())
    }

    /// 更新在线文件信息
    ///
    /// 清理掉磁盘上已不存在的文件记录；被禁用而追加了 `.disabled` / `.disable` 后缀的视作仍存在。
    pub async fn update_online(&self) {
        let mut online = self.read_online_info();
        let dir = self.get_game_path();
        online.retain(|_, value| {
            let file = dir.join(&value.path).join(&value.file);
            if file.exists() {
                false
            } else {
                // 新后缀是 `.disabled`，旧版留下的 `.disable` 同样算还存在
                for suffix in [names::DISABLED_DOT_EXT, names::DISABLE_DOT_EXT] {
                    let disabled_file =
                        PathBuf::from(format!("{}{}", file.display(), suffix));
                    if disabled_file.exists() {
                        return false;
                    }
                }
                true
            }
        });

        self.save_online_info(&online);
    }

    /// 将文件复制到其他地方
    ///
    /// - `path`: 目标目录
    /// - `skip`: 需要跳过的相对路径列表；`None` 表示一个都不跳过
    /// - `is_base`: `true` 复制实例整个目录，`false` 只复制游戏目录
    /// - `gui`: 进度界面回调
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；复制失败返回对应错误
    pub async fn copy_files<P: AsRef<Path>>(
        &self,
        path: P,
        skip: Option<Vec<PathBuf>>,
        is_base: bool,
        gui: ProgressGui,
    ) -> CoreResult<()> {
        let dir = if is_base {
            self.get_base_path()
        } else {
            self.get_game_path()
        };

        path_helper::create_dir_all(path.as_ref())?;

        let mut index = 0usize;
        let list = path_helper::get_all_files(&dir);
        if let Some(gui) = &gui {
            gui.set_progress_now(index, Some(list.len()));
        }
        for item in list.iter() {
            let file = item.strip_prefix(&dir).map_err(|err| {
                ErrorType::FileSystemError(FileSystemErrorData {
                    path: item.clone(),
                    error: err.to_string(),
                })
            })?;

            if let Some(skip) = &skip {
                if skip.contains(&file.to_path_buf()) {
                    index += 1;
                    if let Some(gui) = &gui {
                        gui.set_progress_now(index, Some(list.len()));
                    }
                    continue;
                }
            }

            if let Some(gui) = &gui {
                gui.set_progress_text(Some(item.display().to_string()));
            }

            let now = path.as_ref().join(file);
            path_helper::copy_file_async(item, &now).await?;
        }

        Ok(())
    }

    /// 写入实例图标文件，并把实际写进去的文件名记回实例
    ///
    /// 故意不做成 `&mut self` 方法：那样调用方就得在 `.await` 期间持有实例写锁，
    /// 而实例锁的守卫不是 `Send`，在 `#[async_trait]` 里编译不过，它要求 `future: Send`。
    /// 这里自己控制加锁范围，只在锁内问出"图标写到哪个文件"，写盘时不持锁。
    ///
    /// 调用方也不要带着实例锁进来，`std` 的 `RwLock` 不可重入，会死锁。
    ///
    /// - `game`: 目标实例
    /// - `icon`: 图标输入源
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；写盘失败返回对应错误
    pub async fn save_icon(game: &GameInstance, icon: InputFile) -> CoreResult<()> {
        // 在线整合包图标先读字节判格式：GIF 必须保留 .gif，不能固定写成 icon.png 再转静态图
        if let InputFile::Url(url) = &icon {
            let data = get_work_client().get_bytes(url).await?;
            let ext = icon_extension(&data);
            let file = game
                .read()
                .unwrap()
                .get_base_path()
                .join(format!("icon.{ext}"));
            path_helper::write_bytes_async(&file, &data).await?;
            let name = file
                .file_name()
                .map(|item| item.to_string_lossy().into_owned());
            let mut game = game.write().unwrap();
            game.icon = name;
            game.save();
            return Ok(());
        }

        let (file, name) = {
            let game = game.read().unwrap();
            // 上传图片 = 图标二选一里的"图片"那一支：写默认的 icon.png
            let file = game.get_icon_file_or_default();
            let name = file
                .file_name()
                .map(|item| item.to_string_lossy().into_owned());
            (file, name)
        };

        icon.save_file(file).await?;

        let mut game = game.write().unwrap();
        game.icon = name;
        game.save();

        Ok(())
    }
}
