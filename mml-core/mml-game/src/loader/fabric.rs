//! Fabric 加载器安装

use mml_base::{
    file_item::{FileHash, FileItemObj, LaterRun},
    serialize_tools,
};
use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
use mml_net::{fabric_api, maven_utils::version_name_to_path, url_helper};

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{libraries_path, version_path},
    loader::{fabric_loader_obj::FabricLoaderObj, fabric_meta_obj::FabricMetaObj},
};

/// 取 Fabric 加载器的运行库下载项
///
/// - `mc`: 游戏版本号，如 `1.20.1`
/// - `version`: Fabric 加载器版本号，`None` 表示取最新稳定版
///
/// # 返回值
///
/// 运行库下载项列表；版本不存在返回 `DataNotFound`
pub async fn get_fabric_libs(mc: &str, version: Option<&str>) -> CoreResult<Vec<FileItemObj>> {
    let meta = fabric_api::get_meta().await?;

    let obj = serialize_tools::json_from_bytes::<FabricMetaObj>(&meta)?;

    let fabric = match version {
        Some(version) => obj
            .loader
            .iter()
            .filter(|item| item.version.eq_ignore_ascii_case(version))
            .next(),
        None => obj.loader.iter().filter(|item| item.stable).next(),
    };

    if let Some(fabric) = fabric {
        let data = fabric_api::get_loader(mc, &fabric.version).await?;
        let obj = serialize_tools::json_from_bytes::<FabricLoaderObj>(&data)?;
        let obj = version_path::add_fabric(obj, &data, mc, &fabric.version);
        Ok(obj.make_libs())
    } else {
        Err(ErrorType::DataNotFound(DataNotFoundData::Info))
    }
}

impl FabricLoaderObj {
    /// 把加载器元数据里的运行库转成下载项
    ///
    /// # 返回值
    ///
    /// 各运行库的下载项
    pub fn make_libs(&self) -> Vec<FileItemObj> {
        let mut list = Vec::new();

        for item in &self.libraries {
            let name = version_name_to_path(&item.name);
            list.push(FileItemObj {
                name: item.name.clone(),
                file: libraries_path::get_lib_dir().join(&name),
                url: url_helper::replace_fabric_libraries(&item.url) + &name,
                hash: FileHash::Sha256(item.sha256.clone()),
                later: LaterRun::None,
            });
        }

        list
    }
}

impl InstanceSettingObj {
    /// 取实例所需的 Fabric 运行库
    ///
    /// # 返回值
    ///
    /// 运行库下载项列表；本地已有版本 JSON 时直接读取，否则走网络
    pub async fn get_fabric_libs(&self) -> CoreResult<Vec<FileItemObj>> {
        let fabric =
            version_path::get_fabric(&self.version, &self.loader_version.as_ref().unwrap());
        match fabric {
            Some(fabric) => Ok(fabric.make_libs()),
            None => {
                get_fabric_libs(
                    &self.version,
                    self.loader_version.as_ref().map(|x| x.as_str()),
                )
                .await
            }
        }
    }
}
