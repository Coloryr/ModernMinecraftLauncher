//! Quilt 加载器安装

use mml_base::{
    file_item::{FileHash, FileItemObj, LaterRun},
    serialize_tools,
};
use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
use mml_net::{maven_utils::version_name_to_path, quilt_api, url_helper};

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{libraries_path, version_path},
    loader::{quilt_loader_obj::QuiltLoaderObj, quilt_meta_obj::QuiltMetaObj},
};

/// 获取Quilt下载项目
///
/// - `mc`: 游戏版本号
/// - `version`: quilt版本号
///
/// # 返回值
///
/// 返回运行库下载项列表；版本不存在返回 `DataNotFound`
pub async fn get_quilt_libs(mc: &str, version: Option<&str>) -> CoreResult<Vec<FileItemObj>> {
    let meta = quilt_api::get_meta().await?;

    let obj = serialize_tools::json_from_bytes::<QuiltMetaObj>(&meta)?;
    let fabric = match version {
        Some(version) => obj
            .loader
            .iter()
            .filter(|item| item.version.eq_ignore_ascii_case(version))
            .next(),
        None => obj.loader.iter().next(),
    };

    if let Some(fabric) = fabric {
        let data = quilt_api::get_loader(mc, &fabric.version).await?;
        let obj = serialize_tools::json_from_bytes::<QuiltLoaderObj>(&data)?;
        let obj = version_path::add_quilt(obj, &data, mc, &fabric.version);

        let mut list = Vec::new();

        for item in &obj.libraries {
            let name = version_name_to_path(&item.name);
            list.push(FileItemObj {
                name: item.name.clone(),
                file: libraries_path::get_lib_dir().join(&name),
                url: url_helper::replace_fabric_libraries(&item.url) + &name,
                hash: FileHash::None,
                later: LaterRun::None,
            });
        }

        Ok(list)
    } else {
        Err(ErrorType::DataNotFound(DataNotFoundData::Info))
    }
}

impl InstanceSettingObj {
    /// 获取Quilt下载项目
    ///
    /// # 返回值
    ///
    /// 返回运行库下载项列表；版本不存在返回 `DataNotFound`
    pub async fn get_quilt_libs(&self) -> CoreResult<Vec<FileItemObj>> {
        get_quilt_libs(
            &self.version,
            self.loader_version.as_ref().map(|x| x.as_str()),
        )
        .await
    }
}
