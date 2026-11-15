//! 枚举 SDL 手柄，可选采集一段输入。
//!
//! 用法：`cargo run -p mml-gamepad --example probe [秒数]`
//!
//! macOS 的 GameController 要靠进程的 run loop 才会返回已连接手柄；命令行没有事件循环，
//! 所以枚举与采集期间手动跑一小段 run loop，见 `support/run_loop.rs`。

use std::time::{Duration, Instant};

#[path = "support/run_loop.rs"]
mod run_loop;

fn main() -> Result<(), String> {
    // 与读取线程一致：游戏在前台时也要收事件
    sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");

    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let joystick = sdl.joystick().map_err(|error| error.to_string())?;
    let gamepad = sdl.gamepad().map_err(|error| error.to_string())?;
    let mut events = sdl.event_pump().map_err(|error| error.to_string())?;

    // 给系统一点时间上报已连接设备；macOS 必须跑 run loop
    let warmup = Instant::now();
    while warmup.elapsed() < Duration::from_secs(1) {
        run_loop::run(0.05);
        events.pump_events();
    }

    let joysticks = joystick.joysticks().map_err(|error| error.to_string())?;
    println!("joysticks: {}", joysticks.len());
    for id in &joysticks {
        match joystick.open(*id) {
            Ok(pad) => println!(
                "  [{id:?}] {:?} axes={} buttons={} hats={}",
                pad.name(),
                pad.num_axes(),
                pad.num_buttons(),
                pad.num_hats()
            ),
            Err(error) => println!("  [{id:?}] 打不开：{error:?}"),
        }
    }

    let gamepads = gamepad.gamepads().map_err(|error| error.to_string())?;
    println!("gamepads: {}", gamepads.len());
    for id in &gamepads {
        let name = gamepad
            .name_for_id(*id)
            .unwrap_or_else(|_| String::from("?"));
        let vendor = gamepad.vendor_for_id(*id).unwrap_or_default();
        let product = gamepad.product_for_id(*id).unwrap_or_default();
        println!("  [{id:?}] {name} vendor={vendor:#06x} product={product:#06x}");
    }

    let seconds = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    if seconds > 0
        && let Some(id) = gamepads.first()
    {
        capture(&gamepad, &mut events, *id, seconds)?;
    }
    Ok(())
}

/// 采集一段手柄输入，有变化才打印
///
/// - `subsystem`: 手柄子系统
/// - `events`: SDL 事件泵
/// - `id`: 要采集的手柄实例编号
/// - `seconds`: 采集时长，单位秒
fn capture(
    subsystem: &sdl3::GamepadSubsystem,
    events: &mut sdl3::EventPump,
    id: sdl3::joystick::JoystickId,
    seconds: u64,
) -> Result<(), String> {
    let pad = subsystem.open(id).map_err(|error| error.to_string())?;
    let buttons = [
        ("A", sdl3::gamepad::Button::South),
        ("B", sdl3::gamepad::Button::East),
        ("X", sdl3::gamepad::Button::West),
        ("Y", sdl3::gamepad::Button::North),
        ("LB", sdl3::gamepad::Button::LeftShoulder),
        ("RB", sdl3::gamepad::Button::RightShoulder),
        ("Back", sdl3::gamepad::Button::Back),
        ("Start", sdl3::gamepad::Button::Start),
        ("LS", sdl3::gamepad::Button::LeftStick),
        ("RS", sdl3::gamepad::Button::RightStick),
        ("Up", sdl3::gamepad::Button::DPadUp),
        ("Down", sdl3::gamepad::Button::DPadDown),
        ("Left", sdl3::gamepad::Button::DPadLeft),
        ("Right", sdl3::gamepad::Button::DPadRight),
    ];
    let axes = [
        ("LX", sdl3::gamepad::Axis::LeftX),
        ("LY", sdl3::gamepad::Axis::LeftY),
        ("RX", sdl3::gamepad::Axis::RightX),
        ("RY", sdl3::gamepad::Axis::RightY),
        ("LT", sdl3::gamepad::Axis::TriggerLeft),
        ("RT", sdl3::gamepad::Axis::TriggerRight),
    ];

    println!("{seconds} 秒输入采集中，按按钮或推摇杆");
    let started = Instant::now();
    let mut last = String::new();
    while started.elapsed() < Duration::from_secs(seconds) {
        run_loop::run(0.02);
        events.pump_events();
        let mut parts = Vec::new();
        for (name, button) in buttons {
            if pad.button(button) {
                parts.push(name.to_string());
            }
        }
        for (name, axis) in axes {
            let value = pad.axis(axis);
            if value.unsigned_abs() > 8000 {
                parts.push(format!("{name}={value}"));
            }
        }
        let line = parts.join(" ");
        if line != last {
            println!("{:>5.1}s {line}", started.elapsed().as_secs_f64());
            last = line;
        }
    }
    println!("采集结束");
    Ok(())
}
