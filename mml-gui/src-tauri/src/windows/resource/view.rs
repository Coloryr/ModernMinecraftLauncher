//! 资源管理窗口的视图偏好：左侧分类顺序、上次打开的类别、模组的展示方式
//!
//! 存在实例的 `gui_setting.json` 的 `Gui` 段，见 crate::gui_setting::GameViewSettingObj，
//! 与窗口 / 实例一起走，不进全局配置。
//!
//! **不做成前端本地存储**：换个实例就该换一套，而本地存储是"每台机器一份"，
//! 两处口径不同会出现"切了实例顺序却没变"。

use std::collections::HashMap;

use crate::dtos::ResourceViewDto;

use super::parse_instance;

/// 取某实例的资源窗口视图偏好
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 视图偏好：分类顺序 / 上次打开的类别 / 模组展示方式 / 模组列宽。返回的是**原样存下来
/// 的值**，可能是空数组或空串，排序与默认值由前端补 —— 后端不认识有哪几个分类，
/// 硬编码一份的话新增分类就得改两处
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_view_get(uuid: String) -> Result<ResourceViewDto, String> {
    let instance = parse_instance(&uuid)?;
    let view = crate::gui_setting::load(&instance.read().unwrap()).view;

    Ok(ResourceViewDto {
        order: view.resource_order,
        category: view.resource_category,
        mod_view: view.resource_mod_view,
        mod_col_widths: view.resource_mod_col_widths,
    })
}

/// 保存某实例的资源窗口视图偏好：整份覆盖，未传的项保持原值
///
/// - `uuid`: 实例 uuid
/// - `order`: 左侧分类顺序，元素是分类键
/// - `category`: 上次打开的类别，空串表示没有记录
/// - `mod_view`: 模组的展示方式，前端自己解释的枚举串
/// - `mod_col_widths`: 模组列表各列的宽度，列名 → 像素宽，单位像素
///
/// # 返回值
///
/// 只表示成功 / 失败：实例不存在返回 `err.gameNotFound`
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_view_set(
    uuid: String,
    order: Vec<String>,
    category: String,
    mod_view: String,
    mod_col_widths: HashMap<String, u32>,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;

    // load → 改这一小块 → 存回整份：那份文件还存着日志设置、模组分组、方块图标
    // 读-改-写必须用写锁，否则与其它视图命令并发时会丢更新，与 edit_mod_setting 同理
    let game = instance.write().unwrap();
    let mut setting = crate::gui_setting::load(&game);
    setting.view.resource_order = order;
    setting.view.resource_category = category;
    setting.view.resource_mod_view = mod_view;
    setting.view.resource_mod_col_widths = mod_col_widths;
    crate::gui_setting::save(&game, &setting);

    Ok(())
}
