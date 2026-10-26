//! 实例上的长任务
//!
//! 导出、生成实例信息、生成服务器实例都会整份读实例目录；删实例前必须知道有没有在跑，
//! 否则删到一半那些任务还在往已删 / 已改名的路径里读和写。资源窗口与添加资源窗口各有
//! 自己的任务表（`resource::instance_busy` / `add_resource::instance_busy`），这里补上
//! 导出与生成这两处，`ensure_idle` 一并查。
//!
//! 用法：命令开头 `let _task = instance_task::begin(&uuid)?;`，守卫活到任务结束。

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use uuid::Uuid;

use crate::err_keys;

/// 每个实例上正在跑的任务数
///
/// 用计数而不是集合：导出与生成实例信息可以同时开在同一实例上，其中一个结束不能把另一个
/// 的占用一起丢掉。
static TASKS: LazyLock<Mutex<HashMap<Uuid, usize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// 任务占位，丢弃时把计数减回去
pub(crate) struct TaskGuard(Uuid);

impl Drop for TaskGuard {
    fn drop(&mut self) {
        if let Ok(mut map) = TASKS.lock()
            && let Some(count) = map.get_mut(&self.0)
        {
            *count -= 1;
            if *count == 0 {
                map.remove(&self.0);
            }
        }
    }
}

/// 给一个实例挂上"正在处理"的标记
///
/// - `uuid`: 实例 uuid 字符串，即命令的入参
///
/// # 返回值
///
/// 见 [`TaskGuard`]；uuid 非法或该实例正在删除时返回错误键
pub(crate) fn begin(uuid: &str) -> Result<TaskGuard, String> {
    let id = Uuid::parse_str(uuid).map_err(|_| err_keys::UUID.to_string())?;
    // 正在删的实例不能再开工：标记撤掉之前它的文件随时会消失
    if crate::windows::main::deleting::is_deleting(&id) {
        return Err(err_keys::INSTANCE_BUSY.to_string());
    }
    // 启动准备中（含服务器包同步）不能再开长任务：两侧都会读写实例目录
    if crate::windows::main::launch_gate::is_launching(id) {
        return Err(err_keys::INSTANCE_BUSY.to_string());
    }
    {
        let mut map = TASKS.lock().unwrap();
        *map.entry(id).or_insert(0) += 1;
    }
    // 守卫在锁释放之后才造出来：丢弃它会去拿同一把锁，边持锁边丢就是自死锁
    Ok(TaskGuard(id))
}

/// 这个实例上有没有长任务在跑
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 有任务占着返回 `true`
pub(crate) fn is_busy(instance: &Uuid) -> bool {
    TASKS
        .lock()
        .map(|map| map.contains_key(instance))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_only_while_guarded() {
        let id = Uuid::new_v4();
        assert!(!is_busy(&id));
        let guard = begin(&id.to_string()).expect("占位");
        assert!(is_busy(&id));
        drop(guard);
        assert!(!is_busy(&id));
    }

    #[test]
    fn two_tasks_on_one_instance_need_two_drops() {
        let id = Uuid::new_v4();
        let first = begin(&id.to_string()).expect("占位");
        let second = begin(&id.to_string()).expect("占位");
        drop(first);
        assert!(is_busy(&id));
        drop(second);
        assert!(!is_busy(&id));
    }

    #[test]
    fn deleting_instance_refuses_new_task() {
        let id = Uuid::new_v4();
        let deleting = crate::windows::main::deleting::begin(id).expect("标记删除中");
        assert!(begin(&id.to_string()).is_err());
        drop(deleting);
        assert!(begin(&id.to_string()).is_ok());
    }

    #[test]
    fn invalid_uuid_is_an_error() {
        assert!(begin("not-a-uuid").is_err());
    }
}
