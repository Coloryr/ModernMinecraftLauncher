//! 自定义加载器安装：分析 Forge 型 jar

use std::{io::Read, path::Path};

use mml_base::{
    file_item::{FileHash, FileItemObj},
    hash_helper::{self, HashType},
    serialize_tools,
};
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType, FileSystemErrorData},
    names,
};
use mml_sys::path_helper;
use zip::ZipArchive;

use crate::error::archive_read_err;
use crate::{
    launcher::{custom_loader_obj::CustomLoaderType, instance_setting_obj::InstanceSettingObj},
    launcher_path::{libraries_path, version_path},
    loader::{
        forge_install_obj::ForgeInstallObj,
        forge_launch_obj::{ForgeLaunchObj, ForgeLibrariesObj},
    },
};

/// 自定义加载器 jar 的分析结果
pub struct CutsomLoaderResObj {
    /// 加载器名称，如 `1.20.1-forge-47.2.0`
    pub name: String,
    /// 需要下载的运行库列表，已排除本地齐全的项
    pub libs: Vec<FileItemObj>,
}

impl InstanceSettingObj {
    /// 分析实例自己的加载器 jar
    ///
    /// # 返回值
    ///
    /// 分析结果；读取或解析失败返回对应错误
    pub async fn decode_loader_jar(&self) -> CoreResult<CutsomLoaderResObj> {
        self.decode_loader_jar_with_path(self.get_loader_file())
            .await
    }

    /// 分析指定路径的加载器 jar
    ///
    /// - `path`: 加载器 jar 路径
    ///
    /// # 返回值
    ///
    /// 加载器名与缺失的运行库；文件不是合法的 Forge 型加载器返回 `DataNotFound`
    pub async fn decode_loader_jar_with_path<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> CoreResult<CutsomLoaderResObj> {
        let path = path.as_ref();
        let mut stream = path_helper::open_read(path)?;
        let mut zip = ZipArchive::new(&mut stream).map_err(|err| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: path.to_path_buf(),
                error: err.to_string(),
            })
        })?;

        // 读取 version.json，解析成 ForgeLaunchObj
        let mut version_json = String::new();
        let version_ok = match zip.by_name(names::VERSION_FILE) {
            Ok(mut file) => {
                file.read_to_string(&mut version_json)
                    .map_err(archive_read_err)?;
                true
            }
            Err(_) => false,
        };

        // 读取 install_profile.json，解析成 ForgeInstallObj
        let mut install_json = String::new();
        let install_ok = match zip.by_name(names::FILE_INSTALL_PROFILE) {
            Ok(mut file) => {
                file.read_to_string(&mut install_json)
                    .map_err(archive_read_err)?;
                true
            }
            Err(_) => false,
        };

        // 两个文件都必须存在
        if !version_ok || !install_ok {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        }

        let obj1 = serialize_tools::json_from_str::<ForgeLaunchObj>(&version_json)?;
        let obj2 = serialize_tools::json_from_str::<ForgeInstallObj>(&install_json)?;

        let mut list = Vec::new();
        let libraries_path = libraries_path::get_lib_dir();

        // 收集两个对象中的所有运行库
        let all_libraries: Vec<&ForgeLibrariesObj> =
            obj1.libraries.iter().chain(obj2.libraries.iter()).collect();

        for item in all_libraries {
            if !item.downloads.artifact.url.is_empty() {
                // 有下载地址：本地已有且哈希正确就跳过
                let local = libraries_path.join(&item.downloads.artifact.path);
                let mut skip = false;
                if let Ok(mut read) = path_helper::open_read(&local) {
                    if let Ok(sha1) = hash_helper::gen_hash_from_reader(HashType::Sha1, &mut read) {
                        if sha1.eq_ignore_ascii_case(&item.downloads.artifact.sha1) {
                            skip = true;
                        }
                    }
                }
                if !skip {
                    list.push(FileItemObj {
                        name: item.name.clone(),
                        file: libraries_path.join(&item.downloads.artifact.path),
                        url: item.downloads.artifact.url.clone(),
                        hash: FileHash::Sha1(item.downloads.artifact.sha1.clone()),
                        later: Default::default(),
                    });
                }
            } else {
                // 无下载地址：运行库在 zip 压缩包内部
                let zip_path = format!("maven/{}", item.downloads.artifact.path);
                if let Ok(zip_file) = zip.by_name(&zip_path) {
                    let local = libraries_path.join(&item.downloads.artifact.path);
                    // 检查本地文件是否已存在且哈希正确
                    let mut need_extract = true;
                    if let Ok(mut read) = path_helper::open_read(&local) {
                        if let Ok(sha1) =
                            hash_helper::gen_hash_from_reader(HashType::Sha1, &mut read)
                        {
                            if sha1.eq_ignore_ascii_case(&item.downloads.artifact.sha1) {
                                need_extract = false;
                            }
                        }
                    }
                    if need_extract {
                        // 从 zip 解压到本地
                        path_helper::write_stream(&local, zip_file)?;
                    }
                }
                // 如果 zip 中没有该文件，则跳过此运行库
            }
        }

        // 构建加载器名称
        let name = if !obj2.version.starts_with(&obj2.profile) {
            format!("{}-{}", obj2.profile, obj2.version)
        } else {
            obj2.version.clone()
        };

        // 保存自定义加载器信息
        version_path::add_custom_loader(CustomLoaderType::ForgeLaunch(obj1), self.uuid);

        Ok(CutsomLoaderResObj { name, libs: list })
    }

    /// 取自定义加载器的游戏参数
    ///
    /// # 返回值
    ///
    /// 游戏参数列表；未设置自定义加载器返回空列表
    pub fn get_custom_loader_game_args(&self) -> Vec<String> {
        if let Some(data) = self.get_custom_loader() {
            match data.as_ref() {
                CustomLoaderType::ForgeLaunch(forge) => {
                    let mut args = Vec::<String>::new();
                    if let Some(data) = &forge.minecraft_arguments {
                        let args1: Vec<&str> = data.split(' ').collect();
                        args1.iter().for_each(|item| {
                            args.push(String::from(*item));
                        });
                    }

                    if let Some(data) = &forge.arguments {
                        for item in data.game.iter() {
                            args.push(item.clone());
                        }
                    }

                    args
                }
            }
        } else {
            Default::default()
        }
    }

    /// 取自定义加载器的 JVM 参数
    ///
    /// # 返回值
    ///
    /// JVM 参数列表；未设置自定义加载器返回空列表
    pub fn get_custom_loader_jvm_args(&self) -> Vec<String> {
        if let Some(data) = self.get_custom_loader() {
            match data.as_ref() {
                CustomLoaderType::ForgeLaunch(forge) => {
                    let mut args = Vec::<String>::new();

                    if let Some(data) = &forge.arguments {
                        for item in data.jvm.iter() {
                            args.push(item.clone());
                        }
                    }

                    args
                }
            }
        } else {
            Default::default()
        }
    }

    /// 取自定义加载器的主类
    ///
    /// # 返回值
    ///
    /// 主类名；未设置自定义加载器返回空串
    pub fn get_custom_loader_mainclass(&self) -> String {
        if let Some(data) = self.get_custom_loader() {
            match data.as_ref() {
                CustomLoaderType::ForgeLaunch(forge) => forge.main_class.clone(),
            }
        } else {
            Default::default()
        }
    }
}
