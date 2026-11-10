//! CurseForge 分类与游戏版本查询

use super::*;

/// 版本类型排序：id > 17 按 id 倒序在前，id < 18 按 id 正序在后
fn sort_version_types(
    data: Vec<&CurseForgeVersionTypeDataObj>,
) -> Vec<&CurseForgeVersionTypeDataObj> {
    let mut new_list: Vec<_> = data.iter().copied().filter(|item| item.id > 17).collect();
    new_list.sort_by_key(|item| Reverse(item.id));

    let mut old_list: Vec<_> = data.iter().copied().filter(|item| item.id < 18).collect();
    old_list.sort_by_key(|item| item.id);

    new_list.extend(old_list);
    new_list
}

/// 获取文件类型对应的 class ID
///
/// - `file_type`: 项目文件类型
///
/// # 返回值
///
/// 返回 CurseForge API 对应的 class ID；无对应项返回 0
fn get_classid(file_type: FileType) -> u32 {
    match file_type {
        FileType::Mod => curseforge_api::CLASS_MOD,
        FileType::Save => curseforge_api::CLASS_SAVES,
        FileType::Shaderpack => curseforge_api::CLASS_SHADERPACKS,
        FileType::Resourcepack => curseforge_api::CLASS_RESOURCEPACKS,
        _ => 0,
    }
}

/// 获取分组数据
///
/// - `file_type`: 项目文件类型
///
/// # 返回值
///
/// 返回分类 ID 到分类名的映射表；查询失败返回对应错误
pub async fn get_categories(file_type: FileType) -> CoreResult<HashMap<String, String>> {
    let temp = match CATEGORIES.get() {
        Some(data) => data,
        None => {
            let list = curseforge_api::get_categories().await?;
            CATEGORIES.get_or_init(|| list)
        }
    };

    let classid = get_classid(file_type);

    let mut list: Vec<_> = temp
        .data
        .iter()
        .filter(|item| item.class_id == classid)
        .collect();

    list.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(list
        .into_iter()
        .map(|item| (item.id.to_string(), item.name.clone()))
        .collect())
}

/// 获取支持的游戏版本
///
/// # 返回值
///
/// 返回游戏版本列表，首位为空串表示不限；查询失败返回对应错误
pub async fn get_game_versions() -> CoreResult<Vec<String>> {
    if let Some(data) = GAME_VERSION.get() {
        return Ok(data.clone());
    }

    let types = curseforge_api::get_version_type().await?;
    let type_list: Vec<&CurseForgeVersionTypeDataObj> = types
        .data
        .iter()
        .filter(|item| item.name.starts_with("Minecraft "))
        .collect();

    let versions = curseforge_api::get_version().await?;
    let version_map: HashMap<u32, &CurseForgeVersionDataObj> = versions
        .data
        .iter()
        .map(|item| (item.verion_type, item))
        .collect();

    let mut list = vec![String::new()];
    for item in sort_version_types(type_list) {
        if let Some(data) = version_map.get(&item.id) {
            list.extend_from_slice(&data.versions);
        }
    }

    Ok(GAME_VERSION.get_or_init(|| list).clone())
}
