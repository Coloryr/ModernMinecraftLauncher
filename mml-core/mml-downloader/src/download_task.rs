//! 下载任务模块
//!
//! 定义批量下载任务的结构体 [`DownloadTask`]，
//! 管理一组文件的下载队列、进度统计和完成通知。

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

use crossbeam_queue::SegQueue;
use mml_base::file_item::FileItemObj;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::download_item::DownloadItem;

/// 下载任务
///
/// 封装一组待下载文件的队列，提供进度跟踪、取消、暂停和完成等待功能。
/// 使用无锁队列 [`SegQueue`] 存储待下载项，元素为共享的 `Arc<DownloadItem>`，
/// 并保留全部下载项的引用用于统计总大小与已下载大小。
pub(crate) struct DownloadTask {
    /// 任务编号，全局唯一自增 ID
    pub id: u64,
    /// 取消令牌，用于批量取消任务中的所有下载
    cancel: CancellationToken,
    /// 待下载文件队列，无锁
    items: SegQueue<Arc<DownloadItem>>,
    /// 全部下载项，队列取走后仍可统计进度与字节数
    all: Mutex<Vec<Arc<DownloadItem>>>,
    /// 文件总数量
    pub total_size: usize,
    /// 已完成数量
    pub completed_count: AtomicUsize,
    /// 失败数量
    pub failed_count: AtomicUsize,
    /// 是否暂停，暂停期间不再分配新文件，在途文件阻塞在写入循环
    paused: AtomicBool,
    /// 任务创建时间，用于统计已进行时间
    pub start: Instant,
    /// 任务完成信号量，调用 `wait_done()` 时阻塞直到任务完成
    sem: Semaphore,
}

impl DownloadTask {
    /// 创建下载任务
    ///
    /// - `items`: 需要下载的文件信息列表
    pub fn new(items: Vec<FileItemObj>) -> Self {
        let vec = SegQueue::new();

        let mut all = Vec::with_capacity(items.len());
        for item in items.into_iter().map(DownloadItem::new) {
            let item = Arc::new(item);
            vec.push(item.clone());
            all.push(item);
        }

        let size = vec.len();

        DownloadTask {
            id: crate::gen_task_id(),
            items: vec,
            all: Mutex::new(all),
            total_size: size,
            completed_count: AtomicUsize::new(0),
            failed_count: AtomicUsize::new(0),
            paused: AtomicBool::new(false),
            start: Instant::now(),
            cancel: CancellationToken::new(),
            sem: Semaphore::new(0),
        }
    }

    /// 检查任务是否完成，完成时发送信号
    ///
    /// 按“已完成 + 已失败 == 总数”判定：队列取空只说明文件都被线程取走，
    /// 不能说明都下载完了，在途文件会被误判为任务完成，导致提前返回。
    fn check_done(&self) {
        crate::update_task(self.id, self.progress());
        let completed = self.completed_count.load(Ordering::SeqCst);
        let failed = self.failed_count.load(Ordering::SeqCst);
        if completed + failed >= self.total_size {
            crate::task_done(self);
            self.sem.add_permits(1);
        }
    }

    /// 标记一个文件下载成功
    pub fn done(&self) {
        self.completed_count.fetch_add(1, Ordering::SeqCst);
        self.check_done();
    }

    /// 标记一个文件下载失败
    pub fn fail(&self) {
        self.failed_count.fetch_add(1, Ordering::SeqCst);
        self.check_done();
    }

    /// 从队列中取出一个待下载文件，无锁操作
    pub fn get_item(&self) -> Option<Arc<DownloadItem>> {
        self.items.pop()
    }

    /// 异步等待任务全部完成
    ///
    /// # 返回值
    ///
    /// `true` — 全部文件下载成功
    /// `false` — 有文件下载失败或任务被取消
    pub async fn wait_done(&self) -> bool {
        tokio::select! {
            res = async {
                let _ = self.sem.acquire().await.unwrap();
                self.total_size == self.completed_count.load(Ordering::SeqCst)
            } => res,
            // 取消时立即唤醒等待方：任务已移出队列，不再等在途文件
            // 自行失败收尾，那可能要等网络超时，长达数十秒
            _ = self.cancel.cancelled() => false,
        }
    }

    /// 取消此下载任务
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// 此任务是否已被取消
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// 暂停此任务，在途文件阻塞、不再分配新文件
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    /// 恢复此任务
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    /// 此任务是否处于暂停状态
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// 已进行时间，单位毫秒
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// 统计任务的总大小与已下载大小
    ///
    /// # 返回值
    ///
    /// 返回总大小与已下载大小，单位字节；元信息未知、无 Content-Length 的文件按 0 计
    pub fn bytes(&self) -> (u64, u64) {
        let all = self.all.lock().unwrap();
        all.iter().fold((0, 0), |(all_size, now_size), item| {
            (
                all_size + item.get_all_size(),
                now_size + item.get_now_size(),
            )
        })
    }

    /// 获取下载进度百分比，包含已完成和失败的文件
    fn progress(&self) -> f64 {
        if self.total_size > 0 {
            let completed = self.completed_count.load(Ordering::SeqCst);
            let failed = self.failed_count.load(Ordering::SeqCst);
            ((completed + failed) as f64 / self.total_size as f64) * 100.0
        } else {
            0.0
        }
    }
}
