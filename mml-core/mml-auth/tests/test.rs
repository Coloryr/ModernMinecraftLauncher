use chrono::{DateTime, FixedOffset, Local};

/// 打印本地时间与固定偏移时间的对照，供人工查看
#[test]
fn test_time() {
    let dt = Local::now();
    let time: DateTime<FixedOffset> = dt.fixed_offset();

    println!("Local: {}", dt);
    println!("FixedOffset结果: {}", time);
}
