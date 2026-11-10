//! 实例侧的版本查询
//!
//! 按实例设置取它用的版本与加载器。

use super::*;

use std::sync::Arc;

use mml_names::i18_items::error_type::CoreResult;

use crate::{
    launcher::{custom_loader_obj::CustomLoaderType, instance_setting_obj::InstanceSettingObj},
    loader::{
        LoaderType, fabric_loader_obj::FabricLoaderObj, forge_launch_obj::ForgeLaunchObj,
        optifine_obj::OptifineObj, quilt_loader_obj::QuiltLoaderObj,
    },
    mojang::game_arg_obj::GameArgObj,
};

impl InstanceSettingObj {
    /// 获取游戏版本类型
    ///
    /// # 返回值
    ///
    /// 返回版本类型，取值 release / snapshot / old_beta 等；无信息返回空串
    pub fn get_version_type(&self) -> String {
        let temp = VERSION.read().unwrap();

        if temp.is_none() {
            Default::default()
        } else {
            let temp = temp.clone().unwrap();

            if let Some(data) = temp
                .versions
                .iter()
                .filter(|item| item.id.eq_ignore_ascii_case(&self.version))
                .next()
            {
                data.version_type.clone()
            } else {
                Default::default()
            }
        }
    }

    /// 获取Forge加载器信息
    ///
    /// # 返回值
    ///
    /// 返回启动数据；无加载器版本或加载失败返回 `None`
    pub fn get_forge(&self) -> Option<Arc<ForgeLaunchObj>> {
        match &self.loader_version {
            None => None,
            Some(data) => get_forge(&self.version, &data),
        }
    }

    /// 获取NeoForge加载器信息
    ///
    /// # 返回值
    ///
    /// 返回启动数据；无加载器版本或加载失败返回 `None`
    pub fn get_neoforge(&self) -> Option<Arc<ForgeLaunchObj>> {
        match &self.loader_version {
            None => None,
            Some(data) => get_neoforge(&self.version, &data),
        }
    }

    /// 获取Fabric加载器数据
    ///
    /// # 返回值
    ///
    /// 返回加载器数据；无加载器版本或加载失败返回 `None`
    pub fn get_fabric(&self) -> Option<Arc<FabricLoaderObj>> {
        match &self.loader_version {
            None => None,
            Some(data) => get_fabric(&self.version, &data),
        }
    }

    /// 获取Quilt加载器数据
    ///
    /// # 返回值
    ///
    /// 返回加载器数据；无加载器版本或加载失败返回 `None`
    pub fn get_quilt(&self) -> Option<Arc<QuiltLoaderObj>> {
        match &self.loader_version {
            None => None,
            Some(data) => get_quilt(&self.version, &data),
        }
    }

    /// 获取自定义加载器数据
    ///
    /// # 返回值
    ///
    /// 返回自定义加载器数据；未添加返回 `None`
    pub fn get_custom_loader(&self) -> Option<Arc<CustomLoaderType>> {
        let list = CUSTOM_LOADERS.read().unwrap();
        Some(list.get(&self.uuid)?.clone())
    }

    /// 获取高清修复信息
    ///
    /// # 返回值
    ///
    /// 返回高清修复信息；非高清修复实例或不存在返回 `None`
    pub fn get_optifine(&self) -> Option<Arc<OptifineObj>> {
        if self.loader != LoaderType::OptiFine || self.loader_version.is_none() {
            None
        } else {
            get_optifine(self.loader_version.as_deref().unwrap())
        }
    }

    /// 更新游戏版本json
    ///
    /// # 返回值
    ///
    /// 返回最新的启动参数；在线无此版本或下载失败返回对应错误
    pub async fn check_version_update(&self) -> CoreResult<Arc<GameArgObj>> {
        check_update(&self.version).await
    }
}
