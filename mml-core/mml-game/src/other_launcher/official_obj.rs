//! 官方启动器版本 JSON DTO：version.json

use std::io::Read;
use std::path::Path;

use mml_base::serialize_tools::MiniJsonObj;
use mml_names::i18_items::error_type::{ArgEmptyData, CoreResult, ErrorType};
use mml_sys::path_helper;

/// 官方实例信息
#[derive(Default)]
pub struct OfficialObj {
    /// 版本 ID
    pub id: String,
    /// 继承的版本 ID
    pub inherits_from: String,
    /// 补丁列表，拆分式版本用
    pub patches: Vec<PatchObj>,
    /// 依赖库列表
    pub libraries: Vec<LibrarieObj>,
    /// 启动参数
    pub arguments: ArgumentsObj,
}

/// 补丁信息
pub struct PatchObj {
    /// 补丁 ID，如 `game` / `net.minecraft`
    pub id: String,
    /// 补丁版本
    pub version: String,
}

/// 依赖库信息
pub struct LibrarieObj {
    /// Maven 坐标
    pub name: String,
}

/// 启动参数
#[derive(Default)]
pub struct ArgumentsObj {
    /// 游戏参数，字符串或键值对象
    pub game: Vec<MiniJsonObj>,
}

impl OfficialObj {
    /// 从读取流解析信息
    ///
    /// - `stream`: 数据流，如文件、内存字节
    ///
    /// # 返回值
    ///
    /// 返回解析出的实例信息；JSON 不是对象返回 `ArgEmpty`
    pub fn from_reader<R: Read>(mut stream: R) -> CoreResult<Self> {
        let json = MiniJsonObj::from_stream(&mut stream)?;

        if let Some(data) = json.as_object() {
            let mut obj = OfficialObj {
                id: data.get_string("id"),
                inherits_from: data.get_string("inheritsFrom"),
                ..Default::default()
            };

            if let Some(list) = data.get_list("patches") {
                for item in list {
                    if let Some(list) = item.as_object() {
                        obj.patches.push(PatchObj {
                            id: list.get_string("id"),
                            version: list.get_string("version"),
                        });
                    }
                }
            }

            if let Some(list) = data.get_list("libraries") {
                for item in list {
                    if let Some(list) = item.as_object() {
                        obj.libraries.push(LibrarieObj {
                            name: list.get_string("name"),
                        });
                    }
                }
            }

            if let Some(list) = data.get_list("arguments") {
                obj.arguments.game.extend(list);
            }

            Ok(obj)
        } else {
            Err(ErrorType::ArgEmpty(ArgEmptyData::Version))
        }
    }

    /// 从文件读取信息
    ///
    /// - `file`: 文件位置
    ///
    /// # 返回值
    ///
    /// 返回解析出的实例信息；读取或解析失败返回对应错误
    pub fn read_from_file<P: AsRef<Path>>(file: P) -> CoreResult<Self> {
        let stream = path_helper::open_read(file)?;
        Self::from_reader(stream)
    }
}
