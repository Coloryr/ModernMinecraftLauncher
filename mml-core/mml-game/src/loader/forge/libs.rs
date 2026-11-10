//! Forge 运行库下载项：单条构建、镜像换源与列表合并

use super::*;

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
    item.url = mirror_maven(&format!(
        "{}net/minecraftforge/forge/{mc}-{version}/forge-{mc}-{version}-{}.jar",
        urls::FORGE,
        names::FILE_LAUNCHER
    ));
    item
}

/// 镜像源下把加载器运行库地址换到 BMCLAPI
///
/// Forge / NeoForge 的运行库地址写在加载器清单里，是绝对地址，不会跟着下载源变；
/// 不换的话选了镜像源仍然去连官方 maven。
///
/// - `url`: 加载器清单里的绝对下载地址
///
/// # 返回值
///
/// 镜像源且命中已知 maven 前缀时返回镜像地址，否则原样返回
fn mirror_maven(url: &str) -> String {
    if url_helper::get_source() != mml_config::config_obj::SourceLocal::Bmclapi {
        return url.to_string();
    }
    for prefix in [
        "https://maven.neoforged.net/releases/",
        "https://maven.minecraftforge.net/",
    ] {
        if let Some(rest) = url.strip_prefix(prefix) {
            return format!("{}maven/{rest}", urls::BMCLAPI);
        }
    }
    url.to_string()
}

/// 构建 Forge 运行库下载项目列表
///
/// - `info`: 运行库列表
/// - `mc`: 游戏版本号
/// - `version`: Forge 版本号
/// - `neo`: 是否为 NeoForge
/// - `cleanroom`: 是否为 Cleanroom
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
    cleanroom: bool,
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
            url: mirror_maven(&artifact.url),
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
            if cleanroom {
                crate::loader::cleanroom::build_cleanroom_installer(version, false).await
            } else if neo {
                crate::loader::neoforge::build_neoforge_installer(mc, version, false).await
            } else {
                build_forge_installer(mc, version, false).await
            },
        );
    }

    if !universal {
        if cleanroom {
            list.insert(
                String::from(names::FILE_UNIVERSAL),
                crate::loader::cleanroom::build_cleanroom_universal(version, false).await,
            );
        } else if !neo || version_parse::is_game_version_1202(mc) {
            list.insert(
                String::from(names::FILE_UNIVERSAL),
                if neo {
                    crate::loader::neoforge::build_neoforge_universal(mc, version, false).await
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
        cleanroom: bool,
        v2: bool,
        install: bool,
    ) -> Vec<FileItemObj> {
        build_forge_libs(&self.libraries, mc, version, neo, cleanroom, v2, install).await
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
        cleanroom: bool,
        v2: bool,
        install: bool,
    ) -> Vec<FileItemObj> {
        build_forge_libs(&self.libraries, mc, version, neo, cleanroom, v2, install).await
    }
}
