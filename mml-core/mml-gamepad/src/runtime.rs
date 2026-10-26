//! 设备轮询在独立线程运行，禁用、换目标和断开设备都向旧通道释放按键。

use crate::{Button, Mapper, Settings, Snapshot};
use gilrs::{Axis, Button as NativeButton, Gilrs};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// 已连接设备的标识。
#[derive(Clone, Debug)]
pub struct Device {
    /// gilrs 设备 GUID。
    pub id: String,
    /// 操作系统报告的名称。
    pub name: String,
}

/// 供界面读取的运行快照。
#[derive(Clone, Default)]
pub struct Status {
    /// 已连接设备列表。
    pub devices: Vec<Device>,
    /// 当前设备状态，未连接时为空。
    pub snapshot: Snapshot,
    /// 正在向游戏发送输入。
    pub active: bool,
    /// 设备驱动初始化错误键。
    pub error: Option<String>,
}

/// 可由界面更新的线程共享状态。
struct Shared {
    /// 当前设置。
    settings: Settings,
    /// 最近一次设备采样。
    status: Status,
}

/// 手柄轮询线程的控制句柄。
pub struct Runtime {
    /// 配置与设备状态。
    shared: Arc<Mutex<Shared>>,
    /// 退出请求。
    stop: Arc<AtomicBool>,
    /// 用于退出时等待按键释放。
    thread: Option<thread::JoinHandle<()>>,
}

fn snapshot(gamepad: &gilrs::Gamepad<'_>) -> Snapshot {
    let mut value = Snapshot {
        left_x: gamepad.value(Axis::LeftStickX) as f64,
        left_y: gamepad.value(Axis::LeftStickY) as f64,
        right_x: gamepad.value(Axis::RightStickX) as f64,
        right_y: gamepad.value(Axis::RightStickY) as f64,
        ..Snapshot::default()
    };
    for (button, native) in [
        (Button::South, NativeButton::South),
        (Button::East, NativeButton::East),
        (Button::West, NativeButton::West),
        (Button::North, NativeButton::North),
        (Button::Start, NativeButton::Start),
        (Button::Select, NativeButton::Select),
        (Button::LeftShoulder, NativeButton::LeftTrigger),
        (Button::RightShoulder, NativeButton::RightTrigger),
        (Button::LeftTrigger, NativeButton::LeftTrigger2),
        (Button::RightTrigger, NativeButton::RightTrigger2),
        (Button::LeftStick, NativeButton::LeftThumb),
        (Button::RightStick, NativeButton::RightThumb),
        (Button::Up, NativeButton::DPadUp),
        (Button::Down, NativeButton::DPadDown),
        (Button::Left, NativeButton::DPadLeft),
        (Button::Right, NativeButton::DPadRight),
    ] {
        if gamepad.is_pressed(native)
            || gamepad.button_data(native).is_some_and(|b| b.value() > 0.5)
        {
            value.buttons.insert(button);
        }
    }
    value
}

fn release(mapper: &mut Mapper, target: Option<Uuid>) {
    let messages = mapper.reset();
    if let Some(uuid) = target {
        for message in messages {
            message.send(uuid);
        }
    }
}

fn poll(shared: Arc<Mutex<Shared>>, stop: Arc<AtomicBool>) {
    let mut gilrs = match Gilrs::new() {
        Ok(value) => value,
        Err(_) => {
            shared.lock().unwrap().status.error = Some("err.gamepadDriver".into());
            return;
        }
    };
    let mut mapper = Mapper::default();
    let mut target = None;
    let mut device = None;
    let mut time = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        // 排空事件后再读取状态，热插拔与轴数据由 gilrs 同步更新。
        while gilrs.next_event().is_some() {}
        gilrs.inc();
        let settings = shared.lock().unwrap().settings.clone();
        let devices: Vec<_> = gilrs
            .gamepads()
            .map(|(_, gamepad)| Device {
                id: Uuid::from_bytes(gamepad.uuid()).to_string(),
                name: gamepad.name().to_string(),
            })
            .collect();
        let selected = gilrs.gamepads().find(|(_, gamepad)| {
            settings.device.is_empty()
                || settings.device == Uuid::from_bytes(gamepad.uuid()).to_string()
        });
        let current_device = selected.as_ref().map(|(id, _)| usize::from(*id));
        let input = selected
            .as_ref()
            .map(|(_, gamepad)| snapshot(gamepad))
            .unwrap_or_default();
        let next_target = settings.target.filter(|uuid| {
            settings.enabled
                && selected.is_some()
                && mml_ipc::is_game_connected(*uuid)
                && mml_ipc::get_mouse_state(*uuid).is_some()
        });
        if target != next_target || device != current_device {
            release(&mut mapper, target);
            target = next_target;
            device = current_device;
        }
        let elapsed = time.elapsed().as_secs_f64();
        time = Instant::now();
        let mut active = false;
        if let Some(uuid) = target
            && let Some(grabbed) = mml_ipc::get_mouse_state(uuid)
        {
            active = true;
            for output in mapper.update(&input, grabbed, elapsed, &settings) {
                if !output.send(uuid) {
                    release(&mut mapper, target);
                    target = None;
                    active = false;
                    break;
                }
            }
        }
        shared.lock().unwrap().status = Status {
            devices,
            snapshot: input,
            active,
            error: None,
        };
        thread::sleep(Duration::from_millis(8));
    }
    release(&mut mapper, target);
}

impl Runtime {
    /// 创建轮询线程，初始化失败由 status.error 返回。
    ///
    /// - `settings`: 已验证的初始配置
    ///
    /// # 返回值
    ///
    /// 控制句柄；线程无法创建时返回系统错误。
    pub fn start(settings: Settings) -> std::io::Result<Self> {
        let shared = Arc::new(Mutex::new(Shared {
            settings,
            status: Status::default(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_shared = shared.clone();
        let worker_stop = stop.clone();
        let thread = thread::Builder::new()
            .name("Mml Gamepad".into())
            .spawn(move || poll(worker_shared, worker_stop))?;
        Ok(Self {
            shared,
            stop,
            thread: Some(thread),
        })
    }

    /// 更新设置，最多一帧后生效。
    ///
    /// - `settings`: 新的映射配置
    pub fn configure(&self, settings: Settings) {
        self.shared.lock().unwrap().settings = settings;
    }

    /// 获取最近一帧的设备与输入状态。
    ///
    /// # 返回值
    ///
    /// 可用于手柄测试界面的状态副本。
    pub fn status(&self) -> Status {
        self.shared.lock().unwrap().status.clone()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
