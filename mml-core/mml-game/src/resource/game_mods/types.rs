//! 模组类型与元数据结构

use std::path::PathBuf;

use mml_base::{
    file_item::FileHash,
    range_checker::{self, RangeSyntax, VersionRange},
    serialize_tools::MiniJsonObj,
};
use uuid::Uuid;

use crate::loader::LoaderType;

/// 模组的加载侧，来自元数据的 side 字段或注解扫描
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoadSideType {
    /// 没识别出加载侧
    Unknown,
    /// 仅客户端
    Client,
    /// 仅服务端
    Server,
    /// 客户端与服务端都要
    Both,
}

impl Default for LoadSideType {
    /// 默认按"未识别"处理
    fn default() -> Self {
        LoadSideType::Unknown
    }
}

/// 依赖前置信息
#[derive(Debug, Clone)]
pub struct DependantInfo {
    /// modid
    pub modid: String,
    /// 需求版本
    pub version: VersionRange,
}

impl Default for DependantInfo {
    fn default() -> Self {
        Self {
            modid: Default::default(),
            version: VersionRange::Any,
        }
    }
}

impl DependantInfo {
    /// 1.12.2 旧版依赖解析：`modid@版本范围`，不带 `@` 时整串就是 modid
    ///
    /// 版本串来自 Forge，按 **Maven** 写法解析，见 [`RangeSyntax`]。
    ///
    /// - `input`: 依赖串，形如 `modid@版本范围`，前后空白会被去掉
    ///
    /// # 返回值
    ///
    /// 解析出的依赖信息；没有 `@` 时版本串为空
    pub fn parse_required_name(input: &str) -> DependantInfo {
        let input = input.trim();
        if input.is_empty() {
            return Default::default();
        }

        let (modid, range_str) = match input.split_once('@') {
            Some((m, r)) => {
                let m = m.trim();
                if m.is_empty() {
                    return Default::default();
                }
                (m.to_string(), r.trim())
            }
            // 1.12.2 的 dependants / dependencies 列表里裸 modid 很常见，不能当成版本范围
            None => (input.to_string(), ""),
        };

        let range = if range_str.is_empty() {
            VersionRange::Any
        } else {
            range_checker::parse_version_range_with(range_str, RangeSyntax::Maven)
                .unwrap_or(VersionRange::Any)
        };

        DependantInfo {
            modid,
            version: range,
        }
    }

    /// 新版依赖解析：mods.toml 的 `versionRange`、fabric / quilt 的 depends
    ///
    /// - `modid`: 前置的 modid
    /// - `version`: 版本范围串；空串表示不限版本
    /// - `syntax`: 写法，见 [`RangeSyntax`] —— 裸版本号在两种写法里含义不同
    ///
    /// # 返回值
    ///
    /// 见 [`DependantInfo`]
    pub fn parse_required_version(
        modid: &str,
        version: &str,
        syntax: RangeSyntax,
    ) -> DependantInfo {
        let version_range = if version.is_empty() {
            VersionRange::Any
        } else {
            range_checker::parse_version_range_with(version, syntax).unwrap_or(VersionRange::Any)
        };

        DependantInfo {
            modid: modid.to_string(),
            version: version_range,
        }
    }

    /// 版本列表解析
    ///
    /// - `modid`: 依赖项目的 `modid`
    /// - `json`: 版本列表 JSON
    ///
    /// # 返回值
    ///
    /// 解析出的依赖信息
    pub fn parse_required_json(modid: &str, json: &MiniJsonObj) -> DependantInfo {
        DependantInfo {
            modid: modid.to_string(),
            version: range_checker::parse_json_array_range(json).unwrap_or(VersionRange::Any),
        }
    }
}

/// 前置依赖的类型
#[derive(Debug, Clone)]
pub enum DependantType {
    /// 强制要求的前置
    Required(DependantInfo),
    /// 推荐安装的前置 modid
    Recommend(DependantInfo),
}

/// 一个包里的单份模组元数据，同一个 jar 可能有多条
#[derive(Default)]
pub struct ModItemObj {
    /// 模组标识，取自元数据；缺失时为空串
    pub mod_id: String,
    /// 显示名，元数据没写时回退成 modid
    pub name: String,
    /// 描述，没写时为 `None`
    pub description: Option<String>,
    /// 版本号，没写时为 `None`
    pub version: Option<String>,
    /// 作者列表，可能为空
    pub author: Vec<String>,
    /// 前置依赖，含强制与推荐两种
    pub dependants: Vec<DependantType>,
    /// 主页地址，没写时为 `None`
    pub url: Option<String>,
    /// 图标原始字节，超过 `MAX_ICON_SIZE` 不读；没图标时为 `None`
    pub icon: Option<Vec<u8>>,
    /// 这份元数据声明的加载器
    pub loaders: LoaderType,
    /// 加载侧，默认 `Unknown`
    pub side: LoadSideType,
}

/// 模组目录里的一个文件及其解析结果
#[derive(Default)]
pub struct ModObj {
    /// 稳定标识，文件完整路径的 uuid v5，同一文件每次扫描结果一致
    pub uuid: Uuid,
    /// 元数据条目，一个包可能有多条；读不出元数据时为空
    pub info: Vec<ModItemObj>,
    /// 文件名是否带禁用后缀
    pub disable: bool,
    /// 是否为老式 core mod，由 `META-INF/MANIFEST.MF` 的标记决定
    pub core: bool,
    /// 文件校验值，`read_mod_fast` 只填 SHA1
    pub hash: FileHash,
    /// 内置的模组，递归展开；没有时为空
    pub jar_in_jar: Vec<ModObj>,
    /// 是否读取失败，为真时 `info` 为空、只剩文件名可用
    pub fail: bool,
    /// 文件完整路径；内置模组存的是包内条目名
    pub file: PathBuf,
}

/// 字符串引号状态
#[derive(PartialEq)]
pub(super) enum Quote {
    /// 不在字符串内
    None,
    /// 双引号字符串
    Double,
    /// 单引号字符串
    Single,
}
