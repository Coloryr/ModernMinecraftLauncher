//! 游戏启动统计
//! 统计启动器的启动次数、成功/失败次数、总游戏时长以及每次启动的时间段

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex, OnceLock, RwLock, atomic::AtomicBool},
    time::SystemTime,
};

use chrono::{DateTime, Local, TimeDelta};
use uuid::Uuid;

mod events;
mod file;
mod lifecycle;
mod save;
mod time;

/// 统计数据共享句柄
pub type GameCountObj = Arc<RwLock<CountObj>>;

/// 统计数据文件路径
pub(super) static COUNT_FILE: OnceLock<PathBuf> = OnceLock::new();
/// 统计线程运行标志
pub(super) static IS_RUN: AtomicBool = AtomicBool::new(false);
/// 是否正在保存数据
pub(super) static IS_SAVE: AtomicBool = AtomicBool::new(false);

/// 统计数据
pub(super) static COUNT: LazyLock<GameCountObj> =
    LazyLock::new(|| Arc::new(RwLock::new(CountObj::default())));

/// 上次读 / 写统计文件时的指纹：修改时间 + 长度
///
/// 用户可能在启动器运行期间手动替换 `count.dat`；落盘前对一下指纹，
/// 变了就不写，免得把人家的文件盖掉。
pub(super) static FILE_STAMP: LazyLock<Mutex<Option<(SystemTime, u64)>>> =
    LazyLock::new(|| Mutex::new(None));

/// 启动时统计文件是否读成功
///
/// 读不出来时（文件坏了、或者正被别的程序替换）这次运行**不落盘**，
/// 否则退出时的保存会把用户原来的文件盖掉。
pub(super) static READ_OK: AtomicBool = AtomicBool::new(true);

/// 正在运行的游戏及其上次统计时间
pub(super) static TIME_LIST: LazyLock<Mutex<HashMap<Uuid, DateTime<Local>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 落盘到 COUNT_DATA_FILE 的统计总表
#[derive(Clone, Debug)]
pub struct CountObj {
    /// 累计启动次数，含成功与失败
    pub launch_count: i64,
    /// 启动成功次数
    pub launch_done_count: i64,
    /// 启动失败次数
    pub launch_error_count: i64,
    /// 所有实例累计的游戏时长
    pub all_time: TimeDelta,
    /// 每个实例的运行时间段，按 uuid 分组
    pub game_runs: HashMap<Uuid, Vec<GameTimeData>>,
    /// 每个实例的启动记录，按 uuid 分组
    pub launch_logs: HashMap<Uuid, Vec<LaunchLogData>>,
    /// 实例名快照，按 uuid 分组；实例删掉后仍能显示名字
    pub game_names: HashMap<Uuid, String>,
}

impl Default for CountObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            launch_count: 0,
            launch_done_count: 0,
            launch_error_count: 0,
            all_time: TimeDelta::zero(),
            game_runs: HashMap::new(),
            launch_logs: HashMap::new(),
            game_names: HashMap::new(),
        }
    }
}

/// 一次游戏运行的时间段
#[derive(Clone, Debug)]
pub struct GameTimeData {
    /// 是否仍在运行，关闭后置 `false`
    pub now: bool,
    /// 开始时间，本地时区
    pub start_time: DateTime<Local>,
    /// 结束时间，本地时区；仍在运行时等于开始时间
    pub stop_time: DateTime<Local>,
}

/// 一条启动记录
#[derive(Clone, Debug)]
pub struct LaunchLogData {
    /// 记录时间，本地时区
    pub time: DateTime<Local>,
    /// 是否为错误日志；当前与原版一致，失败也记 `false`
    pub error: bool,
}

pub use self::events::{game_close, launch_done, launch_error};
pub use self::file::merge_file;
pub(crate) use self::lifecycle::remember_name;
pub use self::lifecycle::{get_count, init, stop};
pub use self::save::save_async;

#[cfg(test)]
#[path = "../../tests/unit/game_count_tests.rs"]
mod tests;
