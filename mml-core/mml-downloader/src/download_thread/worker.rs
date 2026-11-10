//! 下载工作线程
//!
//! 线程平时在信号量上休眠，被唤醒后把任务队列里能取到的文件逐个下完；
//! 每个文件都套 panic 兜底，保证线程被打死时该文件仍能记上终态。

use super::*;

/// 下载工作线程
///
/// 封装一个 OS 线程，通过信号量机制在有新任务时被唤醒，
/// 从全局任务队列中取出文件执行下载。
pub(crate) struct DownloadThread {
    /// OS 线程句柄
    handle: Option<JoinHandle<()>>,
    /// 唤醒信号量
    sem: Arc<Sem>,
    /// 停止标志
    is_stop: Arc<AtomicBool>,
}

impl DownloadThread {
    /// 创建并启动下载线程
    ///
    /// - `index`: 线程序号，用于 UI 更新标识
    ///
    /// # 返回值
    ///
    /// 新建的下载线程，后台线程已启动并在信号量上等待唤醒
    pub fn new(index: u32) -> Self {
        let sem = Arc::new(Sem::new(0).unwrap());
        let is_stop = Arc::new(AtomicBool::new(false));

        let sem_clone = Arc::clone(&sem);
        let is_stop_clone = Arc::clone(&is_stop);

        let handle = thread::spawn(move || {
            loop {
                if is_stop_clone.load(Ordering::SeqCst) {
                    break;
                }

                // 等待信号量唤醒
                sem_clone.wait();

                if is_stop_clone.load(Ordering::SeqCst) {
                    break;
                }

                // 消化队列中所有可取的任务，取不到新任务再休眠：
                // 此前每次唤醒只下载一个文件，多文件任务入队只唤醒一轮，剩余文件会卡住
                while let Some(item) = crate::get_item() {
                    if is_stop_clone.load(Ordering::SeqCst) {
                        // 线程正在退出：手里这一项也要记终态，否则任务永远等不到"全部记完"。
                        // 退出多半来自停止下载或改并发数，按取消记账，不冒充失败
                        item.item.set_state(DownloadItemState::Cancel);
                        crate::update(index, &item.item);
                        item.task.mark_cancelled();
                        break;
                    }
                    // 一拿到项目就上报一次，别等 `download` 里的 HTTP 响应头到达才报：
                    // 中间那段（域名解析 / 建连 / 等响应头）界面会一直显示"所有下载线程空闲"，
                    // 而线程其实已经拿着文件在跑了；顺带让文件名在取到的这一刻就换成新的
                    item.item.set_state(DownloadItemState::Init);
                    crate::update(index, &item.item);
                    // panic 兜底：工作线程被打死的话，它手里那一项永远不会被标记，
                    // 任务就永远等不到"全部记完"，调用方跟着无限期挂住。
                    // 这里兜住并按失败记账，顺带把 panic 内容写进日志
                    let task = item.task.clone();
                    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        download(index, item)
                    }));
                    if let Err(payload) = res {
                        mml_log::error_type(ErrorType::DownloadThreadPanic(ErrorData {
                            error: panic_text(payload),
                        }));
                        task.fail();
                    }
                }
            }
        });

        Self {
            handle: Some(handle),
            sem,
            is_stop,
        }
    }

    /// 唤醒线程开始下载
    pub fn run(&self) {
        self.sem.signal();
    }

    /// 停止线程并等待退出
    pub fn stop(&mut self) {
        self.is_stop.store(true, Ordering::SeqCst);
        self.sem.signal();

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// 从 panic 载荷里取一句可读的原因
///
/// - `payload`: `catch_unwind` 返回的载荷
///
/// # 返回值
///
/// panic 时给出的 `String` / `&str`；都不是时返回一句兜底文案
fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(text) => *text,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(text) => (*text).to_string(),
            Err(_) => ErrorType::PanicPayloadUnknown.to_string(),
        },
    }
}
