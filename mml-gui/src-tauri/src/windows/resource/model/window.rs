//! 资源窗口的模型
//!
//! 一个窗口一份，装实例、扫描结果引用与在跑的任务；
//! 关窗前取消任务，窗口销毁时释放。

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::err_keys;

use super::cache::{KIND, ModCache};
use crate::windows::lifecycle::{window_model_by_uuid, window_model_or_insert};
use crate::windows::registry::{family_of_kind, family_of_label, family_window_id};

/// 资源窗口的窗口模型
pub(crate) struct ResourceWindowModel {
    /// 最近一次扫描的结果；模组列表还没加载过时是 `None`
    pub(super) cache: Mutex<Option<ModCache>>,
    /// 在跑任务：自增序号 → 取消令牌，关窗时逐个 cancel
    pub(super) cancels: Mutex<HashMap<u64, CancellationToken>>,
    /// 下一个任务的序号
    pub(super) next_op: AtomicU64,
}

/// 一个在跑任务的守卫
///
/// 拿着它就代表"这个任务还在跑"：关窗时 [`ResourceWindowModel::cancel_all`] 会把令牌
/// 全部取消，任务自己在循环里看 [`OpGuard::is_cancelled`] 收手。
pub(crate) struct OpGuard {
    /// 所属模型，Drop 时把令牌摘掉
    pub(super) model: Arc<ResourceWindowModel>,
    /// 本任务的序号
    pub(super) id: u64,
    /// 本任务的取消令牌
    pub(super) token: CancellationToken,
}

/// 某个实例的资源窗口 uuid
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 派生出的窗口 uuid；注册表里没有 resource 这一族时返回 `None`
pub(crate) fn window_uuid(instance: &Uuid) -> Option<Uuid> {
    family_of_kind(KIND).map(|family| family_window_id(family, instance).uuid)
}

/// 取本实例资源窗口的模型，没有就建一个
///
/// 多窗口模式下窗口打开时就有；单窗口模式下资源窗口只是个页面，模型由这里按需建立，
/// 页面切走也不清 —— 前端那份列表还在内存里，清了它再点检查就没得用了。
///
/// - `instance`: 实例 uuid 字符串，命令的入参
///
/// # 返回值
///
/// 模型；实例 uuid 非法返回 `err.uuid`
pub(crate) fn window_model(instance: &str) -> Result<Arc<ResourceWindowModel>, String> {
    let uuid = Uuid::parse_str(instance).map_err(|_| err_keys::UUID.to_string())?;
    let window = window_uuid(&uuid).ok_or_else(|| err_keys::MODEL_MISSING.to_string())?;
    window_model_or_insert(&window, ResourceWindowModel::new)
        .ok_or_else(|| err_keys::MODEL_MISSING.to_string())
}

/// 取本实例资源窗口的模型，**不新建**
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 模型；没建过时返回 `None`
pub(super) fn existing(instance: &Uuid) -> Option<Arc<ResourceWindowModel>> {
    let window = window_uuid(instance)?;
    window_model_by_uuid(&window)
}

/// 终止某个窗口还在跑的操作，关窗前调用
///
/// - `label`: 窗口标签，形如 `mml-resource-<实例uuid>`
pub(crate) fn cancel_window(label: &str) {
    let Some(hit) = family_of_label(label) else {
        return;
    };
    if hit.family.kind != KIND {
        return;
    }
    if let Some(model) = existing(&hit.instance) {
        model.cancel_all();
    }
}

/// 取这个操作的守卫，窗口模型还没建起来时不挂
///
/// 各分类的读取命令用它：读的过程中删实例会被 [`instance_busy`] 拦住。
/// 模型没建起来说明窗口还没打开过，这时也没有"读取中"可言，直接不挂。
///
/// - `uuid`: 实例 uuid 字符串，即命令的入参
///
/// # 返回值
///
/// 见 [`OpGuard`]；uuid 非法或模型没建起来时返回 `None`
pub(crate) fn begin_if_open(uuid: &str) -> Option<OpGuard> {
    let id = Uuid::parse_str(uuid).ok()?;
    existing(&id).map(|model| model.begin())
}

/// 这个实例的资源窗口现在有没有在跑的操作
///
/// 刷新 / 读取模组、检查更新、下载更新、自动标记、整合包升级都挂着操作守卫，
/// 删实例前用它把"正在处理这个实例"拦住 —— 删到一半那些操作会写到已删的目录里。
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 有操作在跑时返回 `true`；窗口没开过、或没有在跑的操作时返回 `false`
pub(crate) fn instance_busy(instance: &Uuid) -> bool {
    existing(instance).is_some_and(|model| model.in_flight() > 0)
}

impl ResourceWindowModel {
    /// 建一个空模型
    ///
    /// # 返回值
    ///
    /// 缓存为空、没有在跑的任务、序号从 0 开始的模型
    pub(crate) fn new() -> Self {
        Self {
            cache: Mutex::new(None),
            cancels: Mutex::new(HashMap::new()),
            next_op: AtomicU64::new(0),
        }
    }

    /// 记下这一轮扫描
    ///
    /// - `cache`: 内核扫描结果，以及它附带的在线信息 / 游戏版本 / 加载器
    pub(crate) fn set_cache(&self, cache: ModCache) {
        *self.cache.lock().unwrap() = Some(cache);
    }

    /// 读缓存
    ///
    /// 闭包在锁里跑，**不要在里面 await**。
    ///
    /// - `f`: 拿到缓存后的计算
    ///
    /// # 返回值
    ///
    /// `f` 的结果；模组列表还没加载过时返回 `None`
    pub(crate) fn with_cache<T>(&self, f: impl FnOnce(&ModCache) -> T) -> Option<T> {
        self.cache.lock().unwrap().as_ref().map(f)
    }

    /// 改一条在线信息：先按 SHA1 删掉旧条目，再按项目号写入新条目
    ///
    /// - `sha1`: 目标模组的 SHA1，用来认旧条目（旧键是旧项目号，可能刚被改过）
    /// - `key`: 新的项目号；空串表示这条在线信息不要了
    /// - `info`: 新条目；`key` 为空时不看它
    pub(crate) fn set_online(
        &self,
        sha1: &str,
        key: &str,
        info: mml_game::launcher::file_online_info_obj::OnlineInfoObj,
    ) {
        let mut guard = self.cache.lock().unwrap();
        let Some(cache) = guard.as_mut() else {
            return;
        };
        cache.online.retain(|_, item| item.sha1 != sha1);
        if !key.is_empty() {
            cache.online.insert(key.to_string(), info);
        }
    }

    /// 启用 / 禁用之后改掉缓存里那一条
    ///
    /// 改文件名会连 uuid（文件路径的 v5）一起变，缓存按 uuid 记，所以得换键。
    ///
    /// - `old_uuid`: 改名前那一条的 uuid
    /// - `path`: 改名后的路径
    /// - `disable`: 改名后是不是禁用状态
    pub(crate) fn rename_mod(&self, old_uuid: &str, path: &Path, disable: bool) {
        let mut cache = self.cache.lock().unwrap();
        let Some(cache) = cache.as_mut() else {
            return;
        };
        let Some(item) = cache
            .list
            .iter_mut()
            .find(|item| item.uuid.to_string() == old_uuid)
        else {
            return;
        };
        item.file = path.to_path_buf();
        item.disable = disable;
        item.uuid = mml_game::game_mods::gen_mod_uuid(path);
    }

    /// 删除之后把缓存里那一条去掉
    ///
    /// - `uuid`: 被删掉那一条的 uuid
    pub(crate) fn remove_mod(&self, uuid: &str) {
        let mut cache = self.cache.lock().unwrap();
        let Some(cache) = cache.as_mut() else {
            return;
        };
        cache.list.retain(|item| item.uuid.to_string() != uuid);
    }

    /// 登记一个在跑的任务
    ///
    /// # 返回值
    ///
    /// 见 [`OpGuard`]；守卫 Drop 时自动把令牌摘掉
    pub(crate) fn begin(self: &Arc<Self>) -> OpGuard {
        let id = self.next_op.fetch_add(1, Ordering::Relaxed);
        let token = CancellationToken::new();
        self.cancels.lock().unwrap().insert(id, token.clone());
        OpGuard {
            model: self.clone(),
            id,
            token,
        }
    }

    /// 现在有几个操作在跑
    ///
    /// 每个在跑的操作都挂着一个 [`OpGuard`]，守卫 Drop 时从表里摘掉，所以长度就是
    /// "正在处理这个实例的操作数"。删实例前用它判断忙不忙。
    ///
    /// # 返回值
    ///
    /// 在跑的操作个数
    pub(crate) fn in_flight(&self) -> usize {
        self.cancels.lock().unwrap().len()
    }

    /// 取消所有在跑的任务
    pub(crate) fn cancel_all(&self) {
        for (_, token) in self.cancels.lock().unwrap().iter() {
            token.cancel();
        }
    }
}

impl OpGuard {
    /// 本任务的取消令牌，交给 `tokio::select!` 用
    ///
    /// # 返回值
    ///
    /// 与本任务共享取消状态的一份令牌
    pub(crate) fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// 本任务是否已被取消
    ///
    /// # 返回值
    ///
    /// 已取消返回 `true`
    pub(crate) fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl Drop for OpGuard {
    /// 任务结束或被打断时都走到这里，把本任务的令牌从模型里摘掉
    fn drop(&mut self) {
        self.model.cancels.lock().unwrap().remove(&self.id);
    }
}
