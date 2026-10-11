//! Forge / NeoForge 加载器安装与运行库下载项构建

use std::{collections::HashMap, io::Read};

use mml_base::{
    file_item::{FileHash, FileItemObj},
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
        if item.downloads.artifact.path.is_empty() {
            continue;
        }

        let obj = if item.downloads.artifact.url.is_empty() {
            FileItemObj {
                name: item.name.clone(),
                file: libraries_path::get_lib_dir().join(&item.downloads.artifact.path),
                url: Default::default(),
                hash: FileHash::Sha1(item.downloads.artifact.sha1.clone()),
                later: Default::default(),
            }
        } else {
            FileItemObj {
                name: item.name.clone(),
                file: libraries_path::get_lib_dir().join(&item.downloads.artifact.path),
                url: item.downloads.artifact.url.clone(),
                hash: FileHash::Sha1(item.downloads.artifact.sha1.clone()),
                later: Default::default(),
            }
        };

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
        let loaders = info.build_forge_libs(mc, version, neo, v2, false).await;

        let install_info = serialize_tools::json_from_str::<ForgeInstallObj>(&install_json)?;
        let install_info = version_path::add_forge_install(
            install_info,
            &install_json.into_bytes(),
            mc,
            version,
            neo,
        );
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

        Ok(ForgeGetFilesObj {
            loaders: info.build_forge_libs(mc, version, neo, v2, true).await,
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
