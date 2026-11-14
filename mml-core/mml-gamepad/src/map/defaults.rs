//! 内置默认映射
//!
//! 键位与摇杆的默认规则，以及动作字符串解析。

use crate::reader::axis_index;

use super::*;

/// 建一条按钮绑定
pub(super) fn button_rule(button: Button, action: &str) -> ButtonRule {
    ButtonRule {
        button,
        modifier: None,
        action: action.to_string(),
        mode: ButtonMode::Normal,
    }
}

/// 建一条连发模式的按钮绑定
pub(super) fn repeat_rule(button: Button, action: &str) -> ButtonRule {
    ButtonRule {
        button,
        modifier: None,
        action: action.to_string(),
        mode: ButtonMode::Repeat,
    }
}

/// 建一条轴绑定
pub(super) fn axis_rule(axis: Axis, from: f32, to: f32, action: &str) -> AxisRule {
    AxisRule {
        axis,
        from,
        to,
        action: action.to_string(),
    }
}

/// 摇杆对应的轴与速度
pub(super) fn stick_rule(stick: Stick, speed: f32) -> StickRule {
    StickRule {
        axes: stick_axes(stick),
        speed,
        stick,
    }
}

/// 摇杆对应的两个轴在状态数组中的下标
pub(super) fn stick_axes(stick: Stick) -> StickAxes {
    let (x, y) = match stick {
        Stick::Left => (Axis::LeftStickX, Axis::LeftStickY),
        Stick::Right => (Axis::RightStickX, Axis::RightStickY),
    };
    StickAxes {
        x: axis_index(x).expect("手柄轴表应包含左右摇杆"),
        y: axis_index(y).expect("手柄轴表应包含左右摇杆"),
    }
}

/// 轴所属的摇杆；十字键不归属任何摇杆
pub(super) fn stick_of_axis(axis: Axis) -> Option<Stick> {
    match axis {
        Axis::LeftStickX | Axis::LeftStickY | Axis::LeftZ => Some(Stick::Left),
        Axis::RightStickX | Axis::RightStickY | Axis::RightZ => Some(Stick::Right),
        Axis::DPadX | Axis::DPadY => None,
    }
}

/// 解析配置里的动作字符串，`+` 连接表示按顺序输出
///
/// 例：`LeftAlt+E` 先按住 LeftAlt 再按 E，松开时反序。
pub(super) fn parse_combo(text: &str) -> Result<Combo, String> {
    let mut steps = Vec::new();
    for part in text.split('+') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("动作里有空项：{text}"));
        }
        let action = parse_action(part)?;
        if steps.contains(&action) {
            return Err(format!("动作重复：{text}"));
        }
        steps.push(action);
    }
    if steps.is_empty() {
        return Err(format!("动作为空：{text}"));
    }
    Ok(Combo { steps })
}

/// 按规则编号取动作序列
///
/// - `map`: 编译后的映射
/// - `id`: 按钮与轴共用的规则编号
pub(crate) fn combo_of(map: &CompiledMap, id: usize) -> Option<&Combo> {
    map.buttons
        .iter()
        .chain(map.cursor_buttons.iter())
        .find(|rule| rule.id == id)
        .map(|rule| &rule.combo)
        .or_else(|| {
            map.axes
                .iter()
                .find(|rule| rule.id == id)
                .map(|rule| &rule.combo)
        })
}

/// 解析配置里的动作字符串
pub(super) fn parse_action(text: &str) -> Result<Action, String> {
    if let Some(name) = text.strip_prefix("Scroll") {
        return match name {
            "Up" => Ok(Action::Scroll(1)),
            "Down" => Ok(Action::Scroll(-1)),
            _ => Err(format!("未知滚轮动作：{text}")),
        };
    }
    if let Some(name) = text.strip_prefix("Mouse") {
        let button = match name {
            "Left" => 0,
            "Right" => 1,
            "Middle" => 2,
            "4" => 3,
            "5" => 4,
            "6" => 5,
            "7" => 6,
            "8" => 7,
            _ => return Err(format!("未知鼠标按键：{text}")),
        };
        return Ok(Action::Mouse(button));
    }
    key_code(text)
        .map(Action::Key)
        .ok_or_else(|| format!("未知按键：{text}"))
}

/// 键名转 GLFW 键码，大小写不敏感
pub(super) fn key_code(name: &str) -> Option<i32> {
    if name.len() == 1 {
        let ch = name.chars().next()?;
        if ch.is_ascii_alphabetic() {
            return Some(i32::from(ch.to_ascii_uppercase() as u8));
        }
        if ch.is_ascii_digit() {
            return Some(i32::from(ch as u8));
        }
    }

    let upper = name.to_ascii_uppercase();

    if let Some(number) = upper
        .strip_prefix('F')
        .and_then(|rest| rest.parse::<u32>().ok())
    {
        if (1..=25).contains(&number) {
            return Some(289 + number as i32);
        }
    }

    if let Some(rest) = upper.strip_prefix("KP") {
        if let Ok(number) = rest.parse::<u32>() {
            if number <= 9 {
                return Some(320 + number as i32);
            }
        }
        return match rest {
            "DECIMAL" => Some(330),
            "DIVIDE" => Some(331),
            "MULTIPLY" => Some(332),
            "SUBTRACT" => Some(333),
            "ADD" => Some(334),
            "ENTER" => Some(335),
            "EQUAL" => Some(336),
            _ => None,
        };
    }

    let code = match upper.as_str() {
        "SPACE" => 32,
        "APOSTROPHE" => 39,
        "COMMA" => 44,
        "MINUS" => 45,
        "PERIOD" => 46,
        "SLASH" => 47,
        "SEMICOLON" => 59,
        "EQUAL" => 61,
        "LEFTBRACKET" => 91,
        "BACKSLASH" => 92,
        "RIGHTBRACKET" => 93,
        "GRAVE" | "BACKTICK" => 96,
        "ESCAPE" | "ESC" => 256,
        "ENTER" | "RETURN" => 257,
        "TAB" => 258,
        "BACKSPACE" => 259,
        "INSERT" => 260,
        "DELETE" => 261,
        "RIGHT" => 262,
        "LEFT" => 263,
        "DOWN" => 264,
        "UP" => 265,
        "PAGEUP" => 266,
        "PAGEDOWN" => 267,
        "HOME" => 268,
        "END" => 269,
        "CAPSLOCK" => 280,
        "SCROLLLOCK" => 281,
        "NUMLOCK" => 282,
        "PRINTSCREEN" => 283,
        "PAUSE" => 284,
        "LEFTSHIFT" | "SHIFT" => 340,
        "LEFTCONTROL" | "LEFTCTRL" | "CONTROL" | "CTRL" => 341,
        "LEFTALT" | "ALT" => 342,
        "LEFTSUPER" | "SUPER" | "WIN" => 343,
        "RIGHTSHIFT" => 344,
        "RIGHTCONTROL" | "RIGHTCTRL" => 345,
        "RIGHTALT" => 346,
        "RIGHTSUPER" => 347,
        "MENU" => 348,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
#[path = "../../tests/unit/map_tests.rs"]
mod tests;
