//! 游戏实例配置相关

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
};

use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};
use mml_sys::path_helper;

use crate::launcher::instance_setting_obj::InstanceSettingObj;

/// 配置选项表，键 → 值
pub type InstanceCfg = HashMap<String, String>;

/// 从配置文件读取选项
///
/// - `file`: 配置文件路径
/// - `sp`: 键值分隔符；`None` 时取 `:`
///
/// # 返回值
///
/// 返回配置选项表；打开文件失败返回对应错误
pub fn read_options_from_file<P: AsRef<Path>>(
    file: P,
    sp: Option<char>,
) -> CoreResult<InstanceCfg> {
    let file = path_helper::open_read(file)?;
    read_options(file, sp)
}

/// 从数据流读取选项，忽略 `#` 注释行与行内注释
///
/// - `buffer`: 数据流
/// - `sp`: 键值分隔符；`None` 时取 `:`
///
/// # 返回值
///
/// 返回配置选项表；读取失败返回对应错误
pub fn read_options<R: Read>(buffer: R, sp: Option<char>) -> CoreResult<InstanceCfg> {
    let mut reader = BufReader::new(buffer);
    let mut data = HashMap::new();
    let mut line = String::new();
    let sp = sp.unwrap_or(':');
    loop {
        let size = reader.read_line(&mut line).map_err(|err| {
            ErrorType::StreamError(ErrorData {
                error: err.to_string(),
            })
        })?;
        if size == 0 {
            break;
        }

        // 去除 # 注释
        let line_trimmed = line.trim();
        if line_trimmed.starts_with('#') || line_trimmed.is_empty() {
            line.clear();
            continue;
        }
        // 处理行内注释
        let line_content = if let Some(pos) = line_trimmed.find('#') {
            line_trimmed[..pos].trim()
        } else {
            line_trimmed
        };
        if line_content.is_empty() {
            line.clear();
            continue;
        }

        let lists: Vec<&str> = line_content.splitn(2, sp).collect();
        if lists.len() == 1 {
            data.insert(lists[0].to_string(), Default::default());
        } else if lists.len() == 2 {
            data.insert(lists[0].to_string(), lists[1].to_string());
        }
        line.clear();
    }

    Ok(data)
}

impl InstanceSettingObj {
    /// 读取配置文件
    ///
    /// # 返回值
    ///
    /// 返回配置选项表；文件不存在返回空表，读取失败返回对应错误
    pub fn get_options(&self) -> CoreResult<HashMap<String, String>> {
        let file = self.get_optifine_file();
        if file.exists() {
            let stream = path_helper::open_read(file)?;
            read_options(stream, None)
        } else {
            Ok(Default::default())
        }
    }

    /// 保存配置文件
    ///
    /// - `list`: 配置选项
    /// - `sp`: 键值分隔符；`None` 时取 `:`
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；写入失败返回对应错误
    pub fn save_options(&self, list: &HashMap<String, String>, sp: Option<char>) -> CoreResult<()> {
        let file = self.get_optifine_file();
        let mut stream = path_helper::open_write(file)?;
        let sp = sp.unwrap_or(':');

        for (key, value) in list.iter() {
            stream
                .write_fmt(format_args!(
                    "{}{sp}{}{}",
                    key,
                    value,
                    mml_names::get_line_ending()
                ))
                .map_err(|err| {
                    ErrorType::StreamError(ErrorData {
                        error: err.to_string(),
                    })
                })?;
        }

        Ok(())
    }

    /// 读取游戏配置文件 options.txt，`=` 分隔；文件不存在时返回空表
    ///
    /// # 返回值
    ///
    /// 返回配置选项表；读取失败返回对应错误
    pub fn get_minecraft_options(&self) -> CoreResult<InstanceCfg> {
        let file = self.get_option_file();
        if file.exists() {
            read_options_from_file(file, Some('='))
        } else {
            Ok(Default::default())
        }
    }

    /// 保存游戏配置文件 options.txt，`=` 分隔
    ///
    /// - `list`: 配置选项
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；写入失败返回对应错误
    pub fn save_minecraft_options(&self, list: &InstanceCfg) -> CoreResult<()> {
        let file = self.get_option_file();
        let mut stream = path_helper::open_write(file)?;

        for (key, value) in list.iter() {
            stream
                .write_fmt(format_args!(
                    "{}={}{}",
                    key,
                    value,
                    mml_names::get_line_ending()
                ))
                .map_err(|err| {
                    ErrorType::StreamError(ErrorData {
                        error: err.to_string(),
                    })
                })?;
        }

        Ok(())
    }
}
