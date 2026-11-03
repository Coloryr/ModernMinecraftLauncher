//! 把手柄状态映射成键鼠消息，转发给运行中的游戏实例
//!
//! 每帧按映射规则算出应该按下的动作，只把变化发给游戏；摇杆位移按速度累计成
//! 整数像素再发。光标模式与游戏内转动模式由游戏上报的鼠标抓取状态决定：
//! 抓取时左摇杆走 WASD、右摇杆转视角；未抓取时左摇杆移光标，且不再触发
//! 左摇杆上的按键，避免翻背包时走动。
//!
//! 目标实例优先选已抓取鼠标的那个；只有一个游戏连接时直接用它。

use std::{
    collections::{BTreeMap, BTreeSet},
    thread,
    time::{Duration, Instant},
};

use uuid::Uuid;

use mml_names::i18_items::info_type::InfoType;

use crate::map::{self, Action, ButtonMode, CompiledMap, combo_of, in_range};
use crate::reader::{BUTTON_COUNT, PadValues, first_pad_id, pad_values};

/// 转发间隔，约 60 帧每秒
const TICK: Duration = Duration::from_millis(16);
/// 检查映射文件变更的间隔
const RELOAD_CHECK: Duration = Duration::from_secs(1);
/// 按钮值达到该值按按下处理
const BUTTON_DOWN: f32 = 0.5;
/// 单帧时间上限，单位秒；进程被挂起后避免位移突跳
const MAX_DELTA: f64 = 0.1;
/// 连发模式的触发间隔
const REPEAT_INTERVAL: Duration = Duration::from_millis(120);

/// 一帧要做的动作
#[derive(Default)]
struct Frame {
    /// 该保持按下的规则编号
    active: BTreeSet<usize>,
    /// 该点一下的规则编号，连发模式用
    pulses: Vec<usize>,
}

/// 转发状态
struct Relay {
    /// 当前目标实例
    target: Option<Uuid>,
    /// 当前手柄编号
    pad: Option<u64>,
    /// 上次看到的鼠标抓取状态
    grabbed: Option<bool>,
    /// 当前按住的规则编号
    active: BTreeSet<usize>,
    /// 每个动作被几条规则同时按住；最后一条松开时才真的发抬起
    counts: BTreeMap<Action, usize>,
    /// 上一帧按下的规则编号，用来找按下 / 松开的边沿
    pressed: BTreeSet<usize>,
    /// 切换模式里当前保持按下的规则
    toggled: BTreeSet<usize>,
    /// 连发模式里每条规则的下次触发时间
    repeat_at: BTreeMap<usize, Instant>,
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
            active: BTreeSet::new(),
            counts: BTreeMap::new(),
            pressed: BTreeSet::new(),
            toggled: BTreeSet::new(),
            repeat_at: BTreeMap::new(),
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
        let frame = self.step(map, Instant::now(), &desired_rules(map, &filtered, grabbed));
        let released: Vec<usize> = self.active.difference(&frame.active).copied().collect();
        for id in released {
            self.release_rule(uuid, map, id);
        }
        let pressed: Vec<usize> = frame.active.difference(&self.active).copied().collect();
        for id in pressed {
            self.press_rule(uuid, map, id);
        }
        self.active = frame.active;
        // 连发：按间隔点一下，按下与抬起在同一帧里成对发出
        for id in frame.pulses {
            self.press_rule(uuid, map, id);
            self.release_rule(uuid, map, id);
        }

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
        // 读取层的 Y 轴向上为正，鼠标位移向下为正，这里取反
        self.acc_y -= f64::from(y) * f64::from(rule.speed) * delta;
        let dx = self.acc_x.trunc();
        let dy = self.acc_y.trunc();
        if dx != 0.0 || dy != 0.0 {
            mml_ipc::send_mouse_move(uuid, dx, dy);
            self.acc_x -= dx;
            self.acc_y -= dy;
        }
    }

    /**
     * 根据这一帧按下的规则算出实际生效的规则
     *
     * 普通规则按下即生效；切换规则在按下边沿翻转保持状态；连发规则不进常驻集合，
     * 而是按间隔给出"点一下"。
     *
     * - `map`: 编译后的映射
     * - `now`: 这一帧的时间，连发按它算间隔
     * - `pressed`: 这一帧原始命中的规则编号
     */
    fn step(&mut self, map: &CompiledMap, now: Instant, pressed: &BTreeSet<usize>) -> Frame {
        let mut frame = Frame::default();
        for rule in &map.buttons {
            let down = pressed.contains(&rule.id);
            match rule.mode {
                ButtonMode::Normal => {
                    if down {
                        frame.active.insert(rule.id);
                    }
                }
                ButtonMode::Toggle => {
                    // 只在按下的那一帧翻转，按住不放不会来回跳
                    if down && !self.pressed.contains(&rule.id) {
                        if !self.toggled.remove(&rule.id) {
                            self.toggled.insert(rule.id);
                        }
                    }
                    if self.toggled.contains(&rule.id) {
                        frame.active.insert(rule.id);
                    }
                }
                ButtonMode::Repeat => {
                    if down {
                        let due = self.repeat_at.entry(rule.id).or_insert(now);
                        if now >= *due {
                            frame.pulses.push(rule.id);
                            self.repeat_at.insert(rule.id, now + REPEAT_INTERVAL);
                        }
                    } else {
                        self.repeat_at.remove(&rule.id);
                    }
                }
            }
        }
        // 轴规则没有触发方式，命中即生效
        for rule in &map.axes {
            if pressed.contains(&rule.id) {
                frame.active.insert(rule.id);
            }
        }
        self.pressed = pressed.clone();
        frame
    }


    /// 按下一条规则：按序列顺序逐条按下
    ///
    /// - `uuid`: 目标实例
    /// - `map`: 编译后的映射
    /// - `id`: 规则编号
    fn press_rule(&mut self, uuid: Uuid, map: &CompiledMap, id: usize) {
        let Some(combo) = combo_of(map, id) else {
            return;
        };
        for action in combo.press() {
            self.press(uuid, action);
        }
    }

    /// 松开一条规则：按按下顺序的反序逐条松开
    ///
    /// - `uuid`: 目标实例
    /// - `map`: 编译后的映射
    /// - `id`: 规则编号
    fn release_rule(&mut self, uuid: Uuid, map: &CompiledMap, id: usize) {
        let Some(combo) = combo_of(map, id) else {
            return;
        };
        for action in combo.release() {
            self.release(uuid, action);
        }
    }

    /// 按下某个动作，同一个动作被多条规则同时按住时只发一次
    fn press(&mut self, uuid: Uuid, action: Action) {
        let count = self.counts.entry(action).or_insert(0);
        if *count == 0 {
            send_action(uuid, action, true);
        }
        *count += 1;
    }

    /// 松开某个动作，计数归零时才真的发抬起
    fn release(&mut self, uuid: Uuid, action: Action) {
        let Some(count) = self.counts.get_mut(&action) else {
            return;
        };
        if *count > 1 {
            *count -= 1;
            return;
        }
        self.counts.remove(&action);
        send_action(uuid, action, false);
    }

    /// 松开全部已按下的动作，并清掉位移累计
    fn release_all(&mut self) {
        if let Some(uuid) = self.target {
            for action in self.counts.keys() {
                send_action(uuid, *action, false);
            }
        }
        self.active.clear();
        self.counts.clear();
        self.pressed.clear();
        self.toggled.clear();
        self.repeat_at.clear();
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

/// 按映射算出这一帧命中的规则编号
fn desired_rules(map: &CompiledMap, values: &PadValues, grabbed: bool) -> BTreeSet<usize> {
    let mut rules = BTreeSet::new();

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
                    rules.insert(rule.id);
                }
            }
            None => {
                if !combo[rule.index] {
                    rules.insert(rule.id);
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
            rules.insert(rule.id);
        }
    }

    rules
}

/// 测试用：把命中的规则展开成动作集合，忽略顺序与重复计数
#[cfg(test)]
fn desired_actions(
    map: &CompiledMap,
    values: &PadValues,
    grabbed: bool,
) -> std::collections::HashSet<Action> {
    desired_rules(map, values, grabbed)
        .into_iter()
        .filter_map(|id| combo_of(map, id))
        .flat_map(|combo| combo.press())
        .collect()
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
#[path = "../tests/unit/relay_bridge_tests.rs"]
mod bridge_tests;

#[cfg(test)]
#[path = "../tests/unit/relay_tests.rs"]
mod tests;
