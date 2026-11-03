//! SDL2 手柄读取验收：按键、摇杆、扳机、热插拔与后台输入。
//!
//! 用法：cargo run -p mml-gamepad --example gamepad_demo -- 40
//! 参数为时长秒数，缺省 30；每 0.5 秒打印快照，切到游戏前台后应仍有读数。
use std::{
    thread,
    time::{Duration, Instant},
};

/// 每段刷新的间隔
const TICK: Duration = Duration::from_millis(500);

fn main() {
    let seconds = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30)
        .max(2);

    // 日志与配置文件都落在 target/temp，不碰真实运行目录
    let dir = mml_testutil::temp_dir().join(format!("gamepad_demo_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    mml_log::start(&dir).unwrap();
    mml_gamepad::init(&dir);

    println!("=== SDL2 手柄读取：{seconds} 秒 ===");
    println!("请按键、推摇杆并插拔手柄；切到游戏前台后继续操作以检查后台输入。");

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        dump(started.elapsed().as_secs_f64());
        thread::sleep(TICK);
    }

    println!("\n=== 结束 ===");
}

/// 打一次当前手柄状态
///
/// - `elapsed`: 已经跑了多少秒，只用来标在行首
fn dump(elapsed: f64) {
    let pads = mml_gamepad::pads();
    if pads.is_empty() {
        println!("t={elapsed:5.1}s  未检测到手柄（按一下手柄的 Xbox 键唤醒，或确认蓝牙还连着）");
        return;
    }
    for pad in pads {
        let Some(state) = mml_gamepad::pad_state(pad.id) else {
            continue;
        };
        let pressed: Vec<String> = state
            .buttons
            .iter()
            .filter(|item| item.value >= 0.5)
            .map(|item| format!("{:?}={:.2}", item.button, item.value))
            .collect();
        let axis = |name: mml_gamepad::Axis| {
            state
                .axes
                .iter()
                .find(|item| item.axis == name)
                .map_or(0.0, |item| item.value)
        };
        let extras: Vec<String> = state
            .extras
            .iter()
            .filter(|item| item.value >= 0.5)
            .map(|item| item.code.to_string())
            .collect();
        // 静止值也打印，便于观察死区与扳机归零。
        println!(
            "t={elapsed:5.1}s  [{}] 按下={pressed:?} LS=({:.2},{:.2}) RS=({:.2},{:.2}) 扳机=({:.2},{:.2}) 额外={extras:?}",
            pad.name,
            axis(mml_gamepad::Axis::LeftStickX),
            axis(mml_gamepad::Axis::LeftStickY),
            axis(mml_gamepad::Axis::RightStickX),
            axis(mml_gamepad::Axis::RightStickY),
            axis(mml_gamepad::Axis::LeftZ),
            axis(mml_gamepad::Axis::RightZ)
        );
    }
}
