//! 实例导出为整合包

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use mml_base::{
    archives::{ArchiveType, BaseArchiveGui},
    file_item::FileHash,
};
use mml_names::i18_items::error_type::{CoreResult, ErrorType};

use mml_net::curseforge_api::list_obj::AuthorsObj;

use crate::launcher::{
    file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
};

mod archive;
mod curseforge;
mod hmcl;
mod mmc;
mod modrinth;

use self::archive::{colormc, zip_pack};
use self::curseforge::curseforge;
use self::hmcl::hmcl;
use self::mmc::mmc;
pub use self::mmc::{mmc_instance_cfg, mmc_vars};
use self::modrinth::modrinth;

/// 导出压缩包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPackType {
    /// MML 格式
    MML,
    /// CurseForge 格式
    CurseForge,
    /// Modrinth 格式
    Modrinth,
    /// MMC 格式，整包实例，供 MultiMC / Prism 导入
    MMC,
    /// HMCL 格式，整包实例，供 HMCL 导入
    HMCL,
    /// 纯压缩包，只打包文件不带整合包清单
    Zip,
}

/// 导出的在线文件
pub struct OnlineFileExport {
    /// 文件路径
    pub file: PathBuf,
    /// 文件大小，单位字节
    pub size: usize,
    /// 导出方式
    pub pack: ExportPackType,
    /// 下载地址
    pub url: String,
    /// 文件校验值
    pub hash: FileHash,
    /// 文件在线信息，非线上来源时为 `None`
    pub info: Option<OnlineInfoObj>,
}

/// 导出需要的参数
pub struct ExportArg {
    /// 导出保存的位置
    pub file: PathBuf,
    /// 打包类型
    pub pack: ExportPackType,
    /// 压缩类型
    pub archive: ArchiveType,
    /// 在线模组信息
    pub mods: Vec<OnlineFileExport>,
    /// 在线文件信息
    pub files: Vec<OnlineFileExport>,
    /// 不打包的文件
    pub unselect: Vec<PathBuf>,
    /// 一起打包的文件
    pub select: Vec<PathBuf>,
    /// 取消令牌，置位后在写下一个文件前收手；`None` 表示不中途取消
    pub cancel: Option<Arc<AtomicBool>>,
    /// 整合包名字
    pub name: String,
    /// 整合包作者
    pub author: String,
    /// 整合包版本号
    pub version: String,
    /// 整合包简介
    pub summary: String,
    /// 压缩进度条
    pub gui: BaseArchiveGui,
}

impl InstanceSettingObj {
    /// 导出整合包
    ///
    /// - `arg`: 导出参数
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；打包失败返回对应错误
    pub async fn export(&self, arg: ExportArg) -> CoreResult<()> {
        match arg.pack {
            ExportPackType::MML => colormc(self, arg),
            ExportPackType::CurseForge => curseforge(self, arg).await,
            ExportPackType::Modrinth => modrinth(self, arg).await,
            ExportPackType::MMC => mmc(self, arg),
            ExportPackType::HMCL => hmcl(self, arg),
            ExportPackType::Zip => zip_pack(self, arg),
        }
    }
}

/// 导出是否已被取消
///
/// - `cancel`: 取消令牌，`None` 表示不中途取消
///
/// # 返回值
///
/// 已置位时返回任务取消错误，调用方据此停下
fn check_cancel(cancel: &Option<Arc<AtomicBool>>) -> CoreResult<()> {
    if cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::Relaxed))
    {
        return Err(ErrorType::TaskCancel);
    }
    Ok(())
}

/// 将作者列表拼接为字符串
///
/// - `author`: 作者列表
///
/// # 返回值
///
/// 返回拼接后的作者名；空列表返回空串
fn get_author_string(author: &Vec<AuthorsObj>) -> String {
    if author.is_empty() {
        String::new()
    } else {
        let mut data = String::new();

        for item in author.iter() {
            data.push_str(&item.name);
        }

        data
    }
}

#[cfg(test)]
#[path = "../../tests/unit/game_export_tests.rs"]
mod tests;
