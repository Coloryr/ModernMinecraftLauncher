//! 检查游戏实例文件

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

use mml_base::{
    file_item::{FileHash, FileItemObj},
    hash_helper::{self, HashType},
};
use mml_names::{i18_items::error_type::CoreResult, names};
use mml_sys::path_helper;
use tokio::task;

use crate::{
    game_arg::GameLaunchObj, launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::assets_path,
};

/// 检查文件是否需要添加到下载列表
///
/// - `item`: 需要检查的文件
/// - `check_sha1`: 是否校验SHA1
///
/// # 返回值
///
/// 返回true表示文件缺失或校验失败，需要重新下载
fn check_to_add(item: &FileItemObj, check_sha1: bool) -> bool {
    if check_sha1 {
        !item.check_hash()
    } else {
        !item.file.exists() || !item.file.is_file()
    }
}

/// 检查单个文件的SHA1是否匹配
///
/// - `file`: 文件路径
/// - `sha1`: 期望的SHA1值
///
/// # 返回值
///
/// 返回文件存在且SHA1匹配
fn check_file_sha1<P: AsRef<Path>>(file: P, sha1: &str) -> bool {
    if !file.as_ref().exists() || !file.as_ref().is_file() {
        return false;
    }
    if let Ok(mut stream) = path_helper::open_read(file)
        && let Ok(hash) = hash_helper::gen_hash_from_reader(HashType::Sha1, &mut stream)
    {
        return hash.eq_ignore_ascii_case(sha1);
    }
    false
}

impl InstanceSettingObj {
    /// 检查游戏文件
    ///
    /// - `obj`: 启动配置
    ///
    /// # 返回值
    ///
    /// 返回缺失的文件列表；构建下载项失败返回对应错误
    pub async fn get_lost_game_file(&self, obj: &GameLaunchObj) -> CoreResult<Vec<FileItemObj>> {
        let (
            check_core,
            check_core_sha1,
            check_assets,
            check_assets_sha1,
            check_lib,
            check_lib_sha1,
            check_mod,
            check_mod_sha1,
        ) = {
            let config = mml_config::read_config();
            (
                config.check.core,
                config.check.core_sha1,
                config.check.assets,
                config.check.assets_sha1,
                config.check.lib,
                config.check.lib_sha1,
                config.check.game_mod,
                config.check.mod_sha1,
            )
        };

        let mut handles: Vec<task::JoinHandle<Vec<FileItemObj>>> = Vec::new();

        //检查游戏核心文件
        if check_core {
            let game_jar = obj.game_jar.clone();
            let log4j = obj.log4j_xml.clone();
            handles.push(task::spawn_blocking(move || {
                let mut list = Vec::new();
                if check_to_add(&game_jar, check_core_sha1) {
                    list.push(game_jar);
                }
                if let Some(log4j) = log4j {
                    if check_to_add(&log4j, true) {
                        list.push(log4j);
                    }
                }
                list
            }));
        }

        //检查游戏资源文件
        if check_assets {
            let assets = obj.assets.clone();
            handles.push(task::spawn_blocking(move || {
                let Ok(assets) = assets_path::get_index(&assets) else {
                    return Vec::new();
                };

                // 一个版本几千个 object，逐个算 SHA1 是这趟检查最贵的一段 —— 并行扫它
                // （ColorMC 同一处是 Parallel.ForEach，见 CheckHelpers.cs:251）
                let entries = assets.objects.iter().collect::<Vec<_>>();
                let found: Vec<FileItemObj> = entries
                    .into_par_iter()
                    .filter_map(|(name, asset)| {
                        let dir: String = asset.hash.chars().take(2).collect();
                        // 必须 join 两段：with_file_name 会**替换最后一段**，把 objects/80 变成 objects/，
                        // 结果全平铺下到 objects/、游戏按 objects/<前两位>/<hash> 找不到（与 lib.rs / mojang 那两处口径一致）
                        let local = assets_path::get_obj_dir().join(&dir).join(&asset.hash);

                        let need_add = if check_assets_sha1 {
                            !check_file_sha1(&local, &asset.hash)
                        } else {
                            !local.exists() || !local.is_file()
                        };
                        if !need_add {
                            return None;
                        }

                        Some(FileItemObj {
                            name: name.clone(),
                            file: local,
                            url: mml_net::url_helper::get_download_assets(&asset.hash),
                            hash: FileHash::Sha1(asset.hash.clone()),
                            later: Default::default(),
                        })
                    })
                    .collect();

                // 同一个 object 被多个名字引用时只留一份：目标文件相同，重复任务会让两个线程写同一路径
                let mut seen = HashSet::new();
                found
                    .into_iter()
                    .filter(|item| seen.insert(item.file.clone()))
                    .collect()
            }));
        }

        //检查运行库
        if check_lib {
            let game_libs = obj.game_libs.clone();
            let loader_libs = obj.loader_libs.clone();
            let installer_libs = obj.installer_libs.clone();
            handles.push(task::spawn_blocking(move || {
                // 三份运行库清单各自并行筛（ColorMC 同样用 Parallel.ForEach，见 CheckHelpers.cs:368/375/382）
                let pick = |items: &[FileItemObj]| -> Vec<FileItemObj> {
                    items
                        .par_iter()
                        .filter(|item| check_to_add(item, check_lib_sha1))
                        .cloned()
                        .collect()
                };

                let mut list = pick(&game_libs);
                list.extend(pick(&loader_libs));
                list.extend(pick(&installer_libs));
                list
            }));
        }

        //检查整合包mod
        if self.is_modpack && check_mod {
            let mods_path = self.get_mods_path();
            let game_path = self.get_game_path();
            let online_info = self.read_online_info();
            handles.push(task::spawn_blocking(move || {
                let mut mod_files = path_helper::get_all_files(&mods_path);
                let mut sha1_cache: HashMap<PathBuf, String> = HashMap::new();
                let mut list = Vec::new();

                for (_, info) in &online_info {
                    if info.path != names::GAME_MODS_DIR {
                        continue;
                    }

                    let info_file_lower = info.file.to_lowercase();
                    let info_stem = Path::new(&info_file_lower)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string());

                    let mut found = false;
                    if let Some(ref info_stem) = info_stem {
                        mod_files.retain(|mod_file| {
                            if found {
                                return true;
                            }
                            let mod_stem = mod_file
                                .file_stem()
                                .map(|s| s.to_string_lossy().to_lowercase());
                            if let Some(ref mod_stem) = mod_stem {
                                if mod_stem == info_stem {
                                    if check_mod_sha1 {
                                        if let Some(cached_sha1) = sha1_cache.get(mod_file) {
                                            if cached_sha1 == &info.sha1 {
                                                found = true;
                                                return false;
                                            }
                                        } else if let Ok(mut stream) =
                                            path_helper::open_read(mod_file)
                                            && let Ok(sha1) = hash_helper::gen_hash_from_reader(
                                                HashType::Sha1,
                                                &mut stream,
                                            )
                                        {
                                            sha1_cache.insert(mod_file.clone(), sha1.clone());
                                            if sha1.eq_ignore_ascii_case(&info.sha1) {
                                                found = true;
                                                return false;
                                            }
                                        }
                                    } else {
                                        found = true;
                                        return false;
                                    }
                                }
                            }
                            true
                        });
                    }

                    if !found {
                        let path = game_path.join(&info.path).join(&info.file);
                        list.push(FileItemObj {
                            name: info.name.clone(),
                            file: path,
                            url: info.url.clone(),
                            hash: FileHash::Sha1(info.sha1.clone()),
                            later: Default::default(),
                        });
                    }
                }
                list
            }));
        }

        // 等待所有任务完成
        let mut list = Vec::new();
        for handle in handles {
            if let Ok(section_list) = handle.await {
                list.extend(section_list);
            }
        }

        Ok(list)
    }

    // /// 服务器包检查
    // pub async fn server_pack_check(&self) -> CoreResult<()> {

    // }
}
