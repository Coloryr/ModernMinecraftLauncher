//! 设置窗口：资源映射的存量处理

use crate::dtos::ResourceMapResultDto;
use crate::err_keys;

/// 把所有实例里带在线信息的模组 / 资源包搬进公共目录并补硬链接
///
/// 只处理实例里实际存在、带 SHA1 的文件；禁用的文件和缺失的文件跳过。
///
/// # 返回值
///
/// 见 ResourceMapResultDto；资源映射未开启时返回 err.resourceLinkOff
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_map_resources() -> Result<ResourceMapResultDto, String> {
    if !mml_link::is_enabled() {
        return Err(err_keys::RESOURCE_LINK_OFF.to_string());
    }

    tauri::async_runtime::spawn_blocking(|| {
        let mut result = ResourceMapResultDto::default();
        for game in mml_game::get_instances() {
            result.instances += 1;
            let count = mml_game::game_link::map_instance_resources(&game);
            result.mapped += count.mapped;
            result.failed += count.failed;
            result.skipped += count.skipped;
        }
        result
    })
    .await
    .map_err(|err| err.to_string())
}
