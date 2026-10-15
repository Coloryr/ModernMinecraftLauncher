//! Forge / NeoForge 加载器安装与运行库下载项构建

use std::{
    collections::HashMap,
    io::{Read, Seek},
};

use mml_base::{
    file_item::{FileHash, FileItemObj},
    hash_helper::{self, HashType},
    serialize_tools, version_parse,
};
use mml_names::{
    i18_items::error_type::{
        CoreResult, DataNotFoundData, ErrorData, ErrorType, FileSystemErrorData,
    },
    names,
};
use mml_net::{
    maven_utils::{self},
    url_helper, urls,
};
use mml_sys::path_helper;
use zip::ZipArchive;

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{libraries_path, version_path},
    loader::{
        LoaderType,
        forge_install_obj::{ForgeInstallObj, ForgeInstallOldObj},
        forge_launch_obj::{ForgeDownloadsObj, ForgeLaunchObj, ForgeLibrariesObj},
    },
    mojang::game_arg_obj::ArtifactObj,
};

/// 根据名字构建运行库信息
///
/// - `name`: 运行库名字，Maven 坐标
///
/// # 返回值
///
/// 不在已知的旧版 Forge 内置运行库中时返回 `None`
pub fn make_forge_libraries(name: &str) -> Option<ForgeLibrariesObj> {
    let args: Vec<&str> = name.split(':').collect();
    if args.len() < 2 {
        None
    } else if args[0] == "net.minecraftforge" && args[1] == "forge" {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: Default::default(),
                    sha1: Default::default(),
                    url: Default::default(),
                },
            },
        })
    } else if args[0] == "net.minecraft" && args[1] == "launchwrapper" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!(
                        "net/minecraft/launchwrapper/{}/launchwrapper-{}.jar",
                        args[2], args[2]
                    ),
                    sha1: Default::default(),
                    url: format!(
                        "https://libraries.minecraft.net/net/minecraft/launchwrapper/{}/launchwrapper-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "org.ow2.asm" && args[1] == "asm-all" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!("org/ow2/asm/asm-all/{}/asm-all-{}.jar", args[2], args[2]),
                    sha1: Default::default(),
                    url: format!(
                        "https://maven.minecraftforge.net/org/ow2/asm/asm-all/{}/asm-all-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "lzma" && args[1] == "lzma" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!("lzma/lzma/{}/lzma-{}.jar", args[2], args[2]),
                    sha1: Default::default(),
                    url: format!(
                        "https://libraries.minecraft.net/lzma/lzma/{}/lzma-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "net.sf.jopt-simple" && args[1] == "jopt-simple" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!(
                        "net/sf/jopt-simple/jopt-simple/{}/jopt-simple-{}.jar",
                        args[2], args[2]
                    ),
                    sha1: Default::default(),
                    url: format!(
                        "https://libraries.minecraft.net/net/sf/jopt-simple/jopt-simple/{}/jopt-simple-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "com.google.guava" && args[1] == "guava" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!("com/google/guava/guava/{}/guava-{}.jar", args[2], args[2]),
                    sha1: Default::default(),
                    url: format!(
                        "https://libraries.minecraft.net/com/google/guava/guava/{}/guava-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "org.apache.commons" && args[1] == "commons-lang3" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!(
                        "org/apache/commons/commons-lang3/{}/commons-lang3-{}.jar",
                        args[2], args[2]
                    ),
                    sha1: Default::default(),
                    url: format!(
                        "https://maven.minecraftforge.net/org/apache/commons/commons-lang3/{}/commons-lang3-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "net.java.jinput" && args[1] == "jinput" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!("net/java/jinput/jinput/{}/jinput-{}.jar", args[2], args[2]),
                    sha1: Default::default(),
                    url: format!(
                        "https://maven.minecraftforge.net/net/java/jinput/jinput/{}/jinput-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "net.java.jutils" && args[1] == "jutils" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!("net/java/jutils/jutils/{}/jutils-{}.jar", args[2], args[2]),
                    sha1: Default::default(),
                    url: format!(
                        "https://maven.minecraftforge.net/net/java/jutils/jutils/{}/jutils-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "java3d" && args[1] == "vecmath" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!("java3d/vecmath/{}/vecmath-{}.jar", args[2], args[2]),
                    sha1: Default::default(),
                    url: format!(
                        "https://libraries.minecraft.net/java3d/vecmath/{}/vecmath-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "net.sf.trove4j" && args[1] == "trove4j" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!("net/sf/trove4j/trove4j/{}/trove4j-{}.jar", args[2], args[2]),
                    sha1: Default::default(),
                    url: format!(
                        "https://maven.minecraftforge.net/net/sf/trove4j/trove4j/{}/trove4j-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else if args[0] == "io.netty" && args[1] == "netty-all" && args.len() >= 3 {
        Some(ForgeLibrariesObj {
            name: String::from(name),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: format!("io/netty/netty-all/{}/netty-all-{}.jar", args[2], args[2]),
                    sha1: Default::default(),
                    url: format!(
                        "https://maven.minecraftforge.net/io/netty/netty-all/{}/netty-all-{}.jar",
                        args[2], args[2]
                    ),
                },
            },
        })
    } else {
        None
    }
}

/// 创建 Forge 下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: Forge 版本号
/// - `jar_type`: jar 类型，取 `installer`、`universal`、`client`、`launcher` 之一
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
async fn build_forge_item(mc: &str, version: &str, jar_type: &str, hash: bool) -> FileItemObj {
    let mut version = String::from(version);
    version.push_str(&url_helper::forge_url_fix(&mc));

    let name = format!("forge-{mc}-{version}-{jar_type}.jar");
    let url = url_helper::get_forge_jar(mc, &version) + &name;

    let hash = if hash {
        maven_utils::try_get_hash(&url).await
    } else {
        FileHash::None
    };
    FileItemObj {
        name: format!("net.minecraftforge:{mc}-{version}-{jar_type}"),
        file: libraries_path::get_lib_dir()
            .join("net")
            .join("minecraftforge")
            .join("forge")
            .join(format!("{mc}-{version}"))
            .join(name),
        url,
        hash,
        later: Default::default(),
    }
}

/// 创建 NeoForge 下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: NeoForge 版本号
/// - `jar`: jar 类型，取 `installer`、`universal`、`client` 之一
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
async fn build_neoforge_item(mc: &str, version: &str, jar: &str, hash: bool) -> FileItemObj {
    let v2222 = version_parse::is_game_version_1202(mc);
    let name = if v2222 {
        format!("neoforge-{version}-{jar}")
    } else {
        format!("forge-{mc}-{version}-{jar}")
    };

    let base_url = url_helper::get_neoforge_jar(v2222, mc, version);
    let base_path = libraries_path::get_lib_dir().join("net").join("neoforged");

    let url = format!("{base_url}{name}.jar");
    let hash = if hash {
        maven_utils::try_get_hash(&url).await
    } else {
        FileHash::None
    };
    FileItemObj {
        name: format!("net.neoforged:{name}"),
        file: if v2222 {
            base_path
                .join("neoforge")
                .join(version)
                .join(format!("{name}.jar"))
        } else {
            base_path
                .join("forge")
                .join(format!("{mc}-{version}"))
                .join(format!("{name}.jar"))
        },
        url,
        hash,
        later: Default::default(),
    }
}

/// 创建 Forge 安装器下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: Forge 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_forge_installer(mc: &str, version: &str, hash: bool) -> FileItemObj {
    build_forge_item(mc, version, names::FILE_INSTALLER, hash).await
}

/// 创建 Forge universal 包下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: Forge 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_forge_universal(mc: &str, version: &str, hash: bool) -> FileItemObj {
    build_forge_item(mc, version, names::FILE_UNIVERSAL, hash).await
}

/// 创建 Forge client 包下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: Forge 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_forge_client(mc: &str, version: &str, hash: bool) -> FileItemObj {
    build_forge_item(mc, version, names::FILE_CLIENT, hash).await
}

/// 创建 Forge launcher 包下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: Forge 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_forge_launcher(mc: &str, version: &str, hash: bool) -> FileItemObj {
    let mut item = build_forge_item(mc, version, names::FILE_LAUNCHER, hash).await;
    item.url = format!(
        "{}net/minecraftforge/forge/{mc}-{version}/forge-{mc}-{version}-{}.jar",
        urls::FORGE,
        names::FILE_LAUNCHER
    );
    item
}

/// 创建 NeoForge 安装器下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: NeoForge 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_neoforge_installer(mc: &str, version: &str, hash: bool) -> FileItemObj {
    build_neoforge_item(mc, version, names::FILE_INSTALLER, hash).await
}

/// 创建 NeoForge universal 包下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: NeoForge 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_neoforge_universal(mc: &str, version: &str, hash: bool) -> FileItemObj {
    build_neoforge_item(mc, version, names::FILE_UNIVERSAL, hash).await
}

/// 创建 NeoForge client 包下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: NeoForge 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_neoforge_client(mc: &str, version: &str, hash: bool) -> FileItemObj {
    build_neoforge_item(mc, version, names::FILE_CLIENT, hash).await
}

/// 构建 Forge 运行库下载项目列表
///
/// - `info`: 运行库列表
/// - `mc`: 游戏版本号
/// - `version`: Forge 版本号
/// - `neo`: 是否为 NeoForge
/// - `v2`: 是否为 1.13 以上
/// - `install`: 是否为安装器
///
/// # 返回值
///
/// 返回去重后的运行库下载项列表；缺失的 universal / installer / launcher 包自动补齐
pub async fn build_forge_libs(
    info: &Vec<ForgeLibrariesObj>,
    mc: &str,
    version: &str,
    neo: bool,
    v2: bool,
    install: bool,
) -> Vec<FileItemObj> {
    let mut list = HashMap::<String, FileItemObj>::new();

    let mut universal = false;
    let mut installer = false;
    let mut launcher = false;

    for item in info {
        let artifact = &item.downloads.artifact;

        // 路径为空的是由加载器包提供的占位，如旧版 Forge 的 forge 本体，下面按 universal 补齐
        if artifact.path.is_empty() {
            continue;
        }

        let obj = FileItemObj {
            name: item.name.clone(),
            file: libraries_path::get_lib_dir().join(&artifact.path),
            url: artifact.url.clone(),
            hash: FileHash::Sha1(artifact.sha1.clone()),
            later: Default::default(),
        };

        // url 为空的条目文件在安装器 jar 里，已经由 extract_embedded_libs 解到本地；
        // 本地也拿不到的就不进列表，否则下载阶段会发出一个空地址请求
        if obj.url.is_empty() && !obj.check_hash() {
            continue;
        }

        list.insert(item.name.clone(), obj);

        if item.name.ends_with(names::FILE_UNIVERSAL) {
            universal = true;
        } else if item.name.ends_with(names::FILE_INSTALLER) {
            installer = true;
        } else if item.name.ends_with(names::FILE_LAUNCHER) {
            launcher = true;
        }
    }

    if !installer && install {
        list.insert(
            String::from(names::FILE_INSTALLER),
            if neo {
                build_neoforge_installer(mc, version, false).await
            } else {
                build_forge_installer(mc, version, false).await
            },
        );
    }

    if !universal {
        if !neo || version_parse::is_game_version_1202(mc) {
            list.insert(
                String::from(names::FILE_UNIVERSAL),
                if neo {
                    build_neoforge_universal(mc, version, false).await
                } else {
                    build_forge_universal(mc, version, false).await
                },
            );
        }
    }

    if v2 && !version_parse::is_game_version_117(mc) && !launcher {
        list.insert(
            String::from(names::FILE_LAUNCHER),
            build_forge_launcher(mc, version, false).await,
        );
    }

    let mut list1 = Vec::<FileItemObj>::new();

    for (_, item) in list.drain() {
        list1.push(item);
    }

    list1
}

/// Forge 文件获取结果，把加载器库与安装器库分开返回
pub struct ForgeGetFilesObj {
    /// 加载器文件列表，进游戏时要用
    pub loaders: Vec<FileItemObj>,
    /// 安装器文件列表，只在安装阶段用
    pub installs: Vec<FileItemObj>,
}

/// 把安装器内嵌的运行库解到本地
///
/// `version.json` 与 `install_profile.json` 里 url 为空的条目没有下载地址，文件就在安装器
/// jar 的 `maven/<path>` 下，由安装器自己落地。本地已有且 SHA1 对得上时不动它；jar 里也没有
/// 该条目的直接跳过，调用方据此不把它排进下载列表。
///
/// - `zip`: 已打开的安装器压缩包
/// - `libs`: 运行库列表
///
/// # 返回值
///
/// 解包或落盘失败时返回对应错误
fn extract_embedded_libs<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    libs: &[ForgeLibrariesObj],
) -> CoreResult<()> {
    for item in libs {
        let artifact = &item.downloads.artifact;

        // 有下载地址的走下载，路径都没有的没什么可解
        if !artifact.url.is_empty() || artifact.path.is_empty() {
            continue;
        }

        let local = libraries_path::get_lib_dir().join(&artifact.path);

        // 本地已有且校验值对得上就不用再解一遍
        if let Ok(mut read) = path_helper::open_read(&local)
            && let Ok(sha1) = hash_helper::gen_hash_from_reader(HashType::Sha1, &mut read)
            && !artifact.sha1.is_empty()
            && sha1.eq_ignore_ascii_case(&artifact.sha1)
        {
            continue;
        }

        let Ok(mut entry) = zip.by_name(&format!("maven/{}", artifact.path)) else {
            continue;
        };
        path_helper::write_stream(&local, &mut entry)?;
    }

    Ok(())
}

/// 获取 Forge 运行库下载项
///
/// 需要时先下载安装器，再从安装器内解析运行库列表。
///
/// - `mc`: 游戏版本号
/// - `version`: Forge 版本号
/// - `neo`: 是否为 NeoForge
///
/// # 返回值
///
/// 返回加载器与安装器各自的下载项列表；下载或解析失败返回对应错误
async fn get_forge_libs(mc: &str, version: &str, neo: bool) -> CoreResult<ForgeGetFilesObj> {
    let ver = version_path::get_version(mc)?;
    let v2 = ver.is_game_version_v2();

    let installer = if neo {
        build_neoforge_installer(mc, version, true).await
    } else {
        build_forge_installer(mc, version, true).await
    };

    if !installer.check_hash() {
        let res = mml_downloader::start_download_task(vec![installer.clone()]).await;
        if !res {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        }
    }

    let stream = path_helper::open_read(&installer.file)?;
    let mut zip = ZipArchive::new(stream).map_err(|err| {
        ErrorType::ArchiveOpenError(FileSystemErrorData {
            path: installer.file.clone(),
            error: err.to_string(),
        })
    })?;

    // 读取 version.json
    let mut version_json = String::new();
    let version_ok = match zip.by_name(names::VERSION_FILE) {
        Ok(mut file) => {
            file.read_to_string(&mut version_json).map_err(|err| {
                ErrorType::ArchiveReadError(ErrorData {
                    error: err.to_string(),
                })
            })?;
            true
        }
        Err(_) => false,
    };

    // 读取 install_profile.json
    let mut install_json = String::new();
    let install_ok = match zip.by_name(names::FILE_INSTALL_PROFILE) {
        Ok(mut file) => {
            file.read_to_string(&mut install_json).map_err(|err| {
                ErrorType::ArchiveReadError(ErrorData {
                    error: err.to_string(),
                })
            })?;
            true
        }
        Err(_) => false,
    };

    if version_ok && install_ok {
        // 两个文件都在，说明是 1.12.2 以上的新版 Forge
        let info = serialize_tools::json_from_str::<ForgeLaunchObj>(&version_json)?;
        let info = version_path::add_forge(info, &version_json.into_bytes(), mc, version, neo);
        // 两份清单都会把 url 为空的运行库指向安装器内部，先解到本地再构建下载列表
        extract_embedded_libs(&mut zip, &info.libraries)?;

        let loaders = info.build_forge_libs(mc, version, neo, v2, false).await;

        let install_info = serialize_tools::json_from_str::<ForgeInstallObj>(&install_json)?;
        let install_info = version_path::add_forge_install(
            install_info,
            &install_json.into_bytes(),
            mc,
            version,
            neo,
        );
        extract_embedded_libs(&mut zip, &install_info.libraries)?;

        let installs = install_info
            .build_forge_libs(mc, version, neo, v2, true)
            .await;

        Ok(ForgeGetFilesObj { loaders, installs })
    } else if install_ok {
        // 只有 install_profile.json，是旧版 Forge
        let obj = serialize_tools::json_from_str::<ForgeInstallOldObj>(&install_json)?;

        let mut libraries: Vec<ForgeLibrariesObj> = Vec::new();
        for item in &obj.version_info.libraries {
            if let Some(lib) = make_forge_libraries(&item.name) {
                libraries.push(lib);
            } else if !item.url.is_empty() {
                let path = mml_net::maven_utils::version_name_to_path(&item.name);
                libraries.push(ForgeLibrariesObj {
                    name: item.name.clone(),
                    downloads: ForgeDownloadsObj {
                        artifact: ArtifactObj {
                            url: format!("{}{}", item.url, path),
                            path,
                            sha1: Default::default(),
                        },
                    },
                });
            }
        }

        let info = ForgeLaunchObj {
            main_class: obj.version_info.main_class,
            minecraft_arguments: Some(obj.version_info.minecraft_arguments),
            libraries,
            ..Default::default()
        };

        let json_bytes = serialize_tools::json_to_bytes(&info)?;
        let info = version_path::add_forge(info, &json_bytes, mc, version, neo);

        // install 传 false：安装器在函数第一步已下好，不该再进加载器库，
        // 否则它会跟着 loader_libs 上 classpath
        Ok(ForgeGetFilesObj {
            loaders: info.build_forge_libs(mc, version, neo, v2, false).await,
            installs: Vec::new(),
        })
    } else {
        Err(ErrorType::DataNotFound(DataNotFoundData::Info))
    }
}

impl ForgeLaunchObj {
    /// 从启动信息构建 Forge 运行库下载项目列表
    ///
    /// - `mc`: 游戏版本号
    /// - `version`: Forge 版本号
    /// - `neo`: 是否为 NeoForge
    /// - `v2`: 是否为 1.13 以上
    /// - `install`: 是否为安装器
    ///
    /// # 返回值
    ///
    /// 返回运行库下载项列表
    pub async fn build_forge_libs(
        &self,
        mc: &str,
        version: &str,
        neo: bool,
        v2: bool,
        install: bool,
    ) -> Vec<FileItemObj> {
        build_forge_libs(&self.libraries, mc, version, neo, v2, install).await
    }
}

impl ForgeInstallObj {
    /// 从安装信息构建 Forge 运行库下载项目列表
    ///
    /// - `mc`: 游戏版本号
    /// - `version`: Forge 版本号
    /// - `neo`: 是否为 NeoForge
    /// - `v2`: 是否为 1.13 以上
    /// - `install`: 是否为安装器
    ///
    /// # 返回值
    ///
    /// 返回运行库下载项列表
    pub async fn build_forge_libs(
        &self,
        mc: &str,
        version: &str,
        neo: bool,
        v2: bool,
        install: bool,
    ) -> Vec<FileItemObj> {
        build_forge_libs(&self.libraries, mc, version, neo, v2, install).await
    }
}

impl InstanceSettingObj {
    /// 获取 Forge 下载项目
    ///
    /// # 返回值
    ///
    /// 返回加载器与安装器各自的下载项列表；下载或解析失败返回对应错误
    pub async fn get_forge_libs(&self) -> CoreResult<ForgeGetFilesObj> {
        get_forge_libs(
            &self.version,
            &self.loader_version.as_ref().unwrap(),
            self.loader == LoaderType::NeoForge,
        )
        .await
    }

    /// 创建 Forge 安装器下载项目
    ///
    /// - `hash`: 是否联网获取文件哈希
    ///
    /// # 返回值
    ///
    /// 返回下载项
    pub async fn build_forge_installer(&self, hash: bool) -> FileItemObj {
        build_forge_installer(&self.version, &self.loader_version.as_ref().unwrap(), hash).await
    }

    /// 创建 Forge universal 包下载项目
    ///
    /// - `hash`: 是否联网获取文件哈希
    ///
    /// # 返回值
    ///
    /// 返回下载项
    pub async fn build_forge_universal(&self, hash: bool) -> FileItemObj {
        build_forge_universal(&self.version, &self.loader_version.as_ref().unwrap(), hash).await
    }

    /// 创建 Forge client 包下载项目
    ///
    /// - `hash`: 是否联网获取文件哈希
    ///
    /// # 返回值
    ///
    /// 返回下载项
    pub async fn build_forge_client(&self, hash: bool) -> FileItemObj {
        build_forge_client(&self.version, &self.loader_version.as_ref().unwrap(), hash).await
    }

    /// 创建 NeoForge 安装器下载项目
    ///
    /// - `hash`: 是否联网获取文件哈希
    ///
    /// # 返回值
    ///
    /// 返回下载项
    pub async fn build_neoforge_installer(&self, hash: bool) -> FileItemObj {
        build_neoforge_installer(&self.version, &self.loader_version.as_ref().unwrap(), hash).await
    }

    /// 创建 NeoForge universal 包下载项目
    ///
    /// - `hash`: 是否联网获取文件哈希
    ///
    /// # 返回值
    ///
    /// 返回下载项
    pub async fn build_neoforge_universal(&self, hash: bool) -> FileItemObj {
        build_neoforge_universal(&self.version, &self.loader_version.as_ref().unwrap(), hash).await
    }

    /// 创建 NeoForge client 包下载项目
    ///
    /// - `hash`: 是否联网获取文件哈希
    ///
    /// # 返回值
    ///
    /// 返回下载项
    pub async fn build_neoforge_client(&self, hash: bool) -> FileItemObj {
        build_neoforge_client(&self.version, &self.loader_version.as_ref().unwrap(), hash).await
    }
}

#[cfg(test)]
mod embedded_tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Once;
    use uuid::Uuid;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    /// 起内核系统并初始化运行库目录，每进程一次
    ///
    /// [`crate::test_support::boot`] 不含本 crate 的运行库目录，而用例要往它下面落文件；
    /// 该初始化是 `get_or_init`，与别的测试模块重复调用是安全的。
    fn setup() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let run = crate::test_support::boot();
            libraries_path::init(&run).unwrap();
        });
    }

    /// 造一个只含指定条目的安装器压缩包
    ///
    /// - `entries`: 包内路径与内容
    ///
    /// # 返回值
    ///
    /// 返回压缩包路径
    fn make_installer(entries: &[(&str, &[u8])]) -> PathBuf {
        let path = mml_testutil::temp_dir().join(format!("mml-forge-{}.zip", Uuid::new_v4()));
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    /// 造一个 url 为空的运行库条目
    ///
    /// - `path`: 运行库在 Maven 仓库内的相对路径
    /// - `sha1`: 期望的 SHA1 值
    ///
    /// # 返回值
    ///
    /// 返回运行库条目
    fn embedded_lib(path: &str, sha1: &str) -> ForgeLibrariesObj {
        ForgeLibrariesObj {
            name: format!("test:embedded:{path}"),
            downloads: ForgeDownloadsObj {
                artifact: ArtifactObj {
                    path: path.to_string(),
                    url: String::new(),
                    sha1: sha1.to_string(),
                },
            },
        }
    }

    /// 打开一个压缩包
    fn open_zip(path: &PathBuf) -> ZipArchive<std::fs::File> {
        ZipArchive::new(path_helper::open_read(path).unwrap()).unwrap()
    }

    /// 运行库在运行库目录下的落点
    fn local_of(path: &str) -> PathBuf {
        libraries_path::get_lib_dir().join(path)
    }

    /// 空 url 的运行库要从安装器 jar 的 `maven/` 下解到运行库目录
    #[test]
    fn extracts_embedded_lib() {
        setup();
        let data: &[u8] = b"embedded-jar-bytes";
        let rel = format!("test/embedded/{}/lib.jar", Uuid::new_v4().simple());
        let sha1 = hash_helper::gen_hash(HashType::Sha1, data);

        let entry = format!("maven/{rel}");
        let installer = make_installer(&[(&entry, data)]);
        let libs = [embedded_lib(&rel, &sha1)];

        extract_embedded_libs(&mut open_zip(&installer), &libs).unwrap();

        assert_eq!(
            std::fs::read(local_of(&rel)).unwrap(),
            data,
            "内嵌的运行库应被解到运行库目录"
        );
    }

    /// 安装器 jar 里没有该条目时什么都不落，也不报错
    #[test]
    fn missing_entry_is_skipped() {
        setup();
        let rel = format!("test/missing/{}/lib.jar", Uuid::new_v4().simple());
        let installer = make_installer(&[("version.json", b"{}")]);
        let libs = [embedded_lib(&rel, "00")];

        extract_embedded_libs(&mut open_zip(&installer), &libs).unwrap();

        assert!(!local_of(&rel).exists(), "jar 里没有的条目不该落盘");
    }

    /// 本地已有且校验值相符时不再重解一遍
    #[test]
    fn existing_lib_is_kept() {
        setup();
        let data: &[u8] = b"local-jar-bytes";
        let rel = format!("test/kept/{}/lib.jar", Uuid::new_v4().simple());
        let sha1 = hash_helper::gen_hash(HashType::Sha1, data);
        let local = local_of(&rel);
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, data).unwrap();

        let entry = format!("maven/{rel}");
        let installer = make_installer(&[(&entry, b"other-bytes")]);
        let libs = [embedded_lib(&rel, &sha1)];

        extract_embedded_libs(&mut open_zip(&installer), &libs).unwrap();

        assert_eq!(
            std::fs::read(&local).unwrap(),
            data,
            "校验值相符时不该被 jar 里的内容覆盖"
        );
    }

    /// 空 url 且本地拿不到的条目不进下载列表，否则会发出一个空地址请求
    #[tokio::test]
    async fn empty_url_without_local_file_is_not_in_list() {
        setup();
        let rel = format!("test/absent/{}/lib.jar", Uuid::new_v4().simple());
        let info = vec![embedded_lib(&rel, "00")];

        let list = build_forge_libs(&info, "1.12.2", "14.23.5.2864", false, false, false).await;

        assert!(
            !list.iter().any(|item| item.url.is_empty()),
            "下载列表里不该有空地址的条目：{:?}",
            list.iter().map(|item| item.name.clone()).collect::<Vec<_>>()
        );
    }

    /// 空 url 但本地已解出来的条目要留下，它得进 classpath
    #[tokio::test]
    async fn empty_url_with_local_file_stays_in_list() {
        setup();
        let data: &[u8] = b"local-jar-bytes";
        let rel = format!("test/present/{}/lib.jar", Uuid::new_v4().simple());
        let sha1 = hash_helper::gen_hash(HashType::Sha1, data);
        let local = local_of(&rel);
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, data).unwrap();

        let info = vec![embedded_lib(&rel, &sha1)];
        let list = build_forge_libs(&info, "1.12.2", "14.23.5.2864", false, false, false).await;

        assert!(
            list.iter().any(|item| item.file == local),
            "本地已有且校验值相符的条目应留在列表里"
        );
    }
}
