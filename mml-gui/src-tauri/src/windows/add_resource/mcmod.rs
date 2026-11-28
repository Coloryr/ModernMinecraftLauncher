//! MC 百科搜索：调用 api.coloryr.com 的 findmod，把百科条目补成资源列表卡片
//!
//! 列表先用百科名字与占位图立即显示，在线信息由 `add_resource_mcmod_info` 后补：
//! 图标 / 简介 / 下载量优先取 Modrinth，没有 Modrinth 条目再退回 CurseForge。
//! 条目 `source` 是实际能下文件的源，`source = mcmod` 表示没有在线项目。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use mml_game::launcher::FileType;
use mml_game::launcher_path::instance_path::OnlineInfoList;
use mml_game::loader::LoaderType;
use mml_net::curseforge_api::list_obj::CurseForgeListDataObj;
use mml_net::modrinth_api::project_obj::ModrinthProjectObj;
use mml_net::modrinth_api::search_obj::HitObj;
use mml_net::modrinth_api::team_obj::ModrinthTeamObj;
use mml_net::{curseforge_api, modrinth_api, urls};
use serde::Deserialize;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::collect_utils;
use crate::dtos::{McmodDto, PicDto, ProjectDto, ProjectItemDto, SourceTypeDto};
use crate::image_manager;

use super::list::check_download_now;

/// 独立服务的地址；放在这里而不是配置里，启动器不需要用户再填
const API: &str = "https://api.coloryr.com/findmod";
/// 版本列表接口
const GROUP_API: &str = "https://api.coloryr.com/getmcmodgroup";

/// findmod 返回的单条 mcmod 数据
#[derive(Debug, Deserialize)]
struct McmodItem {
    /// 百科条目 ID
    mcmod_id: i64,
    /// 名称
    mcmod_name: String,
    /// 0 = 模组，1 = 整合包
    #[serde(default)]
    mcmod_type: i32,
    /// CurseForge 页面；旧接口可能给 null
    #[serde(default)]
    curseforge_url: Option<String>,
    /// CurseForge 数值 ID；旧接口可能给 null
    #[serde(default)]
    curseforge_id: Option<String>,
    /// Modrinth 页面；旧接口可能给 null
    #[serde(default)]
    modrinth_url: Option<String>,
    /// Modrinth 项目 ID；旧接口可能给 null
    #[serde(default)]
    modrinth_id: Option<String>,
}

/// findmod 的搜索响应
#[derive(Debug, Deserialize)]
struct McmodList {
    /// 业务码，100 才是成功
    res: i32,
    /// 命中总数；老服务端没有这个字段
    #[serde(default)]
    count: Option<u64>,
    /// 条目，键是 mcmod_id
    #[serde(default)]
    data: HashMap<String, McmodItem>,
}

/// getmcmodgroup 的响应，只取版本列表
#[derive(Debug, Deserialize)]
struct McmodGroup {
    #[serde(default)]
    data: McmodGroupData,
}

#[derive(Debug, Default, Deserialize)]
struct McmodGroupData {
    #[serde(default)]
    versions: Vec<String>,
}

/// 从在线源补到的项目数据
enum Fetched {
    /// Modrinth 项目与团队成员
    Modrinth {
        /// 项目详情
        project: ModrinthProjectObj,
        /// 团队成员；头像在主循环里登记，避免多个任务同时写网址表
        team: Vec<ModrinthTeamObj>,
    },
    /// CurseForge 项目，由一页一次的批量查询结果共享
    Curseforge(Arc<CurseForgeListDataObj>),
    /// 两个源都取不到
    None,
}

/// 取字符串字段并去掉前后空白，空串当作没有
fn text_of(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// 按 Modrinth、CurseForge 的顺序补一次项目数据
///
/// CurseForge 数据来自一次批量查询，这里不再逐条发请求：一页 20 条挨个查
/// 会很快耗光 API 配额。
///
/// - `item`: 百科条目，只用到两个项目 ID
/// - `curseforge`: 本次要补的 CurseForge 项目，键是项目 ID
///
/// # 返回值
///
/// Modrinth 优先；取不到再查批量结果；都没有时是 `Fetched::None`
async fn fetch_project(
    item: &McmodDto,
    curseforge: &HashMap<u64, Arc<CurseForgeListDataObj>>,
) -> Fetched {
    let modrinth_id = item.modrinth_id.trim();
    if !modrinth_id.is_empty() {
        // 项目与作者并行取：作者是另一个接口，串行会让一页搜索多等一倍
        let (project, team) = tokio::join!(
            modrinth_api::get_project(modrinth_id),
            modrinth_api::get_team(modrinth_id)
        );
        if let Ok(project) = project {
            return Fetched::Modrinth {
                project,
                team: team.unwrap_or_default(),
            };
        }
    }

    if let Ok(id) = item.curseforge_id.trim().parse::<u64>() {
        if let Some(data) = curseforge.get(&id) {
            return Fetched::Curseforge(Arc::clone(data));
        }
    }

    Fetched::None
}

/// 加载器转 findmod 的 0–4 编号
///
/// - `loader`: 启动器里的加载器
///
/// # 返回值
///
/// 0 = 任意，1 = Forge，2 = Fabric，3 = Quilt，4 = NeoForge
fn loader_id(loader: LoaderType) -> i32 {
    match loader {
        LoaderType::Forge => 1,
        LoaderType::Fabric => 2,
        LoaderType::Quilt => 3,
        LoaderType::NeoForge => 4,
        _ => 0,
    }
}

/// 由百科条目构造只带名字与 ID 的占位卡片
///
/// - `entry`: 百科条目
/// - `file_type`: 资源类型
/// - `uuid`: 目标实例，用来判断下载中
/// - `info`: 实例的在线信息，用来判断已安装
///
/// # 返回值
///
/// 列表卡片；图标 / 简介等在线信息留空，等 `enrich` 补
fn base_item(
    entry: &McmodDto,
    file_type: FileType,
    uuid: Option<&Uuid>,
    info: &OnlineInfoList,
) -> ProjectItemDto {
    let has_real = !entry.modrinth_id.is_empty() || !entry.curseforge_id.is_empty();

    let (source, pid, url) = if !entry.modrinth_id.is_empty() {
        let url = if entry.modrinth_url.is_empty() {
            format!("{}mod/{}", urls::MODRINTH, entry.modrinth_id)
        } else {
            entry.modrinth_url.clone()
        };
        ("modrinth", entry.modrinth_id.clone(), url)
    } else if !entry.curseforge_id.is_empty() {
        (
            "curseforge",
            entry.curseforge_id.clone(),
            entry.curseforge_url.clone(),
        )
    } else {
        ("mcmod", entry.mcmod_id.clone(), entry.mcmod_url.clone())
    };

    let download = has_real
        && ((!entry.modrinth_id.is_empty() && info.get(&entry.modrinth_id).is_some())
            || (!entry.curseforge_id.is_empty() && info.get(&entry.curseforge_id).is_some()));
    let download_now = has_real
        && ((!entry.modrinth_id.is_empty() && check_download_now(uuid, &entry.modrinth_id))
            || (!entry.curseforge_id.is_empty() && check_download_now(uuid, &entry.curseforge_id)));
    let is_star = has_real && collect_utils::is_star(&pid);

    ProjectItemDto {
        name: entry.mcmod_name.clone(),
        summary: String::new(),
        image: None,
        authors: Vec::new(),
        tag: Vec::new(),
        screenshots: Vec::new(),
        download_count: 0,
        date: String::new(),
        download,
        can_star: has_real,
        is_star,
        download_now,
        url,
        mcmod: Some(entry.clone()),
        source: SourceTypeDto {
            file_type: file_type.to_string(),
            source: source.to_string(),
            pid,
            fid: String::new(),
        },
    }
}

/// 搜索 mcmod，只取名字与 ID，不在这里补在线信息
///
/// - `file_type`: 资源类型，只有模组支持
/// - `uuid`: 目标实例；None 表示还没选实例，不做已安装标记
/// - `info`: 实例的在线信息，用来判断是否已安装
/// - `filter`: 搜索词
/// - `page`: 页码，从 0 开始
/// - `version`: 游戏版本
/// - `loader`: 模组加载器
///
/// # 返回值
///
/// 一页项目与命中总数；接口不可用或返回失败时返回错误文案
pub(super) async fn search(
    file_type: FileType,
    uuid: Option<&Uuid>,
    info: &OnlineInfoList,
    filter: Option<&str>,
    page: u32,
    version: Option<&str>,
    loader: LoaderType,
) -> Result<ProjectDto, String> {
    if file_type != FileType::Mod {
        return Err(String::from("mcmod only supports mods"));
    }

    let ids = [
        filter.unwrap_or_default().to_string(),
        page.to_string(),
        String::new(),
        version.unwrap_or_default().to_string(),
        loader_id(loader).to_string(),
    ];
    let body = serde_json::json!({ "type": 2, "ids": ids });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|err| err.to_string())?;
    let resp = client
        .post(API)
        .json(&body)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("mcmod api {}", resp.status()));
    }
    let list = resp
        .json::<McmodList>()
        .await
        .map_err(|err| err.to_string())?;
    if list.res != 100 {
        return Err(format!("mcmod api res {}", list.res));
    }

    let mut entries = Vec::new();
    for item in list.data.into_values() {
        // 这个窗口只列模组，整合包条目跳过
        if item.mcmod_type != 0 {
            continue;
        }
        entries.push(McmodDto {
            mcmod_id: item.mcmod_id.to_string(),
            mcmod_name: item.mcmod_name,
            curseforge_id: text_of(item.curseforge_id.as_deref()),
            curseforge_url: text_of(item.curseforge_url.as_deref()),
            modrinth_id: text_of(item.modrinth_id.as_deref()),
            modrinth_url: text_of(item.modrinth_url.as_deref()),
            mcmod_url: format!("https://www.mcmod.cn/class/{}.html", item.mcmod_id),
        });
    }

    let items = entries
        .iter()
        .map(|entry| base_item(entry, file_type, uuid, info))
        .collect();

    // 老服务端没有 count，用当前页兜底，至少首屏能显示
    let count = match list.count {
        Some(count) if count > 0 => count,
        _ => page as u64 * 20 + entries.len() as u64,
    };

    Ok(ProjectDto { items, count })
}

/// 给已经显示的 mcmod 卡片补在线信息
///
/// 由前端在列表渲染完之后调用；同一页只发一次 CurseForge 批量查询，
/// Modrinth 侧逐条并行取项目与作者。
///
/// - `file_type`: 资源类型
/// - `uuid`: 目标实例，用来判断下载中
/// - `info`: 实例的在线信息，用来判断已安装
/// - `entries`: 百科条目
///
/// # 返回值
///
/// 补好在线信息的条目，顺序不保证；取不到的项目退回占位卡片
pub(super) async fn enrich(
    file_type: FileType,
    uuid: Option<&Uuid>,
    info: &OnlineInfoList,
    entries: Vec<McmodDto>,
) -> Vec<ProjectItemDto> {
    if entries.is_empty() {
        return Vec::new();
    }

    // CurseForge 只查一次：逐条 get_mod_info 很快会耗光 API 配额
    let mut curseforge_ids: Vec<u64> = entries
        .iter()
        .filter_map(|entry| entry.curseforge_id.parse::<u64>().ok())
        .collect();
    curseforge_ids.sort_unstable();
    curseforge_ids.dedup();
    let curseforge = if curseforge_ids.is_empty() {
        HashMap::new()
    } else {
        curseforge_api::get_mods_info(curseforge_ids)
            .await
            .map(|page| {
                page.data
                    .into_iter()
                    .map(|data| (data.id, Arc::new(data)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let curseforge = Arc::new(curseforge);

    // 一页 20 条，逐条补在线信息会等很久；并发拉，回来后再统一构造 DTO
    let mut set = JoinSet::new();
    for entry in entries {
        let curseforge = Arc::clone(&curseforge);
        set.spawn(async move {
            let fetched = fetch_project(&entry, &curseforge).await;
            (entry, fetched)
        });
    }

    let mut items = Vec::new();
    while let Some(joined) = set.join_next().await {
        let Ok((entry, fetched)) = joined else {
            continue;
        };

        let base = base_item(&entry, file_type, uuid, info);

        let dto = match fetched {
            Fetched::Modrinth { project, team } => {
                let authors = team
                    .into_iter()
                    .filter(|member| !member.user.username.is_empty())
                    .map(|member| PicDto {
                        name: member.user.username,
                        logo: member
                            .user
                            .avatar_url
                            .map(|url| image_manager::push_image_url(&url)),
                    })
                    .collect();
                let hit = HitObj {
                    project_id: project.id.clone(),
                    title: entry.mcmod_name.clone(),
                    description: project.description.clone(),
                    categories: project.categories.clone(),
                    downloads: project.downloads,
                    icon_url: if project.icon_url.is_empty() {
                        None
                    } else {
                        Some(project.icon_url.clone())
                    },
                    date_modified: project.updated.clone(),
                    ..Default::default()
                };
                let mut dto = ProjectItemDto::new_modrinth(
                    &hit,
                    file_type,
                    base.download,
                    true,
                    base.is_star,
                    base.download_now,
                    Some(entry.clone()),
                    None,
                )
                .await;
                dto.name = entry.mcmod_name.clone();
                dto.authors = authors;
                dto
            }
            Fetched::Curseforge(data) => {
                let mut dto = ProjectItemDto::new_curseforge(
                    &data,
                    file_type,
                    base.download,
                    true,
                    base.is_star,
                    base.download_now,
                    Some(entry.clone()),
                );
                dto.name = entry.mcmod_name.clone();
                dto
            }
            Fetched::None => base,
        };
        items.push(dto);
    }

    items
}

/// 获取 mcmod 支持的游戏版本列表
///
/// # 返回值
///
/// 版本字符串列表；接口不可用时返回空表，至少不阻断搜索
pub(super) async fn versions() -> Vec<String> {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    else {
        return Vec::new();
    };
    let Ok(resp) = client.post(GROUP_API).send().await else {
        return Vec::new();
    };
    let Ok(group) = resp.json::<McmodGroup>().await else {
        return Vec::new();
    };
    group.data.versions
}
