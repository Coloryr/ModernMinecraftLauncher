//! OptiFine 安装

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
use mml_net::optifine_api;

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::{libraries_path, version_path},
    loader::optifine_obj::OptifineObj,
};

/// 创建optifine下载项目
///
/// - `mc`: 游戏版本
/// - `version`: optifine版本
///
/// # 返回值
///
/// 返回下载项列表；版本不存在或拿不到下载地址返回 `DataNotFound`
pub async fn get_optifine_libs(mc: &str, version: &str) -> CoreResult<Vec<FileItemObj>> {
    let list = optifine_api::get_optifine_version().await?;

    let item = list
        .iter()
        .filter(|item| {
            item.version.eq_ignore_ascii_case(version) && item.mc_version.eq_ignore_ascii_case(mc)
        })
        .next();

    match item.cloned() {
        Some(item) => {
            let item = version_path::add_optifine(OptifineObj::from(item));
            let url =
                optifine_api::get_optifine_download(&item.source, &item.url1, &item.url2).await?;
            match url {
                Some(url) => Ok(vec![FileItemObj {
                    name: item.file_name.clone(),
                    file: libraries_path::get_optifine_file(mc, version),
                    url: url.clone(),
                    hash: FileHash::None,
                    later: LaterRun::None,
                }]),
                None => Err(ErrorType::DataNotFound(DataNotFoundData::Info)),
            }
        }
        None => Err(ErrorType::DataNotFound(DataNotFoundData::Info)),
    }
}

impl InstanceSettingObj {
    /// 创建optifine下载项目
    ///
    /// # 返回值
    ///
    /// 返回下载项列表；版本不存在或拿不到下载地址返回 `DataNotFound`
    pub async fn get_optifine_libs(&self) -> CoreResult<Vec<FileItemObj>> {
        get_optifine_libs(&self.version, &self.loader_version.as_ref().unwrap()).await
    }
}
