//! Forge 安装器解析：读压缩包内的清单，解内嵌运行库并给出两路下载项

use super::*;

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
pub(crate) fn extract_embedded_libs<R: Read + Seek>(
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
/// - `version`: 加载器版本号
/// - `neo`: 是否为 NeoForge
/// - `cleanroom`: 是否为 Cleanroom
///
/// # 返回值
///
/// 返回加载器与安装器各自的下载项列表；下载或解析失败返回对应错误
pub(crate) async fn get_forge_libs(
    mc: &str,
    version: &str,
    neo: bool,
    cleanroom: bool,
) -> CoreResult<ForgeGetFilesObj> {
    let ver = version_path::get_version(mc)?;
    let v2 = ver.is_game_version_v2();

    let installer = if cleanroom {
        crate::loader::cleanroom::build_cleanroom_installer(version, true).await
    } else if neo {
        crate::loader::neoforge::build_neoforge_installer(mc, version, true).await
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
            file.read_to_string(&mut version_json)
                .map_err(archive_read_err)?;
            true
        }
        Err(_) => false,
    };

    // 读取 install_profile.json
    let mut install_json = String::new();
    let install_ok = match zip.by_name(names::FILE_INSTALL_PROFILE) {
        Ok(mut file) => {
            file.read_to_string(&mut install_json)
                .map_err(archive_read_err)?;
            true
        }
        Err(_) => false,
    };

    if version_ok && install_ok {
        // 两个文件都在，说明是 1.12.2 以上的新版 Forge
        let info = serialize_tools::json_from_str::<ForgeLaunchObj>(&version_json)?;
        // 先解析原生库：add_forge 会消费 version_json
        let native_items = if cleanroom {
            crate::loader::cleanroom::build_cleanroom_natives(&version_json, mc)
        } else {
            Vec::new()
        };
        let info = version_path::add_forge(info, &version_json.into_bytes(), mc, version, neo);
        // 两份清单都会把 url 为空的运行库指向安装器内部，先解到本地再构建下载列表
        extract_embedded_libs(&mut zip, &info.libraries)?;

        let mut loaders = info
            .build_forge_libs(mc, version, neo, cleanroom, v2, false)
            .await;
        loaders.extend(native_items);

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
            .build_forge_libs(mc, version, neo, cleanroom, v2, true)
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
            loaders: info
                .build_forge_libs(mc, version, neo, cleanroom, v2, false)
                .await,
            installs: Vec::new(),
        })
    } else {
        Err(ErrorType::DataNotFound(DataNotFoundData::Info))
    }
}
