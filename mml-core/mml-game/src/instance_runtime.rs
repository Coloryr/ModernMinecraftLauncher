//! 实例运行态：运行日志与运行句柄
//!
//! 运行日志按实例 UUID 存内存，运行句柄由启动流程登记、退出轮询线程消费。
//! 公开的查询 / 停止接口由 `lib.rs` 按原路径再导出。

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, LazyLock, RwLock},
};

use uuid::Uuid;

use crate::game_launch::InstanceHandle;
use crate::game_log::{GameLog, GameLogItemObj, InstanceRuntimeLog};
use crate::instance_event::{InstanceLogType, invoke_run_log};
use crate::launcher::instance_setting_obj::InstanceSettingObj;

/// 各实例的运行日志，按实例 UUID 存内存
pub(crate) static RUNTIME_LOGS: LazyLock<RwLock<HashMap<Uuid, InstanceRuntimeLog>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 正在运行的实例句柄，按实例 UUID 存内存
static HANDELS: LazyLock<RwLock<HashMap<Uuid, InstanceHandle>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 添加运行日志
///
/// - `uuid`: 实例 UUID
/// - `data`: 日志原文
pub(crate) fn add_game_log(uuid: &Uuid, data: &str) {
    // 先入库、后发事件，而且要放开写锁再发：事件回调可能回头读日志（GUI 就会读到），
    // 持锁发事件会自锁。每一行都要发 —— 只在新建日志那一次发，实时日志就只剩第一行
    let item = {
        let mut logs = RUNTIME_LOGS.write().unwrap();
        if !logs.contains_key(uuid) {
            logs.insert(uuid.clone(), InstanceRuntimeLog::new());
        }
        logs.get_mut(uuid).unwrap().add_game_log(&data)
    };

    invoke_run_log(uuid.clone(), InstanceLogType::AddLog(item));
}

/// 添加运行日志
///
/// - `uuid`: 实例 UUID
/// - `data`: 已解析的日志项
pub(crate) fn add_game_log_item(uuid: &Uuid, data: GameLog) {
    // 与 [`add_game_log`] 同一条：每一行都发事件，且发之前放开写锁
    let item = {
        let mut logs = RUNTIME_LOGS.write().unwrap();
        if !logs.contains_key(uuid) {
            logs.insert(uuid.clone(), InstanceRuntimeLog::new());
        }
        logs.get_mut(uuid).unwrap().add_log_item(data)
    };

    invoke_run_log(uuid.clone(), InstanceLogType::AddLog(item));
}

/// 清理日志
///
/// - `uuid`: 实例 UUID
pub(crate) fn clear_game_log(uuid: &Uuid) {
    let mut logs = RUNTIME_LOGS.write().unwrap();
    if let Some(log) = logs.get_mut(uuid) {
        log.clear();
    } else {
        let log = InstanceRuntimeLog::new();
        logs.insert(uuid.clone(), log);
    }

    invoke_run_log(uuid.clone(), InstanceLogType::ClearLog);
}

/// 添加启动的游戏实例
///
/// - `handel`: 实例运行句柄
pub(crate) fn add_run_game(handel: InstanceHandle) {
    let mut games = HANDELS.write().unwrap();
    games.insert(handel.uuid, handel);
}

/// 推进运行句柄并摘掉已退出的，返回退出事件
///
/// 由 `lib.rs` 的轮询线程每秒调用一次；句柄表只在内部持有，调用方拿不到锁。
///
/// # 返回值
///
/// 已退出的实例 uuid 与退出码
pub(crate) fn poll_exited() -> Vec<(Uuid, i32)> {
    let mut exited = Vec::new();
    let mut handles = HANDELS.write().unwrap();
    let done: Vec<Uuid> = handles
        .iter()
        .filter_map(|(uuid, handle)| {
            handle.tick();
            if handle.is_exit() { Some(*uuid) } else { None }
        })
        .collect();
    for uuid in done {
        if let Some(handle) = handles.remove(&uuid) {
            exited.push((uuid, handle.code()));
        }
    }
    exited
}

/// 获取正在运行的实例 UUID 列表
///
/// # 返回值
///
/// 返回运行中实例的 UUID 列表，顺序不保证
pub fn get_running_instances() -> Vec<Uuid> {
    HANDELS.read().unwrap().keys().cloned().collect()
}

/// 判断实例是否正在运行
///
/// - `uuid`: 实例 UUID
///
/// # 返回值
///
/// 正在运行返回 `true`
pub fn is_running(uuid: &Uuid) -> bool {
    HANDELS.read().unwrap().contains_key(uuid)
}

/// 强制结束正在运行的实例
///
/// - `uuid`: 实例 UUID
pub fn stop_game(uuid: &Uuid) {
    if let Some(handle) = HANDELS.read().unwrap().get(uuid) {
        handle.kill();
    }
}

impl InstanceSettingObj {
    /// 获取运行中的日志
    ///
    /// # 返回值
    ///
    /// 返回当前运行日志快照；没有运行日志返回 `None`
    pub fn get_runtime_log(&self) -> Option<Arc<VecDeque<GameLogItemObj>>> {
        let logs = RUNTIME_LOGS.read().unwrap();
        logs.get(&self.uuid)
            .map(|log| log.logs.read().unwrap().clone())
    }
}
