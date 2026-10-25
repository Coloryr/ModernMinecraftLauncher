//! 正在删除的实例
//!
//! 删除要挪回收站，大实例几秒到几十秒；这段时间内核列表里还有它，但界面已经该当它不存在。
//! 实例列表只有 `main_get_instances` 一个来源，在这张表上把正在删的挡掉，
//! 主窗口、导出、生成服务器实例、设置这些下拉就都跟着同步，不必改事件契约。
//!
//! 删除失败时标记随守卫撤销，再发一次变更让列表把它放回来。

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use uuid::Uuid;

/// 正在删除的实例；进程级一份，与窗口无关
static DELETING: LazyLock<Mutex<HashSet<Uuid>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// 「删除中」的标记，丢弃即撤销
pub(crate) struct Deleting(Uuid);

impl Drop for Deleting {
    fn drop(&mut self) {
        if let Ok(mut set) = DELETING.lock() {
            set.remove(&self.0);
        }
    }
}

/// 标记一个实例开始删除
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 标记成功返回守卫，丢弃后标记撤销；同一实例已经在删时返回 `None`
pub(crate) fn begin(instance: Uuid) -> Option<Deleting> {
    {
        let mut set = DELETING.lock().unwrap();
        if !set.insert(instance) {
            return None;
        }
    }
    // 守卫必须在锁释放之后才造出来：丢弃它会去拿同一把锁，边持锁边丢就是自死锁
    Some(Deleting(instance))
}

/// 这个实例是不是正在删除
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 正在删返回 `true`，删除结束或被撤销后返回 `false`
pub(crate) fn is_deleting(instance: &Uuid) -> bool {
    DELETING
        .lock()
        .map(|set| set.contains(instance))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_only_while_guarded() {
        let id = Uuid::new_v4();
        assert!(!is_deleting(&id));
        let guard = begin(id).expect("首次标记");
        assert!(is_deleting(&id));
        drop(guard);
        assert!(!is_deleting(&id));
    }

    #[test]
    fn second_begin_is_rejected() {
        let id = Uuid::new_v4();
        let guard = begin(id).expect("首次标记");
        assert!(begin(id).is_none());
        drop(guard);
        assert!(begin(id).is_some());
    }

    #[test]
    fn other_instances_are_not_affected() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let guard = begin(a).expect("首次标记");
        assert!(!is_deleting(&b));
        drop(guard);
        assert!(!is_deleting(&a));
    }
}
