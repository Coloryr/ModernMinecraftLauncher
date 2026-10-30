//! 游戏实例配置相关

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
};

use mml_names::i18_items::error_type::CoreResult;
use mml_sys::path_helper;

use crate::error::stream_err;
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
        let size = reader.read_line(&mut line).map_err(stream_err)?;
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
                .map_err(stream_err)?;
        }

        Ok(())
    }

    /// 读取游戏配置文件 options.txt，`:` 分隔；文件不存在时返回空表
    ///
    /// # 返回值
    ///
    /// 返回配置选项表；读取失败返回对应错误
    pub fn get_minecraft_options(&self) -> CoreResult<InstanceCfg> {
        let file = self.get_option_file();
        if file.exists() {
            read_options_from_file(file, None)
        } else {
            Ok(Default::default())
        }
    }

    /// 保存游戏配置文件 options.txt，`:` 分隔
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
                    "{}:{}{}",
                    key,
                    value,
                    mml_names::get_line_ending()
                ))
                .map_err(stream_err)?;
        }

        Ok(())
    }

    /// 改 options.txt 里的一个键，其余行原样保留
    ///
    /// 单个键的修改不要走整表重写：整表会把值里带 `#` 的行截断，也会打乱原有顺序。
    ///
    /// - `key`: 键名，如 `lang`
    /// - `value`: 新值；`None` 表示删掉这一行
    ///
    /// # 返回值
    ///
    /// 写入失败返回对应错误
    pub fn set_minecraft_option(&self, key: &str, value: Option<&str>) -> CoreResult<()> {
        let file = self.get_option_file();
        if !file.exists() && value.is_none() {
            return Ok(());
        }
        let text = if file.exists() {
            path_helper::read_text(&file)?
        } else {
            String::new()
        };
        let eol = mml_names::get_line_ending();
        let prefix = format!("{key}:");
        let mut out = String::with_capacity(text.len() + value.map_or(0, |v| v.len()) + eol.len());
        let mut found = false;
        for line in text.lines() {
            if line.starts_with(&prefix) {
                // 重复键只认第一条，后面的重复行直接丢掉
                if !found && let Some(value) = value {
                    out.push_str(&prefix);
                    out.push_str(value);
                    out.push_str(&eol);
                }
                found = true;
            } else {
                out.push_str(line);
                out.push_str(&eol);
            }
        }
        if !found && let Some(value) = value {
            out.push_str(&prefix);
            out.push_str(value);
            out.push_str(&eol);
        }
        path_helper::write_text(file, &out)
    }
}
