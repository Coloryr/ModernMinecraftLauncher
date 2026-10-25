//! 服务器实例生成
//!
//! 把本地实例导出成"在线实例"的服务端清单：MML 的 `server.json` 与 `server.json.sha1`，
//! 或 HMCL 的 `server-manifest.json`；要托管的文件一并写进用户选的文件夹。
//!
//! 独立窗口，标签 `mml-server_instance`，入口在主窗口实例右键菜单，
//! 见 `ServerInstanceWindow.vue`。窗口已开着时再次打开只推送目标实例，不重建。
//!
//! 生成是一次性的阻塞任务，关窗不会打断它：任务在后台跑完，期间再次生成会返回忙。

mod files;

use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use mml_base::hash_helper::{gen_hash, HashType};
use mml_game::{
    launcher::{file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj},
    other_launcher::hmcl_obj::HMCLServerFileObj,
    serverpack::{
        generate,
        serverpack_obj::{ServerArchiveItemObj, ServerItemObj},
    },
};
use mml_names::names;
use tauri::{AppHandle, Emitter};

use super::instance_snapshot;
use crate::{
    dtos::log_dto::LogFocusDto,
    dtos::server_instance_dto::{
        ServerInstanceOptionsDto, ServerInstanceProgressDto, ServerInstanceResultDto,
    },
    err_keys,
};

/// 生成任务进行中：同一时间只跑一个，避免两次生成往同一个文件夹里写
static RUNNING: AtomicBool = AtomicBool::new(false);

/// 占用生成名额，命令结束时自动释放
struct RunningGuard;

impl RunningGuard {
    /// 抢名额
    ///
    /// # 返回值
    ///
    /// 已有任务在跑返回 `err.serverInstanceBusy`
    fn acquire() -> Result<Self, String> {
        RUNNING
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .map(|_| Self)
            .map_err(|_| err_keys::SERVER_INSTANCE_BUSY.to_string())
    }
}

impl Drop for RunningGuard {
    /// 释放名额
    fn drop(&mut self) {
        RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// 进度回调：已处理项数、总项数、当前处理的文件
type Progress<'a> = &'a dyn Fn(usize, usize, String);

/// 切换目标实例事件：窗口已存在时再次打开会推送，前端据此换实例
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 目标实例 uuid
#[gui_macros::emit]
pub fn emit_server_instance_focus(app: &AppHandle, event: LogFocusDto) {
    let _ = app.emit(crate::listens::SERVER_INSTANCE_FOCUS, event);
}

/// 生成进度事件，事件名 `server-instance-progress`
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 当前进度
#[gui_macros::emit]
pub fn emit_server_instance_progress(app: &AppHandle, event: ServerInstanceProgressDto) {
    let _ = app.emit(crate::listens::SERVER_INSTANCE_PROGRESS, event);
}

/// 生成服务器实例，把清单与托管文件写进指定文件夹
///
/// - `app`: 应用句柄，进度事件经它推给前端
/// - `uuid`: 源实例 uuid
/// - `options`: 生成选项，见 [`ServerInstanceOptionsDto`]
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]；参数不合法、扫描或写盘失败返回错误键或错误文案
#[gui_macros::ipc_group("server_instance")]
#[gui_macros::command]
pub async fn server_instance_generate(
    app: AppHandle,
    uuid: String,
    options: ServerInstanceOptionsDto,
) -> Result<ServerInstanceResultDto, String> {
    let guard = RunningGuard::acquire()?;
    // 生成过程中整个实例都在被读，删实例要等它结束
    let task = crate::windows::instance_task::begin(&uuid)?;
    let game = instance_snapshot(&uuid)?;
    let result = tokio::task::spawn_blocking(move || {
        let progress = |now: usize, total: usize, text: String| {
            emit_server_instance_progress(
                &app,
                ServerInstanceProgressDto {
                    now: now as u64,
                    total: total as u64,
                    text,
                },
            );
        };
        generate(&game, &options, &progress)
    })
    .await
    .map_err(|e| e.to_string())?;
    drop(task);
    drop(guard);
    result
}

/// 配置文件的同步方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigMode {
    /// 整个目录打成一个压缩包，客户端下载后解压
    Archive,
    /// 目录里的文件逐个列进清单，客户端按原路径下载
    Files,
}

impl ConfigMode {
    /// 按前端给的值解析同步方式
    ///
    /// - `value`: `archive` / `files`；空串按压缩包处理
    ///
    /// # 返回值
    ///
    /// 不认识时返回 `err.serverInstanceMode`
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "" | "archive" => Ok(ConfigMode::Archive),
            "files" => Ok(ConfigMode::Files),
            _ => Err(err_keys::SERVER_INSTANCE_MODE.to_string()),
        }
    }
}

/// 生成目标格式
enum Format {
    /// MML 的 `server.json`，配 `server.json.sha1`
    Mml,
    /// HMCL 的 `server-manifest.json`
    Hmcl,
}

impl Format {
    /// 按前端给的值解析格式
    ///
    /// - `value`: `mml` 或 `hmcl`
    ///
    /// # 返回值
    ///
    /// 不认识时返回 `err.serverInstanceFormat`
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "mml" => Ok(Format::Mml),
            "hmcl" => Ok(Format::Hmcl),
            _ => Err(err_keys::SERVER_INSTANCE_FORMAT.to_string()),
        }
    }
}

/// 本次要写进包里的内容
struct Contents {
    /// 勾选的模组，带手填的下载信息
    mods: Vec<files::ModPick>,
    /// 配置目录与各自的同步方式
    configs: Vec<(String, ConfigMode)>,
    /// 资源包文件，路径相对游戏目录
    packs: Vec<PathBuf>,
}

/// 生成入口：校验参数 → 展开勾选 → 按格式写盘
///
/// - `game`: 源实例
/// - `options`: 生成选项
/// - `progress`: 进度回调
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]
fn generate(
    game: &InstanceSettingObj,
    options: &ServerInstanceOptionsDto,
    progress: Progress<'_>,
) -> Result<ServerInstanceResultDto, String> {
    let format = Format::parse(&options.format)?;
    let url = normalize_url(&options.url)?;
    let base = game.get_game_path();
    let out = prepare_folder(&game.get_base_path(), &options.folder)?;

    let mods = files::mods(&base, &options.mods)?;
    let mut configs = Vec::new();
    for item in &options.configs {
        let dir = item.dir.trim();
        // 只认顶层目录：压缩包名与清单里的 dir 都按它拼，带路径分层的写不清
        if dir.is_empty() || dir.contains(['/', '\\']) {
            return Err(err_keys::SERVER_INSTANCE_DIR.to_string());
        }
        configs.push((dir.to_string(), ConfigMode::parse(&item.mode)?));
    }
    let packs = options
        .resource_packs
        .iter()
        .map(|path| files::file(&base, path))
        .collect::<Result<Vec<_>, _>>()?;

    if mods.is_empty() && configs.is_empty() && packs.is_empty() {
        return Err(err_keys::SERVER_INSTANCE_EMPTY.to_string());
    }

    let contents = Contents {
        mods,
        configs,
        packs,
    };

    match format {
        Format::Mml => {
            let online = online_sources(game);
            generate_mml(game, &online, &base, &out, &url, options, &contents, progress)
        }
        Format::Hmcl => generate_hmcl(game, &base, &out, &url, options, &contents, progress),
    }
}

/// 校验发布地址并去掉末尾的斜杠
///
/// - `value`: 前端填的地址
///
/// # 返回值
///
/// 规范化后的地址，拼托管文件时直接用；不是 http(s) 地址时返回
/// `err.serverInstanceUrl`
fn normalize_url(value: &str) -> Result<String, String> {
    let url = value.trim().trim_end_matches('/');
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match rest {
        Some(rest) if !rest.is_empty() => Ok(url.to_string()),
        _ => Err(err_keys::SERVER_INSTANCE_URL.to_string()),
    }
}

/// 准备输出文件夹，并确认它不在实例目录里
///
/// 写进实例目录会把刚生成的文件当成下一次的同步内容，升级流程也会把它们删掉。
///
/// - `instance`: 实例根目录
/// - `value`: 前端选的文件夹
///
/// # 返回值
///
/// 规范化后的文件夹；为空、建不出来或落在实例里时返回 `err.serverInstanceFolder`
fn prepare_folder(instance: &Path, value: &str) -> Result<PathBuf, String> {
    let folder = value.trim();
    if folder.is_empty() {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }
    let path = PathBuf::from(folder);
    fs::create_dir_all(&path).map_err(|_| err_keys::SERVER_INSTANCE_FOLDER.to_string())?;
    let path = path
        .canonicalize()
        .map_err(|_| err_keys::SERVER_INSTANCE_FOLDER.to_string())?;
    if !path.is_dir() {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }

    if let Ok(instance) = instance.canonicalize()
        && path.starts_with(&instance)
    {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }
    Ok(path)
}

/// 生成 MML 服务器实例
///
/// 模组与资源包有在线来源就用平台地址、不复制；没有的复制进输出文件夹由发布者托管。
/// 配置目录按各自的同步方式打包或逐个下发。
///
/// - `game`: 源实例
/// - `online`: 实例记录的在线来源，键是文件名
/// - `base`: 游戏目录
/// - `out`: 输出文件夹
/// - `url`: 发布地址，托管文件的下载地址按它拼
/// - `options`: 生成选项
/// - `contents`: 勾选展开后的内容
/// - `progress`: 进度回调
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]
#[allow(clippy::too_many_arguments)]
fn generate_mml(
    game: &InstanceSettingObj,
    online: &HashMap<String, OnlineInfoObj>,
    base: &Path,
    out: &Path,
    url: &str,
    options: &ServerInstanceOptionsDto,
    contents: &Contents,
    progress: Progress<'_>,
) -> Result<ServerInstanceResultDto, String> {
    // 先把目录内容列出来：进度总数与后面的打包都要用
    let mut config_files = Vec::new();
    for (dir, mode) in &contents.configs {
        config_files.push((dir.clone(), *mode, files::dir_files(base, dir)?));
    }

    let total = contents.mods.len()
        + config_files
            .iter()
            .map(|(_, mode, list)| match mode {
                ConfigMode::Archive => 1,
                ConfigMode::Files => list.len(),
            })
            .sum::<usize>()
        + contents.packs.len();

    let mut now = 0;
    let mut items = Vec::new();
    let mut written = BTreeSet::new();

    // 图标也带一份：玩家导入时按清单里的 Icon 去 <发布地址>/icon.png 下载
    let icon = copy_icon(game, out)?;
    if let Some(name) = &icon {
        written.insert(name.clone());
    }
    let mut archived = 0;
    let mut archives = Vec::new();

    for entry in &contents.mods {
        let wire = files::wire_path(&entry.path);
        progress(now, total, wire.clone());
        now += 1;

        // 只有地址能改，编号一律用实例记录的
        let recorded = online.get(&file_name(&entry.path));
        let download = pick(&entry.url, recorded.map(|info| info.url.as_str()));
        let pid = recorded.map(|info| info.modid.as_str()).unwrap_or_default();
        let fid = recorded.map(|info| info.fileid.as_str()).unwrap_or_default();

        // 地址优先，其次是"项目号 + 文件号"能解析出来的；都没有就只能自己托管
        let online_download = !download.is_empty() || (!pid.is_empty() && !fid.is_empty());
        let item = if online_download {
            source_item(&wire, pid, fid, &download, base, &entry.path)?
        } else {
            written.insert(top_of(&wire));
            hosted_item(base, out, &entry.path, url)?
        };
        items.push(item);
    }

    for (dir, mode, list) in &config_files {
        match mode {
            ConfigMode::Archive => {
                let name = generate::archive_name(dir);
                progress(now, total, name.clone());
                now += 1;

                let zip = out.join(&name);
                files::zip_dir(base, dir, list, &zip)?;
                archives.push(ServerArchiveItemObj {
                    file: name.clone(),
                    dir: dir.clone(),
                    // 整个目录都同步，服务端删掉的文件客户端也要清掉
                    delete_old: true,
                    url: format!("{url}/{}", files::encode_url_path(&name)),
                    sha1: Some(files::sha1(&zip)?),
                    sha256: None,
                });
                archived += list.len();
                written.insert(name);
            }
            ConfigMode::Files => {
                for rel in list {
                    let wire = files::wire_path(rel);
                    progress(now, total, wire.clone());
                    now += 1;
                    items.push(hosted_item(base, out, rel, url)?);
                    written.insert(top_of(&wire));
                }
            }
        }
    }

    for rel in &contents.packs {
        let wire = files::wire_path(rel);
        progress(now, total, wire.clone());
        now += 1;

        // 资源包同理：能在线下载的就不复制
        let item = match online.get(&file_name(rel)) {
            Some(info) => source_item(&wire, &info.modid, &info.fileid, &info.url, base, rel)?,
            None => {
                written.insert(top_of(&wire));
                hosted_item(base, out, rel, url)?
            }
        };
        items.push(item);
    }

    let count = items.len() + archived;
    let mut pack = generate::build_server_pack(
        game,
        &options.name,
        &options.version,
        &options.text,
        items,
        archives,
    );
    pack.icon = icon;
    let bytes = serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?;
    // 客户端拿本地 server.json 的 SHA1 与 server.json.sha1 逐字比对，这里不能带换行
    let sha1 = gen_hash(HashType::Sha1, &bytes);
    write(out, names::SERVER_FILE, &bytes)?;
    write(
        out,
        &format!("{}{}", names::SERVER_FILE, names::SHA1_DOT_EXT),
        sha1.as_bytes(),
    )?;

    Ok(ServerInstanceResultDto {
        manifest: names::SERVER_FILE.to_string(),
        files: written.into_iter().collect(),
        count,
    })
}

/// 生成 HMCL 服务器实例
///
/// HMCL 没有压缩包，也没有"按项目号去平台下载"这条路，所有文件都得由服务端托管；
/// 配置目录的同步方式对它不起作用，一律逐文件复制。
///
/// - `game`: 源实例，加载器组件从它取
/// - `base`: 游戏目录
/// - `out`: 输出文件夹
/// - `url`: 发布地址，写进 `file_api`，客户端按 `file_api/路径` 下载
/// - `options`: 生成选项
/// - `contents`: 勾选展开后的内容
/// - `progress`: 进度回调
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]；加载器 HMCL 表达不了时返回
/// `err.serverInstanceLoader`
fn generate_hmcl(
    game: &InstanceSettingObj,
    base: &Path,
    out: &Path,
    url: &str,
    options: &ServerInstanceOptionsDto,
    contents: &Contents,
    progress: Progress<'_>,
) -> Result<ServerInstanceResultDto, String> {
    let addons =
        generate::hmcl_addons(game).ok_or_else(|| err_keys::SERVER_INSTANCE_LOADER.to_string())?;

    // 模组 + 各配置目录里的文件 + 资源包，按路径去重
    let mut all: BTreeSet<PathBuf> = contents.mods.iter().map(|item| item.path.clone()).collect();
    for (dir, _) in &contents.configs {
        all.extend(files::dir_files(base, dir)?);
    }
    all.extend(contents.packs.iter().cloned());

    let mut list = Vec::new();
    let mut top = BTreeSet::new();
    let total = all.len();
    for (now, rel) in all.iter().enumerate() {
        let wire = files::wire_path(rel);
        progress(now, total, wire.clone());

        files::copy_into(base, out, rel)?;
        top.insert(top_of(&wire));
        list.push(HMCLServerFileObj {
            path: wire,
            hash: files::sha1(&base.join(rel))?,
        });
    }

    let pack = generate::build_hmcl_pack(
        &options.name,
        &options.version,
        &options.text,
        url,
        addons,
        list,
    );
    let bytes = serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?;
    write(out, names::SERVER_MANIFEST_FILE, &bytes)?;

    Ok(ServerInstanceResultDto {
        manifest: names::SERVER_MANIFEST_FILE.to_string(),
        files: top.into_iter().collect(),
        count: total,
    })
}

/// 取手填值，没填就用实例记录的
///
/// - `typed`: 手动填的内容，空串表示没填
/// - `recorded`: 实例记录的在线来源，可能没有
///
/// # 返回值
///
/// 去掉首尾空白后的值；两边都没有时是空串
fn pick(typed: &str, recorded: Option<&str>) -> String {
    let typed = typed.trim();
    if !typed.is_empty() {
        return typed.to_string();
    }
    recorded.map(|value| value.trim().to_string()).unwrap_or_default()
}

/// 在线下载的条目：不复制文件，客户端按地址或项目号自己下载
///
/// - `wire`: 相对游戏目录的正斜杠路径
/// - `pid`: 项目号，可为空
/// - `fid`: 文件号，可为空
/// - `download`: 下载地址，可为空
/// - `base`: 游戏目录
/// - `rel`: 相对游戏目录的路径
///
/// # 返回值
///
/// 见 [`ServerItemObj`]；算哈希失败返回错误文案
fn source_item(
    wire: &str,
    pid: &str,
    fid: &str,
    download: &str,
    base: &Path,
    rel: &Path,
) -> Result<ServerItemObj, String> {
    Ok(ServerItemObj {
        file: wire.to_string(),
        pid: Some(pid.trim().to_string()).filter(|id| !id.is_empty()),
        fid: Some(fid.trim().to_string()).filter(|id| !id.is_empty()),
        url: Some(download.trim().to_string()).filter(|url| !url.is_empty()),
        sha1: Some(files::sha1(&base.join(rel))?),
        sha256: None,
    })
}

/// 一个要托管的文件：复制进输出文件夹并生成清单条目
///
/// - `base`: 游戏目录
/// - `out`: 输出文件夹
/// - `rel`: 相对游戏目录的路径
/// - `url`: 发布地址
///
/// # 返回值
///
/// 见 [`ServerItemObj`]；复制或算哈希失败返回错误文案
fn hosted_item(
    base: &Path,
    out: &Path,
    rel: &Path,
    url: &str,
) -> Result<ServerItemObj, String> {
    files::copy_into(base, out, rel)?;
    let wire = files::wire_path(rel);
    Ok(ServerItemObj {
        file: wire.clone(),
        pid: None,
        fid: None,
        url: Some(format!("{url}/{}", files::encode_url_path(&wire))),
        sha1: Some(files::sha1(&base.join(rel))?),
        sha256: None,
    })
}

/// 取相对路径的第一段，用来汇报写出了哪些顶层条目
///
/// - `wire`: 已经转成正斜杠的相对路径
///
/// # 返回值
///
/// 形如 `config`；一层都没有时是空串
fn top_of(wire: &str) -> String {
    wire.split('/').next().unwrap_or_default().to_string()
}

/// 实例记录的在线来源，键是文件名
///
/// 有下载地址、或有项目号与文件号都算：前者直接下发地址，后者由客户端按编号解析。
///
/// - `game`: 源实例
///
/// # 返回值
///
/// 文件名 → 来源信息；一份都没有时是空表
fn online_sources(game: &InstanceSettingObj) -> HashMap<String, OnlineInfoObj> {
    let mut map = HashMap::new();
    for info in game.read_online_info().into_values() {
        let indexed = !info.modid.trim().is_empty() && !info.fileid.trim().is_empty();
        if indexed || !info.url.trim().is_empty() {
            map.insert(info.file.clone(), info);
        }
    }
    map
}

/// 取相对路径的文件名，清单按文件名查在线来源
///
/// - `rel`: 相对游戏目录的路径
///
/// # 返回值
///
/// 文件名；没有文件名时是空串
fn file_name(rel: &Path) -> String {
    rel.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 把实例图标复制进输出文件夹
///
/// 玩家导入时按清单里的 `Icon` 去 `<发布地址>/<文件名>` 下载，所以这里只放一份文件；
/// 实例没有图标文件时返回 `None`。
///
/// - `game`: 源实例
/// - `out`: 输出文件夹
///
/// # 返回值
///
/// 写进清单的文件名；没有图标时是 `None`
fn copy_icon(game: &InstanceSettingObj, out: &Path) -> Result<Option<String>, String> {
    let source = game.get_icon_file_or_default();
    if !source.is_file() {
        return Ok(None);
    }
    let name = names::ICON_FILE.to_string();
    fs::copy(&source, out.join(&name)).map_err(|e| e.to_string())?;
    Ok(Some(name))
}

/// 写一个文件到输出文件夹
///
/// - `out`: 输出文件夹
/// - `name`: 文件名
/// - `bytes`: 文件内容
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`，失败返回错误文案
fn write(out: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    fs::write(out.join(name), bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtos::server_instance_dto::{ServerInstanceConfigDto, ServerInstancePickDto};
    use mml_game::{
        launcher::file_online_info_obj::OnlineInfoObj, loader::LoaderType,
        other_launcher::hmcl_obj::HMCLServerObj, serverpack::serverpack_obj::ServerPackObj,
    };

    /// 勾选一组模组路径
    fn picks(paths: &[&str]) -> Vec<ServerInstancePickDto> {
        paths
            .iter()
            .map(|path| ServerInstancePickDto {
                path: path.to_string(),
                exclude: false,
                url: String::new(),
            })
            .collect()
    }

    /// 测试用的生成选项
    fn options(
        format: &str,
        mods: &[&str],
        configs: &[(&str, &str)],
        packs: &[&str],
    ) -> ServerInstanceOptionsDto {
        ServerInstanceOptionsDto {
            format: format.into(),
            folder: String::new(),
            url: "https://example.invalid/pack".into(),
            name: "服务器".into(),
            version: "1.0.0".into(),
            text: "说明".into(),
            mods: picks(mods),
            configs: configs
                .iter()
                .map(|(dir, mode)| ServerInstanceConfigDto {
                    dir: dir.to_string(),
                    mode: mode.to_string(),
                })
                .collect(),
            resource_packs: packs.iter().map(|path| path.to_string()).collect(),
        }
    }

    /// 按选项展开勾选，等价于 generate 里的那一步
    fn contents(base: &Path, options: &ServerInstanceOptionsDto) -> Contents {
        Contents {
            mods: files::mods(base, &options.mods).unwrap(),
            configs: options
                .configs
                .iter()
                .map(|item| (item.dir.clone(), ConfigMode::parse(&item.mode).unwrap()))
                .collect(),
            packs: options
                .resource_packs
                .iter()
                .map(|path| files::file(base, path).unwrap())
                .collect(),
        }
    }

    /// 一个原版 Forge 实例
    fn game() -> InstanceSettingObj {
        InstanceSettingObj {
            name: "服务器".into(),
            version: "1.21.1".into(),
            loader: LoaderType::Forge,
            loader_version: Some("52.0.1".into()),
            ..Default::default()
        }
    }

    /// 游戏的 SHA1，用来核对清单里写出来的值
    fn sha1(bytes: &[u8]) -> String {
        mml_base::hash_helper::gen_hash(HashType::Sha1, bytes)
    }

    /// 一个模组的在线来源
    fn online(file: &str, pid: &str, fid: &str) -> OnlineInfoObj {
        OnlineInfoObj {
            file: file.into(),
            modid: pid.into(),
            fileid: fid.into(),
            url: format!("https://example.invalid/{file}"),
            ..Default::default()
        }
    }

    #[test]
    fn normalize_url_requires_http_and_drops_trailing_slash() {
        assert_eq!(
            normalize_url(" https://example.invalid/pack/ ").unwrap(),
            "https://example.invalid/pack"
        );
        assert_eq!(
            normalize_url("http://127.0.0.1:8080").unwrap(),
            "http://127.0.0.1:8080"
        );
        for bad in ["", "  ", "example.invalid", "ftp://example.invalid", "https://"] {
            assert!(normalize_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn prepare_folder_rejects_instance_directory() {
        let instance = crate::test_support::temp_dir("server-instance-prepare");
        let inside = instance.join("out");
        assert_eq!(
            prepare_folder(&instance, inside.to_str().unwrap()).unwrap_err(),
            err_keys::SERVER_INSTANCE_FOLDER
        );

        let outside = crate::test_support::temp_dir("server-instance-prepare-out");
        assert_eq!(
            prepare_folder(&instance, outside.to_str().unwrap()).unwrap(),
            outside.canonicalize().unwrap()
        );
        assert!(prepare_folder(&instance, "  ").is_err());

        crate::test_support::cleanup(&[&instance, &outside]);
    }

    /// 模组可以整个目录勾，也能单独取消其中一个
    #[test]
    fn mods_follow_selection_and_exclusions() {
        let base = crate::test_support::temp_dir("server-instance-mods");
        fs::create_dir_all(base.join("mods/sub")).unwrap();
        fs::write(base.join("mods/a.jar"), b"a").unwrap();
        fs::write(base.join("mods/sub/b.jar"), b"b").unwrap();

        let paths = |list: Vec<files::ModPick>| -> Vec<PathBuf> {
            list.into_iter().map(|item| item.path).collect()
        };

        let list = files::mods(&base, &picks(&["mods"])).unwrap();
        assert_eq!(
            paths(list),
            vec![PathBuf::from("mods/a.jar"), PathBuf::from("mods/sub/b.jar")]
        );

        let mut exclude = picks(&["mods"]);
        exclude.push(ServerInstancePickDto {
            path: "mods/sub/b.jar".into(),
            exclude: true,
            url: String::new(),
        });
        assert_eq!(paths(files::mods(&base, &exclude).unwrap()), vec![PathBuf::from("mods/a.jar")]);

        assert!(files::mods(&base, &[]).unwrap().is_empty());

        crate::test_support::cleanup(&[&base]);
    }

    /// 目录同步方式：压缩包与逐文件各写各的
    #[test]
    fn mml_pack_writes_manifest_sidecar_and_archive() {
        // 打压缩包会打日志，日志系统每进程只 boot 一次
        crate::test_support::ensure_boot();
        let base = crate::test_support::temp_dir("server-instance-mml");
        let out = crate::test_support::temp_dir("server-instance-mml-out");
        fs::create_dir_all(base.join("mods")).unwrap();
        fs::create_dir_all(base.join("config")).unwrap();
        fs::write(base.join("mods/online.jar"), b"online").unwrap();
        fs::write(base.join("mods/local.jar"), b"local").unwrap();
        fs::write(base.join("config/a.toml"), b"a = 1").unwrap();

        let online = HashMap::from([("online.jar".to_string(), online("online.jar", "123", "456"))]);
        let options = options("mml", &["mods"], &[("config", "archive")], &[]);
        let contents = contents(&base, &options);
        let result = generate_mml(
            &game(),
            &online,
            &base,
            &out,
            "https://example.invalid/pack",
            &options,
            &contents,
            &|_, _, _| {},
        )
        .unwrap();

        let manifest = fs::read(out.join("server.json")).unwrap();
        let pack: ServerPackObj = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(pack.online_list.len(), 2);
        let online_jar = pack
            .online_list
            .iter()
            .find(|item| item.file == "mods/online.jar")
            .unwrap();
        assert_eq!(online_jar.pid.as_deref(), Some("123"));
        assert_eq!(online_jar.sha1.as_deref(), Some(sha1(b"online").as_str()));
        // 有在线来源的不复制进文件夹
        assert!(!out.join("mods/online.jar").exists());

        // 没有在线来源的由我们托管：复制进文件夹并给托管地址
        let local = pack
            .online_list
            .iter()
            .find(|item| item.file == "mods/local.jar")
            .unwrap();
        assert!(local.pid.is_none() && local.fid.is_none());
        assert_eq!(
            local.url.as_deref(),
            Some("https://example.invalid/pack/mods/local.jar")
        );
        assert_eq!(fs::read(out.join("mods/local.jar")).unwrap(), b"local");
        assert_eq!(pack.archive_list.len(), 1);
        assert_eq!(pack.archive_list[0].file, "config.zip");
        assert_eq!(pack.archive_list[0].dir, "config");
        assert!(pack.archive_list[0].delete_old);
        assert_eq!(
            pack.archive_list[0].url,
            "https://example.invalid/pack/config.zip"
        );
        assert!(out.join("config.zip").is_file());

        // 客户端逐字比对 SHA1，末尾多一个换行就永远对不上
        let sidecar = fs::read_to_string(out.join("server.json.sha1")).unwrap();
        assert_eq!(sidecar, sha1(&manifest));
        assert!(!sidecar.ends_with('\n'));

        assert_eq!(result.manifest, "server.json");
        assert_eq!(result.files, ["config.zip", "mods"]);
        // 三个文件都同步：online.jar、local.jar 与 config/a.toml
        assert_eq!(result.count, 3);

        crate::test_support::cleanup(&[&base, &out]);
    }

    /// 两个目录各用各的方式：config 打压缩包，kubejs 逐文件
    #[test]
    fn mml_mixes_archive_and_per_file_dirs() {
        crate::test_support::ensure_boot();
        let base = crate::test_support::temp_dir("server-instance-mixed");
        let out = crate::test_support::temp_dir("server-instance-mixed-out");
        fs::create_dir_all(base.join("config")).unwrap();
        fs::create_dir_all(base.join("kubejs")).unwrap();
        fs::write(base.join("config/a.toml"), b"a").unwrap();
        fs::write(base.join("kubejs/startup.js"), b"js").unwrap();

        let options = options(
            "mml",
            &[],
            &[("config", "archive"), ("kubejs", "files")],
            &[],
        );
        let contents = contents(&base, &options);
        let result = generate_mml(
            &game(),
            &HashMap::new(),
            &base,
            &out,
            "https://example.invalid/pack",
            &options,
            &contents,
            &|_, _, _| {},
        )
        .unwrap();

        let pack: ServerPackObj =
            serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
        assert_eq!(pack.archive_list.len(), 1);
        assert_eq!(pack.archive_list[0].file, "config.zip");
        // 逐文件目录进清单，文件也复制进输出文件夹
        assert_eq!(pack.online_list.len(), 1);
        assert_eq!(pack.online_list[0].file, "kubejs/startup.js");
        assert_eq!(
            pack.online_list[0].url.as_deref(),
            Some("https://example.invalid/pack/kubejs/startup.js")
        );
        assert_eq!(fs::read(out.join("kubejs/startup.js")).unwrap(), b"js");
        assert_eq!(result.files, ["config.zip", "kubejs"]);
        assert_eq!(result.count, 2);

        crate::test_support::cleanup(&[&base, &out]);
    }

    /// 手填的下载地址优先于实例记录；既没地址又没编号的只能本地托管
    #[test]
    fn mods_take_manual_url_over_recorded() {
        crate::test_support::ensure_boot();
        let base = crate::test_support::temp_dir("server-instance-manual");
        let out = crate::test_support::temp_dir("server-instance-manual-out");
        fs::create_dir_all(base.join("mods")).unwrap();
        fs::write(base.join("mods/a.jar"), b"a").unwrap();
        fs::write(base.join("mods/b.jar"), b"b").unwrap();

        let mut manual = picks(&["mods/a.jar", "mods/b.jar"]);
        manual[0].url = " https://typed.example/a.jar ".into();

        let online = HashMap::from([("a.jar".to_string(), online("a.jar", "1", "2"))]);
        let options = options("mml", &[], &[], &[]);
        let contents = Contents {
            mods: files::mods(&base, &manual).unwrap(),
            configs: Vec::new(),
            packs: Vec::new(),
        };
        generate_mml(
            &game(),
            &online,
            &base,
            &out,
            "https://example.invalid/pack",
            &options,
            &contents,
            &|_, _, _| {},
        )
        .unwrap();

        let pack: ServerPackObj =
            serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
        assert_eq!(pack.online_list.len(), 2);

        // a：手填地址盖掉实例记录，编号仍是记录里的，文件不复制
        let a = pack.online_list.iter().find(|item| item.file == "mods/a.jar").unwrap();
        assert_eq!(a.url.as_deref(), Some("https://typed.example/a.jar"));
        assert_eq!(a.pid.as_deref(), Some("1"));
        assert_eq!(a.fid.as_deref(), Some("2"));
        assert!(!out.join("mods/a.jar").exists());

        // b：没有地址也没有编号，复制进文件夹并给托管地址
        let b = pack.online_list.iter().find(|item| item.file == "mods/b.jar").unwrap();
        assert_eq!(
            b.url.as_deref(),
            Some("https://example.invalid/pack/mods/b.jar")
        );
        assert_eq!(fs::read(out.join("mods/b.jar")).unwrap(), b"b");

        crate::test_support::cleanup(&[&base, &out]);
    }

    /// 资源包有在线来源时走平台下载，不复制进文件夹
    #[test]
    fn mml_resource_pack_with_source_is_not_copied() {
        crate::test_support::ensure_boot();
        let base = crate::test_support::temp_dir("server-instance-pack-online");
        let out = crate::test_support::temp_dir("server-instance-pack-online-out");
        fs::create_dir_all(base.join("resourcepacks")).unwrap();
        fs::write(base.join("resourcepacks/p.zip"), b"p").unwrap();

        let online = HashMap::from([(
            "p.zip".to_string(),
            OnlineInfoObj {
                file: "p.zip".into(),
                modid: "111".into(),
                fileid: "222".into(),
                url: "https://cdn.example.invalid/p.zip".into(),
                ..Default::default()
            },
        )]);
        let options = options("mml", &[], &[], &["resourcepacks/p.zip"]);
        let contents = contents(&base, &options);
        let result = generate_mml(
            &game(),
            &online,
            &base,
            &out,
            "https://example.invalid/pack",
            &options,
            &contents,
            &|_, _, _| {},
        )
        .unwrap();

        let pack: ServerPackObj =
            serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
        assert_eq!(pack.online_list.len(), 1);
        assert_eq!(
            pack.online_list[0].url.as_deref(),
            Some("https://cdn.example.invalid/p.zip")
        );
        assert_eq!(pack.online_list[0].pid.as_deref(), Some("111"));
        // 用了在线地址就不复制
        assert!(!out.join("resourcepacks/p.zip").exists());
        assert!(result.files.is_empty());
        assert_eq!(result.count, 1);

        crate::test_support::cleanup(&[&base, &out]);
    }

    /// 资源包没有在线来源时逐个下发，路径要转义
    #[test]
    fn mml_resource_packs_are_hosted_per_file() {
        crate::test_support::ensure_boot();
        let base = crate::test_support::temp_dir("server-instance-packs");
        let out = crate::test_support::temp_dir("server-instance-packs-out");
        fs::create_dir_all(base.join("resourcepacks")).unwrap();
        fs::write(base.join("resourcepacks/中文 1.zip"), b"pack").unwrap();

        let options = options("mml", &[], &[], &["resourcepacks/中文 1.zip"]);
        let contents = contents(&base, &options);
        let result = generate_mml(
            &game(),
            &HashMap::new(),
            &base,
            &out,
            "https://example.invalid/pack",
            &options,
            &contents,
            &|_, _, _| {},
        )
        .unwrap();

        let pack: ServerPackObj =
            serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
        assert_eq!(pack.online_list.len(), 1);
        assert_eq!(
            pack.online_list[0].url.as_deref(),
            Some("https://example.invalid/pack/resourcepacks/%E4%B8%AD%E6%96%87%201.zip")
        );
        assert_eq!(pack.online_list[0].sha1.as_deref(), Some(sha1(b"pack").as_str()));
        assert!(pack.archive_list.is_empty());
        assert_eq!(fs::read(out.join("resourcepacks/中文 1.zip")).unwrap(), b"pack");
        assert_eq!(result.files, ["resourcepacks"]);
        assert_eq!(result.count, 1);

        crate::test_support::cleanup(&[&base, &out]);
    }

    #[test]
    fn hmcl_pack_copies_files_and_lists_hashes() {
        let base = crate::test_support::temp_dir("server-instance-hmcl");
        let out = crate::test_support::temp_dir("server-instance-hmcl-out");
        fs::create_dir_all(base.join("mods")).unwrap();
        fs::create_dir_all(base.join("config")).unwrap();
        fs::create_dir_all(base.join("resourcepacks")).unwrap();
        fs::write(base.join("mods/a.jar"), b"mod").unwrap();
        fs::write(base.join("config/x.toml"), b"x = 1").unwrap();
        fs::write(base.join("resourcepacks/p.zip"), b"p").unwrap();

        let options = options(
            "hmcl",
            &["mods"],
            &[("config", "archive")],
            &["resourcepacks/p.zip"],
        );
        let contents = contents(&base, &options);
        let result = generate_hmcl(
            &game(),
            &base,
            &out,
            "https://example.invalid/pack",
            &options,
            &contents,
            &|_, _, _| {},
        )
        .unwrap();

        assert_eq!(fs::read(out.join("mods/a.jar")).unwrap(), b"mod");
        assert_eq!(fs::read(out.join("config/x.toml")).unwrap(), b"x = 1");
        assert_eq!(fs::read(out.join("resourcepacks/p.zip")).unwrap(), b"p");

        let manifest = fs::read(out.join("server-manifest.json")).unwrap();
        let pack: HMCLServerObj = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(pack.name, "服务器");
        assert_eq!(pack.file_api, "https://example.invalid/pack");
        assert_eq!(pack.addons.len(), 2);
        assert_eq!(pack.files.len(), 3);
        let paths: Vec<&str> = pack.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "config/x.toml",
                "mods/a.jar",
                "resourcepacks/p.zip"
            ]
        );
        assert_eq!(pack.files[1].hash, sha1(b"mod"));

        assert_eq!(result.manifest, "server-manifest.json");
        assert_eq!(result.files, ["config", "mods", "resourcepacks"]);
        assert_eq!(result.count, 3);

        crate::test_support::cleanup(&[&base, &out]);
    }

    #[test]
    fn hmcl_pack_rejects_loader_it_cannot_express() {
        let base = crate::test_support::temp_dir("server-instance-hmcl-loader");
        let out = crate::test_support::temp_dir("server-instance-hmcl-loader-out");
        fs::create_dir_all(base.join("mods")).unwrap();
        fs::write(base.join("mods/a.jar"), b"mod").unwrap();

        let mut item = game();
        item.loader = LoaderType::OptiFine;
        let options = options("hmcl", &["mods"], &[], &[]);
        let contents = contents(&base, &options);
        let err = generate_hmcl(
            &item,
            &base,
            &out,
            "https://example.invalid/pack",
            &options,
            &contents,
            &|_, _, _| {},
        )
        .unwrap_err();
        assert_eq!(err, err_keys::SERVER_INSTANCE_LOADER);

        crate::test_support::cleanup(&[&base, &out]);
    }

    /// 实例自带图标时复制进输出文件夹，没有图标时返回 None
    #[test]
    fn copy_icon_follows_instance_icon() {
        crate::test_support::ensure_boot();
        // 实例路径要 mml-game 的运行目录：本 crate 的 boot 只起 mml_base / 日志
        static GAME: std::sync::Once = std::sync::Once::new();
        GAME.call_once(|| {
            mml_game::init(mml_base::get_base_dir()).unwrap();
        });
        let dir = crate::test_support::temp_dir("icon-src");
        let out = crate::test_support::temp_dir("icon-out");

        let mut game = InstanceSettingObj {
            // 绝对路径：脱离测试运行目录，直接指向临时目录本身
            dir: dir.to_string_lossy().to_string(),
            icon: Some(names::ICON_FILE.to_string()),
            ..Default::default()
        };
        assert_eq!(copy_icon(&game, &out).unwrap(), None, "没有图标文件时不带");

        let bytes = b"fake png bytes";
        std::fs::write(dir.join(names::ICON_FILE), bytes).unwrap();
        assert_eq!(
            copy_icon(&game, &out).unwrap(),
            Some(names::ICON_FILE.to_string())
        );
        assert_eq!(std::fs::read(out.join(names::ICON_FILE)).unwrap(), bytes);

        // 图标名被清掉时按默认 icon.png 找
        game.icon = None;
        assert!(copy_icon(&game, &out).unwrap().is_some());

        crate::test_support::cleanup(&[&dir, &out]);
    }
}
