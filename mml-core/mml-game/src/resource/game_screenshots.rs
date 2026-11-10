//! 游戏实例屏幕截图相关

use std::path::PathBuf;

use mml_names::{i18_items::error_type::CoreResult, names};
use mml_sys::path_helper;

use crate::launcher::instance_setting_obj::InstanceSettingObj;

/// 一张截图
pub struct ScreenshotObj {
    /// 文件名，含扩展名
    pub name: String,
    /// 截图的完整路径
    pub file: PathBuf,
}

impl ScreenshotObj {
    /// 把这张截图移入回收站
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；失败返回对应错误
    pub fn delete(&self) -> CoreResult<()> {
        path_helper::move_to_trash(&self.file)
    }
}

impl InstanceSettingObj {
    /// 取实例截图目录下的全部截图
    ///
    /// # 返回值
    ///
    /// 截图目录下的 PNG 文件列表
    pub fn get_screenshots(&self) -> Vec<ScreenshotObj> {
        let dir = self.get_screenshots_path();
        let mut list = Vec::new();
        for item in path_helper::get_files(dir).iter() {
            if let Some(ext) = item.extension()
                && let Some(name) = item.file_name()
            {
                if ext.eq_ignore_ascii_case(names::PNG_EXT) {
                    list.push(ScreenshotObj {
                        name: name.to_string_lossy().to_string(),
                        file: item.clone(),
                    });
                }
            }
        }

        list
    }

    /// 把实例的全部截图移入回收站
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；任一文件删除失败返回对应错误
    pub fn clear_screenshots(&self) -> CoreResult<()> {
        let dir = self.get_screenshots_path();
        for item in path_helper::get_files(dir).iter() {
            path_helper::move_to_trash(item)?;
        }

        Ok(())
    }
}
