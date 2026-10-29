//! 存档云同步只在实例停止时运行，窗口关闭时取消网络请求。

use super::{model, parse_instance};
use crate::{
    dtos::integration_dto::{CloudConfigDto, CloudStatusDto, CloudWorldDto},
    err_keys, integrations,
    windows::{instance_task, main::launch_gate},
};
use mml_cloud::client::{Client, Settings};
use uuid::Uuid;

/// 组装当前实例的云连接参数
///
/// 远端绑定优先取 `cloud_instances` 里记的那份，没有就按本地实例 uuid 当远端 uuid。
///
/// - `uuid`: 本地实例 uuid，只用来校验与查绑定
///
/// # 返回值
///
/// 见 [`CloudConfigDto`]；uuid 非法返回 `err.uuid`，实例不存在返回 `err.gameNotFound`
fn connection(uuid: &str) -> Result<CloudConfigDto, String> {
    let id = Uuid::parse_str(uuid).map_err(|_| err_keys::UUID.to_string())?;
    parse_instance(uuid)?;
    let config = integrations::get();
    Ok(CloudConfigDto {
        server: config.cloud.server,
        server_key: config.cloud.server_key,
        client_key: config.cloud.client_key,
        remote_uuid: config.cloud_instances.get(&id).unwrap_or(&id).to_string(),
    })
}

/// 归一连接参数：校验云端 uuid，去掉服务器地址的尾部斜杠
///
/// - `config`: 界面传进来的连接参数
///
/// # 返回值
///
/// 归一后的参数；云端 uuid 不是合法 uuid 时返回 `err.cloudSettings`
fn normalize(mut config: CloudConfigDto) -> Result<CloudConfigDto, String> {
    config.remote_uuid = Uuid::parse_str(config.remote_uuid.trim())
        .map_err(|_| "err.cloudSettings".to_string())?
        .to_string();
    config.server = config.server.trim().trim_end_matches('/').into();
    Ok(config)
}

/// 按连接参数建云客户端
///
/// - `config`: 已归一的连接参数
///
/// # 返回值
///
/// 见 [`Client`]；服务器地址或密钥不被接受时返回错误键
fn client(config: &CloudConfigDto) -> Result<Client, String> {
    Client::new(Settings {
        server: config.server.clone(),
        server_key: config.server_key.clone(),
        client_key: config.client_key.clone(),
    })
    .map_err(|error| error.key.to_string())
}

/// 获取当前实例的云连接和远端绑定。
///
/// - `uuid`: 本地实例 UUID
///
/// # 返回值
///
/// 连接参数；首次使用时以本地 UUID 作为远端 UUID。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_cloud_get_config(uuid: String) -> Result<CloudConfigDto, String> {
    connection(&uuid)
}

/// 校验并保存云连接，不依赖前端本地缓存。
///
/// - `uuid`: 本地实例 UUID
/// - `config`: 连接参数与云端 UUID
///
/// # 返回值
///
/// 参数非法或实例占用时返回错误键。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_cloud_save_config(uuid: String, config: CloudConfigDto) -> Result<(), String> {
    parse_instance(&uuid)?;
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::UUID.to_string())?;
    let config = normalize(config)?;
    let remote =
        Uuid::parse_str(&config.remote_uuid).map_err(|_| "err.cloudSettings".to_string())?;
    let _sync = launch_gate::begin_sync(id)?;
    client(&config)?;
    integrations::update(|value| {
        value.cloud = Settings {
            server: config.server,
            server_key: config.server_key,
            client_key: config.client_key,
        };
        value.cloud_instances.insert(id, remote);
    });
    Ok(())
}

/// 查询云空间、实例开关及云端存档。
///
/// - `uuid`: 本地实例 UUID
/// - `config`: 界面已确认的连接快照
///
/// # 返回值
///
/// 云端状态；网络错误或窗口关闭时返回错误键。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_cloud_status(
    uuid: String,
    config: CloudConfigDto,
) -> Result<CloudStatusDto, String> {
    parse_instance(&uuid)?;
    let config = normalize(config)?;
    let client = client(&config)?;
    let model = model::window_model(&uuid)?;
    let op = model.begin();
    let cancel = op.token();
    let request = async {
        let storage = client.storage().await?;
        let state = client.state(&config.remote_uuid).await?;
        let worlds = if state.have {
            client.worlds(&config.remote_uuid).await?
        } else {
            Vec::new()
        };
        Ok::<_, mml_cloud::Error>(CloudStatusDto {
            enabled: state.have,
            used: storage.used,
            quota: storage.size,
            worlds: worlds
                .into_iter()
                .map(|world| CloudWorldDto {
                    name: world.name,
                    time: world.time,
                    files: world.files.len() as u32,
                })
                .collect(),
        })
    };
    tokio::select! {
        _ = cancel.cancelled() => Err(err_keys::CANCELLED.into()),
        result = request => result.map_err(|error| error.key.to_string()),
    }
}

/// 开启或暂停云端实例，暂停保留云端存档。
///
/// - `uuid`: 本地实例 UUID
/// - `config`: 界面已确认的连接快照
/// - `enabled`: 是否开放同步
///
/// # 返回值
///
/// 服务端确认后的结果。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_cloud_set_enabled(
    uuid: String,
    config: CloudConfigDto,
    enabled: bool,
) -> Result<(), String> {
    let config = normalize(config)?;
    let _task = instance_task::begin(&uuid)?;
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::UUID.to_string())?;
    let _sync = launch_gate::begin_sync(id)?;
    let instance = parse_instance(&uuid)?;
    let name = instance.read().unwrap().name.clone();
    let client = client(&config)?;
    let model = model::window_model(&uuid)?;
    let op = model.begin();
    let token = op.token();
    tokio::select! {
        _ = token.cancelled() => Err(err_keys::CANCELLED.into()),
        result = client.set_enabled(&config.remote_uuid, &name, enabled) =>
            result.map_err(|error| error.key.to_string()),
    }
}

/// 同步或删除一个云端存档，磁盘操作在阻塞线程完成。
///
/// - `uuid`: 本地实例 UUID
/// - `config`: 界面已确认的连接快照
/// - `name`: saves 下的纯目录名
/// - `direction`: upload、download 或 delete
///
/// # 返回值
///
/// 是否有内容变化；下载覆盖前自动保留旧目录。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_cloud_transfer(
    uuid: String,
    config: CloudConfigDto,
    name: String,
    direction: String,
) -> Result<bool, String> {
    let config = normalize(config)?;
    if !matches!(direction.as_str(), "upload" | "download" | "delete") {
        return Err("err.cloudProtocol".into());
    }
    mml_cloud::archive::validate_name(&name).map_err(|error| error.key.to_string())?;
    let task = instance_task::begin(&uuid)?;
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::UUID.to_string())?;
    let sync = launch_gate::begin_sync(id)?;
    let instance = parse_instance(&uuid)?;
    let (root, backups) = {
        let game = instance.read().unwrap();
        (
            game.get_saves_path().join(&name),
            game.get_backup_path().join("cloud").join(&name),
        )
    };
    let client = client(&config)?;
    let model = model::window_model(&uuid)?;
    let op = model.begin();
    let cancel = op.token();
    let runtime = tokio::runtime::Handle::current();
    tauri::async_runtime::spawn_blocking(move || {
        // 取消只打断网络等待，磁盘提交结束后才释放实例预约。
        let (_task, _sync, _op) = (task, sync, op);
        runtime.block_on(async move {
            let request = async {
                match direction.as_str() {
                    "upload" => client.upload(&config.remote_uuid, &name, &root).await,
                    "download" => {
                        client
                            .download(&config.remote_uuid, &name, &root, &backups)
                            .await
                    }
                    _ => client
                        .delete(&config.remote_uuid, &name)
                        .await
                        .map(|_| true),
                }
            };
            tokio::select! {
                _ = cancel.cancelled() => Err(err_keys::CANCELLED.into()),
                result = request => result.map_err(|error| error.key.to_string()),
            }
        })
    })
    .await
    .map_err(|_| "err.cloudIo".to_string())?
}

/// 打开云下载前保留的本地存档备份目录。
///
/// - `uuid`: 本地实例 UUID
///
/// # 返回值
///
/// 目录可访问时打开资源管理器。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_cloud_open_backups(uuid: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let path = instance.read().unwrap().get_backup_path().join("cloud");
    std::fs::create_dir_all(&path).map_err(|_| "err.cloudIo".to_string())?;
    mml_sys::open_helper::open_dir(&path);
    Ok(())
}
