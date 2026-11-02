//! 手柄读取对比：同一条命令行里先看 gilrs（WGI），再切到 XInput
//!
//! 用来验证"WGI 只报已连接、收不到输入"这类问题：
//! 前一半时间不打开 XInput 兼容模式，后一半时间打开，按同一个手柄对比两段输出。
//!
//! 用法：
//!
//! ```text
//! cargo run -p mml-gamepad --example gamepad_demo -- 40
//! ```
//!
//! 参数是总时长，单位秒，缺省 30；跑起来后按手柄上的键、推摇杆，每 0.5 秒打一行。
use std::{
    thread,
    time::{Duration, Instant},
};

/// 每段刷新的间隔
const TICK: Duration = Duration::from_millis(500);
/// 中途切换 XInput 的模式点，单位占总时长的比例
const SWITCH_AT: f64 = 0.5;

fn main() {
    let seconds = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(180)
        .max(2);

    // 日志与配置文件都落在 target/temp，不碰真实运行目录
    let dir = mml_testutil::temp_dir().join(format!("gamepad_demo_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    mml_log::start(&dir).unwrap();
    mml_gamepad::init(&dir);

    println!("=== 手柄读取对比：{seconds} 秒 ===");
    println!("前一半不打开 XInput 兼容模式，后一半打开；请全程按手柄 / 推摇杆。");

    let started = Instant::now();
    let mut xinput_on = false;
    let mut printed = 0u64;
    while started.elapsed() < Duration::from_secs(seconds) {
        let elapsed = started.elapsed().as_secs_f64();
        let should_be_on = elapsed / seconds as f64 >= SWITCH_AT;
        if should_be_on != xinput_on {
            xinput_on = should_be_on;
            mml_gamepad::set_xinput(xinput_on).unwrap();
            println!(
                "\n--- t={elapsed:.1}s  XInput 兼容模式：{} ---",
                if xinput_on { "开" } else { "关" }
            );
        }

        if printed % 2 == 0 {
            dump(elapsed);
        }
        printed += 1;
        thread::sleep(TICK);
    }

    mml_gamepad::set_xinput(false).ok();
    println!("\n=== 结束 ===");
    std::process::exit(0);
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
        // 摇杆与扳机一律打出来（不按也有值）：WGI 那边是恒定的 0，XInput 这边有真实的静止偏移
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
