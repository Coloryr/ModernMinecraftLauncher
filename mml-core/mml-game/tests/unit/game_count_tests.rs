use super::*;
use std::io::Cursor;

/// 跨零点的刻度往返：日期与时刻都不能被时区换算挪走
#[test]
fn ticks_round_trip_across_midnight() {
    let before = Local
        .with_ymd_and_hms(2026, 9, 19, 23, 59, 58)
        .single()
        .unwrap();
    let after = Local
        .with_ymd_and_hms(2026, 9, 20, 0, 0, 3)
        .single()
        .unwrap();

    let back_before = ticks_to_date(date_to_ticks(before));
    let back_after = ticks_to_date(date_to_ticks(after));

    assert_eq!(back_before.naive_local(), before.naive_local());
    assert_eq!(back_after.naive_local(), after.naive_local());
    assert_eq!((back_after - back_before).num_seconds(), 5);
}

/// 读 ColorMC 生成的 count.dat：里面有一条 09-19 13:45 → 09-20 00:35 的记录，
/// 按时区换算成 UTC 再转回来会整体偏一天，必须按本地墙上时间原样还原
#[test]
fn reads_color_mc_count_across_midnight() {
    let bytes = include_bytes!("../count/count.dat");
    let file = NbtFile::read(&mut Cursor::new(bytes.as_slice())).unwrap();
    let nbt = file.nbt.as_compound().unwrap();

    assert_eq!(nbt.get_long("LaunchCount"), Some(1533));

    let runs = nbt.get_list("GameRuns").unwrap();
    let mut cross = None;
    for item in runs.iter() {
        let Some(com) = item.as_compound() else {
            continue;
        };
        if com.get_string("Key").as_deref() != Some("03962c67-c32d-40e8-81f3-42640ca062d4") {
            continue;
        }
        let Some(list) = com.get_list("List") else {
            continue;
        };
        for run in list.iter() {
            let Some(com1) = run.as_compound() else {
                continue;
            };
            let start = ticks_to_date(com1.get_long("StartTime").unwrap_or(0));
            let stop = ticks_to_date(com1.get_long("StopTime").unwrap_or(0));
            if start.date_naive() != stop.date_naive() {
                cross = Some((start, stop));
            }
        }
    }

    let (start, stop) = cross.expect("fixture 应有一条跨天记录");
    assert_eq!(
        start.naive_local().format("%Y-%m-%d %H:%M:%S").to_string(),
        "2026-09-19 13:45:58"
    );
    assert_eq!(
        stop.naive_local().format("%Y-%m-%d %H:%M:%S").to_string(),
        "2026-09-20 00:35:16"
    );
    assert!(stop > start);
}

/// 磁盘上那份统计比内存里多时不能落盘
///
/// 统计只增不减：内存里这份要是比磁盘上的少，说明读到的是空的 / 旧的那一份
/// （文件在启动器读它的时候被换掉了），写回去就把用户的统计盖没了。
#[test]
fn disk_with_more_data_blocks_save() {
    let dir = mml_testutil::temp_path(format!("count-guard-{}", Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(names::COUNT_DATA_FILE);
    std::fs::write(&file, include_bytes!("../count/count.dat")).unwrap();

    // 内存里是空的：磁盘上那份（1533 次启动）更多，必须拦住
    let empty = CountObj::default();
    let (disk, mem) = disk_more_than(&file, &empty).expect("磁盘上那份更多时应当拦住");
    assert_eq!(mem, 0);
    assert!(disk > 0);

    // 内存里就是这份文件读出来的：不拦
    let same = parse_file(&file).unwrap();
    assert!(disk_more_than(&file, &same).is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

/// 读不出来的统计文件不能被空统计覆盖
///
/// 用户手动替换 `count.dat` 时，读到的可能是一半的文件；以前这里会退回空统计，
/// 退出时再把空统计写回去，把用户原来的数据毁掉。
#[test]
fn unreadable_count_is_not_overwritten() {
    // 读失败要写日志，日志系统是单例，得先起来
    crate::test_support::boot();
    let dir = mml_testutil::temp_path(format!("count-broken-{}", Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(names::COUNT_DATA_FILE);
    // 半截 gzip：模拟复制到一半的文件
    std::fs::write(&file, [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]).unwrap();

    init(&dir);
    stop();

    assert_eq!(
        std::fs::read(&file).unwrap().len(),
        8,
        "读不出来的文件不该被覆盖"
    );
    assert!(dir.join(format!("{}.bak", names::COUNT_DATA_FILE)).exists());

    let _ = std::fs::remove_dir_all(&dir);
}
