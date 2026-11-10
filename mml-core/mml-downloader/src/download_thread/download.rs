//! 单文件下载主流程
//!
//! 负责已存在文件的复用判断、分段下载与重试、临时文件落位，以及下载完成后的后处理；
//! HTTP 响应与流式落盘的细节见 `transfer`。

use super::*;

/// 下载单个文件
///
/// - `index`: 下载线程序号
/// - `obj`: 下载项目，包含任务和文件信息
pub(crate) fn download(index: u32, mut obj: DownloadObj) {
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
            let check_file = mml_config::read_config().http.check_file;
            if check_file {
                let mut file = match path_helper::open_read(&obj.item.base.file) {
                    Ok(file) => file,
                    Err(err) => {
                        mml_log::error_type(err);
                        obj.item.set_state(DownloadItemState::Error);
                        crate::update(index, &obj.item);
                        obj.task.fail();
                        return;
                    }
                };
                let check = check_hash(&obj.item.base.file, &obj.item.base.hash, &mut file);
                if let Err(err) = check {
                    mml_log::error_type(err);
                } else {
                    // 后处理可能在上一次下载后失败，复用文件时仍须执行。
                    drop(file);
                    finish_download(index, &obj);
                    return;
                }
            }
        }
    }

    let mut times = 0;
    let mut use_break = false;
    let mut server_ranges = true;
    // 重试次数耗尽后放弃此文件，不能把残缺的临时文件移到目标路径
    let mut is_fail = false;
    // 任务被取消：与失败分开，见循环末尾的收尾
    let mut is_cancel = false;

    let temp_file = crate::gen_temp_file();

    loop {
        // 任务已被取消：立即放弃，不再重试；stop 时下载线程要 join，取消必须尽快打断在途下载
        if obj.task.is_cancelled() {
            is_cancel = true;
            break;
        }

        // 发起 HTTP 请求，支持断点续传。客户端先落成局部变量：请求 Future 借用它，
        // 写成 `get_work_client().get(..)` 的话临时 Arc 活不过这个 Future
        let client = mml_net::get_work_client();
        let offset = if use_break && server_ranges {
            std::fs::metadata(&temp_file)
                .map(|meta| meta.len())
                .unwrap_or(0)
        } else {
            0
        };
        let result = if offset > 0 {
            block_on_cancellable(&obj.task, client.get_ranges(&obj.item.base.url, offset))
        } else {
            block_on_cancellable(&obj.task, client.get(&obj.item.base.url))
        };
        let Some(result) = result else {
            is_cancel = true;
            break;
        };
        let mut resp = match result {
            Ok(resp) => resp,
            Err(err) => {
                if obj.task.is_cancelled() {
                    is_cancel = true;
                    break;
                }
                obj.item.add_error();
                if is_need_err(&obj.item.base, err, &mut times) {
                    is_fail = true;
                    break;
                }
                continue;
            }
        };

        if !resp.status().is_success() {
            if offset > 0 {
                server_ranges = false;
            }
            obj.item.add_error();
            let err = ErrorType::HttpError(HttpErrorData {
                url: obj.item.base.url.clone(),
                error: resp.status().canonical_reason().unwrap_or_default().into(),
                status: Some(resp.status().as_u16()),
            });
            if is_need_err(&obj.item.base, err, &mut times) {
                is_fail = true;
                break;
            }
            continue;
        }

        let append = if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            let Some(total) = resumed_total(&resp, offset) else {
                server_ranges = false;
                obj.item.add_error();
                if is_need_err(
                    &obj.item.base,
                    ErrorType::DownloadRangeError(obj.item.base.url.clone()),
                    &mut times,
                ) {
                    is_fail = true;
                    break;
                }
                continue;
            };
            obj.item.set_now_size(offset);
            obj.item.set_all_size(total);
            offset > 0
        } else {
            if offset > 0 && resp.status() != reqwest::StatusCode::OK {
                server_ranges = false;
                continue;
            }
            obj.item.set_now_size(0);
            obj.item.set_all_size(resp.content_length().unwrap_or(0));
            false
        };

        // 收到完整响应时重新截断；只有确认 Range 起点后才保留原文件。
        let file = if append {
            path_helper::open_read_write(&temp_file).and_then(|mut file| {
                file.seek(SeekFrom::End(0)).map_err(stream_err)?;
                Ok(file)
            })
        } else {
            path_helper::open_write(&temp_file)
        };
        let mut file = match file {
            Ok(file) => file,
            Err(err) => {
                obj.item.add_error();
                if is_need_err(&obj.item.base, err, &mut times) {
                    is_fail = true;
                    break;
                }
                continue;
            }
        };

        use_break = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT
            || resp
                .headers()
                .get("Accept-Ranges")
                .and_then(|value| value.to_str().ok())
                == Some("bytes");

        obj.item.set_state(DownloadItemState::GetInfo);
        crate::update(index, &obj.item);

        let result = block_on(write_file(index, &mut obj, &mut resp, &mut file));
        if let Err(err) = result {
            if matches!(
                &err,
                ErrorType::DownloadFileHashError(_) | ErrorType::DownloadFileSizeError(_)
            ) {
                server_ranges = false;
            }
            // 取消是 write_file 自己报出来的，优先于失败判定
            if obj.task.is_cancelled() {
                is_cancel = true;
                break;
            }
            obj.item.add_error();
            if is_need_err(&obj.item.base, err, &mut times) {
                is_fail = true;
                break;
            } else {
                continue;
            }
        }

        break;
    }

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

    // 移动失败也必须记录任务终态，避免调用方一直等待完成。
    if let Err(err) = path_helper::move_file(&temp_file, &obj.item.base.file) {
        mml_log::error_type(err);
        let _ = path_helper::delete(&temp_file);
        obj.item.set_state(DownloadItemState::Error);
        crate::update(index, &obj.item);
        obj.task.fail();
        return;
    }

    finish_download(index, &obj);
}

/// 后处理成功才完成下载项，失败保留文件供下一次重试。
///
/// - `index`: 下载线程序号
/// - `obj`: 下载项目
fn finish_download(index: u32, obj: &DownloadObj) {
    obj.item.set_state(DownloadItemState::Action);
    crate::update(index, &obj.item);
    let res: Result<(), ErrorType> = match &obj.item.base.later {
        LaterRun::None => Ok(()),
        LaterRun::UnpackNative(path_buf) => match path_helper::open_read(&obj.item.base.file) {
            Ok(file) => later_tasks::unpack_native(path_buf, file),
            Err(err) => Err(err),
        },
        LaterRun::UnpackSave(path_buf) => {
            BaseArchive::decompress(ArchiveType::Zip, &obj.item.base.file, path_buf, None)
        }
        // 新文件就位后立即回收旧版本，无需等待整批下载完成。
        LaterRun::ReplaceOld(old) => path_helper::move_to_trash(old),
    };

    if let Err(err) = res {
        mml_log::error_type(err);
        obj.item.set_state(DownloadItemState::Error);
        crate::update(index, &obj.item);
        obj.task.fail();
        return;
    }

    obj.item.set_state(DownloadItemState::Done);
    crate::update(index, &obj.item);
    obj.task.done();
}
