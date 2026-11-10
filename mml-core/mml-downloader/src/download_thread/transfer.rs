//! 响应校验与流式落盘
//!
//! 断点续传时先核对 206 响应的 `Content-Range` 与已知长度，再边读响应块边写临时文件。

use super::*;

/// 校验开放式 Range 的起止位置及已知响应长度
///
/// - `resp`: 206 响应，读取它的 `Content-Range` 与 `Content-Length`
/// - `offset`: 续传起点，必须与 `Content-Range` 的起点一致
///
/// # 返回值
///
/// 范围自洽时返回完整文件大小；响应头缺失或不自洽时返回 `None`
pub(crate) fn resumed_total(resp: &Response, offset: u64) -> Option<u64> {
    let range = resp
        .headers()
        .get("Content-Range")?
        .to_str()
        .ok()?
        .strip_prefix("bytes ")?;
    let (bounds, total) = range.split_once('/')?;
    let (start, end) = bounds.split_once('-')?;
    let start: u64 = start.parse().ok()?;
    let end: u64 = end.parse().ok()?;
    let total: u64 = total.parse().ok()?;
    if start != offset || end < start || end.checked_add(1)? != total {
        return None;
    }
    if let Some(length) = resp.content_length() {
        if length != total - start {
            return None;
        }
    }
    Some(total)
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
pub(crate) async fn write_file(
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
                file.write_all(&data)
                    .map_err(|err| StreamError(ErrorData::from_error(err)))?;

                obj.item.set_state(DownloadItemState::Download);
                obj.item.add_progress(data.len() as u64);

                crate::update(index, &obj.item);
            }
            Err(e) => {
                return Err(StreamError(ErrorData::from_error(e)));
            }
        }
    }

    // 下载完成后校验
    let config = mml_config::read_config();
    if config.http.check_file {
        let now = file.stream_position().unwrap();
        let all = obj.item.get_all_size();
        if all > 0 && now != all {
            obj.item.set_state(DownloadItemState::Error);

            crate::update(index, &obj.item);

            return Err(ErrorType::DownloadFileSizeError(
                DownloadFileSizeErrorData {
                    file: obj.item.base.file.clone(),
                    url: obj.item.base.url.clone(),
                    now,
                    size: all,
                },
            ));
        }

        file.seek(SeekFrom::Start(0)).unwrap();
        return check_hash(&obj.item.base.file, &obj.item.base.hash, file);
    }

    Ok(())
}
