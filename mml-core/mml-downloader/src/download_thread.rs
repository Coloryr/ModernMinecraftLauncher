//! 下载线程模块
//!
//! 实现下载工作线程和文件下载的核心逻辑。
//!
//! # 下载流程
//!
//! 1. **检查已存在文件** — 文件已存在且哈希匹配 → 跳过
//! 2. **发起 HTTP 请求** — 支持 Range 断点续传，需服务器返回 `Accept-Ranges: bytes`
//! 3. **流式写入临时文件** — 边下载边写入，实时更新进度
//! 4. **下载后校验** — 检查文件大小和哈希值
//! 5. **后处理** — 解压 native 库 / 存档文件
//! 6. **移至目标路径** — 校验通过后原子移动临时文件到最终位置
//!
//! # 重试策略
//!
//! 单文件最多重试 5 次，超过后放弃。

use std::{
    fs::File,
    future::Future,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use mml_base::{
    archives::{ArchiveType, BaseArchive},
    file_item::{FileHash, LaterRun},
    hash_helper::{self, HashType},
};
use mml_names::i18_items::error_type::{
    CoreResult, DownloadFileHashErrorData, DownloadFileOverFailData, DownloadFileSizeErrorData,
    ErrorData,
    ErrorType::{self, StreamError},
};
use mml_sys::path_helper;
use reqwest::Response;
use semka::Sem;

use crate::{
    DownloadObj, download_item::DownloadItemState, download_task::DownloadTask, later_tasks,
};

/// 用于在下载线程中执行异步任务的 Tokio 运行时
///
/// 全部下载线程共用 `OnceLock` 里这一份 `current_thread` 运行时，
/// 各线程通过 `block_on` 把异步 HTTP 请求转成同步调用。
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// 在当前线程中阻塞执行异步任务
///
/// - `f`: 待执行的异步任务
///
/// # 返回值
///
/// 返回异步任务的输出
fn block_on<F: Future>(f: F) -> F::Output {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
        })
        .block_on(f)
}

/// 在当前线程中阻塞执行一个 HTTP 请求，任务被取消时立即放弃它
///
/// 请求挂住时（服务端不回包、连接半开）自身要等读超时才结束，而 `stop()` 会 join
/// 下载线程，取消只靠请求结束后的标志检查就来不及了 —— 退出会跟着一起卡住。
/// 这里让请求与取消令牌竞速，取消时直接丢弃请求 Future，连接随之断开。
///
/// - `task`: 请求所属的下载任务，用它的取消令牌
/// - `fut`: 待执行的请求 Future
///
/// # 返回值
///
/// `None` 表示任务已被取消，请求结果作废；其余情况是请求自身的输出
fn block_on_cancellable<F, T>(task: &Arc<DownloadTask>, fut: F) -> Option<T>
where
    F: Future<Output = T>,
{
    let cancel = task.cancel_token();
    block_on(async move {
        tokio::select! {
            res = fut => Some(res),
            _ = cancel.cancelled() => None,
        }
    })
}

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
                        break;
                    }
                    // 一拿到项目就上报一次，别等 `download` 里的 HTTP 响应头到达才报：
                    // 中间那段（域名解析 / 建连 / 等响应头）界面会一直显示"所有下载线程空闲"，
                    // 而线程其实已经拿着文件在跑了；顺带让文件名在取到的这一刻就换成新的
                    item.item.set_state(DownloadItemState::Init);
                    crate::update(index, &item.item);
                    download(index, item);
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

/// 检查错误次数是否超过阈值 5 次
///
/// - `err`: 错误信息
/// - `times`: 当前错误次数，可变引用，会自增
///
/// # 返回值
///
/// `true` — 应停止重试，`false` — 可继续重试
fn is_need_err(err: ErrorType, times: &mut i32) -> bool {
    *times += 1;
    mml_log::error_type(err);
    if *times > 5 {
        return true;
    }

    return false;
}

/// 下载单个文件
///
/// - `index`: 下载线程序号
/// - `obj`: 下载项目，包含任务和文件信息
fn download(index: u32, mut obj: DownloadObj) {
    // ============================================================
    // 第一步：检查文件是否已存在
    // ============================================================
    if obj.item.base.file.exists() {
        if obj.item.overwrite {
            if let Err(err) = path_helper::delete(&obj.item.base.file) {
                mml_log::error_type(ErrorType::DownloadFileOverFail(DownloadFileOverFailData {
                    file: obj.item.base.file.clone(),
                    error: Box::new(err),
                }));
                // 这条早退也要落到终态，否则界面上这一行会一直停在取到项目时的「初始化」
                obj.item.set_state(DownloadItemState::Error);
                crate::update(index, &obj.item);
                obj.task.fail();
                return;
            }
        } else {
            let config = mml_config::read_config();
            let mut file = path_helper::open_read(&obj.item.base.file).unwrap();
            if config.http.check_file {
                let check = check_hash(&obj.item.base.file, &obj.item.base.hash, &mut file);
                if let Err(err) = check {
                    mml_log::error_type(err);
                } else {
                    // 文件已存在且哈希匹配，直接跳过
                    obj.item.set_state(DownloadItemState::Done);
                    crate::update(index, &obj.item);
                    obj.task.done();
                    return;
                }
            }
        }
    }

    // ============================================================
    // 第二步：循环下载，失败时重试
    // ============================================================
    let mut times = 0;
    let mut use_break = false;
    let mut server_ranges = true;
    let mut is_keep = false;
    // 重试次数耗尽后放弃此文件，不能把残缺的临时文件移到目标路径
    let mut is_fail = false;
    // 任务被取消：与失败分开，见循环末尾的收尾
    let mut is_cancel = false;

    let mut temp_file;

    loop {
        temp_file = crate::gen_temp_file();

        // 任务已被取消：立即放弃，不再重试；stop 时下载线程要 join，取消必须尽快打断在途下载
        if obj.task.is_cancelled() {
            is_cancel = true;
            break;
        }

        let file = if is_keep {
            path_helper::open_append(&temp_file)
        } else {
            path_helper::open_write(&temp_file)
        };

        if let Err(err) = file {
            obj.item.add_error();
            if is_need_err(err, &mut times) {
                is_fail = true;
                break;
            } else {
                continue;
            }
        }

        let mut file = file.unwrap();

        // 发起 HTTP 请求，支持断点续传。客户端先落成局部变量：请求 Future 借用它，
        // 写成 `get_work_client().get(..)` 的话临时 Arc 活不过这个 Future
        let client = mml_net::get_work_client();
        let mut resp = if use_break && server_ranges {
            let result = block_on_cancellable(
                &obj.task,
                client.get_ranges(&obj.item.base.url, obj.item.get_now_size()),
            );
            // 取消优先于重试：请求是被取消打断的，不是网络故障
            let Some(result) = result else {
                is_cancel = true;
                break;
            };
            if let Err(err) = result {
                if obj.task.is_cancelled() {
                    is_cancel = true;
                    break;
                }
                obj.item.add_error();
                if is_need_err(err, &mut times) {
                    is_fail = true;
                    break;
                } else {
                    continue;
                }
            }

            let resp = result.unwrap();

            if !resp.status().is_success() {
                server_ranges = false;
                continue;
            }

            is_keep = true;

            resp
        } else {
            let result = block_on_cancellable(&obj.task, client.get(&obj.item.base.url));
            let Some(result) = result else {
                is_cancel = true;
                break;
            };
            if let Err(err) = result {
                if obj.task.is_cancelled() {
                    is_cancel = true;
                    break;
                }
                obj.item.add_error();
                if is_need_err(err, &mut times) {
                    is_fail = true;
                    break;
                } else {
                    continue;
                }
            }

            let resp = result.unwrap();

            obj.item.set_now_size(0);
            obj.item.set_all_size(match resp.content_length() {
                Some(data) => data,
                None => 0,
            });

            resp
        };

        // 检测服务器是否支持断点续传
        if let Some(range) = resp.headers().get("Accept-Ranges")
            && range.to_str().unwrap().starts_with("bytes")
        {
            use_break = true;
        }

        obj.item.set_state(DownloadItemState::GetInfo);
        crate::update(index, &obj.item);

        // ============================================================
        // 第三步：流式写入文件
        // ============================================================
        let result = block_on(write_file(index, &mut obj, &mut resp, &mut file));
        if let Err(err) = result {
            // 取消是 write_file 自己报出来的，优先于失败判定
            if obj.task.is_cancelled() {
                is_cancel = true;
                break;
            }
            obj.item.add_error();
            if is_need_err(err, &mut times) {
                is_fail = true;
                break;
            } else {
                continue;
            }
        }

        break;
    }

    // ============================================================
    // 第四步：移动到最终路径
    // ============================================================
    if is_cancel {
        // 取消不是失败：状态记"已取消"、不计入任务失败数，残缺的临时文件也不留在缓存里
        let _ = path_helper::delete(&temp_file);
        obj.item.set_state(DownloadItemState::Cancel);
        crate::update(index, &obj.item);
        obj.task.mark_cancelled();
        return;
    }

    if is_fail {
        // 下载失败：不移动残缺的临时文件，删除后按失败计入任务
        let _ = path_helper::delete(&temp_file);
        obj.item.set_state(DownloadItemState::Error);
        crate::update(index, &obj.item);
        obj.task.fail();
        return;
    }

    path_helper::move_file(&temp_file, &obj.item.base.file).unwrap();

    // ============================================================
    // 第五步：后处理，解压 native 库 / 存档
    // ============================================================
    let res: Result<(), ErrorType> = match &obj.item.base.later {
        LaterRun::None => Ok(()),
        LaterRun::UnpackNative(path_buf) => match path_helper::open_read(&obj.item.base.file) {
            Ok(file) => later_tasks::unpack_native(path_buf, file),
            Err(err) => Err(err),
        },
        LaterRun::UnpackSave(path_buf) => {
            BaseArchive::decompress(ArchiveType::Zip, &obj.item.base.file, path_buf, None)
        }
    };

    if let Err(err) = res {
        mml_log::error_type(err);
    }

    obj.item.set_state(DownloadItemState::Done);
    crate::update(index, &obj.item);
    obj.task.done();
}

/// 流式写入下载数据到文件
///
/// - `index`: 线程序号
/// - `obj`: 下载项目
/// - `resp`: HTTP 响应流
/// - `file`: 目标文件句柄
///
/// # 返回值
///
/// 写入与校验成功返回 `Ok(())`；任务被取消、写入失败或大小/哈希校验不通过时返回相应错误
async fn write_file(
    index: u32,
    obj: &mut DownloadObj,
    resp: &mut Response,
    file: &mut File,
) -> CoreResult<()> {
    // 取消令牌单独拿出来：下面读块要与它竞速，不能借 obj
    let cancel = obj.task.cancel_token();

    loop {
        // 任务已被取消：立即中断在途下载，stop 或用户取消时不再等当前文件下完
        if obj.task.is_cancelled() {
            obj.item.set_state(DownloadItemState::Cancel);
            crate::update(index, &obj.item);
            return Err(ErrorType::TaskCancel);
        }

        // 任务暂停：原地等待恢复，期间同时响应取消，避免暂停期间无法取消
        while obj.task.is_paused() {
            if obj.task.is_cancelled() {
                obj.item.set_state(DownloadItemState::Cancel);
                crate::update(index, &obj.item);
                return Err(ErrorType::TaskCancel);
            }
            thread::sleep(std::time::Duration::from_millis(50));
        }

        // 读块同样与取消竞速：服务端停发时 chunk() 要等读超时才结束，取消不能陪着一起等
        let chunk = tokio::select! {
            chunk = resp.chunk() => Some(chunk),
            _ = cancel.cancelled() => None,
        };
        let Some(chunk) = chunk else {
            obj.item.set_state(DownloadItemState::Cancel);
            crate::update(index, &obj.item);
            return Err(ErrorType::TaskCancel);
        };

        match chunk {
            Ok(None) => break,
            Ok(Some(data)) => {
                file.write_all(&data).map_err(|err| {
                    StreamError(ErrorData {
                        error: err.to_string(),
                    })
                })?;

                obj.item.set_state(DownloadItemState::Download);
                obj.item.add_progress(data.len() as u64);

                crate::update(index, &obj.item);
            }
            Err(e) => {
                return Err(StreamError(ErrorData {
                    error: e.to_string(),
                }));
            }
        }
    }

    // 下载完成后校验
    let config = mml_config::read_config();
    if config.http.check_file {
        let now = file.stream_position().unwrap();
        if now != obj.item.get_all_size() {
            obj.item.set_state(DownloadItemState::Error);

            crate::update(index, &obj.item);

            return Err(ErrorType::DownloadFileSizeError(
                DownloadFileSizeErrorData {
                    file: obj.item.base.file.clone(),
                    url: obj.item.base.url.clone(),
                    now: now,
                    size: obj.item.get_all_size(),
                },
            ));
        }

        file.seek(SeekFrom::Start(0)).unwrap();
        return check_hash(&obj.item.base.file, &obj.item.base.hash, file);
    }

    Ok(())
}

/// 校验文件哈希
///
/// 支持 MD5、SHA1、SHA256、SHA512 及组合校验：SHA1+SHA256、SHA1+SHA512，
/// 比较时忽略大小写。
///
/// - `file`: 文件路径
/// - `hash`: 期望的哈希值
/// - `stream`: 文件读取流
///
/// # 返回值
///
/// 校验通过返回 `Ok(())`；不匹配或哈希计算失败时返回相应错误
fn check_hash<R: Read + Seek>(file: &PathBuf, hash: &FileHash, stream: &mut R) -> CoreResult<()> {
    match hash {
        FileHash::None => Ok(()),
        FileHash::Md5(md5) => match hash_helper::gen_hash_from_reader(HashType::Md5, stream) {
            Ok(hash) => {
                if hash.eq_ignore_ascii_case(md5) {
                    Ok(())
                } else {
                    Err(ErrorType::DownloadFileHashError(
                        DownloadFileHashErrorData {
                            file: file.clone(),
                            now: hash.clone(),
                            hash: md5.clone(),
                        },
                    ))
                }
            }
            Err(err) => Err(err),
        },
        FileHash::Sha1(sha1) => match hash_helper::gen_hash_from_reader(HashType::Sha1, stream) {
            Ok(hash) => {
                if hash.eq_ignore_ascii_case(sha1) {
                    Ok(())
                } else {
                    Err(ErrorType::DownloadFileHashError(
                        DownloadFileHashErrorData {
                            file: file.clone(),
                            now: hash.clone(),
                            hash: sha1.clone(),
                        },
                    ))
                }
            }
            Err(err) => Err(err),
        },
        FileHash::Sha256(sha256) => {
            match hash_helper::gen_hash_from_reader(HashType::Sha256, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha256) {
                        Ok(())
                    } else {
                        Err(ErrorType::DownloadFileHashError(
                            DownloadFileHashErrorData {
                                file: file.clone(),
                                now: hash.clone(),
                                hash: sha256.clone(),
                            },
                        ))
                    }
                }
                Err(err) => Err(err),
            }
        }
        FileHash::Sha1Sha256(sha1, sha256) => {
            let sha1 = match hash_helper::gen_hash_from_reader(HashType::Sha1, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha1) {
                        Ok(())
                    } else {
                        Err(ErrorType::DownloadFileHashError(
                            DownloadFileHashErrorData {
                                file: file.clone(),
                                now: hash.clone(),
                                hash: sha1.clone(),
                            },
                        ))
                    }
                }
                Err(err) => Err(err),
            };

            if sha1.is_err() {
                return sha1;
            }

            stream.seek(SeekFrom::Start(0)).map_err(|err| {
                ErrorType::StreamError(ErrorData {
                    error: err.to_string(),
                })
            })?;

            let sha256 = match hash_helper::gen_hash_from_reader(HashType::Sha256, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha256) {
                        Ok(())
                    } else {
                        Err(ErrorType::DownloadFileHashError(
                            DownloadFileHashErrorData {
                                file: file.clone(),
                                now: hash.clone(),
                                hash: sha256.clone(),
                            },
                        ))
                    }
                }
                Err(err) => Err(err),
            };

            if sha256.is_err() {
                return sha256;
            }

            Ok(())
        }
        FileHash::Sha512(sha512) => {
            match hash_helper::gen_hash_from_reader(HashType::Sha512, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha512) {
                        Ok(())
                    } else {
                        Err(ErrorType::DownloadFileHashError(
                            DownloadFileHashErrorData {
                                file: file.clone(),
                                now: hash.clone(),
                                hash: sha512.clone(),
                            },
                        ))
                    }
                }
                Err(err) => Err(err),
            }
        }
        FileHash::Sha1Sha512(sha1, sha512) => {
            let sha1 = match hash_helper::gen_hash_from_reader(HashType::Sha1, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha1) {
                        Ok(())
                    } else {
                        Err(ErrorType::DownloadFileHashError(
                            DownloadFileHashErrorData {
                                file: file.clone(),
                                now: hash.clone(),
                                hash: sha1.clone(),
                            },
                        ))
                    }
                }
                Err(err) => Err(err),
            };

            if sha1.is_err() {
                return sha1;
            }

            stream.seek(SeekFrom::Start(0)).map_err(|err| {
                ErrorType::StreamError(ErrorData {
                    error: err.to_string(),
                })
            })?;

            let sha512 = match hash_helper::gen_hash_from_reader(HashType::Sha512, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha512) {
                        Ok(())
                    } else {
                        Err(ErrorType::DownloadFileHashError(
                            DownloadFileHashErrorData {
                                file: file.clone(),
                                now: hash.clone(),
                                hash: sha512.clone(),
                            },
                        ))
                    }
                }
                Err(err) => Err(err),
            };

            if sha512.is_err() {
                return sha512;
            }

            Ok(())
        }
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use mml_base::file_item::FileHash;

    use super::*;

    /// "hello" 的标准哈希值，公开测试向量
    const HELLO_MD5: &str = "5d41402abc4b2a76b9719d911017c592";
    const HELLO_SHA1: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    const HELLO_SHA512: &str = "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043";

    /// 在临时目录创建内容为 "hello" 的测试文件
    ///
    /// - `name`: 临时子目录的用例特征
    ///
    /// # 返回值
    ///
    /// 返回已写入 `hello` 的文件路径
    fn make_hello_file(name: &str) -> PathBuf {
        let dir = crate::test_util::make_temp_dir(name);
        let file = dir.join("hello.txt");
        fs::write(&file, b"hello").unwrap();
        file
    }

    /// 重试次数未达上限 5 次时继续重试
    #[test]
    fn is_need_err_allows_under_threshold() {
        crate::test_util::ensure_env();

        let mut times = 0;
        for _ in 0..5 {
            assert!(!is_need_err(ErrorType::TaskCancel, &mut times));
        }
        assert_eq!(times, 5);
    }

    /// 重试次数超过上限 5 次后放弃
    #[test]
    fn is_need_err_stops_over_threshold() {
        crate::test_util::ensure_env();

        let mut times = 5;
        assert!(is_need_err(ErrorType::TaskCancel, &mut times));
        assert_eq!(times, 6);
    }

    /// 无需校验时直接通过
    #[test]
    fn check_hash_none_passes() {
        let file = make_hello_file("hash-none");
        let mut stream = fs::File::open(&file).unwrap();
        assert!(check_hash(&file, &FileHash::None, &mut stream).is_ok());
    }

    /// 单哈希 MD5/SHA1/SHA256/SHA512 匹配，且大小写不敏感
    #[test]
    fn check_hash_single_matches() {
        let file = make_hello_file("hash-single");
        let cases = [
            (FileHash::Md5(HELLO_MD5.to_string()), HELLO_MD5),
            (FileHash::Sha1(HELLO_SHA1.to_string()), HELLO_SHA1),
            (FileHash::Sha256(HELLO_SHA256.to_string()), HELLO_SHA256),
            (FileHash::Sha512(HELLO_SHA512.to_string()), HELLO_SHA512),
        ];

        for (hash, expected) in cases {
            // 小写匹配
            let mut stream = fs::File::open(&file).unwrap();
            assert!(
                check_hash(&file, &hash, &mut stream).is_ok(),
                "{expected} 应校验通过"
            );

            // 大写匹配，校验值大小写不敏感
            let upper = hash_to_upper(&hash);
            let mut stream = fs::File::open(&file).unwrap();
            assert!(
                check_hash(&file, &upper, &mut stream).is_ok(),
                "{expected} 大写应校验通过"
            );
        }
    }

    /// 单哈希不匹配时返回错误
    #[test]
    fn check_hash_single_mismatch() {
        let file = make_hello_file("hash-mismatch");
        let wrong = FileHash::Sha256("0".repeat(64));
        let mut stream = fs::File::open(&file).unwrap();
        let result = check_hash(&file, &wrong, &mut stream);
        assert!(result.is_err(), "错误的 SHA256 应校验失败");
    }

    /// 组合校验 SHA1+SHA256：两者都匹配才通过
    #[test]
    fn check_hash_sha1_sha256() {
        let file = make_hello_file("hash-combo256");

        // 都匹配
        let mut stream = fs::File::open(&file).unwrap();
        let combo = FileHash::Sha1Sha256(HELLO_SHA1.to_string(), HELLO_SHA256.to_string());
        assert!(check_hash(&file, &combo, &mut stream).is_ok());

        // SHA1 匹配但 SHA256 错误
        let mut stream = fs::File::open(&file).unwrap();
        let combo = FileHash::Sha1Sha256(HELLO_SHA1.to_string(), "0".repeat(64));
        assert!(check_hash(&file, &combo, &mut stream).is_err());

        // SHA1 错误时短路，不再校验第二个
        let mut stream = fs::File::open(&file).unwrap();
        let combo = FileHash::Sha1Sha256("0".repeat(40), HELLO_SHA256.to_string());
        assert!(check_hash(&file, &combo, &mut stream).is_err());
    }

    /// 组合校验 SHA1+SHA512：两者都匹配才通过
    #[test]
    fn check_hash_sha1_sha512() {
        let file = make_hello_file("hash-combo512");

        // 都匹配
        let mut stream = fs::File::open(&file).unwrap();
        let combo = FileHash::Sha1Sha512(HELLO_SHA1.to_string(), HELLO_SHA512.to_string());
        assert!(check_hash(&file, &combo, &mut stream).is_ok());

        // SHA1 匹配但 SHA512 错误
        let mut stream = fs::File::open(&file).unwrap();
        let combo = FileHash::Sha1Sha512(HELLO_SHA1.to_string(), "0".repeat(128));
        assert!(check_hash(&file, &combo, &mut stream).is_err());

        // SHA1 错误
        let mut stream = fs::File::open(&file).unwrap();
        let combo = FileHash::Sha1Sha512("0".repeat(40), HELLO_SHA512.to_string());
        assert!(check_hash(&file, &combo, &mut stream).is_err());
    }

    /// 将校验值转为大写，用于测试大小写不敏感比较
    fn hash_to_upper(hash: &FileHash) -> FileHash {
        match hash {
            FileHash::Md5(v) => FileHash::Md5(v.to_uppercase()),
            FileHash::Sha1(v) => FileHash::Sha1(v.to_uppercase()),
            FileHash::Sha256(v) => FileHash::Sha256(v.to_uppercase()),
            FileHash::Sha512(v) => FileHash::Sha512(v.to_uppercase()),
            _ => unreachable!("仅用于单哈希测试"),
        }
    }
}
