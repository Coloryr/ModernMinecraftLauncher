//! 下载项目模块
//!
//! 定义单个下载文件的状态跟踪结构体 [`DownloadItem`]，
//! 使用原子变量实现线程安全的状态和进度更新。

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use mml_base::file_item::FileItemObj;

/// 下载项的状态机
///
/// ```text
/// Wait → Init → GetInfo → Download → Done
///                    ↓          ↓
///                 Action     Error / Cancel
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadItemState {
    /// 等待分配到下载线程，整数编码 0
    Wait,
    /// 正在下载文件数据，整数编码 1
    Download,
    /// 正在获取文件元信息，如大小，整数编码 2
    GetInfo,
    /// 暂停，整数编码 3
    Pause,
    /// 初始化中，整数编码 4
    Init,
    /// 执行下载后处理，如解压，整数编码 5
    Action,
    /// 下载完成，整数编码 6
    Done,
    /// 下载出错，整数编码 7
    Error,
    /// 被取消，如用户停止全部下载或退出启动器，整数编码 8
    Cancel,
}

impl DownloadItemState {
    /// 将状态转换为整数，用于原子存储
    ///
    /// # 返回值
    ///
    /// 状态对应的整数编码，`Wait` 为 0，其余依次递增
    pub fn state_to_int(&self) -> u32 {
        match self {
            DownloadItemState::Wait => 0,
            DownloadItemState::Download => 1,
            DownloadItemState::GetInfo => 2,
            DownloadItemState::Pause => 3,
            DownloadItemState::Init => 4,
            DownloadItemState::Action => 5,
            DownloadItemState::Done => 6,
            DownloadItemState::Error => 7,
            DownloadItemState::Cancel => 8,
        }
    }

    /// 从整数恢复状态
    ///
    /// - `value`: 状态整数编码
    ///
    /// # 返回值
    ///
    /// 返回编码对应的状态，未知整数统一回落到 `Error`
    pub fn int_to_state(value: u32) -> DownloadItemState {
        match value {
            0 => DownloadItemState::Wait,
            1 => DownloadItemState::Download,
            2 => DownloadItemState::GetInfo,
            3 => DownloadItemState::Pause,
            4 => DownloadItemState::Init,
            5 => DownloadItemState::Action,
            6 => DownloadItemState::Done,
            7 => DownloadItemState::Error,
            8 => DownloadItemState::Cancel,
            _ => DownloadItemState::Error,
        }
    }
}

/// 单个下载文件的状态跟踪
///
/// 所有字段使用原子变量，支持多线程安全的读写。
pub struct DownloadItem {
    /// 文件基本信息，含 URL、路径、哈希等
    pub base: FileItemObj,
    /// 下载时是否覆盖已存在的文件，缺省为 `false`
    pub overwrite: bool,
    /// 文件总大小，单位字节；元信息未知时为 0
    all_size: AtomicU64,
    /// 已下载大小，单位字节；断点续传时从续传起点开始累加
    now_size: AtomicU64,
    /// 当前下载状态，取值见 [`DownloadItemState`]
    state: AtomicU32,
    /// 累计错误次数，达到 5 次后放弃该文件
    error: AtomicU32,
}

impl DownloadItem {
    /// 创建下载项目
    ///
    /// - `file`: 文件基本信息
    ///
    /// # 返回值
    ///
    /// 新建的下载项，大小为 0、状态为 `Wait`
    pub fn new(file: FileItemObj) -> Self {
        DownloadItem {
            base: file,
            overwrite: false,
            all_size: AtomicU64::new(0),
            now_size: AtomicU64::new(0),
            state: AtomicU32::new(0),
            error: AtomicU32::new(0),
        }
    }

    /// 设置是否覆盖已存在文件，构建器模式
    ///
    /// - `overwrite`: `true` 表示下载时覆盖已存在的文件
    ///
    /// # 返回值
    ///
    /// 返回设置后的自身，便于链式调用
    pub fn set_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    /// 获取当前下载进度百分比
    ///
    /// # 返回值
    ///
    /// 返回 0.0–100.0 的进度；总大小未知或为 0 时返回 0.0
    pub fn progress(&self) -> f64 {
        let size = self.all_size.load(Ordering::Acquire);
        if size > 0 {
            (self.now_size.load(Ordering::Acquire) as f64 / size as f64) * 100.0
        } else {
            0.0
        }
    }

    /// 累加已下载字节数
    ///
    /// - `size`: 本次新增的字节数
    pub fn add_progress(&self, size: u64) {
        self.now_size.fetch_add(size, Ordering::Relaxed);
    }

    /// 设置已下载字节数，用于断点续传恢复
    ///
    /// - `size`: 已下载的字节数，断点续传时为续传起点
    pub fn set_now_size(&self, size: u64) {
        self.now_size.store(size, Ordering::Relaxed);
    }

    /// 设置文件总大小
    ///
    /// - `size`: 文件总大小，单位字节；元信息未知时传 0
    pub fn set_all_size(&self, size: u64) {
        self.all_size.store(size, Ordering::Relaxed);
    }

    /// 获取文件总大小
    ///
    /// # 返回值
    ///
    /// 返回文件总大小，单位字节；元信息未知时为 0
    pub fn get_all_size(&self) -> u64 {
        self.all_size.load(Ordering::Acquire)
    }

    /// 累加错误计数
    pub fn add_error(&self) {
        self.error.fetch_add(1, Ordering::Relaxed);
    }

    /// 设置当前下载状态
    ///
    /// - `state`: 要写入的状态
    pub fn set_state(&self, state: DownloadItemState) {
        self.state.store(state.state_to_int(), Ordering::Relaxed);
    }

    /// 获取当前下载状态
    ///
    /// # 返回值
    ///
    /// 返回最近一次写入的状态，未设置过时为 `Wait`
    pub fn get_state(&self) -> DownloadItemState {
        DownloadItemState::int_to_state(self.state.load(Ordering::Acquire))
    }

    /// 获取已下载字节数
    ///
    /// # 返回值
    ///
    /// 返回当前累计的已下载字节数
    pub fn get_now_size(&self) -> u64 {
        self.now_size.load(Ordering::Acquire)
    }
}

#[cfg(test)]
#[path = "../tests/unit/download_item_tests.rs"]
mod tests;
