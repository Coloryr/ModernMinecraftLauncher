use mml_game::launcher::game_time_obj::GameTimeObj;

/// 把游戏时长格式化成 `[天数.]时:分:秒.百分秒`
///
/// - `time`: 游戏时长记录
///
/// # 返回值
///
/// 返回格式化后的文本；不满一天时省略天数段
pub fn format_game_time(time: &GameTimeObj) -> String {
    let total_seconds = time.game_time.num_seconds();
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let nanos = time.game_time.subsec_nanos();

    if days > 0 {
        format!(
            "{}.{:02}:{:02}:{:02}.{:07}",
            days,
            hours,
            minutes,
            seconds,
            nanos / 100
        )
    } else {
        format!(
            "{:02}:{:02}:{:02}.{:07}",
            hours,
            minutes,
            seconds,
            nanos / 100
        )
    }
}

/// 把游戏时长换算成秒
///
/// - `time`: 游戏时长记录
///
/// # 返回值
///
/// 返回秒数，带小数
pub fn game_time_seconds_f64(time: &GameTimeObj) -> f64 {
    time.game_time.num_milliseconds() as f64 / 1000.0
}

/// 解析两种时长文本并校验序列化往返：不满一天与带天数
#[test]
fn time_dec_test() {
    // 第一种格式：没有天数
    let json_data1 = r#"
    {
        "AddTime": "2024-06-23T18:38:40.9238897+08:00",
        "LastTime": "2024-11-16T20:19:01.8224679+08:00",
        "GameTime": "00:16:49.9106414",
        "LastPlay": "00:00:17.1875992"
    }"#;

    // 第二种格式：带天数
    let json_data2 = r#"
    {
        "AddTime": "2025-02-08T23:08:24.4002849+08:00",
        "LastTime": "2025-12-18T16:48:56.4176388+08:00",
        "GameTime": "6.04:35:49.3989028",
        "LastPlay": "00:00:01.2633654"
    }"#;

    println!("========== 测试1: 标准格式 ==========");
    let record1: GameTimeObj = serde_json::from_str(json_data1).unwrap();
    println!("添加时间: {}", record1.add_time);
    println!("最后时间: {}", record1.last_play);
    println!(
        "游戏时长: {} (原始: 00:16:49.9106414)",
        format_game_time(&record1)
    );
    println!("游戏时长(秒): {:.3}", game_time_seconds_f64(&record1));
    println!("最后游玩: {:?}", record1.last_play);

    println!("\n========== 测试2: 带天数格式 ==========");
    let record2: GameTimeObj = serde_json::from_str(json_data2).unwrap();
    println!("添加时间: {}", record2.add_time);
    println!("最后时间: {}", record2.last_time);
    println!(
        "游戏时长: {} (原始: 6.04:35:49.3989028)",
        format_game_time(&record2)
    );
    println!("游戏时长(秒): {:.3}", game_time_seconds_f64(&record2));
    println!("最后游玩: {:?}", record2.last_play);

    // 验证总秒数
    let expected_seconds = 6 * 86400 + 4 * 3600 + 35 * 60 + 49;
    println!("\n验证: 预期总秒数 = {} 秒", expected_seconds);
    println!("实际总秒数 = {} 秒", record2.game_time.num_seconds());
    assert_eq!(record2.game_time.num_seconds(), expected_seconds);

    // 测试序列化往返
    println!("\n========== 测试序列化 ==========");
    let serialized = serde_json::to_string_pretty(&record2).unwrap();
    println!("序列化结果:\n{}", serialized);

    let deserialized: GameTimeObj = serde_json::from_str(&serialized).unwrap();
    assert_eq!(record2.game_time, deserialized.game_time);
}
