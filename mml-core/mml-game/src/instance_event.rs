//! 实例事件：退出 / 变更 / 目录外部增删 / 运行日志
//!
//! 订阅与发射集中在这里；实例注册、分组与运行句柄仍留在 `lib.rs`。
//! 事件类型是公开 API，`lib.rs` 按原路径再导出。

use std::sync::LazyLock;

use mml_base::events::EventArgHandler;
use uuid::Uuid;

use crate::game_log::GameLogItemObj;

/// 实例结束运行事件
pub struct InstanceExit {
    /// 实例 UUID
    pub uuid: Uuid,
    /// 进程退出码，正常退出为 0
    pub code: i32,
}

/// 实例修改事件参数
pub enum InstanceChange {
    /// 添加实例，携带实例 UUID
    AddInstance(Uuid),
    /// 删除实例，携带实例 UUID
    RemoveInstance(Uuid),
    /// 移动分组，依次为实例 UUID 与分组 uuid，`None` 表示默认分组
    MoveGroup(Uuid, Option<Uuid>),
}

/// 实例目录被外部增删的事件参数
///
/// 只由实例根目录的监视线程发出；启动器自己的增删走 `InstanceChange`。
pub enum InstanceWatchChange {
    /// 外部新增实例，携带实例 UUID
    Add(Uuid),
    /// 外部删除实例，携带实例 UUID
    Remove(Uuid),
}

/// 实例日志事件参数
pub enum InstanceLogType {
    /// 新增一条日志
    AddLog(GameLogItemObj),
    /// 清空日志
    ClearLog,
}

/// 实例日志事件
pub struct InstanceLog {
    /// 实例 UUID
    pub uuid: Uuid,
    /// 本次日志变更的内容
    pub log: InstanceLogType,
}

/// 实例退出事件
static EXIT_EVENT: LazyLock<EventArgHandler<InstanceExit>> =
    LazyLock::new(|| EventArgHandler::new());
/// 实例修改事件
static CHANGE_EVENT: LazyLock<EventArgHandler<InstanceChange>> =
    LazyLock::new(|| EventArgHandler::new());
/// 实例运行日志事件
static LOG_EVENT: LazyLock<EventArgHandler<InstanceLog>> = LazyLock::new(|| EventArgHandler::new());
/// 实例目录外部增删事件
static WATCH_EVENT: LazyLock<EventArgHandler<InstanceWatchChange>> =
    LazyLock::new(|| EventArgHandler::new());

/// 订阅实例退出事件
///
/// - `handler`: 事件回调
///
/// # 返回值
///
/// 返回回调 ID，供 `remove_exit` 取消订阅
pub fn add_exit<F>(handler: F) -> u64
where
    F: Fn(&InstanceExit) + Send + Sync + 'static,
{
    EXIT_EVENT.add_handler(handler)
}

/// 订阅实例变更事件
///
/// - `handler`: 事件回调
///
/// # 返回值
///
/// 返回回调 ID，供 `remove_change` 取消订阅
pub fn add_change<F>(handler: F) -> u64
where
    F: Fn(&InstanceChange) + Send + Sync + 'static,
{
    CHANGE_EVENT.add_handler(handler)
}

/// 订阅实例目录外部增删事件
///
/// - `handler`: 事件回调
///
/// # 返回值
///
/// 返回回调 ID，供 `remove_watch_change` 取消订阅
pub fn add_watch_change<F>(handler: F) -> u64
where
    F: Fn(&InstanceWatchChange) + Send + Sync + 'static,
{
    WATCH_EVENT.add_handler(handler)
}

/// 订阅实例运行日志事件
///
/// - `handler`: 事件回调
///
/// # 返回值
///
/// 返回回调 ID，供 `remove_run_log` 取消订阅
pub fn add_run_log<F>(handler: F) -> u64
where
    F: Fn(&InstanceLog) + Send + Sync + 'static,
{
    LOG_EVENT.add_handler(handler)
}

/// 取消订阅实例退出事件
///
/// - `id`: `add_exit` 返回的回调 ID
pub fn remove_exit(id: u64) {
    EXIT_EVENT.remove_handel(id);
}

/// 取消订阅实例变更事件
///
/// - `id`: `add_change` 返回的回调 ID
pub fn remove_change(id: u64) {
    CHANGE_EVENT.remove_handel(id);
}

/// 取消订阅实例目录外部增删事件
///
/// - `id`: `add_watch_change` 返回的回调 ID
pub fn remove_watch_change(id: u64) {
    WATCH_EVENT.remove_handel(id);
}

/// 取消订阅实例运行日志事件
///
/// - `id`: `add_run_log` 返回的回调 ID
pub fn remove_run_log(id: u64) {
    LOG_EVENT.remove_handel(id);
}

/// 发出实例退出事件，由退出轮询线程调用
///
/// - `uuid`: 实例 UUID
/// - `code`: 进程退出码
pub(crate) fn invoke_exit(uuid: Uuid, code: i32) {
    EXIT_EVENT.emit(InstanceExit { uuid, code });
}

/// 发出实例变更事件
///
/// - `change`: 变更内容
pub(crate) fn invoke_change(change: InstanceChange) {
    CHANGE_EVENT.emit(change);
}

/// 发出实例目录外部增删事件
///
/// - `change`: 变更内容
pub(crate) fn invoke_watch_change(change: InstanceWatchChange) {
    WATCH_EVENT.emit(change);
}

/// 发出实例运行日志事件
///
/// - `uuid`: 实例 UUID
/// - `log`: 日志变更内容
pub(crate) fn invoke_run_log(uuid: Uuid, log: InstanceLogType) {
    LOG_EVENT.emit(InstanceLog { uuid, log });
}
