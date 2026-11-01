use std::sync::atomic::Ordering;

use mml_base::file_item::FileItemObj;

use crate::download_task::{DownloadTask, TaskOutcome};

use super::test_util::{GUI_EVENTS, ensure_env};

/// 构造一个简单的下载文件项
fn file_item(name: &str) -> FileItemObj {
    FileItemObj {
        name: name.to_string(),
        ..Default::default()
    }
}

/// 任务队列：FIFO 出队 + 计数
#[test]
fn task_queue_fifo_and_counts() {
    ensure_env();

    let task = DownloadTask::new(vec![file_item("a"), file_item("b")]);
    assert_eq!(task.total_size, 2);
    assert_eq!(task.completed_count.load(Ordering::SeqCst), 0);
    assert_eq!(task.failed_count.load(Ordering::SeqCst), 0);

    // SegQueue 是先进先出
    let first = task.get_item().unwrap();
    assert_eq!(first.base.name, "a");
    let second = task.get_item().unwrap();
    assert_eq!(second.base.name, "b");
    // 队列取空后返回 None
    assert!(task.get_item().is_none());
}

/// 成功完成的任务：wait_done 返回 Done，并从全局任务队列移除
#[test]
fn task_done_waits_and_completes() {
    ensure_env();

    let task = DownloadTask::new(vec![file_item("a")]);
    let id = task.id;

    // 队列非空时任务未完成，wait_done 不会被提前唤醒；
    // 这里先取走唯一文件再标记完成，保证不会阻塞
    let _item = task.get_item().unwrap();
    task.done();
    assert_eq!(task.completed_count.load(Ordering::SeqCst), 1);

    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let res = rt.block_on(task.wait_done());
    assert_eq!(res, TaskOutcome::Done);

    // 任务原本就不在全局队列中，此处确认移除无副作用
    assert!(super::get_tasks().iter().all(|t| t.id != id));
}

/// 存在失败文件的任务：wait_done 返回 Failed
#[test]
fn task_failed_reports_false() {
    ensure_env();

    let task = DownloadTask::new(vec![file_item("a")]);
    let _item = task.get_item().unwrap();
    task.fail();
    assert_eq!(task.failed_count.load(Ordering::SeqCst), 1);

    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let res = rt.block_on(task.wait_done());
    assert_eq!(res, TaskOutcome::Failed);
}

/// 任务被取消：结束方式是 Cancelled，且不计入失败数
#[test]
fn task_cancelled_is_not_a_failure() {
    ensure_env();

    let task = DownloadTask::new(vec![file_item("a")]);
    let _item = task.get_item().unwrap();
    task.mark_cancelled();
    assert_eq!(task.failed_count.load(Ordering::SeqCst), 0);

    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    assert_eq!(rt.block_on(task.wait_done()), TaskOutcome::Cancelled);
}

/// 取消令牌已触发：wait_done 立刻返回 Cancelled，不等在途文件自行收尾
#[test]
fn task_cancel_token_reports_cancelled() {
    ensure_env();

    let task = DownloadTask::new(vec![file_item("a")]);
    let _item = task.get_item().unwrap();
    task.cancel();

    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    assert_eq!(rt.block_on(task.wait_done()), TaskOutcome::Cancelled);
}

/// 任务进度通过 GUI 回调上报，1/2 完成即 50%
#[test]
fn task_progress_notifies_gui() {
    ensure_env();

    let task = DownloadTask::new(vec![file_item("a"), file_item("b")]);
    let id = task.id;

    let _item = task.get_item().unwrap();
    task.done();

    let events = GUI_EVENTS.get().unwrap().lock().unwrap();
    assert!(
        events.iter().any(|e| e == &format!("UpdateTask:{id}:50")),
        "应收到 50% 进度事件，实际事件: {:?}",
        *events
    );
}

