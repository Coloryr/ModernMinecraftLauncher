//! 资源窗口的窗口模型
//!
//! 这一层是窗口运行态，跟窗口同生共死（登记在 windows/lifecycle 的模型表里）：
//!
//! - **扫描缓存**：模组列表扫一次要几秒，每个 jar 都得把元数据读一遍；而更新检查、
//!   依赖检测要的就是"这个窗口现在看到的这份列表"。列表命令扫完存进来，后面的操作直接读，
//!   不再重扫 `mods` 目录。
//! - **在跑任务的取消令牌**：关窗时逐个 cancel（见 [`cancel_window`]），
//!   扫描 / 检查 / 下载更新这些慢操作在循环里看它，别对着已经关掉的窗口继续跑。
//!
//! 模型按**派生窗口 uuid** 登记，与句柄表、几何同一套键：资源窗口是"一个实例一个窗口"
//! （见 registry 的窗口族），命令侧手里只有实例 uuid，用 [`window_uuid`] 换算。

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use mml_game::game_mods::ModObj;
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::loader::LoaderType;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::err_keys;

use super::super::lifecycle::{window_model_by_uuid, window_model_or_insert};
use super::super::registry::{family_of_kind, family_of_label, family_window_id};

/// 资源窗口在注册表里的 kind，也是窗口族表里的族名
const KIND: &str = "resource";

/// 一次模组扫描的结果
///
/// 与列表命令返回给前端的那份是同一趟扫描：列表走 DTO，这里留内核口径的原始数据，
/// 更新检查、依赖检测直接在上面算。
pub(crate) struct ModCache {
    /// 内核扫描出的条目，含 jar-in-jar 内置项
    pub(crate) list: Vec<ModObj>,
    /// 实例的在线信息表，键是项目编号
    pub(crate) online: HashMap<String, OnlineInfoObj>,
    /// 实例的游戏版本，更新检查按它过滤平台上的文件
    pub(crate) version: String,
    /// 实例的加载器
    pub(crate) loader: LoaderType,
}

/// 资源窗口的窗口模型
pub(crate) struct ResourceWindowModel {
    /// 最近一次扫描的结果；模组列表还没加载过时是 `None`
    cache: Mutex<Option<ModCache>>,
    /// 在跑任务：自增序号 → 取消令牌，关窗时逐个 cancel
    cancels: Mutex<HashMap<u64, CancellationToken>>,
    /// 下一个任务的序号
    next_op: AtomicU64,
}

impl ResourceWindowModel {
    /// 建一个空模型
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

/// 一个在跑任务的守卫
///
/// 拿着它就代表"这个任务还在跑"：关窗时 [`ResourceWindowModel::cancel_all`] 会把令牌
/// 全部取消，任务自己在循环里看 [`OpGuard::is_cancelled`] 收手。
pub(crate) struct OpGuard {
    /// 所属模型，Drop 时把令牌摘掉
    model: Arc<ResourceWindowModel>,
    /// 本任务的序号
    id: u64,
    /// 本任务的取消令牌
    token: CancellationToken,
}

impl OpGuard {
    /// 本任务的取消令牌，交给 `tokio::select!` 用
    pub(crate) fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// 本任务是否已被取消
    pub(crate) fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl Drop for OpGuard {
    fn drop(&mut self) {
        self.model.cancels.lock().unwrap().remove(&self.id);
    }
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
fn existing(instance: &Uuid) -> Option<Arc<ResourceWindowModel>> {
    let window = window_uuid(instance)?;
    window_model_by_uuid(&window)
}

/// 终止某个窗口还在跑的操作，关窗前调用
///
/// - `label`: 窗口标签，形如 `mml-resource-<实例uuid>`
pub(crate) fn cancel_window(label: &str) {
    let Some((family, instance)) = family_of_label(label) else {
        return;
    };
    if family.kind != KIND {
        return;
    }
    if let Some(model) = existing(&instance) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 造一份缓存：两条模组，第二条是禁用状态
    fn cache() -> ModCache {
        let first = ModObj {
            uuid: Uuid::new_v4(),
            file: PathBuf::from("mods/a.jar"),
            ..Default::default()
        };
        let second = ModObj {
            uuid: Uuid::new_v4(),
            file: PathBuf::from("mods/b.jar.disabled"),
            disable: true,
            ..Default::default()
        };
        ModCache {
            list: vec![first, second],
            online: HashMap::new(),
            version: "1.20.1".to_string(),
            loader: LoaderType::Normal,
        }
    }

    /// 缓存存进去能读出来；没存过时读不到
    #[test]
    fn cache_round_trip() {
        let model = ResourceWindowModel::new();
        assert!(model.with_cache(|_| ()).is_none());

        model.set_cache(cache());
        assert_eq!(model.with_cache(|cache| cache.list.len()), Some(2));
    }

    /// 启用 / 禁用之后：文件名、启用状态、uuid（路径的 v5）都跟着换
    #[test]
    fn rename_mod_updates_entry() {
        let model = ResourceWindowModel::new();
        model.set_cache(cache());
        let old_uuid = model
            .with_cache(|cache| cache.list[0].uuid.to_string())
            .unwrap();
        let new_path = PathBuf::from("mods/a.jar.disabled");

        model.rename_mod(&old_uuid, &new_path, true);

        model
            .with_cache(|cache| {
                let item = cache
                    .list
                    .iter()
                    .find(|item| item.file == new_path)
                    .expect("改名后的那一条还在");
                assert!(item.disable);
                assert_eq!(item.uuid, mml_game::game_mods::gen_mod_uuid(&new_path));
                // 旧 uuid 那一条不该同时留着
                let stale = cache
                    .list
                    .iter()
                    .any(|item| item.uuid.to_string() == old_uuid);
                assert!(!stale);
            })
            .unwrap();
    }

    /// 删除之后那一条不再留在缓存里
    #[test]
    fn remove_mod_drops_entry() {
        let model = ResourceWindowModel::new();
        model.set_cache(cache());
        let uuid = model
            .with_cache(|cache| cache.list[1].uuid.to_string())
            .unwrap();

        model.remove_mod(&uuid);

        assert_eq!(model.with_cache(|cache| cache.list.len()), Some(1));
    }

    /// 关窗取消：在跑的都被取消，之后新起的任务不受影响；守卫掉了自己从表里摘掉
    #[test]
    fn cancel_all_only_hits_running_ops() {
        let model = Arc::new(ResourceWindowModel::new());
        let first = model.begin();
        let second = model.begin();
        assert!(!first.is_cancelled());

        model.cancel_all();
        assert!(first.is_cancelled());
        assert!(second.is_cancelled());

        drop(first);
        let third = model.begin();
        assert_eq!(model.cancels.lock().unwrap().len(), 2);
        assert!(!third.is_cancelled());
    }

    /// 模型存进窗口表再取出来：存的是模型本体，取的时候类型要对得上
    ///
    /// 存成 `Arc<Mutex<模型>>` 而按 `Arc<模型>` 取，downcast 会失败，
    /// 命令侧就报"窗口模型未初始化"。
    #[test]
    fn model_round_trips_through_window_table() {
        let uuid = Uuid::new_v4();
        let first = window_model_or_insert(&uuid, ResourceWindowModel::new).expect("应当建出模型");
        let second = window_model_by_uuid::<ResourceWindowModel>(&uuid).expect("应当取到模型");
        assert!(Arc::ptr_eq(&first, &second));

        crate::windows::lifecycle::remove_window_model(&uuid);
        assert!(window_model_by_uuid::<ResourceWindowModel>(&uuid).is_none());
    }
}
