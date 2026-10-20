//! NeoForge 加载器下载项
//!
//! 从 Forge 抽出：NeoForge 的 maven 布局与 Forge 不同，尤其是 1.20.2+ 换成了
//! `net.neoforged:neoforge` 坐标；安装器解析、运行库合并仍走 `forge` 那套公共流程。

use mml_base::{
    file_item::{FileHash, FileItemObj},
    version_parse,
};
use mml_names::names;
use mml_net::{maven_utils, url_helper};

use crate::launcher::instance_setting_obj::InstanceSettingObj;
use crate::launcher_path::libraries_path;

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

impl InstanceSettingObj {
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
