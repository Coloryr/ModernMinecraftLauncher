//! 服务器实例清单生成
//!
//! 从已展开的生成选项出发，决定每个模组 / 资源包 / 配置目录是走在线来源、复制托管，
//! 还是打成压缩包，最后写出 `server.json` 与 `server.json.sha1`。
//! 参数校验与配置目录解析在 `config.rs`，命令与进度事件在 `mod.rs`。

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use mml_base::hash_helper::{HashType, gen_hash};
use mml_game::{
    launcher::{
        ModPackType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    serverpack::{
        generate,
        serverpack_obj::{
            ServerArchiveAction, ServerArchiveItemObj, ServerBaseObj, ServerItemObj,
        },
    },
};
use mml_names::names;

use super::{
    config::{
        ConfigDir, ConfigMode, Contents, CustomAction, CustomPick, PackPick, config_dir,
        config_snapshot,
        config_deletions, normalize_url, prepare_folder,
    },
    files, mapping,
};
use crate::windows::export::list::custom_ids;
use crate::{
    dtos::server_instance_dto::{
        ServerInstanceChangesDto, ServerInstanceOptionsDto, ServerInstanceResultDto,
    },
    err_keys,
};

/// 进度回调：已处理项数、总项数、当前处理的文件
pub(super) type Progress<'a> = &'a dyn Fn(usize, usize, String);

/// 生成入口：校验参数 → 展开勾选 → 写 MML 清单
///
/// - `game`: 源实例
/// - `options`: 生成选项
/// - `progress`: 进度回调
///
/// # 返回值
///
/// 见 [`ServerInstanceResultDto`]
pub(super) fn generate(
    game: &InstanceSettingObj,
    options: &ServerInstanceOptionsDto,
    progress: Progress<'_>,
) -> Result<ServerInstanceResultDto, String> {
    // 服务器实例本身就是同步结果，再导出会循环覆盖，直接拒绝
    if game.modpack_type == ModPackType::ServerPack {
        return Err(err_keys::SERVER_INSTANCE_FROM_SERVER.to_string());
    }
    let url = normalize_url(&options.url)?;
    let base = game.get_game_path();
    let out = prepare_folder(&game.get_base_path(), &options.folder)?;

    let mods = files::mods(&base, &options.mods)?;
    let mut configs = Vec::new();
    for item in &options.configs {
        let dir = config_dir(&item.dir)?;
        let mode = ConfigMode::parse(&item.mode)?;
        // 没单独给就用方式自带的默认值；删除目录必须删旧目录，不给关
        let delete_old = matches!(mode, ConfigMode::Delete)
            || item.delete_old.unwrap_or_else(|| mode.deletes_old());
        configs.push(ConfigDir {
            dir,
            mode,
            exclude: item.exclude.clone(),
            delete: item.delete.clone(),
            delete_old,
        });
    }
    let packs = options
        .resource_packs
        .iter()
        .map(|pick| {
            Ok(PackPick {
                path: files::file(&base, &pick.path)?,
                url: pick.url.trim().to_string(),
                pid: pick.pid.trim().to_string(),
                fid: pick.fid.trim().to_string(),
                custom: pick.custom,
                edited: pick.edited,
                type_changed: pick.type_changed,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let custom = options
        .custom
        .iter()
        .map(|item| {
            let action = match item.action.trim() {
                "add" => CustomAction::Add,
                "delete" => CustomAction::Delete,
                _ => return Err(err_keys::SERVER_INSTANCE_DIR.to_string()),
            };
            // 添加有两种来源：实例里已有的文件，或者界面里直接填的内容
            let (path, content) = match action {
                CustomAction::Add => match &item.content {
                    Some(text) => (
                        files::custom_target(&base, &item.path)?,
                        Some(text.clone()),
                    ),
                    None => (files::file(&base, &item.path)?, None),
                },
                CustomAction::Delete => (files::custom_target(&base, &item.path)?, None),
            };
            Ok(CustomPick {
                path,
                action,
                content,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    if mods.is_empty()
        && configs.is_empty()
        && packs.is_empty()
        && custom.is_empty()
        && options.base_files.is_empty()
    {
        return Err(err_keys::SERVER_INSTANCE_EMPTY.to_string());
    }

    let contents = Contents {
        mods,
        configs,
        packs,
        custom,
    };
    let online = online_sources(game);
    generate_mml(
        game, &online, &base, &out, &url, options, &contents, progress,
    )
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
pub(super) fn generate_mml(
    game: &InstanceSettingObj,
    online: &HashMap<String, OnlineInfoObj>,
    base: &Path,
    out: &Path,
    url: &str,
    options: &ServerInstanceOptionsDto,
    contents: &Contents,
    progress: Progress<'_>,
) -> Result<ServerInstanceResultDto, String> {
    // 父目录排在子目录前面：清单里的 Archives 按这个顺序，客户端先解父包再解子包
    let mut selected: Vec<&ConfigDir> = contents.configs.iter().collect();
    selected.sort_by(|a, b| {
        let depth = a.dir.matches('/').count();
        let other = b.dir.matches('/').count();
        depth.cmp(&other).then_with(|| a.dir.cmp(&b.dir))
    });

    // 单文件删除不进包，另发 delete 条目让客户端按路径删掉
    let deletions = config_deletions(base, &contents.configs)?;

    // 先把目录内容列出来：进度总数与后面的打包都要用
    let mut config_files: Vec<(&ConfigDir, Vec<PathBuf>)> = Vec::new();
    for config in selected {
        let mut list = match config.mode {
            ConfigMode::ArchiveAll | ConfigMode::FilesAll => files::dir_files(base, &config.dir)?,
            ConfigMode::ArchiveSelf | ConfigMode::FilesSelf => {
                files::dir_direct_files(base, &config.dir)?
            }
            // 删除目录与被排除的子目录不需要读内容
            ConfigMode::Delete | ConfigMode::None => Vec::new(),
        };
        // 父目录的包要排掉所有单独配置过的下级目录，那些目录自己的条目负责。
        // 用全部下级而不是直接子目录：中间层保持"跟随上级"时不会出现在清单里，
        // 只排直接子目录会漏掉更深的排除项。
        let prefix = format!("{}/", config.dir);
        let children: Vec<&str> = contents
            .configs
            .iter()
            .map(|item| item.dir.as_str())
            .filter(|dir| dir.starts_with(&prefix))
            .collect();
        if !children.is_empty() && config.mode.recursive() {
            list.retain(|rel| !children.iter().any(|child| rel.starts_with(*child)));
        }
        files::retain_excluded(base, &mut list, &config.exclude)?;
        // 标记删除的文件同样不进包，删掉它们交给下面的删除条目
        files::retain_excluded(base, &mut list, &config.delete)?;
        // 删除目录与空目录整包同步要留一条删除项；其余情况没有文件就不生成条目。
        // 整包同步把文件全取消勾选、或所有下级都单独配过时不写条目，避免误删客户端目录。
        let keep_empty = matches!(config.mode, ConfigMode::Delete)
            || (matches!(config.mode, ConfigMode::ArchiveAll)
                && config.exclude.is_empty()
                && config.delete.is_empty()
                && children.is_empty());
        if list.is_empty() && !keep_empty {
            continue;
        }
        config_files.push((config, list));
    }

    // 配置目录的文件快照，给下次打开做“改动对比”；失败要在写任何产出之前
    let config_snapshot = config_snapshot(base, &contents.configs)?;

    let total = contents.mods.len()
        + config_files
            .iter()
            .map(|(config, list)| if config.mode.is_archive() { 1 } else { list.len() })
            .sum::<usize>()
        + contents.packs.len()
        + deletions.len()
        + contents.custom.len();

    let mut now = 0;
    let mut items = Vec::new();
    let mut written = BTreeSet::new();
    let mut ids = mapping::IdMap::load(&game.get_base_path());
    let mut changed = BTreeSet::new();

    // 基础文件：实例根目录下、与 .minecraft 同级的文件，打成一个固定包。
    // 客户端只在第一次同步时解压，后续版本不再拉取。
    let base_files = files::base_files(&game.get_base_path(), &options.base_files)?;
    let base_count = base_files.len();
    let base_item = if base_files.is_empty() {
        None
    } else {
        let name = "base.zip".to_string();
        let file = out.join(&name);
        files::zip_files(&game.get_base_path(), &base_files, &file)?;
        written.insert(name.clone());
        Some(ServerBaseObj {
            file: name.clone(),
            url: format!("{url}/{}", files::encode_url_path(&name)),
            sha1: Some(files::sha1(&file)?),
            sha256: None,
        })
    };

    // 图标也带一份：玩家导入时按清单里的 Icon 去 <发布地址>/icon.png 下载
    let icon = copy_icon(game, out)?;
    if let Some(name) = &icon {
        written.insert(name.clone());
    }
    let mut archived = 0;
    let mut archives = Vec::new();
    let mut mod_snapshot: BTreeMap<String, mapping::EntrySnapshot> = BTreeMap::new();

    for entry in &contents.mods {
        let wire = files::wire_path(&entry.path);
        let key = mod_key(base, &entry.path, &wire);
        progress(now, total, wire.clone());
        now += 1;

        // 只有地址能改，编号一律用实例记录的
        let recorded = online.get(&file_name(&entry.path));
        let download = pick(&entry.url, recorded.map(|info| info.url.as_str()));
        let pid = if entry.edited {
            entry.pid.as_str()
        } else {
            recorded.map(|info| info.modid.as_str()).unwrap_or_default()
        };
        let fid = if entry.edited {
            entry.fid.as_str()
        } else {
            recorded.map(|info| info.fileid.as_str()).unwrap_or_default()
        };

        // 地址优先，其次是"项目号 + 文件号"能解析出来的；都没有就只能自己托管
        let has_ids = recorded.is_some_and(|info| {
            !info.modid.trim().is_empty() || !info.fileid.trim().is_empty()
        });
        let custom = if entry.type_changed || entry.edited {
            entry.custom
        } else {
            !has_ids
        };
        // 自定义编号不能拿去平台解析；没有地址时仍然复制托管
        let online_download =
            !download.is_empty() || (!custom && !pid.is_empty() && !fid.is_empty());
        let mut item = if online_download {
            source_item(&wire, pid, fid, &download, base, &entry.path, custom)?
        } else {
            written.insert(top_of(&wire));
            hosted_item(
                base,
                out,
                &entry.path,
                url,
                custom,
                &entry.pid,
                &entry.fid,
            )?
        };
        // 可选模组由清单下发；客户端首次同步时装成禁用
        item.optional = entry.optional;
        // 说明由发布端手填，跟着文件条目走
        item.text = entry.text.trim().to_string();
        let sha1 = item.sha1.clone().unwrap_or_default();
        mod_snapshot.insert(
            key.clone(),
            mapping::EntrySnapshot {
                path: wire.clone(),
                sha1: sha1.clone(),
                optional: entry.optional,
            },
        );
        if item.custom {
            let assigned = if entry.edited {
                ids.save_edited(&key, &wire, &sha1, &entry.pid, &entry.fid)?
            } else {
                ids.assign(&key, &wire, &sha1)
            };
            item.pid = Some(assigned.project_id);
            item.fid = Some(assigned.file_id);
            if assigned.changed {
                changed.insert(wire.clone());
            }
        }
        items.push(item);
    }
    let mod_changes = compare_mods(&ids, &mod_snapshot);
    ids.set_mods(mod_snapshot);

    for (config, list) in &config_files {
        let dir = &config.dir;
        if config.mode.is_archive() {
            let name = generate::archive_name(dir);
            progress(now, total, name.clone());
            now += 1;

            // 没有内容可下发：写 DeletePath，客户端按 Path 删掉整个目录，不再打包空 zip
            if list.is_empty() {
                let assigned = ids.assign(&format!("config-archive:{dir}"), dir, "");
                if assigned.changed {
                    changed.insert(dir.clone());
                }
                archives.push(ServerArchiveItemObj {
                    action: ServerArchiveAction::DeletePath,
                    file: String::new(),
                    dir: dir.clone(),
                    // 删除条目不看 Over：目标一定整个删掉
                    delete_old: false,
                    url: None,
                    sha1: None,
                    sha256: None,
                });
                continue;
            }

            let zip = out.join(&name);
            files::zip_dir(base, dir, list, &zip)?;
            let sha1 = files::sha1(&zip)?;
            let assigned = ids.assign(&format!("config-archive:{dir}"), dir, &sha1);
            if assigned.changed {
                changed.insert(dir.clone());
            }
            archives.push(ServerArchiveItemObj {
                action: ServerArchiveAction::Extract,
                file: name.clone(),
                dir: dir.clone(),
                // 是否先删旧目录由界面上的勾选决定
                delete_old: config.delete_old,
                url: Some(format!("{url}/{}", files::encode_url_path(&name))),
                sha1: Some(sha1),
                sha256: None,
            });
            archived += list.len();
            written.insert(name);
            continue;
        }

        for rel in list {
            let wire = files::wire_path(rel);
            progress(now, total, wire.clone());
            now += 1;
            let mut item = hosted_item(base, out, rel, url, true, "", "")?;
            let assigned = ids.assign(
                &format!("config:{wire}"),
                &wire,
                item.sha1.as_deref().unwrap_or_default(),
            );
            item.pid = Some(assigned.project_id);
            item.fid = Some(assigned.file_id);
            if assigned.changed {
                changed.insert(wire.clone());
                changed.insert(dir.clone());
            }
            items.push(item);
            written.insert(top_of(&wire));
        }
    }

    // 单文件删除：写一条 DeleteFile 条目，客户端按 Path 删掉这个文件，不需要真的打包
    for rel in &deletions {
        let wire = files::wire_path(rel);
        progress(now, total, wire.clone());
        now += 1;
        archives.push(ServerArchiveItemObj {
            action: ServerArchiveAction::DeleteFile,
            file: String::new(),
            dir: wire.clone(),
            delete_old: false,
            url: None,
            sha1: None,
            sha256: None,
        });
        changed.insert(wire);
    }

    let mut pack_snapshot: BTreeMap<String, mapping::EntrySnapshot> = BTreeMap::new();
    for pack in &contents.packs {
        let rel = &pack.path;
        let wire = files::wire_path(rel);
        let key = format!("pack:{wire}");
        progress(now, total, wire.clone());
        now += 1;

        // 资源包同理：手填地址 / 编号优先，其次用实例记录
        let recorded = online.get(&file_name(rel));
        let download = pick(&pack.url, recorded.map(|info| info.url.as_str()));
        let pid = if pack.edited {
            pack.pid.as_str()
        } else {
            recorded.map(|info| info.modid.as_str()).unwrap_or_default()
        };
        let fid = if pack.edited {
            pack.fid.as_str()
        } else {
            recorded.map(|info| info.fileid.as_str()).unwrap_or_default()
        };
        let has_ids = recorded.is_some_and(|info| {
            !info.modid.trim().is_empty() || !info.fileid.trim().is_empty()
        });
        let custom = if pack.type_changed || pack.edited {
            pack.custom
        } else {
            !has_ids
        };
        // 自定义编号不能拿去平台解析；没有地址时仍然复制托管
        let online_download =
            !download.is_empty() || (!custom && !pid.is_empty() && !fid.is_empty());
        let mut item = if online_download {
            source_item(&wire, pid, fid, &download, base, rel, custom)?
        } else {
            written.insert(top_of(&wire));
            hosted_item(base, out, rel, url, custom, &pack.pid, &pack.fid)?
        };
        let sha1 = item.sha1.clone().unwrap_or_default();
        pack_snapshot.insert(
            key.clone(),
            mapping::EntrySnapshot {
                path: wire.clone(),
                sha1: sha1.clone(),
                optional: false,
            },
        );
        if item.custom {
            let assigned = if pack.edited {
                ids.save_edited(&key, &wire, &sha1, &pack.pid, &pack.fid)?
            } else {
                ids.assign(&key, &wire, &sha1)
            };
            item.pid = Some(assigned.project_id);
            item.fid = Some(assigned.file_id);
            if assigned.changed {
                changed.insert(wire.clone());
            }
        }
        items.push(item);
    }
    let pack_changes = compare_packs(&ids, &pack_snapshot);
    ids.set_packs(pack_snapshot);

    // 自定义条目：添加的当普通文件托管，删除的写 DeleteFile 条目
    for pick in &contents.custom {
        let wire = files::wire_path(&pick.path);
        progress(now, total, wire.clone());
        now += 1;
        match pick.action {
            CustomAction::Add => {
                if items.iter().any(|item| item.file == wire) {
                    continue;
                }
                let mut item = match &pick.content {
                    Some(text) => {
                        files::write_into(out, &pick.path, text)?;
                        content_item(out, &pick.path, url)?
                    }
                    None => hosted_item(base, out, &pick.path, url, true, "", "")?,
                };
                let assigned = ids.assign(
                    &format!("custom:add:{wire}"),
                    &wire,
                    item.sha1.as_deref().unwrap_or_default(),
                );
                item.pid = Some(assigned.project_id);
                item.fid = Some(assigned.file_id);
                if assigned.changed {
                    changed.insert(wire.clone());
                }
                items.push(item);
                written.insert(top_of(&wire));
            }
            CustomAction::Delete => {
                if deletions.iter().any(|rel| files::wire_path(rel) == wire)
                    || archives.iter().any(|item| item.dir == wire)
                {
                    continue;
                }
                archives.push(ServerArchiveItemObj {
                    action: ServerArchiveAction::DeleteFile,
                    file: String::new(),
                    dir: wire.clone(),
                    delete_old: false,
                    url: None,
                    sha1: None,
                    sha256: None,
                });
                changed.insert(wire);
            }
        }
    }

    let count = items.len() + archived + base_count;
    let mut pack = generate::build_server_pack(
        game,
        &options.name,
        &options.version,
        &options.text,
        items,
        archives,
    );
    pack.icon = icon;
    pack.base = base_item;
    // 清单是给客户端读的，压缩成紧凑 JSON，别为可读性多占几倍体积
    let bytes = serde_json::to_vec(&pack).map_err(|e| e.to_string())?;
    // 客户端拿本地 server.json 的 SHA1 与 server.json.sha1 逐字比对，这里不能带换行
    let sha1 = gen_hash(HashType::Sha1, &bytes);
    write(out, names::SERVER_FILE, &bytes)?;
    write(
        out,
        &format!("{}{}", names::SERVER_FILE, names::SHA1_DOT_EXT),
        sha1.as_bytes(),
    )?;
    ids.set_meta(mapping::Meta {
        name: options.name.clone(),
        version: options.version.clone(),
        text: options.text.clone(),
        url: url.to_string(),
        folder: options.folder.clone(),
        base_files: options.base_files.clone(),
        configs: Some(
            options
                .configs
                .iter()
                .map(|item| mapping::MetaConfig {
                    dir: item.dir.clone(),
                    mode: item.mode.clone(),
                    exclude: item.exclude.clone(),
                    delete: item.delete.clone(),
                    delete_old: item.delete_old,
                })
                .collect(),
        ),
        mod_texts: contents
            .mods
            .iter()
            .filter(|item| !item.text.trim().is_empty())
            .map(|item| (files::wire_path(&item.path), item.text.trim().to_string()))
            .collect(),
        config_files: config_snapshot,
        custom: options
            .custom
            .iter()
            .map(|item| mapping::MetaCustom {
                path: item.path.trim().to_string(),
                action: item.action.trim().to_string(),
                content: item.content.clone(),
            })
            .collect(),
    });
    ids.save(&game.get_base_path())?;

    Ok(ServerInstanceResultDto {
        manifest: names::SERVER_FILE.to_string(),
        files: written.into_iter().collect(),
        count,
        changed: changed.into_iter().collect(),
        mod_changes,
        pack_changes,
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
    recorded
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

/// 取模组的稳定逻辑键：优先真实 modid，读不到时退回路径
///
/// - `base`: 游戏目录
/// - `rel`: 模组相对游戏目录的路径
/// - `wire`: 正斜杠相对路径
///
/// # 返回值
///
/// 形如 `mod:jei`；读不到元数据时是 `mod:mods/foo.jar`
fn mod_key(base: &Path, rel: &Path, wire: &str) -> String {
    let modid = mml_game::game_mods::read_mod_info(base.join(rel))
        .ok()
        .and_then(|item| {
            item.info
                .iter()
                .map(|info| info.mod_id.trim().to_string())
                .find(|modid| !modid.is_empty())
        });
    modid
        .map(|modid| format!("mod:{modid}"))
        .unwrap_or_else(|| format!("mod:{wire}"))
}

/// 对比上次快照，算新增 / 删除 / 修改的模组
///
/// - `ids`: 当前映射
/// - `current`: 本次实际写进清单的模组快照
///
/// # 返回值
///
/// 按路径列出的三类变化；第一次生成没有旧快照时全部为空
fn compare_mods(
    ids: &mapping::IdMap,
    current: &BTreeMap<String, mapping::EntrySnapshot>,
) -> ServerInstanceChangesDto {
    compare_snapshot(ids.loaded(), ids.mods(), current)
}

/// 对比上次资源包快照，算新增 / 删除 / 修改
///
/// - `ids`: 当前映射
/// - `current`: 本次实际写进清单的资源包快照
///
/// # 返回值
///
/// 按路径列出的三类变化；第一次生成没有旧快照时全部为空
fn compare_packs(
    ids: &mapping::IdMap,
    current: &BTreeMap<String, mapping::EntrySnapshot>,
) -> ServerInstanceChangesDto {
    compare_snapshot(ids.loaded(), ids.packs(), current)
}

/// 对比旧快照与本次快照
///
/// - `loaded`: 映射文件是否来自上次生成
/// - `old`: 旧快照
/// - `current`: 本次快照
///
/// # 返回值
///
/// 按路径列出的新增 / 删除 / 修改
fn compare_snapshot(
    loaded: bool,
    old: &BTreeMap<String, mapping::EntrySnapshot>,
    current: &BTreeMap<String, mapping::EntrySnapshot>,
) -> ServerInstanceChangesDto {
    let mut changes = ServerInstanceChangesDto::default();
    if !loaded {
        return changes;
    }
    for (key, item) in current {
        match old.get(key) {
            None => changes.added.push(item.path.clone()),
            Some(previous) if previous.sha1 != item.sha1 || previous.path != item.path => {
                changes.modified.push(item.path.clone());
            }
            Some(_) => {}
        }
    }
    for (key, item) in old {
        if !current.contains_key(key) {
            changes.removed.push(item.path.clone());
        }
    }
    changes
}

/// 解析条目的项目 / 文件编号
///
/// 手填优先；都没填且按自定义编号处理时，按相对路径现生成一对。
///
/// - `wire`: 相对游戏目录的正斜杠路径
/// - `pid`: 手填项目号
/// - `fid`: 手填文件号
/// - `custom`: 是否按自定义编号处理
///
/// # 返回值
///
/// `(项目号, 文件号, 是否自定义)`；空串一律转成 `None`
fn resolve_ids(wire: &str, pid: &str, fid: &str, custom: bool) -> (Option<String>, Option<String>, bool) {
    let pid = pid.trim();
    let fid = fid.trim();
    if !pid.is_empty() || !fid.is_empty() {
        return (
            Some(pid.to_string()).filter(|id| !id.is_empty()),
            Some(fid.to_string()).filter(|id| !id.is_empty()),
            custom,
        );
    }
    if custom {
        let (pid, fid) = custom_ids(wire);
        (Some(pid), Some(fid), true)
    } else {
        (None, None, false)
    }
}

/// 清单条目的公共默认值：说明留空、无 SHA256、非可选
fn manifest_item(
    file: String,
    pid: Option<String>,
    fid: Option<String>,
    url: Option<String>,
    sha1: String,
    custom: bool,
) -> ServerItemObj {
    ServerItemObj {
        file,
        text: String::new(),
        pid,
        fid,
        url,
        sha1: Some(sha1),
        sha256: None,
        custom,
        optional: false,
    }
}

/// 在线下载的条目：不复制文件，客户端按地址或项目号自己下载
///
/// - `wire`: 相对游戏目录的正斜杠路径
/// - `pid`: 项目号，可为空
/// - `fid`: 文件号，可为空
/// - `download`: 下载地址，可为空
/// - `base`: 游戏目录
/// - `rel`: 相对游戏目录的路径
/// - `custom`: 是否按自定义编号处理；没有编号时自动生成
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
    custom: bool,
) -> Result<ServerItemObj, String> {
    let (pid, fid, custom) = resolve_ids(wire, pid, fid, custom);
    Ok(manifest_item(
        wire.to_string(),
        pid,
        fid,
        Some(download.trim().to_string()).filter(|url| !url.is_empty()),
        files::sha1(&base.join(rel))?,
        custom,
    ))
}

/// 一个要托管的文件：复制进输出文件夹并生成清单条目
///
/// - `base`: 游戏目录
/// - `out`: 输出文件夹
/// - `rel`: 相对游戏目录的路径
/// - `url`: 发布地址
/// - `custom`: 是否生成自定义编号
///
/// # 返回值
///
/// 见 [`ServerItemObj`]；复制或算哈希失败返回错误文案
fn hosted_item(
    base: &Path,
    out: &Path,
    rel: &Path,
    url: &str,
    custom: bool,
    pid: &str,
    fid: &str,
) -> Result<ServerItemObj, String> {
    files::copy_into(base, out, rel)?;
    let wire = files::wire_path(rel);
    let (pid, fid, custom) = resolve_ids(&wire, pid, fid, custom);
    Ok(manifest_item(
        wire.clone(),
        pid,
        fid,
        Some(format!("{url}/{}", files::encode_url_path(&wire))),
        files::sha1(&base.join(rel))?,
        custom,
    ))
}

/// 内容由发布者填写、直接写进输出文件夹的条目
///
/// - `out`: 输出文件夹
/// - `rel`: 相对游戏目录的路径，文件已经写好
/// - `url`: 发布地址
///
/// # 返回值
///
/// 见 [`ServerItemObj`]；算哈希失败返回错误文案
fn content_item(out: &Path, rel: &Path, url: &str) -> Result<ServerItemObj, String> {
    let wire = files::wire_path(rel);
    let encoded = files::encode_url_path(&wire);
    let (pid, fid) = custom_ids(&wire);
    Ok(manifest_item(
        wire,
        Some(pid),
        Some(fid),
        Some(format!("{url}/{encoded}")),
        files::sha1(&out.join(rel))?,
        true,
    ))
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
pub(super) fn copy_icon(game: &InstanceSettingObj, out: &Path) -> Result<Option<String>, String> {
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
