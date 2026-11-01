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
        DownloadItemState::Cancel,
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
    assert_eq!(DownloadItemState::int_to_state(9), DownloadItemState::Error);
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

