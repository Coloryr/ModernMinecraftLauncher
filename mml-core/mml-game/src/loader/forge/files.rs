//! 实例设置上的 Forge 下载入口，把实例的加载器版本转给运行库构建

use super::*;

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
            self.loader == LoaderType::Cleanroom,
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
}
