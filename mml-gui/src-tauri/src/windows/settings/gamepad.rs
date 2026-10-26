//! 设置页的手柄配置与只读实时测试。

use crate::{
    dtos::integration_dto::{GamepadDeviceDto, GamepadSettingsDto, GamepadStatusDto},
    integrations,
};
use uuid::Uuid;

/// 读取持久化的手柄映射设置。
///
/// # 返回值
///
/// 当前手柄设置。
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_gamepad_get() -> GamepadSettingsDto {
    integrations::get().gamepad.into()
}

/// 验证配置、更新设备线程并排队保存。
///
/// - `config`: 手柄设置
///
/// # 返回值
///
/// 目标实例或范围非法时返回错误键。
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_gamepad_save(config: GamepadSettingsDto) -> Result<(), String> {
    let target = config
        .target
        .filter(|value| !value.is_empty())
        .map(|value| Uuid::parse_str(&value))
        .transpose()
        .map_err(|_| "err.gamepadSettings".to_string())?;
    if let Some(id) = target
        && mml_game::get_instance(&id).is_none()
    {
        return Err("err.gameNotFound".into());
    }
    let settings = mml_gamepad::Settings {
        enabled: config.enabled,
        target,
        device: config.device,
        deadzone: config.deadzone,
        look_speed: config.look_speed,
        cursor_speed: config.cursor_speed,
    };
    if !settings.valid() {
        return Err("err.gamepadSettings".into());
    }
    integrations::configure_gamepad(settings);
    Ok(())
}

/// 返回最近一帧的设备、轴和按钮数据，不触发任何输入。
///
/// # 返回值
///
/// 界面测试用的实时快照。
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_gamepad_status() -> GamepadStatusDto {
    let status = integrations::gamepad_status();
    GamepadStatusDto {
        devices: status
            .devices
            .into_iter()
            .map(|device| GamepadDeviceDto {
                id: device.id,
                name: device.name,
            })
            .collect(),
        axes: vec![
            status.snapshot.left_x,
            status.snapshot.left_y,
            status.snapshot.right_x,
            status.snapshot.right_y,
        ],
        buttons: status
            .snapshot
            .buttons
            .into_iter()
            .map(|button| format!("{button:?}"))
            .collect(),
        active: status.active,
        error: status.error,
    }
}
