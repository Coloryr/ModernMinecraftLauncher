//! 模组启用与停用
//!
//! 停用只改文件名后缀，不移动文件；启用时把后缀去掉。

use std::path::{Path, PathBuf};

use mml_base::{
    file_item::FileHash,
    hash_helper::{self, HashType},
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
use mml_sys::path_helper;

use crate::{gui_hook::ProgressGui, launcher::instance_setting_obj::InstanceSettingObj};

use super::jar::{read_mod, scan_mod_files};
use super::types::ModObj;

/// 为文件追加禁用后缀
///
/// 后缀用 `.disabled`，与 MMC / HMCL 一致；旧版留下的 `.disable` 读取时仍然认。
///
/// - `path`: 模组文件路径
///
/// # 返回值
///
/// 成功返回**改名后的新路径**，调用方要拿它更新身份，因为模组的 uuid 是路径的 v5、
/// 文件名一变 uuid 就变；重命名失败返回对应错误
pub fn add_disable_suffix(path: &Path) -> CoreResult<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| ErrorType::InvalidOperation)?;

    let mut new_name = file_name.to_os_string();
    new_name.push(names::DISABLED_DOT_EXT);
    let new_path = path.with_file_name(new_name);

    path_helper::move_file(path, &new_path)?;
    Ok(new_path)
}

/// 移除文件的禁用后缀
///
/// - `path`: 模组文件路径
///
/// # 返回值
///
/// 成功返回**改名后的新路径**，无后缀时原样返回、什么都没改；
/// 重命名失败返回对应错误
pub fn remove_disable_suffix(path: &Path) -> CoreResult<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| ErrorType::InvalidOperation)?;

    let name_str = file_name
        .to_str()
        .ok_or_else(|| ErrorType::InvalidOperation)?;

    if let Some(stripped) = name_str.strip_suffix(names::DISABLE_DOT_EXT) {
        let new_path = path.with_file_name(stripped);
        path_helper::move_file(path, &new_path)?;
        Ok(new_path)
    } else if let Some(stripped) = name_str.strip_suffix(names::DISABLED_DOT_EXT) {
        let new_path = path.with_file_name(stripped);
        path_helper::move_file(path, &new_path)?;
        Ok(new_path)
    } else {
        Ok(path.to_path_buf())
    }
}

impl ModObj {
    /// 删除模组文件，进回收站
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；删除失败返回对应错误
    pub fn delete(&self) -> CoreResult<()> {
        path_helper::move_to_trash(&self.file)
    }

    /// 禁用模组
    ///
    /// # 返回值
    ///
    /// 成功返回**改名后的新路径**，前端要拿它更新那一行的 uuid / 文件名 / 路径；
    /// 已禁用、文件不存在或重命名失败返回对应错误
    pub fn disable(&self) -> CoreResult<PathBuf> {
        if self.disable || !self.file.exists() {
            return Err(ErrorType::InvalidOperation);
        }

        add_disable_suffix(&self.file)
    }

    /// 启用模组
    ///
    /// # 返回值
    ///
    /// 成功返回**改名后的新路径**；未禁用、文件不存在或重命名失败返回对应错误
    pub fn enable(&self) -> CoreResult<PathBuf> {
        if !self.disable || !self.file.exists() {
            return Err(ErrorType::InvalidOperation);
        }

        remove_disable_suffix(&self.file)
    }
}

impl InstanceSettingObj {
    /// 扫描模组
    ///
    /// - `gui`: 进度回调，可传 `None`；每完成一个文件报一次 `(已完成, 总数)`
    ///
    /// # 返回值
    ///
    /// 返回模组列表，只有哈希、不解析模组信息；失败文件标记 `fail`
    pub async fn read_mod_fast(&self, gui: ProgressGui) -> Vec<ModObj> {
        let dir = self.get_mods_path();
        let files = path_helper::get_files(dir);

        tokio::task::spawn_blocking(move || {
            scan_mod_files(
                files,
                |item| {
                    let hash = hash_helper::gen_hash_from_file(HashType::Sha1, item)?;
                    Ok(ModObj {
                        hash: FileHash::Sha1(hash),
                        file: item.clone(),
                        ..Default::default()
                    })
                },
                gui,
            )
        })
        .await
        .unwrap_or_default()
    }

    /// 读取模组列表
    ///
    /// - `sha256`: 是否连 SHA256 一起算，否则只算 SHA1
    /// - `gui`: 进度回调，可传 `None`；每完成一个文件报一次 `(已完成, 总数)`
    ///
    /// # 返回值
    ///
    /// 返回模组列表，含解析出的模组信息；失败文件标记 `fail`
    pub async fn read_mod(&self, sha256: bool, gui: ProgressGui) -> Vec<ModObj> {
        let dir = self.get_mods_path();
        let files = path_helper::get_files(dir);

        tokio::task::spawn_blocking(move || {
            scan_mod_files(files, |item| read_mod(item, sha256), gui)
        })
        .await
        .unwrap_or_default()
    }
}
