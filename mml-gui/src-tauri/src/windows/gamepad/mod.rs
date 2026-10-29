//! 手柄配置窗口：映射配置的规格 + IPC 命令
//!
//! 读取、映射与转发都在内核 crate mml-gamepad 里；这里只做 DTO 转换与错误键映射。

use mml_gamepad::{
    Axis, AxisRule, Button, ButtonRule, GamepadConfig, GamepadError, MouseRule, Stick,
};

use crate::dtos::{
    GamepadAxisDto, GamepadAxisRuleDto, GamepadAxisValueDto, GamepadButtonDto,
    GamepadButtonRuleDto, GamepadButtonValueDto, GamepadConfigDto, GamepadDto, GamepadMouseDto,
    GamepadProfileDto, GamepadStateDto, GamepadStickDto,
};
use crate::err_keys;

/// 列出当前已连接的手柄
///
/// # 返回值
///
/// 按编号升序返回；没有手柄时为空列表
#[gui_macros::command]
pub fn gamepad_list() -> Vec<GamepadDto> {
    mml_gamepad::pads()
        .into_iter()
        .map(|pad| GamepadDto {
            id: pad.id,
            name: pad.name,
        })
        .collect()
}

/// 读取一个手柄的当前状态
///
/// - `id`: 手柄编号，即 gamepad_list 返回的编号
///
/// # 返回值
///
/// 手柄已连接时返回快照，找不到返回 None
#[gui_macros::command]
pub fn gamepad_state(id: u64) -> Option<GamepadStateDto> {
    mml_gamepad::pad_state(id).map(|state| GamepadStateDto {
        id: state.id,
        name: state.name,
        buttons: state
            .buttons
            .into_iter()
            .map(|item| GamepadButtonValueDto {
                button: button_to_dto(item.button),
                value: item.value,
            })
            .collect(),
        axes: state
            .axes
            .into_iter()
            .map(|item| GamepadAxisValueDto {
                axis: axis_to_dto(item.axis),
                value: item.value,
            })
            .collect(),
    })
}

/// 列出配置目录里的全部映射
///
/// # 返回值
///
/// 按名字排序；读取失败的条目也在列表里，原因放在 error 字段
#[gui_macros::command]
pub fn gamepad_get_profiles() -> Vec<GamepadProfileDto> {
    mml_gamepad::profiles()
        .into_iter()
        .map(|item| GamepadProfileDto {
            name: item.name,
            active: item.active,
            error: item.error,
        })
        .collect()
}

/// 读取一份映射
///
/// - `name`: 配置名，即文件名去掉 `.json`
///
/// # 返回值
///
/// 配置内容；文件不存在或不合法时返回错误键
#[gui_macros::command]
pub fn gamepad_get_config(name: String) -> Result<GamepadConfigDto, String> {
    mml_gamepad::config(&name)
        .map(|config| config_to_dto(&config))
        .map_err(error_key)
}

/// 保存一份映射
///
/// - `name`: 配置名，即文件名去掉 `.json`
/// - `config`: 界面编辑后的整份映射
///
/// # 返回值
///
/// 保存成功返回 Ok；文件不存在或内容不合法时返回错误键
#[gui_macros::command]
pub fn gamepad_save_config(name: String, config: GamepadConfigDto) -> Result<(), String> {
    let config = config_from_dto(config);
    mml_gamepad::save(&name, &config).map_err(error_key)
}

/// 新建一份映射，内容复制当前生效的那份
///
/// - `name`: 新配置的显示名，同时作为文件名
///
/// # 返回值
///
/// 建好返回 Ok；重名或名字不合法时返回错误键
#[gui_macros::command]
pub fn gamepad_create_profile(name: String) -> Result<(), String> {
    mml_gamepad::create(&name).map_err(error_key)
}

/// 删除一份映射，至少保留一份
///
/// - `name`: 要删除的配置
///
/// # 返回值
///
/// 删除成功返回 Ok；只剩一份或文件不存在时返回错误键
#[gui_macros::command]
pub fn gamepad_delete_profile(name: String) -> Result<(), String> {
    mml_gamepad::delete(&name).map_err(error_key)
}

/// 把一份映射设为当前生效
///
/// - `name`: 要生效的配置
///
/// # 返回值
///
/// 改完返回 Ok；文件不存在时返回错误键
#[gui_macros::command]
pub fn gamepad_set_active(name: String) -> Result<(), String> {
    mml_gamepad::set_active(&name).map_err(error_key)
}

/// 配置界面可选的键鼠动作名
///
/// # 返回值
///
/// 按键盘 / 功能键 / 鼠标 / 滚轮排好的动作名
#[gui_macros::command]
pub fn gamepad_get_actions() -> Vec<String> {
    mml_gamepad::action_names()
}

/// 把内核错误换成界面的错误键
fn error_key(error: GamepadError) -> String {
    match error {
        GamepadError::InvalidName => err_keys::GAMEPAD_PROFILE_INVALID.to_string(),
        GamepadError::Exists => err_keys::GAMEPAD_PROFILE_EXISTS.to_string(),
        GamepadError::Missing => err_keys::GAMEPAD_PROFILE_MISSING.to_string(),
        GamepadError::Last => err_keys::GAMEPAD_PROFILE_LAST.to_string(),
        GamepadError::InvalidConfig => err_keys::GAMEPAD_CONFIG_INVALID.to_string(),
        GamepadError::Io(message) => message,
    }
}

/// 按钮 DTO 转内核枚举
fn button_from_dto(button: GamepadButtonDto) -> Button {
    match button {
        GamepadButtonDto::South => Button::South,
        GamepadButtonDto::East => Button::East,
        GamepadButtonDto::North => Button::North,
        GamepadButtonDto::West => Button::West,
        GamepadButtonDto::C => Button::C,
        GamepadButtonDto::Z => Button::Z,
        GamepadButtonDto::LeftTrigger => Button::LeftTrigger,
        GamepadButtonDto::LeftTrigger2 => Button::LeftTrigger2,
        GamepadButtonDto::RightTrigger => Button::RightTrigger,
        GamepadButtonDto::RightTrigger2 => Button::RightTrigger2,
        GamepadButtonDto::Select => Button::Select,
        GamepadButtonDto::Start => Button::Start,
        GamepadButtonDto::Mode => Button::Mode,
        GamepadButtonDto::LeftThumb => Button::LeftThumb,
        GamepadButtonDto::RightThumb => Button::RightThumb,
        GamepadButtonDto::DPadUp => Button::DPadUp,
        GamepadButtonDto::DPadDown => Button::DPadDown,
        GamepadButtonDto::DPadLeft => Button::DPadLeft,
        GamepadButtonDto::DPadRight => Button::DPadRight,
    }
}

/// 内核按钮转 DTO
fn button_to_dto(button: Button) -> GamepadButtonDto {
    match button {
        Button::South => GamepadButtonDto::South,
        Button::East => GamepadButtonDto::East,
        Button::North => GamepadButtonDto::North,
        Button::West => GamepadButtonDto::West,
        Button::C => GamepadButtonDto::C,
        Button::Z => GamepadButtonDto::Z,
        Button::LeftTrigger => GamepadButtonDto::LeftTrigger,
        Button::LeftTrigger2 => GamepadButtonDto::LeftTrigger2,
        Button::RightTrigger => GamepadButtonDto::RightTrigger,
        Button::RightTrigger2 => GamepadButtonDto::RightTrigger2,
        Button::Select => GamepadButtonDto::Select,
        Button::Start => GamepadButtonDto::Start,
        Button::Mode => GamepadButtonDto::Mode,
        Button::LeftThumb => GamepadButtonDto::LeftThumb,
        Button::RightThumb => GamepadButtonDto::RightThumb,
        Button::DPadUp => GamepadButtonDto::DPadUp,
        Button::DPadDown => GamepadButtonDto::DPadDown,
        Button::DPadLeft => GamepadButtonDto::DPadLeft,
        Button::DPadRight => GamepadButtonDto::DPadRight,
    }
}

/// 轴 DTO 转内核枚举
fn axis_from_dto(axis: GamepadAxisDto) -> Axis {
    match axis {
        GamepadAxisDto::LeftStickX => Axis::LeftStickX,
        GamepadAxisDto::LeftStickY => Axis::LeftStickY,
        GamepadAxisDto::LeftZ => Axis::LeftZ,
        GamepadAxisDto::RightStickX => Axis::RightStickX,
        GamepadAxisDto::RightStickY => Axis::RightStickY,
        GamepadAxisDto::RightZ => Axis::RightZ,
        GamepadAxisDto::DPadX => Axis::DPadX,
        GamepadAxisDto::DPadY => Axis::DPadY,
    }
}

/// 内核轴转 DTO
fn axis_to_dto(axis: Axis) -> GamepadAxisDto {
    match axis {
        Axis::LeftStickX => GamepadAxisDto::LeftStickX,
        Axis::LeftStickY => GamepadAxisDto::LeftStickY,
        Axis::LeftZ => GamepadAxisDto::LeftZ,
        Axis::RightStickX => GamepadAxisDto::RightStickX,
        Axis::RightStickY => GamepadAxisDto::RightStickY,
        Axis::RightZ => GamepadAxisDto::RightZ,
        Axis::DPadX => GamepadAxisDto::DPadX,
        Axis::DPadY => GamepadAxisDto::DPadY,
    }
}

/// 摇杆 DTO 转内核枚举
fn stick_from_dto(stick: GamepadStickDto) -> Stick {
    match stick {
        GamepadStickDto::Left => Stick::Left,
        GamepadStickDto::Right => Stick::Right,
    }
}

/// 内核摇杆转 DTO
fn stick_to_dto(stick: Stick) -> GamepadStickDto {
    match stick {
        Stick::Left => GamepadStickDto::Left,
        Stick::Right => GamepadStickDto::Right,
    }
}

/// 内核配置转 DTO
fn config_to_dto(config: &GamepadConfig) -> GamepadConfigDto {
    GamepadConfigDto {
        selected: config.selected,
        buttons: config
            .buttons
            .iter()
            .map(|rule| GamepadButtonRuleDto {
                button: button_to_dto(rule.button),
                modifier: rule.modifier.map(button_to_dto),
                action: rule.action.clone(),
            })
            .collect(),
        axes: config
            .axes
            .iter()
            .map(|rule| GamepadAxisRuleDto {
                axis: axis_to_dto(rule.axis),
                from: rule.from,
                to: rule.to,
                action: rule.action.clone(),
            })
            .collect(),
        mouse: GamepadMouseDto {
            cursor_stick: stick_to_dto(config.mouse.cursor_stick),
            turn_stick: stick_to_dto(config.mouse.turn_stick),
            cursor_speed: config.mouse.cursor_speed,
            turn_speed: config.mouse.turn_speed,
            deadzone: config.mouse.deadzone,
        },
    }
}

/// DTO 转内核配置
fn config_from_dto(dto: GamepadConfigDto) -> GamepadConfig {
    GamepadConfig {
        selected: dto.selected,
        buttons: dto
            .buttons
            .into_iter()
            .map(|rule| ButtonRule {
                button: button_from_dto(rule.button),
                modifier: rule.modifier.map(button_from_dto),
                action: rule.action,
            })
            .collect(),
        axes: dto
            .axes
            .into_iter()
            .map(|rule| AxisRule {
                axis: axis_from_dto(rule.axis),
                from: rule.from,
                to: rule.to,
                action: rule.action,
            })
            .collect(),
        mouse: MouseRule {
            cursor_stick: stick_from_dto(dto.mouse.cursor_stick),
            turn_stick: stick_from_dto(dto.mouse.turn_stick),
            cursor_speed: dto.mouse.cursor_speed,
            turn_speed: dto.mouse.turn_speed,
            deadzone: dto.mouse.deadzone,
        },
    }
}
