//! 时间换算
//!
//! 统计文件里时间以 100ns tick 存，这里与 `DateTime` / `TimeDelta` 互转。

use chrono::{DateTime, Local, LocalResult, TimeDelta, TimeZone, Utc};

/// 将 .NET 刻度数转换为本地时间
///
/// ColorMC 存的是 `DateTime.Now.Ticks`，刻度里记的是**本地墙上时间**，不带时区；
/// 读回时先还原成同样的墙上时间，再挂到本地时区，不能再当 UTC 转一次 ——
/// 否则 22 点之后的记录会整体跨到第二天。
///
/// - `ticks`: .NET 刻度数，单位 100 纳秒
///
/// # 返回值
///
/// 返回本地时间；无效值返回当前时间
pub(super) fn ticks_to_date(ticks: i64) -> DateTime<Local> {
    let mut secs = ticks / 10_000_000 - 62135596800;
    let mut nanos = (ticks % 10_000_000) * 100;
    if nanos < 0 {
        secs -= 1;
        nanos += 1_000_000_000;
    }
    let Some(utc) = DateTime::<Utc>::from_timestamp(secs, nanos as u32) else {
        return Local::now();
    };
    let naive = utc.naive_utc();
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(date) => date,
        // 夏令时回拨的重叠时刻取较早的一次，与 .NET 的 DateTime 行为一致
        LocalResult::Ambiguous(first, _) => first,
        // 夏令时跳过的时刻在本地时区不存在：按 UTC 偏移兜底，至少不 panic
        LocalResult::None => utc.with_timezone(&Local),
    }
}

/// 将本地时间转换为 .NET 刻度数
///
/// 与 ColorMC 同一口径：写的是本地墙上时间，不是 UTC 时间戳。
///
/// - `date`: 本地时间
///
/// # 返回值
///
/// 返回 .NET 刻度数，单位 100 纳秒
pub(super) fn date_to_ticks(date: DateTime<Local>) -> i64 {
    let local_secs = date.timestamp() + date.offset().local_minus_utc() as i64;
    (local_secs + 62135596800) * 10_000_000 + date.timestamp_subsec_nanos() as i64 / 100
}

/// 将时长转换为 .NET 刻度数
///
/// - `duration`: 时长
///
/// # 返回值
///
/// 返回 .NET 刻度数，单位 100 纳秒
pub(super) fn duration_to_ticks(duration: TimeDelta) -> i64 {
    duration.num_seconds().saturating_mul(10_000_000) + (duration.subsec_nanos() as i64) / 100
}

/// 将 .NET 刻度数转换为时长
///
/// - `ticks`: .NET 刻度数，单位 100 纳秒
///
/// # 返回值
///
/// 返回时长；负数或溢出返回零时长
pub(super) fn duration_from_ticks(ticks: i64) -> TimeDelta {
    if ticks < 0 {
        return TimeDelta::zero();
    }
    let secs = ticks / 10_000_000;
    let nanos = ((ticks % 10_000_000) * 100) as u32;
    TimeDelta::new(secs, nanos).unwrap_or(TimeDelta::zero())
}
