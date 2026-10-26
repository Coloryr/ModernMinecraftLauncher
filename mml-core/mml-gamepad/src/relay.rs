//! 把手柄状态映射成键鼠消息，转发给运行中的游戏实例
//!
//! 每帧按映射规则算出应该按下的动作，只把变化发给游戏；摇杆位移按速度累计成
//! 整数像素再发。光标模式与游戏内转动模式由游戏上报的鼠标抓取状态决定：
//! 抓取时左摇杆走 WASD、右摇杆转视角；未抓取时左摇杆移光标，且不再触发
//! 左摇杆上的按键，避免翻背包时走动。
//!
//! 目标实例优先选已抓取鼠标的那个；只有一个游戏连接时直接用它。

use std::{
    collections::HashSet,
    thread,
    time::{Duration, Instant},
};

use uuid::Uuid;

use mml_names::i18_items::info_type::InfoType;

use crate::map::{self, Action, CompiledMap, in_range};
use crate::reader::{BUTTON_COUNT, PadValues, first_pad_id, pad_values};

/// 转发间隔，约 60 帧每秒
const TICK: Duration = Duration::from_millis(16);
/// 检查映射文件变更的间隔
const RELOAD_CHECK: Duration = Duration::from_secs(1);
/// 按钮值达到该值按按下处理
const BUTTON_DOWN: f32 = 0.5;
/// 单帧时间上限，单位秒；进程被挂起后避免位移突跳
const MAX_DELTA: f64 = 0.1;

/// 转发状态
struct Relay {
    /// 当前目标实例
    target: Option<Uuid>,
    /// 当前手柄编号
    pad: Option<u64>,
    /// 上次看到的鼠标抓取状态
    grabbed: Option<bool>,
    /// 已按下的动作
    held: HashSet<Action>,
    /// 切目标、设备或模式时仍按住的按钮，松开后才允许触发
    blocked: [bool; BUTTON_COUNT],
    /// 累计的水平位移，单位像素
    acc_x: f64,
    /// 累计的垂直位移，单位像素
    acc_y: f64,
}

/// 启动转发线程
pub(super) fn start() {
    let _ = thread::Builder::new()
        .name("Mml Gamepad Relay".to_string())
        .spawn(run);
}

/// 转发线程主体
fn run() {
    let mut relay = Relay::new();
    let mut last = Instant::now();
    let mut reload_at = Instant::now();

    loop {
        thread::sleep(TICK);
        let now = Instant::now();
        let delta = (now - last).as_secs_f64().min(MAX_DELTA);
        last = now;

        if now >= reload_at {
            map::reload_if_changed();
            reload_at = now + RELOAD_CHECK;
        }

        relay.tick(delta);
    }
}

impl Relay {
    /// 空状态
    fn new() -> Self {
        Self {
            target: None,
            pad: None,
            grabbed: None,
            held: HashSet::new(),
            blocked: [false; BUTTON_COUNT],
            acc_x: 0.0,
            acc_y: 0.0,
        }
    }

    /// 走一帧
    ///
    /// - `delta`: 距上一帧的时间，单位秒
    fn tick(&mut self, delta: f64) {
        let target = target_instance();
        let pad = target.and_then(|_| first_pad_id());
        let values = pad.and_then(pad_values);
        self.forward(target, pad, values.as_ref(), &map::current(), delta);
    }

    /// 使用同一条转发链路处理设备快照，便于脱离物理驱动验证输入
    fn forward(
        &mut self,
        target: Option<Uuid>,
        pad: Option<u64>,
        values: Option<&PadValues>,
        map: &CompiledMap,
        delta: f64,
    ) {
        if target != self.target {
            self.release_all();
            self.target = target;
            if let Some(uuid) = target {
                mml_log::info_type(InfoType::GamepadForwardStart {
                    uuid: uuid.to_string(),
                });
            }
        }
        let Some(uuid) = self.target else {
            self.pad = None;
            return;
        };

        let Some(pad) = pad else {
            self.release_all();
            self.pad = None;
            return;
        };
        if self.pad != Some(pad) {
            self.release_all();
            self.pad = Some(pad);
        }
        let Some(values) = values else {
            self.release_all();
            self.pad = None;
            return;
        };

        let grabbed = mml_ipc::get_mouse_state(uuid).unwrap_or(false);
        if self.grabbed != Some(grabbed) {
            self.release_all();
            self.grabbed = Some(grabbed);
            self.blocked = values.buttons.map(|value| value >= BUTTON_DOWN);
        }

        let mut filtered = PadValues {
            buttons: values.buttons,
            axes: values.axes,
        };
        for (index, blocked) in self.blocked.iter_mut().enumerate() {
            *blocked &= values.buttons[index] >= BUTTON_DOWN;
            if *blocked {
                filtered.buttons[index] = 0.0;
            }
        }
        let desired = desired_actions(map, &filtered, grabbed);
        for action in &self.held {
            if !desired.contains(action) {
                send_action(uuid, *action, false);
            }
        }
        for action in &desired {
            if !self.held.contains(action) {
                send_action(uuid, *action, true);
            }
        }
        self.held = desired;

        let rule = if grabbed {
            map.mouse.turn
        } else {
            map.mouse.cursor
        };
        let mut x = values.axes[rule.axes.x];
        let mut y = values.axes[rule.axes.y];
        if x.abs() < map.mouse.deadzone {
            x = 0.0;
        }
        if y.abs() < map.mouse.deadzone {
            y = 0.0;
        }
        let delta = delta.clamp(0.0, MAX_DELTA);
        self.acc_x += f64::from(x) * f64::from(rule.speed) * delta;
        // gilrs 的 Y 轴向上为正，鼠标位移向下为正，这里取反
        self.acc_y -= f64::from(y) * f64::from(rule.speed) * delta;
        let dx = self.acc_x.trunc();
        let dy = self.acc_y.trunc();
        if dx != 0.0 || dy != 0.0 {
            mml_ipc::send_mouse_move(uuid, dx, dy);
            self.acc_x -= dx;
            self.acc_y -= dy;
        }
    }

    /// 松开全部已按下的动作，并清掉位移累计
    fn release_all(&mut self) {
        if let Some(uuid) = self.target {
            for action in &self.held {
                send_action(uuid, *action, false);
            }
        }
        self.held.clear();
        self.grabbed = None;
        self.blocked.fill(false);
        self.acc_x = 0.0;
        self.acc_y = 0.0;
    }
}

/// 选一个转发目标
///
/// 优先已抓取鼠标的实例；多开且都没有抓取时取编号最小的
fn target_instance() -> Option<Uuid> {
    let uuids = mml_ipc::get_game_uuids();
    match uuids.len() {
        0 => None,
        1 => uuids.first().copied(),
        _ => uuids
            .iter()
            .copied()
            .find(|uuid| mml_ipc::get_mouse_state(*uuid) == Some(true))
            .or_else(|| uuids.first().copied()),
    }
}

/// 按映射算出这一帧要按下的动作
fn desired_actions(map: &CompiledMap, values: &PadValues, grabbed: bool) -> HashSet<Action> {
    let mut actions = HashSet::new();

    // 组合键优先：同一按钮上带修饰键的规则命中时，不触发它不带修饰的绑定
    let mut combo = [false; BUTTON_COUNT];
    for rule in &map.buttons {
        if values.buttons[rule.index] < BUTTON_DOWN {
            continue;
        }
        if let Some(modifier) = rule.modifier
            && values.buttons[modifier] >= BUTTON_DOWN
        {
            combo[rule.index] = true;
        }
    }

    for rule in &map.buttons {
        if values.buttons[rule.index] < BUTTON_DOWN {
            continue;
        }
        match rule.modifier {
            Some(modifier) => {
                if values.buttons[modifier] >= BUTTON_DOWN {
                    actions.insert(rule.action);
                }
            }
            None => {
                if !combo[rule.index] {
                    actions.insert(rule.action);
                }
            }
        }
    }

    for rule in &map.axes {
        // 光标模式不触发光标摇杆上的按键，避免翻背包时走动
        if !grabbed && rule.stick == Some(map.mouse.cursor.stick) {
            continue;
        }
        if in_range(values.axes[rule.index], rule.from, rule.to) {
            actions.insert(rule.action);
        }
    }

    actions
}

/// 发一个动作的按下或抬起
fn send_action(uuid: Uuid, action: Action, down: bool) {
    match action {
        Action::Key(key) => {
            mml_ipc::send_key(uuid, key, down);
        }
        Action::Mouse(button) => {
            mml_ipc::send_mouse_button(uuid, button, down);
        }
        // 滚轮没有抬起，按下时发一格
        Action::Scroll(amount) => {
            if down {
                mml_ipc::send_mouse_scroll(uuid, amount);
            }
        }
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod bridge_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Axis, Button};
    use crate::reader::{AXIS_COUNT, BUTTON_COUNT, axis_index, button_index};

    /// 空的手柄值
    fn empty_values() -> PadValues {
        PadValues {
            buttons: [0.0; BUTTON_COUNT],
            axes: [0.0; AXIS_COUNT],
        }
    }

    /// 按钮按下映射成对应按键
    #[test]
    fn button_press_maps_to_key() {
        let map = map::GamepadConfig::default().compile().unwrap();
        let mut values = empty_values();
        values.buttons[button_index(Button::South).unwrap()] = 1.0;

        assert!(desired_actions(&map, &values, true).contains(&Action::Key(32)));
    }

    /// 光标模式下左摇杆不触发 WASD，转动模式下触发
    #[test]
    fn cursor_mode_skips_stick_keys() {
        let map = map::GamepadConfig::default().compile().unwrap();
        let mut values = empty_values();
        values.axes[axis_index(Axis::LeftStickY).unwrap()] = 1.0;

        // 向上推是 W，光标模式下不触发
        assert!(desired_actions(&map, &values, true).contains(&Action::Key(87)));
        assert!(!desired_actions(&map, &values, true).contains(&Action::Key(83)));
        assert!(!desired_actions(&map, &values, false).contains(&Action::Key(87)));
    }

    /// 左摇杆向下推是 S
    #[test]
    fn stick_down_maps_to_back() {
        let map = map::GamepadConfig::default().compile().unwrap();
        let mut values = empty_values();
        values.axes[axis_index(Axis::LeftStickY).unwrap()] = -1.0;

        let actions = desired_actions(&map, &values, true);
        assert!(actions.contains(&Action::Key(83)));
        assert!(!actions.contains(&Action::Key(87)));
    }

    /// 按住修饰键时组合键生效，同按钮的普通绑定被压住
    #[test]
    fn modifier_combo_overrides_plain_binding() {
        let mut config = map::GamepadConfig::default();
        // 默认已有 South → Space，再加一条 LeftTrigger + South → F
        config.buttons.push(map::ButtonRule {
            button: Button::South,
            modifier: Some(Button::LeftTrigger),
            action: String::from("F"),
        });
        let map = config.compile().unwrap();

        let mut values = empty_values();
        values.buttons[button_index(Button::South).unwrap()] = 1.0;

        // 只按 A：空格
        let plain = desired_actions(&map, &values, true);
        assert!(plain.contains(&Action::Key(32)));
        assert!(!plain.contains(&Action::Key(70)));

        // 按住 LT 再按 A：F 生效，空格被压住
        values.buttons[button_index(Button::LeftTrigger).unwrap()] = 1.0;
        let combo = desired_actions(&map, &values, true);
        assert!(combo.contains(&Action::Key(70)));
        assert!(!combo.contains(&Action::Key(32)));
    }

    /// 十字键上映射成滚轮向上
    #[test]
    fn dpad_maps_to_scroll() {
        let map = map::GamepadConfig::default().compile().unwrap();
        let mut values = empty_values();
        values.buttons[button_index(Button::DPadUp).unwrap()] = 1.0;

        assert!(desired_actions(&map, &values, true).contains(&Action::Scroll(1)));
    }
}
