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
///                 Action     Error
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
}

impl DownloadItemState {
    /// 将状态转换为整数，用于原子存储
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
        }
    }

    /// 从整数恢复状态
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
mod tests {
    use super::*;

    /// 构造一个测试用下载项
    fn item() -> DownloadItem {
        DownloadItem::new(FileItemObj {
            name: "test.bin".to_string(),
            ..Default::default()
        })
    }

    /// 状态机：所有状态与整数编码可无损往返
    #[test]
    fn state_round_trip() {
        let states = [
            DownloadItemState::Wait,
            DownloadItemState::Download,
            DownloadItemState::GetInfo,
            DownloadItemState::Pause,
            DownloadItemState::Init,
            DownloadItemState::Action,
            DownloadItemState::Done,
            DownloadItemState::Error,
        ];

        for (index, state) in states.iter().enumerate() {
            assert_eq!(state.state_to_int(), index as u32, "{:?} 编码错误", state);
            assert_eq!(
                DownloadItemState::int_to_state(index as u32),
                *state,
                "{:?} 解码错误",
                state
            );
        }
    }

    /// 状态机：未知整数统一回落到 Error 状态
    #[test]
    fn unknown_int_maps_to_error() {
        assert_eq!(DownloadItemState::int_to_state(8), DownloadItemState::Error);
        assert_eq!(
            DownloadItemState::int_to_state(u32::MAX),
            DownloadItemState::Error
        );
    }

    /// 通过原子变量设置/读取状态
    #[test]
    fn state_store_and_load() {
        let item = item();
        assert_eq!(item.get_state(), DownloadItemState::Wait);

        item.set_state(DownloadItemState::Download);
        assert_eq!(item.get_state(), DownloadItemState::Download);

        item.set_state(DownloadItemState::Done);
        assert_eq!(item.get_state(), DownloadItemState::Done);
    }

    /// 进度计算：总大小为 0 时返回 0，避免除零
    #[test]
    fn progress_zero_size_is_zero() {
        let item = item();
        assert_eq!(item.progress(), 0.0);

        // 只设置已下载大小、总大小仍为 0
        item.add_progress(100);
        assert_eq!(item.get_now_size(), 100);
        assert_eq!(item.progress(), 0.0);
    }

    /// 进度计算：正常累加与断点续传恢复
    #[test]
    fn progress_accumulate_and_restore() {
        let item = item();
        item.set_all_size(200);
        assert_eq!(item.get_all_size(), 200);

        item.set_now_size(50); // 模拟断点续传起始位置
        assert_eq!(item.progress(), 25.0);

        item.add_progress(50);
        assert_eq!(item.get_now_size(), 100);
        assert_eq!(item.progress(), 50.0);
    }

    /// 覆盖标志的构建器写法
    #[test]
    fn overwrite_builder() {
        let item = item().set_overwrite(true);
        assert!(item.overwrite);

        let item = item.set_overwrite(false);
        assert!(!item.overwrite);
    }

    /// 新建下载项的默认值
    #[test]
    fn new_item_defaults() {
        let item = item();
        assert_eq!(item.get_all_size(), 0);
        assert_eq!(item.get_now_size(), 0);
        assert_eq!(item.get_state(), DownloadItemState::Wait);
        assert!(!item.overwrite);
        assert_eq!(item.base.name, "test.bin");
    }
}
