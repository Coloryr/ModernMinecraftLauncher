//! Modrinth 分类、加载器、游戏版本与作者头像查询

use super::*;

/// 获取分类图标地址
///
/// - `name`: 分类名
///
/// # 返回值
///
/// 返回图标地址；分类不存在返回 `None`
pub async fn get_categories_icon(name: &str) -> Option<String> {
    let cap = match CATEGORISE.get() {
        Some(cap) => cap,
        None => {
            let list = modrinth_api::get_categories().await;
            match list {
                Ok(list) => CATEGORISE.get_or_init(|| list),
                Err(err) => {
                    mml_log::error_type(err);
                    &Vec::new()
                }
            }
        }
    };

    cap.iter()
        .find(|item| item.name == name)
        .map(|item| item.icon.clone())
}

/// 获取分组
///
/// - `file_type`: 项目文件类型
///
/// # 返回值
///
/// 返回分类名映射表；查询失败返回对应错误
pub async fn get_categories(file_type: FileType) -> CoreResult<HashMap<String, String>> {
    let cap = match CATEGORISE.get() {
        Some(cap) => cap,
        None => {
            let list = modrinth_api::get_categories().await?;
            CATEGORISE.get_or_init(|| list)
        }
    };

    let project_type = match file_type {
        FileType::Shaderpack => modrinth_api::CLASS_SHADERPACK,
        FileType::Resourcepack => modrinth_api::CLASS_RESOURCEPACK,
        FileType::Modpack => modrinth_api::CLASS_MODPACK,
        _ => modrinth_api::CLASS_MOD,
    };

    let map = cap
        .iter()
        .filter(|item| item.project_type == project_type && item.header == "categories")
        .map(|item| (item.name.clone(), item.name.clone()))
        .collect();

    Ok(map)
}

/// 把搜索命中的标签排成"加载器在前、分类在后"
///
/// Modrinth 的搜索接口把加载器与分类混在同一个 `categories` 数组里返回（命中项没有
/// `loaders` 字段），顺序由服务端定，于是分类会夹在加载器中间。这里按 `tag/loader`
/// 的名字表把加载器认出来整体挪到前面，认不出来的一律当分类、保持原顺序 ——
/// 与项目详情页一致（那边接口里 `loaders` / `categories` 本来就是分开的）。
///
/// - `names`: 命中项的 `categories`，原样传入
///
/// # 返回值
///
/// 排好序的标签名；加载器表取不到时原样返回
pub async fn order_tags(names: &[String]) -> Vec<String> {
    let loaders = match LOADERS.get() {
        Some(list) => list,
        None => {
            let list = modrinth_api::get_loaders().await;
            match list {
                Ok(list) => {
                    LOADERS.get_or_init(|| list.into_iter().map(|item| item.name).collect())
                }
                Err(err) => {
                    mml_log::error_type(err);
                    &Vec::new()
                }
            }
        }
    };

    let (loader_tags, rest): (Vec<&String>, Vec<&String>) =
        names.iter().partition(|name| loaders.contains(*name));

    loader_tags.into_iter().chain(rest).cloned().collect()
}

/// 一批命中项里去重后的作者 ID
///
/// - `hits`: 搜索命中项
///
/// # 返回值
///
/// 按出现顺序去重的作者 ID；空 ID 跳过
fn author_ids(hits: &[HitObj]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for hit in hits {
        if !hit.author_id.is_empty() && !ids.contains(&hit.author_id) {
            ids.push(hit.author_id.clone());
        }
    }
    ids
}

/// 批量取一批搜索命中项的作者头像
///
/// Modrinth 的搜索结果只给作者名，不给头像；头像按作者 ID 另查一次，
/// 一批 ID 走一次请求。查不到（接口出错等）就少几个头像，不影响列表本身。
///
/// - `hits`: 搜索命中项，只读其中的 `author_id`
///
/// # 返回值
///
/// 作者 ID → 头像 URL；一个都没查到或请求失败时是空表
pub async fn author_avatars(hits: &[HitObj]) -> HashMap<String, String> {
    let ids = author_ids(hits);
    if ids.is_empty() {
        return HashMap::new();
    }

    match modrinth_api::get_users(&ids).await {
        Ok(list) => list
            .into_iter()
            .filter_map(|item| item.avatar_url.map(|url| (item.id, url)))
            .collect(),
        Err(err) => {
            mml_log::error_type(modrinth_avatar_err(err));
            HashMap::new()
        }
    }
}

/// 获取所有游戏版本
///
/// # 返回值
///
/// 返回游戏版本列表，首位为空串表示不限；查询失败返回对应错误
pub async fn get_game_versions() -> CoreResult<Vec<String>> {
    match GAME_VERSIONS.get() {
        Some(versions) => Ok(versions.clone()),
        None => {
            let list = modrinth_api::get_game_versions().await?;
            let mut list1: Vec<String> = list.into_iter().map(|item| item.version).collect();

            list1.insert(0, String::new());

            Ok(GAME_VERSIONS.get_or_init(|| list1).clone())
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/modrinth_mod_tests.rs"]
mod tests;
