//! 账户持久化存储模块
//!
//! 本模块管理启动器中所有账户的全局存储，提供账户的加载、保存、
//! 查询、导入和删除功能。
//!
//! # 存储方式
//!
//! - 内存中：使用 `LazyLock<RwLock<HashMap<UserKeyObj, LoginObj>>>` 作为全局单例存储
//! - 磁盘上：以 JSON 格式序列化为 `Vec<LoginObj>`，保存到程序内部目录的 `auth.json` 文件中
//!
//! # 使用流程
//!
//! 1. 启动时调用 [`init()`] 从磁盘加载已有账户
//! 2. 登录成功后调用 `LoginObj::save()` 保存新账户
//! 3. 登录前通过 [`get()`] 查找已保存的账户
//! 4. 切换账户调用 `set_current()`，删除账户调用 `LoginObj::delete()`

use std::{
    collections::HashMap,
    path::Path,
    sync::{LazyLock, RwLock},
};

use crate::{AuthType, LoginObj, UserKeyObj, normalize_uuid};
use mml_base::{inner_path, serialize_tools};
use mml_config::config_save;
use mml_names::{i18_items::error_type::CoreResult, names, uuids};

/// 全局账户存储，键由 UUID 与认证类型组成
static AUTHS: LazyLock<RwLock<HashMap<UserKeyObj, LoginObj>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 当前使用的账户键，随 `auth_select.json` 持久化，`init` 时载回
static CURRENT_USER: LazyLock<RwLock<Option<UserKeyObj>>> = LazyLock::new(|| RwLock::new(None));

static SELECTION_LOCK: LazyLock<RwLock<Option<(uuid::Uuid, UserKeyObj)>>> = LazyLock::new(|| RwLock::new(None));

/// 联机会话持有的账户选择锁，释放后允许切换。
pub struct CurrentUserGuard {
    id: uuid::Uuid,
    pub key: UserKeyObj,
}

impl Drop for CurrentUserGuard {
    fn drop(&mut self) {
        let mut lock = SELECTION_LOCK.write().unwrap();
        if lock.as_ref().is_some_and(|(id, _)| *id == self.id) { *lock = None; }
    }
}

/// 锁定当前账户。
/// # 返回值
/// 已被其它会话锁定或未选择账户时返回空。
pub fn lock_current() -> Option<CurrentUserGuard> {
    let mut lock = SELECTION_LOCK.write().unwrap();
    if lock.is_some() { return None; }
    let key = get_current()?;
    let id = uuid::Uuid::new_v4();
    *lock = Some((id, key.clone()));
    Some(CurrentUserGuard { id, key })
}

/// 查询当前账户的联机锁。
/// # 返回值
/// 存在联机会话时返回 true。
pub fn current_is_locked() -> bool { SELECTION_LOCK.read().unwrap().is_some() }

/// 从磁盘加载账户列表到内存
///
/// 读取失败时会打印错误日志并触发一次保存操作。
///
/// - `path`: 账户数据文件的路径
fn load<P: AsRef<Path>>(path: P) {
    match import(path) {
        // 旧数据可能把同一账户的无连字符 UUID 另存成一条，合并后立刻写回磁盘
        Ok(true) => save(),
        Ok(false) => {}
        Err(err) => {
            mml_log::error_type(err);

            save();
        }
    }
}

/// 从磁盘加载选中的账户
///
/// 文件内容即 `Option<UserKeyObj>` 的直接序列化，`None` 时为 `null`，不包外层结构；
/// 历史上曾按包一层 `user` 字段的结构解析，与写入格式不符导致重启后选中账户静默丢失。
///
/// - `path`: 选中账户数据文件的路径
fn load_select<P: AsRef<Path>>(path: P) {
    let json = serialize_tools::json_from_file::<Option<UserKeyObj>>(path);
    if let Err(err) = json {
        mml_log::error_type(err);
        return;
    }

    let Some(mut key) = json.unwrap() else {
        *CURRENT_USER.write().unwrap() = None;
        return;
    };
    let raw_uuid = key.uuid.clone();
    key.uuid = normalize_uuid(&raw_uuid);
    let changed = key.uuid != raw_uuid;
    *CURRENT_USER.write().unwrap() = Some(key);
    // 旧文件里的带连字符写法顺手改成统一写法
    if changed {
        save_select();
    }
}

/// 将内存中所有账户持久化到磁盘
fn save() {
    let auths: Vec<LoginObj> = AUTHS.read().unwrap().values().cloned().collect();
    let file = inner_path::get_inner_path().join(names::AUTH_FILE);
    config_save::save(uuids::AUTH_UUID, &auths, &file);
}

/// 保存选中的账户
fn save_select() {
    let user = CURRENT_USER.read().unwrap().clone();
    let file = inner_path::get_inner_path().join(names::AUTH_SELECT_FILE);
    config_save::save(uuids::AUTH_SELECT_UUID, &user, &file);
}

/// 初始化账户存储，仅在程序启动阶段调用一次
pub fn init() {
    let local = inner_path::get_inner_path().join(names::AUTH_FILE);

    if local.exists() {
        load(&local);
    } else {
        save();
    }

    let local = inner_path::get_inner_path().join(names::AUTH_SELECT_FILE);
    if local.exists() {
        load_select(&local);
    }
}

/// 根据 UUID 和认证类型查询已保存的账户
///
/// - `uuid`: 账户标识，Minecraft UUID
/// - `auth_type`: 账户认证类型
///
/// # 返回值
///
/// 找到则返回 `Some(LoginObj)` 克隆，未找到则返回 `None`
pub fn get(uuid: &str, auth_type: AuthType) -> Option<LoginObj> {
    let auths = AUTHS.read().unwrap();
    auths
        .get(&UserKeyObj {
            uuid: normalize_uuid(uuid),
            auth_type,
        })
        .cloned()
}

/// 从 JSON 文件批量导入账户列表
///
/// 文件内容应为 `Vec<LoginObj>` 格式的 JSON 数组。
/// 导入的账户按 `UserKeyObj` 为唯一键合并到已有存储中；
/// 同一个键下有多条记录时保留最近登录的一条，UUID 只作为键的一部分参与判重。
///
/// - `file`: JSON 文件路径
///
/// # 返回值
///
/// 需要回写时返回 `true`：遇到了同键账户，或有 UUID 被统一成不带连字符的写法；
/// 失败时返回反序列化错误
pub fn import<P: AsRef<Path>>(file: P) -> CoreResult<bool> {
    let json = serialize_tools::json_from_file::<Vec<LoginObj>>(file)?;

    let mut auths = AUTHS.write().unwrap();
    let mut changed = false;

    for mut item in json.into_iter() {
        let raw_uuid = item.uuid.clone();
        item.uuid = normalize_uuid(&raw_uuid);
        changed |= item.uuid != raw_uuid;
        let key = item.get_key();
        let replace = match auths.get(&key) {
            None => true,
            Some(old) => {
                changed = true;
                newer_than(&item, old)
            }
        };
        if replace {
            auths.insert(key, item);
        }
    }

    Ok(changed)
}

/// 同键的两条账户记录保留哪一条
///
/// - `new`: 待导入的记录
/// - `old`: 存储中已有的记录
///
/// # 返回值
///
/// 待导入记录登录时间更新时返回 `true`；时间相同时未过期的一方优先
fn newer_than(new: &LoginObj, old: &LoginObj) -> bool {
    if new.last_login != old.last_login {
        return new.last_login > old.last_login;
    }
    old.is_expired() && !new.is_expired()
}

/// 清除所有已保存的账户，内存与磁盘一并清空
///
/// 谨慎使用，此操作不可逆。
pub fn clear_auths() {
    let lock = SELECTION_LOCK.read().unwrap();
    if lock.is_some() { return; }
    // 先释放写锁再落盘：save() 内部要拿读锁，同线程写锁未释放时重入会死锁
    {
        let mut auths = AUTHS.write().unwrap();
        auths.clear();
    }

    save();
}

/// 获取全部账户，按最后登录时间倒序，最近使用的排在前面
///
/// # 返回值
///
/// 全部账户的克隆，最近登录的排在最前
pub fn get_all() -> Vec<LoginObj> {
    let mut list: Vec<LoginObj> = AUTHS.read().unwrap().values().cloned().collect();
    list.sort_by(|a, b| b.last_login.cmp(&a.last_login));
    list
}

/// 获取当前使用的账户
///
/// # 返回值
///
/// 当前选中账户的键，未选中时为 `None`
pub fn get_current() -> Option<UserKeyObj> {
    CURRENT_USER.read().unwrap().clone()
}

/// 设置当前使用的账户
///
/// - `user`: 要选中的账户键，传 `None` 表示取消选中
/// # 返回值
/// 联机会话锁定时拒绝切换并返回 false。
pub fn set_current(user: Option<UserKeyObj>) -> bool {
    let user = user.map(|mut key| {
        key.uuid = normalize_uuid(&key.uuid);
        key
    });
    let lock = SELECTION_LOCK.read().unwrap();
    if lock.as_ref().is_some_and(|(_, key)| user.as_ref() != Some(key)) { return false; }
    *CURRENT_USER.write().unwrap() = user;

    save_select();
    true
}

/// 检查是否拥有正版登录账户
///
/// # 返回值
///
/// 存在微软正版账户返回 `true`
pub fn check_online_user() -> bool {
    AUTHS
        .read()
        .unwrap()
        .iter()
        .any(|(key, _)| matches!(key.auth_type, AuthType::OAuth))
}

impl LoginObj {
    /// 将当前账户保存到全局存储并持久化到磁盘，相同键覆盖更新
    pub fn save(&self) {
        let mut item = self.clone();
        item.uuid = normalize_uuid(&item.uuid);
        // 同 clear_auths：先释放写锁再落盘，防重入死锁
        {
            let mut auths = AUTHS.write().unwrap();
            auths.insert(item.get_key(), item);
        }

        save();
    }

    /// 从全局存储中删除当前账户并更新磁盘数据。
    /// # 返回值
    /// 账户被联机会话锁定时拒绝删除并返回 false。
    pub fn delete(&self) -> bool {
        let key = self.get_key();
        let lock = SELECTION_LOCK.read().unwrap();
        if lock.as_ref().is_some_and(|(_, selected)| *selected == key) { return false; }

        // 同 clear_auths：先释放写锁再落盘，防重入死锁
        {
            let mut auths = AUTHS.write().unwrap();
            auths.remove(&key);
        }

        save();
        true
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn lobby_lease_blocks_switch_delete_and_clear_until_dropped() {
        let account = LoginObj { uuid: "lobby-player".into(), auth_type: AuthType::OAuth, ..Default::default() };
        let key = account.get_key();
        *CURRENT_USER.write().unwrap() = Some(key.clone());
        AUTHS.write().unwrap().insert(key.clone(), account.clone());
        let lease = lock_current().unwrap();
        assert!(current_is_locked());
        assert!(lock_current().is_none());
        assert!(!set_current(None));
        assert!(!set_current(Some(UserKeyObj { uuid: "another-player".into(), auth_type: AuthType::OAuth })));
        assert!(!account.delete());
        clear_auths();
        assert_eq!(get_current(), Some(key.clone()));
        assert!(get(&key.uuid, key.auth_type).is_some());
        drop(lease);
        assert!(!current_is_locked());
        assert!(lock_current().is_some());
        *CURRENT_USER.write().unwrap() = None;
        AUTHS.write().unwrap().clear();
    }
}
