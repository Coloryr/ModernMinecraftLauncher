//! 光影包相关
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};

use mml_names::{
    i18_items::error_type::{ArgErrorData, CoreResult, ErrorType, FileSystemErrorData},
    names,
};
use mml_sys::path_helper;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use zip::ZipArchive;

use crate::{game_options, launcher::instance_setting_obj::InstanceSettingObj};

/// 光影包
pub struct ShaderpackObj {
    /// 显示名，取自语言文件，取不到时回退成文件名
    pub name: String,
    /// 说明文字，取自语言文件，可能为空
    pub comment: String,
    /// 光影包文件或目录的完整路径
    pub file: PathBuf,
}

impl Default for ShaderpackObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            name: Default::default(),
            comment: Default::default(),
            file: Default::default(),
        }
    }
}

impl ShaderpackObj {
    /// 删除光影包，进回收站
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；删除失败返回对应错误
    pub fn delete(&self) -> CoreResult<()> {
        path_helper::move_to_trash(&self.file)
    }
}

/// 从光影包语言文件读取名字与说明
///
/// - `stream`: 语言文件数据流
/// - `obj`: 写入目标
fn read_data<R: Read>(stream: R, obj: &mut ShaderpackObj) {
    // 从语言文件中读取名字，可能会有更多
    let options = game_options::read_options(stream, Some('='));
    if let Ok(data) = options {
        if let Some(about) = data.get("option.ABOUT") {
            obj.name = about.clone();
        }
        if let Some(about) = data.get("screen.ABOUT") {
            obj.name = about.clone();
        }
        if let Some(about) = data.get("option.ACERCADE") {
            obj.name = about.clone();
        }

        if let Some(about) = data.get("option.ABOUT.comment") {
            obj.comment = about.clone();
        }
        if let Some(about) = data.get("option.ACERCADE.comment") {
            obj.comment = about.clone();
        }
    }
}

/// 读取光影包信息
///
/// - `path`: 光影包文件路径，必须是 zip
///
/// # 返回值
///
/// 返回光影包信息；不是 zip 文件返回 `ArgError`
pub fn read_shaderpacks<P: AsRef<Path>>(path: P) -> CoreResult<ShaderpackObj> {
    if let Some(ext) = path.as_ref().extension()
        && ext.eq_ignore_ascii_case(names::ZIP_EXT)
    {
        let mut obj = ShaderpackObj {
            file: path.as_ref().to_path_buf(),
            ..Default::default()
        };

        let stream = path_helper::open_read(path.as_ref())?;
        let mut zip = ZipArchive::new(&stream).map_err(|err| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: path.as_ref().to_path_buf(),
                error: err.to_string(),
            })
        })?;

        for index in 0..zip.len() {
            let temp = zip.by_index(index);
            if let Ok(file) = temp {
                let lang = mml_names::get_lang(mml_names::get_lang_type());
                if file.is_file() && file.name_raw().ends_with(format!("lang/{lang}.lang").as_bytes()) {
                    read_data(file, &mut obj);
                } else if file.is_file() && file.name_raw().ends_with(b"lang/en_US.lang") {
                    read_data(file, &mut obj);
                }
            }
        }

        if obj.name.is_empty() {
            obj.name = path
                .as_ref()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
        }

        return Ok(obj);
    }

    Err(ErrorType::ArgError(ArgErrorData::ArchiveType))
}

/// 读取未打包成 zip 的光影包目录
///
/// - `path`: 光影包目录路径
///
/// # 返回值
///
/// 返回光影包信息；目录读取失败返回对应错误
fn read_shaderpacks_dir<P: AsRef<Path>>(path: P) -> CoreResult<ShaderpackObj> {
    let mut obj = ShaderpackObj {
        file: path.as_ref().to_path_buf(),
        ..Default::default()
    };

    let lang = mml_names::get_lang(mml_names::get_lang_type());
    let files = path_helper::get_all_files(&path);
    for file in files {
        if file.is_file() && file.ends_with(format!("lang/{lang}.lang")) {
            let stream = path_helper::open_read(file)?;
            read_data(stream, &mut obj);
        } else if file.is_file() && file.ends_with("lang/en_US.lang") {
            let stream = path_helper::open_read(file)?;
            read_data(stream, &mut obj);
        }
    }

    if obj.name.is_empty() {
        obj.name = path
            .as_ref()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
    }

    return Ok(obj);
}

impl InstanceSettingObj {
    /// 获取光影包列表
    ///
    /// # 返回值
    ///
    /// 返回光影包列表，读取失败的文件记录日志后跳过
    pub async fn get_shaderpacks(&self) -> Vec<ShaderpackObj> {
        let path = self.get_shaderpacks_path();

        tokio::task::spawn_blocking(move || {
            let list = Mutex::new(Vec::new());
            let files = path_helper::get_files(&path);
            files
                .par_iter()
                .for_each(|item| match read_shaderpacks(item) {
                    Ok(obj) => {
                        list.lock().unwrap().push(obj);
                    }
                    Err(err) => {
                        mml_log::error_type(err);
                    }
                });

            let dirs = path_helper::get_dirs(&path);
            dirs.par_iter()
                .for_each(|item| match read_shaderpacks_dir(item) {
                    Ok(obj) => {
                        list.lock().unwrap().push(obj);
                    }
                    Err(err) => {
                        mml_log::error_type(err);
                    }
                });

            list.into_inner().unwrap()
        })
        .await
        .unwrap_or_default()
    }
}
