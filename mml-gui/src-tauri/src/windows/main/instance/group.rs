//! 分组与实例的增删移
//!
//! 实例归属与组内次序都在内核分组表的 `group_save.json`，本模块只转发；
//! 删除与改名都要先确认实例空闲，删除开始就广播变更并关掉挂在它名下的窗口。

use std::path::Path;

use tauri::{AppHandle, WebviewWindow};
use uuid::Uuid;

use crate::err_keys;
use crate::windows;

use crate::windows::main::{deleting, emit_instance_change, model};

/// 新建分组
///
/// 分组表在 mml-game 的 `group_save.json`，GUI 不再自己另存一份 ——
/// 之前模型里那份 `extra_groups` 既不落盘、又与内核表各说各话，
/// 新建的空分组下一次 `getGroups` 就被内核列表覆盖掉了。
///
/// - `app`: 应用句柄，用于发变更事件
/// - `name`: 分组名；重名不建组
///
/// # 返回值
///
/// 返回新分组的 uuid；名字不合法返回 `Err`，重名返回 `None`，前端据此提示"已存在"
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_add_group(app: AppHandle, name: String) -> Result<Option<String>, String> {
    mml_game::check_name(&name).map_err(|e| e.to_string())?;
    let Some(uuid) = mml_game::add_group(&name) else {
        return Ok(None);
    };
    emit_instance_change(&app, "group");
    Ok(Some(uuid.to_string()))
}

/// 删除分组，组内实例移入默认分组
///
/// - `app`: 应用句柄，用于发变更事件
/// - `uuid`: 分组 uuid；空 / 非法即默认分组
///
/// # 返回值
///
/// 真删掉了返回 `true`；默认分组或分组不存在返回 `false`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_remove_group(app: AppHandle, uuid: String) -> Result<bool, String> {
    // 空白 uuid 就是默认分组，不允许删除
    let Some(uuid) = windows::parse_group_id(Some(uuid)) else {
        return Ok(false);
    };
    // 核心分组表：组内实例移入默认分组，随后统一广播一次变更刷新列表
    if !mml_game::remove_group(&uuid) {
        return Ok(false);
    }
    emit_instance_change(&app, "group");
    Ok(true)
}

/// 调整分组显示顺序，默认分组只是初始排首位，同样可以换位置
///
/// 顺序由内核分组表保存：拿当前顺序、把该组挪到 `index` 位，再整表提交。
/// 只改内存里的数组是不行的 —— 文件里存的还是旧顺序，重启就回来了。
///
/// - `app`: 应用句柄，用于发变更事件
/// - `uuid`: 要挪的分组 uuid
/// - `index`: 目标位置，从 0 起；负数按 0、超出表长按表尾
///
/// # 返回值
///
/// 挪成功返回 `true`；uuid 为空或不是 uuid、该组不在表里时返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_move_group(app: AppHandle, uuid: String, index: i64) -> Result<bool, String> {
    // 这里**不**按 `parse_group_id` 的"非法 = 默认分组"兜底：空 / 非法的 uuid 指不到
    // 任何组，不像 `main_move_instance` 那样有默认分组这个有意义的落点，直接报错更安全
    let Some(uuid) = windows::parse_group_id(Some(uuid)) else {
        return Err(err_keys::UUID.to_string());
    };

    let mut list: Vec<Uuid> = mml_game::get_group_list()
        .into_iter()
        .map(|g| g.uuid)
        .collect();
    let from = list
        .iter()
        .position(|g| *g == uuid)
        .ok_or_else(|| err_keys::GROUP_NOT_FOUND.to_string())?;
    list.remove(from);
    let at = (index.max(0) as usize).min(list.len());
    list.insert(at, uuid);

    mml_game::reorder_groups(&list);
    emit_instance_change(&app, "group");
    Ok(true)
}

/// 移动实例到指定分组与组内位置：支持同组排序与跨组移动
///
/// 归属与组内次序都在内核分组表的 `group_save.json`：内核一次完成"落组 + 插到 index 位"，
/// 前端下次拉列表就按表的数组顺序拿到新的 `order`。
///
/// 这里不再改各实例的 `gui_setting.json` —— 那个 `Order` 已经是旧机制，
/// 两份顺序各写各的正是"拖完看着对了、重启就乱"的来源。
///
/// - `app`: 应用句柄，用于发变更事件
/// - `uuid`: 实例 uuid
/// - `group`: 目标分组 uuid；空 / 非法 = 默认分组
/// - `index`: 目标组内位置，从 0 起；负数按 0 处理
///
/// # 返回值
///
/// 移动成功返回 `true`；uuid 非法或实例不存在返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_move_instance(
    app: AppHandle,
    uuid: String,
    group: Option<String>,
    index: i64,
) -> Result<bool, String> {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return Err(err_keys::UUID.to_string());
    };
    if mml_game::get_instance(&id).is_none() {
        return Err(err_keys::GAME_NOT_FOUND.to_string());
    }

    // 前端传的是分组 uuid；空 / 非法 = 默认分组
    let group = windows::parse_group_id(group);

    mml_game::move_instance(&id, group, index.max(0) as usize);

    emit_instance_change(&app, "edit");
    Ok(true)
}

/// 删除实例：删除开始就把界面这一侧处理掉，文件挪回收站放后台线程慢慢做
///
/// 挪回收站大实例要几十秒，等它做完再更新界面的话，这期间列表里还留着能点的实例、
/// 挂在它名下的窗口也还开着。所以顺序是：先标记成删除中（`main_get_instances` 据此过滤）、
/// 广播变更、关窗口，再删文件；删失败时撤销标记并广播一次，实例回到列表里。
///
/// - `app`: 应用句柄，用于发变更事件
/// - `window`: 调用方窗口，用于取本窗口模型清理遗留数据
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 核心实例或本地存储任一删掉了就返回 `true`；都没有则 `false`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub async fn main_delete_instance(
    app: AppHandle,
    window: WebviewWindow,
    uuid: String,
) -> Result<bool, String> {
    let id = Uuid::parse_str(&uuid).ok();
    // 正忙时不许删，见 ensure_idle
    if let Some(id) = id {
        ensure_idle(&id)?;
    }

    // 标记、广播、关窗口都在删除文件之前：标记在位时列表命令就不会再列出它
    let guard = match id {
        Some(id) => match deleting::begin(id) {
            Some(guard) => Some(guard),
            // 同一个实例已经在删：不重复删，结果以那一次为准
            None => return Ok(false),
        },
        None => None,
    };
    if let Some(id) = id {
        emit_instance_change(&app, "remove");
        // 挂在它名下的窗口一并关掉：资源管理 / 日志 / 添加资源 / 配置文件
        crate::windows::lifecycle::close_instance_windows(&app, &id);
    }

    let deleted = match tauri::async_runtime::spawn_blocking(move || -> Result<bool, String> {
        let Some(id) = id else {
            return Ok(false);
        };
        // 核心实例：删除实例数据与文件
        if mml_game::get_instance(&id).is_none() {
            return Ok(false);
        }
        mml_game::delete_instance(&id).map_err(|e| e.to_string())?;
        Ok(true)
    })
    .await
    {
        Ok(result) => result,
        // 后台线程 panic：同样当作删失败处理，否则标记撤了、列表却停在"已删除"
        Err(err) => Err(err.to_string()),
    };

    let deleted = match deleted {
        Ok(deleted) => deleted,
        Err(e) => {
            // 删失败（文件被占用之类）：实例还在，撤掉标记再让列表把它放回来
            drop(guard);
            emit_instance_change(&app, "add");
            return Err(e);
        }
    };

    // 本地存储：清理遗留实例项与运行态缓存，含运行状态 / 日志 / 启动参数
    let mut removed_local = false;
    if let Ok(store) = model(&window) {
        let mut store = store.lock().unwrap();
        let before = store.instances.len();
        store.instances.retain(|i| i.uuid != uuid);
        store.args.remove(&uuid);
        store.running.remove(&uuid);
        removed_local = store.instances.len() < before;
    }
    drop(guard);
    Ok(deleted || removed_local)
}

/// 实例上正在进行的操作
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Busy {
    /// 游戏进程还在跑
    Running,
    /// 有任务在跑：下载、扫描、导出、生成、启动准备
    Task,
}

/// 查这个实例忙不忙
///
/// 只看 uuid 与实例目录，**不锁实例本身**：列表查询是在持有实例读锁时调用它的。
///
/// - `instance`: 实例 uuid
/// - `base`: 实例目录，下载任务按它判断
///
/// # 返回值
///
/// 忙时的种类；空闲是 `None`
pub(crate) fn busy_of(instance: &Uuid, base: &Path) -> Option<Busy> {
    // 游戏进程还在跑
    if mml_game::is_running(instance) {
        return Some(Busy::Running);
    }
    // 启动准备中：服务器包同步、文件检查都会读写实例目录，不能删、改名或复制
    if crate::windows::main::launch_gate::is_launching(*instance) {
        return Some(Busy::Task);
    }
    // 还有文件在往这个实例的目录里下（模组更新、资源下载等）
    if mml_downloader::is_busy_under(base) {
        return Some(Busy::Task);
    }
    // 添加资源窗口还在往这个实例里下资源
    if crate::windows::add_resource::instance_busy(instance) {
        return Some(Busy::Task);
    }
    // 资源窗口里还有操作在跑：读取各分类、检查更新、下载更新、自动标记…
    if crate::windows::resource::instance_busy(instance) {
        return Some(Busy::Task);
    }
    // 导出、生成实例信息、生成服务器实例：都会整份读这个实例的模组与配置
    if crate::windows::instance_task::is_busy(instance) {
        return Some(Busy::Task);
    }
    None
}

/// 实例正忙时给出要报给前端的错误
///
/// 删除与重命名都要过这一关：两种操作都动实例目录，而正在写的东西会跟着写到
/// 已经删掉 / 改名的路径上，留下对不上的目录与注册表。
///
/// - `instance`: 实例 uuid
///
/// # 返回值
///
/// 忙时是给前端翻的错误键；空闲是 `Ok(())`
pub(super) fn ensure_idle(instance: &Uuid) -> Result<(), String> {
    let Some(game) = mml_game::get_instance(instance) else {
        return Ok(());
    };
    let base = game.read().unwrap().get_base_path();
    match busy_of(instance, &base) {
        None => Ok(()),
        Some(Busy::Running) => Err(err_keys::INSTANCE_RUNNING.to_string()),
        Some(Busy::Task) => Err(err_keys::INSTANCE_BUSY.to_string()),
    }
}

/// 生成短 uuid，形如 `mml-<16 位十六进制>`，给不落内核的遗留实例用
///
/// 原来是 `subsec_nanos() ^ pid`：Windows 的时钟粒度远粗于 1ns，**同一个 tick 内
/// 连续创建两个实例会拿到同一个 id**，而本地存储正是按 uuid 索引的，会互相覆盖。
/// 这里改成完整纳秒 + 进程内自增序号：同 tick 靠序号区分，跨 tick 靠纳秒。
///
/// # 返回值
///
/// 16 位十六进制字符串，不含 `mml-` 前缀，由调用方拼
pub(super) fn uuid_short() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 进程内自增序号，乘一个奇数常量后异或，保证 seq 不同则结果必不同
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);

    format!(
        "{:016x}",
        nanos ^ ((std::process::id() as u64) << 32) ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 不存在实例目录的路径：下载任务按它判断，必然没有
    const NO_DIR: &str = "target/temp/not-an-instance";

    #[test]
    fn idle_instance_has_no_busy_reason() {
        let id = Uuid::new_v4();
        assert!(busy_of(&id, Path::new(NO_DIR)).is_none());
    }

    #[test]
    fn task_makes_instance_busy() {
        let id = Uuid::new_v4();
        let guard = crate::windows::instance_task::begin(&id.to_string()).expect("占位");
        assert_eq!(busy_of(&id, Path::new(NO_DIR)), Some(Busy::Task));
        drop(guard);
        assert!(busy_of(&id, Path::new(NO_DIR)).is_none());
    }

    /// 启动预约期间算忙，删除 / 改名会被拦住；预约释放后恢复
    #[test]
    fn launching_makes_instance_busy() {
        let id = Uuid::new_v4();
        let guard = crate::windows::main::launch_gate::begin_launch(id).expect("预约启动");
        assert_eq!(busy_of(&id, Path::new(NO_DIR)), Some(Busy::Task));
        drop(guard);
        assert!(busy_of(&id, Path::new(NO_DIR)).is_none());
    }
}
