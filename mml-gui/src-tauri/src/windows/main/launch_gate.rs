//! 实例启动与服务器包覆盖互斥，导入成功后由重启的新进程接管。

use crate::err_keys;
use std::{
    collections::HashSet,
    sync::{LazyLock, Mutex},
};
use uuid::Uuid;

static GATE: LazyLock<Gate> = LazyLock::new(Gate::default);

/// 启动准备和文件导入共享的互斥状态。
#[derive(Default)]
struct State {
    /// 从读取快照到启动结束的实例。
    launching: HashSet<Uuid>,
    /// 正在同步存档，阻止启动与第二次同步。
    syncing: HashSet<Uuid>,
    /// 正在导入或等待重启。
    importing: bool,
}

/// 保护进程内所有实例的启动与导入操作。
#[derive(Default)]
struct Gate {
    /// 启动与导入共用的短时状态锁。
    state: Mutex<State>,
}

/// 覆盖读取配置到完成启动的预约，任何退出路径都会释放。
pub(super) struct LaunchGuard<'a> {
    /// 本次登记所属的状态锁。
    gate: &'a Gate,
    /// 本次启动的实例。
    uuid: Uuid,
}

/// 同步期间持有的实例文件预约。
pub(crate) struct SyncGuard<'a> {
    /// 本次同步所属的状态锁。
    gate: &'a Gate,
    /// 本次同步的实例。
    uuid: Uuid,
}

impl Drop for SyncGuard<'_> {
    fn drop(&mut self) {
        self.gate.state.lock().unwrap().syncing.remove(&self.uuid);
    }
}

impl Drop for LaunchGuard<'_> {
    fn drop(&mut self) {
        self.gate.state.lock().unwrap().launching.remove(&self.uuid);
    }
}

/// 导入失败时释放预约，成功后保持到进程重启。
pub(crate) struct ImportGuard<'a> {
    /// 本次导入所属的状态锁。
    gate: &'a Gate,
    /// 成功后保持禁止启动，直到进程重启。
    committed: bool,
}

impl ImportGuard<'_> {
    /// 标记配置已导入，禁止旧进程继续启动实例。
    pub(crate) fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for ImportGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.gate.state.lock().unwrap().importing = false;
        }
    }
}

impl Gate {
    fn sync(&self, uuid: Uuid) -> Result<SyncGuard<'_>, String> {
        let mut state = self.state.lock().unwrap();
        if state.importing || state.launching.contains(&uuid) || state.syncing.contains(&uuid) {
            return Err(err_keys::INSTANCE_BUSY.into());
        }
        if mml_game::is_running(&uuid) {
            return Err(err_keys::INSTANCE_RUNNING.into());
        }
        state.syncing.insert(uuid);
        Ok(SyncGuard { gate: self, uuid })
    }

    fn launch(&self, uuid: Uuid) -> Result<LaunchGuard<'_>, String> {
        let mut state = self.state.lock().unwrap();
        if state.importing {
            return Err(err_keys::SERVER_PACK_BUSY.into());
        }
        if state.syncing.contains(&uuid) {
            return Err(err_keys::INSTANCE_BUSY.into());
        }
        if mml_game::is_running(&uuid) || !state.launching.insert(uuid) {
            return Err(err_keys::INSTANCE_RUNNING.into());
        }
        Ok(LaunchGuard { gate: self, uuid })
    }

    fn import(&self) -> Result<ImportGuard<'_>, String> {
        let mut state = self.state.lock().unwrap();
        if state.importing
            || !state.launching.is_empty()
            || !state.syncing.is_empty()
            || !mml_game::get_running_instances().is_empty()
        {
            return Err(err_keys::SERVER_PACK_BUSY.into());
        }
        state.importing = true;
        Ok(ImportGuard {
            gate: self,
            committed: false,
        })
    }
}

/// 预约实例启动。
/// - `uuid`: 待启动实例。
/// # 返回值
/// 启动预约，或正在导入、实例已运行的错误。
pub(super) fn begin_launch(uuid: Uuid) -> Result<LaunchGuard<'static>, String> {
    GATE.launch(uuid)
}

/// 预约服务器包导入。
/// # 返回值
/// 导入预约，或有启动、运行、导入任务的错误。
pub(crate) fn import_guard() -> Result<ImportGuard<'static>, String> {
    GATE.import()
}

/// 同步存档时阻止启动和同实例的重叠同步。
///
/// - `uuid`: 本地实例 UUID
///
/// # 返回值
///
/// 任务结束自动释放的预约；运行、启动或导入中返回占用错误。
pub(crate) fn begin_sync(uuid: Uuid) -> Result<SyncGuard<'static>, String> {
    GATE.sync(uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_blocks_launch_import_and_second_sync_until_released() {
        let gate = Gate::default();
        let uuid = Uuid::new_v4();
        let sync = gate.sync(uuid).unwrap();
        assert!(gate.launch(uuid).is_err());
        assert!(gate.sync(uuid).is_err());
        assert!(gate.import().is_err());
        assert!(gate.launch(Uuid::new_v4()).is_ok());
        drop(sync);
        let launch = gate.launch(uuid).unwrap();
        assert!(gate.sync(uuid).is_err());
        drop(launch);
        let import = gate.import().unwrap();
        assert!(gate.sync(uuid).is_err());
        drop(import);
        assert!(gate.sync(uuid).is_ok());
    }

    #[test]
    fn import_and_launch_reservations_are_exclusive() {
        let gate = Gate::default();
        let uuid = Uuid::new_v4();
        let launch = gate.launch(uuid).unwrap();
        assert!(gate.launch(uuid).is_err());
        assert!(gate.import().is_err());
        drop(launch);
        let import = gate.import().unwrap();
        assert!(gate.launch(uuid).is_err());
        assert!(gate.import().is_err());
        drop(import);
        assert!(gate.launch(uuid).is_ok());
    }

    #[test]
    fn successful_import_rejects_launches_until_restart() {
        let gate = Gate::default();
        let mut import = gate.import().unwrap();
        import.commit();
        drop(import);
        std::thread::scope(|scope| {
            scope
                .spawn(|| assert!(gate.launch(Uuid::new_v4()).is_err()))
                .join()
                .unwrap();
        });
        assert!(gate.import().is_err());
    }
}
