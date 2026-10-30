//! 整合包安装
//!
//! 子模块:
//!
//! | 模块 | 职责 |
//! | --- | --- |
//! | `curseforge_worker` | CurseForge 整合包安装 |
//! | `modrinth_worker` | Modrinth 整合包安装 |

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use mml_base::{
    archives::{BaseArchive, BaseArchiveGui, IBaseArchiveGui},
    file_item::FileItemObj,
};
use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    GameInstance,
    gui_hook::{AddInstanceGui, AddModPackGui},
    loader::LoaderType,
};

pub mod curseforge_worker;
pub mod diff;
pub mod manifest;
pub mod modrinth_worker;

/// 整合包安装流程的统一接口，CurseForge 与 Modrinth 各实现一份
#[async_trait]
pub(crate) trait ModPackWorker {
    /// 读取整合包主信息
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；读取失败返回对应错误
    fn read_info(&mut self) -> CoreResult<()>;
    /// 读取整合包要求的游戏版本与加载器
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；读取失败返回对应错误
    async fn read_version(&mut self) -> CoreResult<()>;
    /// 创建游戏实例
    ///
    /// - `name`: 实例名，`None` 时用整合包名
    /// - `group`: 目标分组 uuid，`None` 表示默认分组
    ///
    /// # 返回值
    ///
    /// 新实例的 UUID；创建失败返回对应错误
    async fn create_instance(&self, name: Option<String>, group: Option<Uuid>) -> CoreResult<Uuid>;
    /// 这次安装 / 升级对应的默认实例名，形如 `{整合包名}-{版本}`
    ///
    /// # 返回值
    ///
    /// 已读到整合包信息时返回该名字，否则返回 `None`
    fn default_instance_name(&self) -> Option<String>;
    /// 解压整合包内的文件
    ///
    /// - `unselect`: 不解压的条目路径列表，`None` 表示全解压
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；解压失败返回对应错误
    async fn extract(&self, unselect: Option<Vec<String>>) -> CoreResult<()>;
    /// 查询整合包内各模组的下载信息
    ///
    /// # 返回值
    ///
    /// 是否成功取得全部模组信息；查询失败返回对应错误
    async fn get_info(&self) -> CoreResult<bool>;
    /// 下载所需文件
    async fn download(&self);
    /// 更新游戏实例版本信息
    ///
    /// - `game`: 目标实例
    fn update_game(&mut self, game: &GameInstance);
    /// 按最新清单检查整合包更新
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；查询失败返回对应错误
    async fn check_upgrade(&self) -> CoreResult<()>;
}

/// 升级成功后把实例名与实例目录改成新版整合包对应的默认名
///
/// 改名失败只记日志、不让升级失败：文件与 pid / fid 都已按新版本写好，名字是次要的。
///
/// - `game`: 目标实例
/// - `name`: 新版整合包的默认实例名，来自 [`ModPackWorker::default_instance_name`]
pub(crate) fn rename_after_upgrade(game: &GameInstance, name: Option<String>) {
    let Some(name) = name else {
        return;
    };
    // 先放开读锁再改名：rename_instance 会重新取同一实例的写锁，持锁调用会自锁
    let uuid = {
        let game = game.read().unwrap();
        game.uuid
    };
    if let Err(err) = crate::rename_instance(&uuid, &name) {
        mml_log::error_type(err);
    }
}

/// 整合包安装器的公共状态，两条安装路径共用
pub struct BaseModPackWorker {
    /// 整合包压缩包
    pub archive: BaseArchive,
    /// 实例创建界面回调
    pub instance_gui: AddInstanceGui,
    /// 整合包安装界面回调
    pub pack_gui: AddModPackGui,
    /// 压缩包操作界面回调
    pub archive_gui: BaseArchiveGui,
    /// 加载器类型，创建时默认 Normal
    pub loader: LoaderType,
    /// 加载器版本，空串表示未指定
    pub loader_version: String,
    /// 游戏版本，空串表示未指定
    pub game_version: String,
    /// 创建出来的游戏实例，未创建时为 `None`
    pub game: Option<GameInstance>,
    /// 待下载文件列表，用 Mutex 让 `&self` 方法也能改
    pub downloads: Mutex<Vec<FileItemObj>>,
    /// 取消令牌，触发后安装流程尽快退出
    pub cancel: CancellationToken,
    /// 整合包图标 URL；可缺省
    pub icon: Option<String>,
}

/// 整合包解压的进度桥：同一份解压进度同时写给压缩包弹窗与安装任务
struct PackExtractGui {
    /// 添加实例窗口自己的压缩包解压进度；没有该窗口时为 `None`
    archive: BaseArchiveGui,
    /// 安装任务的主进度回调；写进子进度，任务列表据此画解压条
    pack: AddModPackGui,
    /// 本批总文件数；`start` 写入，`update` 时用来带上总数
    total: AtomicUsize,
}

impl IBaseArchiveGui for PackExtractGui {
    fn start(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
        if let Some(pack) = &self.pack {
            pack.set_sub_text(None);
            pack.set_sub_now(0, Some(total));
        }
        if let Some(archive) = &self.archive {
            archive.start(total);
        }
    }

    fn update(&self, filename: Option<String>, current: usize) {
        let total = self.total.load(Ordering::Relaxed);
        if let Some(pack) = &self.pack {
            // 完整路径交给前端按宽度截成一行；这里只保留原值，别提前丢了上级目录
            pack.set_sub_text(filename.clone());
            pack.set_sub_now(current, Some(total));
        }
        if let Some(archive) = &self.archive {
            archive.update(filename, current);
        }
    }

    fn done(&self) {
        let total = self.total.load(Ordering::Relaxed);
        if let Some(pack) = &self.pack {
            pack.set_sub_text(None);
            pack.set_sub_now(total, Some(total));
        }
        if let Some(archive) = &self.archive {
            archive.done();
        }
    }

    fn cancelled(&self) -> bool {
        self.archive.as_ref().is_some_and(|gui| gui.cancelled())
    }

    fn file_rename(&self, name: &str) -> bool {
        // 任务进度弹窗没有确认入口：沿用无 GUI 时的自动改名
        match &self.archive {
            Some(gui) => gui.file_rename(name),
            None => true,
        }
    }
}

impl BaseModPackWorker {
    /// 创建安装器
    ///
    /// - `archive`: 整合包压缩包
    /// - `instance_gui`: 实例创建界面回调
    /// - `pack_gui`: 整合包安装界面回调
    /// - `archive_gui`: 压缩包操作界面回调
    /// - `cancel`: 取消令牌
    /// - `icon`: 整合包图标 URL，在线安装时由调用方给，创建实例时写进实例，`None` 表示没有
    ///
    /// # 返回值
    ///
    /// 新建的安装器：加载器为 `Normal`、版本为空串，实例与待下载列表都还没建
    pub fn new(
        archive: BaseArchive,
        instance_gui: AddInstanceGui,
        pack_gui: AddModPackGui,
        archive_gui: BaseArchiveGui,
        cancel: CancellationToken,
        icon: Option<String>,
    ) -> Self {
        Self {
            archive,
            instance_gui,
            pack_gui,
            archive_gui,
            loader: LoaderType::Normal,
            loader_version: String::new(),
            game_version: String::new(),
            game: None,
            downloads: Mutex::new(Vec::new()),
            cancel,
            icon,
        }
    }

    /// 解压整合包覆盖文件到游戏目录
    ///
    /// - `prefix`: 需要去掉的目录前缀，如 `overrides`
    /// - `unselect`: 不解压的条目路径列表，`None` 表示全解压
    ///
    /// `prefix/` 下的文件去掉前缀后写入游戏根目录，其余文件直接写入游戏基础目录，
    /// 进度通过 `archive_gui` 上报。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；实例未创建或解压失败返回对应错误
    pub fn extract_pack_files(
        &self,
        prefix: &str,
        unselect: Option<Vec<String>>,
    ) -> CoreResult<()> {
        let Some(game) = &self.game else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::GameInstance));
        };

        // 解压期间不持有实例锁
        let (base_path, game_path) = {
            let game = game.read().unwrap();
            (game.get_base_path(), game.get_game_path())
        };
        let prefix = format!("{prefix}/");

        if self.cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let extract_gui = if self.pack_gui.is_some() || self.archive_gui.is_some() {
            Some(Arc::new(PackExtractGui {
                archive: self.archive_gui.clone(),
                pack: self.pack_gui.clone(),
                total: AtomicUsize::new(0),
            }) as Arc<dyn IBaseArchiveGui>)
        } else {
            None
        };

        self.archive.extract_where(
            |entry| {
                // 跳过不需要解压的条目
                if let Some(ref unselect) = unselect {
                    if unselect.iter().any(|u| u == &entry.name) {
                        return None;
                    }
                }
                let output = if let Some(rel) = entry.name.strip_prefix(&prefix) {
                    // 覆盖文件：去除 prefix 前缀后放到游戏根目录
                    game_path.join(rel)
                } else {
                    base_path.join(&entry.name)
                };
                Some(output)
            },
            extract_gui.as_deref(),
        )
    }
}
