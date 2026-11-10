//! Forge 加载器安装与运行库下载项构建
//!
//! NeoForge / Cleanroom 的下载项分别在 `neoforge` / `cleanroom`；三者共用这里的
//! 安装器解析、内嵌运行库解包与运行库列表合并流程。
//!
//! 子模块:
//!
//! | 模块 | 职责 |
//! | --- | --- |
//! | `libs` | 运行库下载项构建与合并 |
//! | `install` | 安装器压缩包解析、内嵌运行库解包 |
//! | `files` | 实例设置上的 Forge 下载入口 |

pub(crate) use std::{
    collections::HashMap,
    io::{Read, Seek},
};

pub(crate) use mml_base::{
    file_item::{FileHash, FileItemObj},
    hash_helper::{self, HashType},
    serialize_tools, version_parse,
};
pub(crate) use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType, FileSystemErrorData},
    names,
};
pub(crate) use mml_net::{
    maven_utils::{self},
    url_helper, urls,
};
pub(crate) use mml_sys::path_helper;
pub(crate) use zip::ZipArchive;

pub(crate) use crate::error::archive_read_err;
pub(crate) use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{libraries_path, version_path},
    loader::{
        LoaderType,
        forge_install_obj::{ForgeInstallObj, ForgeInstallOldObj},
        forge_launch_obj::{ForgeDownloadsObj, ForgeLaunchObj, ForgeLibrariesObj},
    },
    mojang::game_arg_obj::ArtifactObj,
};

mod files;
mod install;
mod libs;

pub use self::install::ForgeGetFilesObj;
// 空 url 运行库的解包只在单元测试里直接调用，正常构建不需要这个导出
#[cfg(test)]
pub(crate) use self::install::extract_embedded_libs;
pub(crate) use self::install::get_forge_libs;
pub use self::libs::{
    build_forge_client, build_forge_installer, build_forge_launcher, build_forge_libs,
    build_forge_universal, make_forge_libraries,
};

#[cfg(test)]
#[path = "../../../tests/unit/loader_forge_embedded_tests.rs"]
mod embedded_tests;
