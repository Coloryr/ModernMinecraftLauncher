//! 资源管理窗口的材质包分类：列表 / 删除 / 启用 / 禁用
//!
//! 启用与禁用走 `InstanceSettingObj` 的 `options.txt` 读写，与内核同一套口径，
//! 列表里的图标 / 描述来自 `pack.mcmeta`。

use crate::dtos::PackItemDto;
use mml_game::game_resourcepacks::ResourcepackObj;
use mml_sys::path_helper;

use super::{KIND_RESOURCEPACKS, block_on_instance, data_url, parse_instance, resource_file};

// ==================== 材质包 ====================

/// 材质包列表：解析 pack.mcmeta 与图标
///
/// - `uuid`: 实例 uuid
/// - `lang`: 界面语言代码，取值 `zh_cn` / `en_us`，与前端 `locale` 同值。简介写成
///   `translate` 组件时按它去查资源包自带的 `assets/<命名空间>/lang/<语言>.json`，
///   见 `mml_game::game_resourcepacks`
///
/// # 返回值
///
/// 材质包列表；取不到文件名的条目已被过滤，实例不存在或解析失败返回错误串
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_list_resourcepacks(
    uuid: String,
    lang: String,
) -> Result<Vec<PackItemDto>, String> {
    // 读的过程中删实例会被拦住，见 resource::instance_busy
    let _op = super::model::begin_if_open(&uuid);
    let instance = parse_instance(&uuid)?;

    let list = block_on_instance(instance, move |game| {
        tokio::runtime::Handle::current().block_on(async { game.get_resourcepacks(&lang).await })
    })
    .await?;

    Ok(list
        .iter()
        .filter_map(|item| {
            let file = item.path.file_name()?.to_string_lossy().to_string();
            if file.is_empty() {
                return None;
            }
            Some(PackItemDto {
                file,
                description: item.description.clone(),
                pack_format: item.pack_format,
                min_format: item.min_format,
                max_format: item.max_format,
                fail: item.fail,
                enable: item.enable,
                icon: data_url(item.icon.as_ref()),
            })
        })
        .collect())
}

/// 删除材质包，文件进回收站
///
/// - `uuid`: 实例 uuid
/// - `file`: 材质包文件名，不是完整路径
///
/// # 返回值
///
/// 只表示成功 / 失败：文件名非法返回 `err.fileName`，其余是内核错误串
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_delete_resourcepack(uuid: String, file: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let file = resource_file(&instance, KIND_RESOURCEPACKS, &file)?;
    path_helper::move_to_trash(&file).map_err(|err| err.to_string())?;
    // 同步删掉映射记录，避免启动检查把删掉的资源链回来
    let (id, base) = {
        let game = instance.read().unwrap();
        (game.uuid, game.get_base_path())
    };
    mml_link::remove_link(id, &base, &file);
    Ok(())
}

/// 启用材质包：把这一条加进 options.txt 的 `resourcePacks`
///
/// **薄封装**：改 options.txt 的逻辑在 `mml_game::game_resourcepacks` 的
/// `InstanceSettingObj::enable_resourcepacks` 里，这里只做两件事 —— 按文件名定位包
/// 以防路径穿越，见 [`resource_file`]，再把结果转成 IPC 错误串。
///
/// 给内核的 `ResourcepackObj` 只填了 `path`：开 / 关一个包只需要知道是哪一个文件，
/// 而文件全名从路径就能推出来，不必为一次改名把整份列表的哈希重算一遍。
///
/// - `uuid`: 实例 uuid
/// - `file`: 材质包文件名，options.txt 里记的是 `file/<文件名>`
///
/// # 返回值
///
/// 只表示成功 / 失败：文件名非法返回 `err.fileName`，其余是内核错误串
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_resourcepack_enable(uuid: String, file: String) -> Result<(), String> {
    resourcepack_set(uuid, file, true).await
}

/// 禁用材质包：把这一条从 options.txt 的 `resourcePacks` 里摘掉
///
/// - `uuid`: 实例 uuid
/// - `file`: 材质包文件名，options.txt 里记的是 `file/<文件名>`
///
/// # 返回值
///
/// 只表示成功 / 失败：文件名非法返回 `err.fileName`，其余是内核错误串
///
/// 与 [`resource_resourcepack_enable`] 同一条路径，见那边的说明。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_resourcepack_disable(uuid: String, file: String) -> Result<(), String> {
    resourcepack_set(uuid, file, false).await
}

/// 启用 / 禁用材质包的公共实现，`enable` 决定走内核哪一个方法
///
/// - `uuid`: 实例 uuid
/// - `file`: 材质包文件名，不是完整路径
/// - `enable`: `true` 走启用、`false` 走禁用
///
/// # 返回值
///
/// 只表示成功 / 失败：文件名非法返回 `err.fileName`，其余是内核错误串
async fn resourcepack_set(uuid: String, file: String, enable: bool) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let path = resource_file(&instance, KIND_RESOURCEPACKS, &file)?;
    let pack = ResourcepackObj {
        path,
        ..Default::default()
    };

    block_on_instance(instance, move |game| {
        if enable {
            game.enable_resourcepacks(&pack)
        } else {
            game.disable_resourcepacks(&pack)
        }
        .map_err(|err| err.to_string())
    })
    .await?
}
