//! 游戏实例运行库相关

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Mutex,
};

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_names::names;
use mml_net::{maven_utils, url_helper};
use mml_sys::{ArchEnum, Os};
use tokio::task;

use crate::{
    GameInstance,
    launcher_path::libraries_path,
    loader::forge,
    mojang::{
        self,
        game_arg_obj::{GameArgObj, GameLibrariesObj},
    },
};

impl GameArgObj {
    /// 创建游戏运行库项目
    ///
    /// - `native`: 原生库解压目录
    /// - `game`: 游戏实例，目录回退查找用，可为 `None`
    ///
    /// # 返回值
    ///
    /// 返回需要下载的文件列表
    pub async fn build_game_libraries(
        &self,
        native: &Path,
        game: Option<GameInstance>,
    ) -> Vec<FileItemObj> {
        // 在 spawn_blocking 闭包内克隆所需数据，以满足 'static 生命周期要求
        let libraries = self.libraries.clone();
        let native = native.to_path_buf();

        // 阶段 1：在阻塞线程中处理所有运行库
        let (mut list, mut natives, natives_arm) = task::spawn_blocking(move || {
            let list = Mutex::new(Vec::<FileItemObj>::new());
            let keys = Mutex::new(HashSet::<String>::new());
            let natives = Mutex::new(HashMap::<String, bool>::new());
            let natives_arm = Mutex::new(Vec::<String>::new());

            if let Some(libs) = libraries.as_ref() {
                #[cfg(debug_assertions)]
                {
                    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

                    libs.par_iter().for_each(|item| {
                        process_one_library(
                            item,
                            &list,
                            &keys,
                            &natives,
                            &natives_arm,
                            &native,
                            &game,
                        );
                    });
                }
                #[cfg(not(debug_assertions))]
                {
                    for item in libs.iter() {
                        process_one_library(
                            item,
                            &list,
                            &keys,
                            &natives,
                            &natives_arm,
                            &native,
                            &game,
                        );
                    }
                }
            }

            (
                list.into_inner().unwrap(),
                natives.into_inner().unwrap(),
                natives_arm.into_inner().unwrap(),
            )
        })
        .await
        .unwrap();

        // 阶段 2：ARM 本地库回退，网络请求只能在异步上下文里做
        let sys = mml_sys::get_system_info();
        if sys.is_arm {
            for item in natives_arm.iter() {
                natives.remove(item);
            }

            for item in natives.keys() {
                let path: Vec<&str> = item.split(':').collect();
                let path1: Vec<&str> = path[0].split('.').collect();

                let mut base_dir = String::new();
                for item1 in path1 {
                    base_dir.push_str(item1);
                    base_dir.push('/');
                }

                let system = match sys.os {
                    Os::Linux => "linux",
                    Os::MacOS => "macos",
                    _ => "windows",
                };

                let name = format!("{}:{}-{}-natives-{}-arm64", item, path[1], path[2], system);
                let dir = format!(
                    "{}{}/{}/{}-{}-natives-{}-arm64{}",
                    base_dir,
                    path[1],
                    path[2],
                    path[1],
                    path[2],
                    system,
                    names::JAR_DOT_EXT
                );

                let hash = maven_utils::test_hash(&dir).await;

                if let Some(data) = hash {
                    list.push(FileItemObj {
                        name,
                        file: libraries_path::get_lib_dir().join(dir),
                        url: data.url,
                        hash: data.hash,
                        later: Default::default(),
                    });
                }

                let name = format!(
                    "{}:{}-{}-natives-{}-aarch_64",
                    item, path[1], path[2], system
                );
                let dir = format!(
                    "{}{}/{}/{}-{}-natives-{}-aarch_64{}",
                    base_dir,
                    path[1],
                    path[2],
                    path[1],
                    path[2],
                    system,
                    names::JAR_DOT_EXT
                );

                let hash = maven_utils::test_hash(&dir).await;

                if let Some(data) = hash {
                    list.push(FileItemObj {
                        name,
                        file: libraries_path::get_lib_dir().join(dir),
                        url: data.url,
                        hash: data.hash,
                        later: Default::default(),
                    });
                }
            }
        }

        list
    }
}

/// MMC 本地库的文件名：`组:构件:版本[:分类]` → `构件-版本[-分类].jar`
///
/// 与 `maven_utils::version_name_to_path` 的区别是**不带目录**：MMC 的 `MMC-hint: local`
/// 把文件平铺在实例目录的 `libraries/` 下，Labymod 的 `LabyMod-4.jar` 就是这样。
///
/// - `name`: Maven 坐标
///
/// # 返回值
///
/// 文件名；坐标段数不足时原样返回
fn local_lib_name(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return name.to_string();
    }

    let mut file = format!("{}-{}", parts[1], parts[2]);
    if let Some(classifier) = parts.get(3) {
        file.push('-');
        file.push_str(classifier);
    }
    file.push_str(".jar");
    file
}

/// 处理单个运行库条目，把需要的下载项推进列表
///
/// - `item`: 运行库信息
/// - `list`: 下载项列表，并发写入
/// - `keys`: 已处理运行库的 SHA1 集合，用于去重
/// - `natives`: 原生库键集合
/// - `natives_arm`: ARM 原生库键列表
/// - `native`: 原生库解压路径
/// - `game`: 游戏实例，目录回退查找用，可为 `None`
fn process_one_library(
    item: &GameLibrariesObj,
    list: &Mutex<Vec<FileItemObj>>,
    keys: &Mutex<HashSet<String>>,
    natives: &Mutex<HashMap<String, bool>>,
    natives_arm: &Mutex<Vec<String>>,
    native: &Path,
    game: &Option<GameInstance>,
) {
    // 检查规则是否允许
    if !mojang::check_allow(&item.rules) {
        return;
    }

    let mut isadd = false;
    let sys = mml_sys::get_system_info();

    // MMC 的本地库（`MMC-hint: local`）：文件在**实例目录**的 libraries 下、按
    // `构件-版本[-分类].jar` 平铺，不下载、只进 classpath。Labymod 就是这样挂 LabyMod-4.jar 的；
    // 少了这一步它会按嵌套 maven 路径去找，找不到、也进不了 classpath。
    // 文件不在就是真的缺（MMC 那边也是本地文件），这里不下也不报，留给启动时的 Java 去报
    if item.mmc_hint.as_deref() == Some("local") {
        if let Some(game) = game {
            let dir = game.read().unwrap().get_libraries_path();
            let file = dir.join(local_lib_name(&item.name));
            if file.exists() && file.is_file() {
                list.lock().unwrap().push(FileItemObj {
                    name: item.name.clone(),
                    file,
                    url: Default::default(),
                    hash: Default::default(),
                    later: Default::default(),
                });
            }
        }
        return;
    }

    // 旧版 - 直接使用 url 字段
    if !item.url.is_empty() {
        isadd = true;
        let file = maven_utils::version_name_to_path(&item.name);
        let mut url = format!("{}{}", item.url, file);
        url_helper::change_source(&mut url);
        list.lock().unwrap().push(FileItemObj {
            name: item.name.clone(),
            file: libraries_path::get_lib_dir().join(&file),
            url,
            hash: Default::default(),
            later: Default::default(),
        });
    }

    // 全系统 artifact 处理
    // 只要求 artifact 存在（path / url / sha1 任一非空）：MMC / Forge 的 profile 常只写 url + sha1，
    // 路径靠 Maven 坐标推。ColorMC 判的也是"Downloads.Artifact != null"；
    // 我们之前要求 path 非空，会把 org.lwjgl3.json 那类整批库丢掉（实测 72 个声明产出 0 个）
    if !(item.downloads.artifact.path.is_empty()
        && item.downloads.artifact.url.is_empty()
        && item.downloads.artifact.sha1.is_empty())
    {
        // SHA1 去重：只在 sha1 有效时做，且没有 native 分类器才跳过。
        // MMC 的 patch 里大量库不带 sha1（清一色空串），按"空 == 空"去重会把整个
        // org.lwjgl3 这类库全丢掉 —— 表现就是启动时缺 org.lwjgl.system.Platform
        if !item.downloads.artifact.sha1.is_empty() {
            let keys = keys.lock().unwrap();
            if keys.contains(&item.downloads.artifact.sha1)
                && item.downloads.classifiers.is_none()
            {
                return;
            }
        }

        isadd = true;

        // Natives 追踪
        if item.name.contains("natives") {
            if let Some(index) = item.name.rfind(':') {
                let key = &item.name[..index];
                if item.name.ends_with("arm64") || item.name.ends_with("aarch_64") {
                    natives_arm.lock().unwrap().push(key.to_string());
                    natives.lock().unwrap().remove(key);
                } else {
                    natives.lock().unwrap().insert(key.to_string(), true);
                }
            }
        }

        // 确定文件路径
        let file = if item.downloads.artifact.path.is_empty() {
            maven_utils::version_name_to_path(&item.name)
        } else {
            item.downloads.artifact.path.clone()
        };

        let url = url_helper::replace_minecraft_libraries(&item.downloads.artifact.url);

        list.lock().unwrap().push(FileItemObj {
            name: item.name.clone(),
            file: libraries_path::get_lib_dir().join(&file),
            url,
            // 没有 sha1 的项用 None：Sha1("") 会让校验永远不过、每次启动都重下
            hash: if item.downloads.artifact.sha1.is_empty() {
                Default::default()
            } else {
                FileHash::Sha1(item.downloads.artifact.sha1.clone())
            },
            later: Default::default(),
        });

        if !item.downloads.artifact.sha1.is_empty() {
            keys.lock()
                .unwrap()
                .insert(item.downloads.artifact.sha1.clone());
        }
    }

    // 分系统 classifiers (native) 处理
    if let Some(classifiers) = &item.downloads.classifiers {
        // 根据操作系统选择对应的 native 库
        let mut lib = match sys.os {
            Os::Windows => &classifiers.natives_windows,
            Os::Linux => &classifiers.natives_linux,
            Os::MacOS => &classifiers.natives_osx,
            _ => return, // 不支持的操作系统，跳过该库的 native 处理
        };

        // Windows 下如果主 native 为空，尝试 32/64 位特定版本
        if lib.path.is_empty() && sys.os == Os::Windows {
            if sys.system_arch == ArchEnum::X86 {
                lib = &classifiers.natives_windows_32;
            } else {
                lib = &classifiers.natives_windows_64;
            }
        }

        if !lib.path.is_empty() {
            // SHA1 去重
            {
                let keys = keys.lock().unwrap();
                if keys.contains(&lib.sha1) {
                    return;
                }
            }

            isadd = true;

            natives.lock().unwrap().insert(item.name.clone(), true);

            let file = if lib.path.is_empty() {
                maven_utils::version_name_to_path(&format!("{}-native{}", item.name, sys.os))
            } else {
                lib.path.clone()
            };

            list.lock().unwrap().push(FileItemObj {
                name: format!("{}-native{}", item.name, sys.os),
                file: libraries_path::get_lib_dir().join(&file),
                url: url_helper::replace_minecraft_libraries(&lib.url),
                hash: FileHash::Sha1(lib.sha1.clone()),
                later: LaterRun::UnpackNative(native.to_path_buf()),
            });

            keys.lock().unwrap().insert(lib.sha1.clone());
        }
    }

    // 游戏目录回退 - 库未找到时尝试从游戏实例目录寻找
    if !isadd
        && !item.name.is_empty()
        && let Some(game) = game
    {
        let lib = forge::make_forge_libraries(&item.name);
        if let Some(lib) = lib {
            list.lock().unwrap().push(FileItemObj {
                name: item.name.clone(),
                file: libraries_path::get_lib_dir().join(lib.downloads.artifact.path),
                url: url_helper::replace_forge_libraries(&lib.downloads.artifact.url),
                hash: Default::default(),
                later: Default::default(),
            });
        } else {
            let dir = game.read().unwrap().get_libraries_path();
            if dir.exists() && dir.is_dir() {
                let file = dir.join(maven_utils::version_name_to_path(&item.name));
                if file.exists() && file.is_file() {
                    list.lock().unwrap().push(FileItemObj {
                        name: item.name.clone(),
                        file,
                        url: Default::default(),
                        hash: Default::default(),
                        later: Default::default(),
                    });
                }
            }
        }
    }
}
