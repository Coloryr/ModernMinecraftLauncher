//! 游戏实例模组相关
use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use mml_base::{
    file_item::FileHash,
    hash_helper::{self, HashType},
    range_checker::{self, VersionRange},
    serialize_tools::{MiniJsonObj, MiniTomlMap},
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorData, ErrorType, FileSystemErrorData},
    names,
};
use mml_sys::path_helper;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use uuid::Uuid;
use zip::ZipArchive;

use crate::{
    class_scan, gui_hook::ProgressGui, launcher::instance_setting_obj::InstanceSettingObj,
    loader::LoaderType,
};

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
    /// 1.12.2旧版依赖解析：`modid@版本范围`，不带 `@` 时整串就是 modid
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
            range_checker::parse_version_range(range_str).unwrap_or(VersionRange::Any)
        };

        DependantInfo {
            modid,
            version: range,
        }
    }

    /// 新版依赖解析
    pub fn parse_required_version(modid: &str, version: &str) -> DependantInfo {
        let version_range = if version.is_empty() {
            VersionRange::Any
        } else {
            range_checker::parse_version_range(version).unwrap_or(VersionRange::Any)
        };

        DependantInfo {
            modid: modid.to_string(),
            version: version_range,
        }
    }

    /// 版本列表解析
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

impl Default for ModItemObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            mod_id: Default::default(),
            name: Default::default(),
            description: Default::default(),
            version: Default::default(),
            author: Default::default(),
            dependants: Default::default(),
            loaders: Default::default(),
            side: Default::default(),
            url: Default::default(),
            icon: Default::default(),
        }
    }
}

/// 模组目录里的一个文件及其解析结果
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

impl Default for ModObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            uuid: Default::default(),
            info: Default::default(),
            disable: Default::default(),
            core: Default::default(),
            hash: Default::default(),
            jar_in_jar: Default::default(),
            fail: Default::default(),
            file: Default::default(),
        }
    }
}

/// 字符串引号状态
#[derive(PartialEq)]
enum Quote {
    /// 不在字符串内
    None,
    /// 双引号字符串
    Double,
    /// 单引号字符串
    Single,
}

/// 容错处理 mcmod.info 中常见的非法 JSON
///
/// - 单引号字符串：`'value'` → `"value"`
/// - 数组内未加引号的标识符：`[mod_minecraftForge]` → `["mod_minecraftForge"]`
/// - 字符串内未转义的控制字符：换行符 → `\n`，回车符 → `\r`，制表符 → `\t`
///
/// - `json`: 原始 json 文本
///
/// # 返回值
///
/// 返回容错处理后的 json 文本
fn sanitize_mcmod_json(json: &str) -> String {
    let mut result = String::with_capacity(json.len() + 64);
    let chars: Vec<char> = json.chars().collect();
    let mut i = 0;
    let mut quote = Quote::None;
    let mut escape = false;

    while i < chars.len() {
        let ch = chars[i];

        // 处理转义模式
        if escape {
            escape = false;
            match quote {
                Quote::Single => match ch {
                    '\'' => result.push('\''),       // \' → '
                    '"' => result.push_str("\\\""),  // \" → \"
                    '\\' => result.push_str("\\\\"), // \\ → \\
                    'n' => result.push_str("\\n"),
                    'r' => result.push_str("\\r"),
                    't' => result.push_str("\\t"),
                    '/' => result.push_str("\\/"),
                    other => {
                        result.push_str("\\\\");
                        result.push(other);
                    }
                },
                Quote::Double | Quote::None => {
                    result.push(ch);
                }
            }
            i += 1;
            continue;
        }

        // 反斜杠进入转义模式
        if ch == '\\' {
            match quote {
                Quote::Double => {
                    escape = true;
                    result.push(ch); // 双引号内的转义已合法，直接保留
                }
                Quote::Single => {
                    escape = true; // 单引号内需要转换，不先推入 \
                }
                Quote::None => {
                    result.push(ch);
                }
            }
            i += 1;
            continue;
        }

        // 双引号
        if ch == '"' {
            match quote {
                Quote::Double => {
                    quote = Quote::None;
                    result.push(ch);
                }
                Quote::Single => {
                    // 单引号字符串内的双引号 → 转义
                    result.push_str("\\\"");
                }
                Quote::None => {
                    quote = Quote::Double;
                    result.push(ch);
                }
            }
            i += 1;
            continue;
        }

        // 单引号 → 统一转换为双引号
        if ch == '\'' {
            match quote {
                Quote::Single => {
                    quote = Quote::None;
                    result.push('"');
                }
                Quote::Double => {
                    result.push(ch); // 双引号内的单引号是普通字符
                }
                Quote::None => {
                    quote = Quote::Single;
                    result.push('"');
                }
            }
            i += 1;
            continue;
        }

        // 字符串内未转义的控制字符
        if quote != Quote::None {
            match ch {
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                other => result.push(other),
            }
            i += 1;
            continue;
        }

        // 仅在字符串外部处理数组内裸标识符
        if ch == '[' || ch == ',' {
            result.push(ch);
            i += 1;

            // 跳过空白
            while i < chars.len() && chars[i].is_whitespace() {
                result.push(chars[i]);
                i += 1;
            }

            // 检测裸标识符，即以字母或下划线开头
            if i < chars.len() && (chars[i].is_alphabetic() || chars[i] == '_') {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();

                // 不引用 JSON 字面量
                if matches!(word.as_str(), "true" | "false" | "null") || word.parse::<f64>().is_ok()
                {
                    result.push_str(&word);
                } else {
                    result.push('"');
                    result.push_str(&word);
                    result.push('"');
                }
            }

            continue;
        }

        result.push(ch);
        i += 1;
    }

    result
}

/// 读取 Forge 的 mcmod.info
///
/// - `reader`: mcmod.info 文件流
/// - `mod_info`: 模组信息，解析结果追加到其中
///
/// # 返回值
///
/// 成功返回 `Ok(())`；读取或解析失败返回对应错误
fn read_forge_json(mut reader: impl Read, mod_info: &mut ModObj) -> CoreResult<()> {
    let mut json = String::new();
    reader.read_to_string(&mut json).map_err(|err| {
        ErrorType::ArchiveReadError(ErrorData {
            error: err.to_string(),
        })
    })?;

    let obj = match MiniJsonObj::from_str(&json) {
        Ok(obj) => obj,
        Err(_) => {
            // 尝试容错处理：修复数组内未加引号的标识符
            let sanitized = sanitize_mcmod_json(&json);
            MiniJsonObj::from_str(&sanitized)?
        }
    };

    let values = if obj.is_list() {
        obj.as_list()
    } else if let Some(obj) = obj.as_object() {
        obj.get_list("modList")
    } else {
        None
    };

    if let Some(values) = values {
        mod_info.info.extend(values.iter().map(|v| {
            let mut info = ModItemObj::default();

            if let Some(map) = v.as_object() {
                info.mod_id = map.get_string("modid");
                info.name = map.get_opt_string("name").unwrap_or(info.mod_id.clone());
                info.description = map.get_opt_string("description");
                info.version = map.get_opt_string("version");
                info.url = map.get_opt_string("url");
                info.loaders = LoaderType::Forge;

                info.author = map.extract_strings("authorList");
                info.dependants
                    .extend(map.extract_strings("dependants").iter().map(|item| {
                        DependantType::Recommend(DependantInfo::parse_required_name(item))
                    }));
                info.dependants
                    .extend(map.extract_strings("dependencies").iter().map(|item| {
                        DependantType::Recommend(DependantInfo::parse_required_name(item))
                    }));
                info.dependants.extend(
                    map.extract_strings("requiredMods").iter().map(|item| {
                        DependantType::Required(DependantInfo::parse_required_name(item))
                    }),
                );
            }

            info
        }));
    }

    Ok(())
}

/// 读取 Forge / NeoForge 的 mods.toml
///
/// - `reader`: mods.toml 文件流
/// - `loader`: 加载器类型，决定写进条目的 `loaders`
/// - `mod_info`: 模组信息，解析结果追加到其中
///
/// # 返回值
///
/// 成功返回 `Ok(())`；读取或解析失败返回对应错误
fn read_forge_toml(
    mut reader: impl Read,
    loader: LoaderType,
    mod_info: &mut ModObj,
) -> CoreResult<()> {
    let obj = MiniTomlMap::from_stream(&mut reader)?;

    // 读取 mods
    if let Some(values) = obj.get_list("mods") {
        for item in values.iter() {
            let mut info = ModItemObj::default();

            info.mod_id = item
                .get_opt_string("modid")
                .or(item.get_opt_string("modId"))
                .unwrap_or_default();
            info.name = item
                .get_opt_string("displayName")
                .unwrap_or(info.mod_id.clone());
            info.description = item.get_opt_string("description");
            info.version = item.get_opt_string("version");
            info.url = item.get_opt_string("displayURL");
            info.loaders = loader;

            let authors = |key: &str| -> Vec<String> {
                item.get_opt_string(key)
                    .map(|s| s.split(',').map(String::from).collect())
                    .unwrap_or_default()
            };

            info.author.extend(authors("authorList"));
            info.author.extend(authors("authors"));

            mod_info.info.push(info);
        }
    }

    // 处理依赖关系
    if let Some(table) = obj.get_object("dependencies") {
        for (key, value) in table.iter() {
            let Some(map) = value.as_object() else {
                continue;
            };

            let key_str = key.to_string();
            let Some(mod_item) = mod_info
                .info
                .iter_mut()
                .find(|item| item.mod_id.eq_ignore_ascii_case(&key_str))
            else {
                continue;
            };

            if let Some(modid) = map.get_opt_string("modid").or(map.get_opt_string("modId")) {
                if modid.eq_ignore_ascii_case("minecraft") {
                    if let Some(side) = map.get_opt_string("side") {
                        mod_item.side = match side.to_ascii_lowercase().as_str() {
                            "both" => LoadSideType::Both,
                            "client" => LoadSideType::Client,
                            "server" => LoadSideType::Server,
                            _ => mod_item.side,
                        };
                    }
                    if let Some(version) = map.get_opt_string("versionRange") {
                        mod_item.dependants.push(DependantType::Required(
                            DependantInfo::parse_required_version(&modid, &version),
                        ));
                    }
                } else {
                    let is_mandatory = map.get_bool("mandatory");
                    let is_required = map
                        .get_opt_string("type")
                        .map(|s| s.eq_ignore_ascii_case("required"))
                        .unwrap_or(false);
                    let version = map.get_opt_string("versionRange").unwrap_or_default();

                    let dep_type = if is_required || !is_mandatory {
                        DependantType::Required(DependantInfo::parse_required_version(
                            &modid, &version,
                        ))
                    } else {
                        DependantType::Recommend(DependantInfo::parse_required_version(
                            &modid, &version,
                        ))
                    };

                    mod_item.dependants.push(dep_type);
                }
            }
        }
    }

    Ok(())
}

/// 读取 Fabric 的 fabric.mod.json
///
/// - `reader`: fabric.mod.json 文件流
/// - `mod_info`: 模组信息，解析结果追加到其中
///
/// # 返回值
///
/// 成功返回 `Ok(())`；读取或解析失败返回对应错误
fn read_fabric_json(reader: impl Read, mod_info: &mut ModObj) -> CoreResult<()> {
    let obj = MiniJsonObj::from_stream(reader)?;

    let mut info = ModItemObj::default();
    // 加载器表示"这份元数据是哪个加载器的"，与下面各字段解析成功与否无关 ——
    // 漏了这一句就会留在默认值 `Normal`，界面上把 Fabric 模组显示成「原版」
    info.loaders = LoaderType::Fabric;

    if let Some(map) = obj.as_object() {
        info.mod_id = map.get_string("id");
        info.name = map.get_string("name");
        info.description = map.get_opt_string("description");
        info.version = map.get_opt_string("version");
        if let Some(map1) = map.get_object("contact") {
            info.url = map1.get_opt_string("homepage");
        }

        if let Some(str) = map.get_opt_string("environment") {
            info.side = if str.eq_ignore_ascii_case("client") {
                LoadSideType::Client
            } else if str.eq_ignore_ascii_case("server") {
                LoadSideType::Server
            } else if str.eq_ignore_ascii_case("*") {
                LoadSideType::Both
            } else {
                LoadSideType::Unknown
            };
        }

        if let Some(list) = map.get_list("authors") {
            for item in list.iter() {
                if item.is_str() {
                    info.author.push(item.as_string().unwrap());
                } else if item.is_obj()
                    && let Some(value) = item
                        .as_object()
                        .and_then(|item| item.get_opt_string("name"))
                {
                    info.author.push(value);
                }
            }
        }

        if let Some(str) = map.get_object("depends") {
            for (key, version) in str.iter() {
                info.dependants.push(DependantType::Required(
                    DependantInfo::parse_required_version(
                        key,
                        &version.as_string().unwrap_or_default(),
                    ),
                ));
            }
        }

        if let Some(str) = map.get_object("suggests") {
            for (key, version) in str.iter() {
                info.dependants.push(DependantType::Recommend(
                    DependantInfo::parse_required_version(
                        key,
                        &version.as_string().unwrap_or_default(),
                    ),
                ));
            }
        }
    }

    mod_info.info.push(info);

    Ok(())
}

/// 读取 Quilt 的 quilt.mod.json
///
/// - `reader`: quilt.mod.json 文件流
/// - `mod_info`: 模组信息，解析结果追加到其中
///
/// # 返回值
///
/// 成功返回 `Ok(())`；读取或解析失败返回对应错误
fn read_quilt_json(reader: impl Read, mod_info: &mut ModObj) -> CoreResult<()> {
    let obj = MiniJsonObj::from_stream(reader)?;

    let mut info = ModItemObj::default();
    // 同上：不写这一句就会显示成「原版」，Quilt 与 Fabric 是同一个毛病
    info.loaders = LoaderType::Quilt;

    if let Some(map) = obj
        .as_object()
        .and_then(|item| item.get_object("quilt_loader"))
    {
        info.mod_id = map.get_string("id");
        info.version = map.get_opt_string("version");
        if let Some(map) = map.get_object("metadata") {
            if let Some(map1) = map.get_object("contact") {
                info.url = map1.get_opt_string("homepage");
            }

            info.name = map.get_string("name");
            info.description = map.get_opt_string("description");

            if let Some(map1) = map.get_object("contributors") {
                for (item, _) in map1.iter() {
                    info.author.push(item.to_string());
                }
            }
        }

        if let Some(list) = map.get_object("depends") {
            for (_, value) in list.iter() {
                let Some(obj) = value.as_object() else {
                    continue;
                };

                let str = obj.get_string("id");
                if let Some(versions) = obj.get("versions") {
                    if versions.is_str() {
                        info.dependants.push(DependantType::Required(
                            DependantInfo::parse_required_version(
                                &str,
                                &versions.as_string().unwrap_or_default(),
                            ),
                        ));
                    } else if versions.is_list() {
                        info.dependants.push(DependantType::Required(
                            DependantInfo::parse_required_json(&str, versions),
                        ));
                    } else {
                        info.dependants.push(DependantType::Required(DependantInfo {
                            modid: str,
                            version: VersionRange::Any,
                        }));
                    }
                } else {
                    info.dependants.push(DependantType::Required(DependantInfo {
                        modid: str,
                        version: VersionRange::Any,
                    }));
                }
            }
        }
    }

    mod_info.info.push(info);

    Ok(())
}

/// 解析 MANIFEST.MF 内容
///
/// - `content`: MANIFEST.MF 文本
///
/// # 返回值
///
/// 返回键值对映射，支持以空格开头的续行
fn parse_manifest(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current_key = String::new();
    let mut current_value = String::new();

    for line in content.lines() {
        if line.is_empty() {
            continue;
        }

        if line.starts_with(' ') {
            // 续行
            current_value.push_str(line.trim());
        } else if let Some((key, value)) = line.split_once(':') {
            if !current_key.is_empty() {
                map.insert(current_key, current_value.trim().to_string());
            }
            current_key = key.trim().to_string();
            current_value = value.trim().to_string();
        }
    }

    if !current_key.is_empty() {
        map.insert(current_key, current_value.trim().to_string());
    }
    map
}

/// 读取 core mod 信息，靠 MANIFEST.MF 识别
///
/// - `archive`: 模组压缩包
/// - `mod_info`: 模组信息，识别出 core mod 时追加一条
///
/// # 返回值
///
/// 成功返回 `Ok(())`；读取失败返回对应错误，没有 MANIFEST.MF 时直接成功
fn read_core_mod(
    archive: &mut ZipArchive<impl Read + Seek>,
    mod_info: &mut ModObj,
) -> CoreResult<()> {
    // 读取 META-INF/MANIFEST.MF
    let mut manifest_file = match archive.by_name("META-INF/MANIFEST.MF") {
        Ok(file) => file,
        Err(_) => return Ok(()),
    };

    let mut info = ModItemObj::default();

    let mut content = String::new();
    manifest_file.read_to_string(&mut content).map_err(|err| {
        ErrorType::ArchiveReadError(ErrorData {
            error: err.to_string(),
        })
    })?;

    let manifest = parse_manifest(&content);

    // 只有真的识别出 core mod 标记才算 core mod
    let mut is_core = false;

    // 检查 FMLCorePlugin，即 Forge core mod 主类
    if let Some(core_plugin) = manifest.get("FMLCorePlugin") {
        info.mod_id = core_plugin.clone();
        info.name = core_plugin
            .rsplit('.')
            .next()
            .unwrap_or(core_plugin)
            .to_string();
        info.loaders = LoaderType::Forge;
        is_core = true;
    }

    // 检查 TweakClass，即 LaunchWrapper 注入类
    if let Some(tweak_class) = manifest.get("TweakClass") {
        if info.mod_id.is_empty() {
            info.mod_id = tweak_class.clone();
            info.name = tweak_class
                .rsplit('.')
                .next()
                .unwrap_or(tweak_class)
                .to_string();
        }
        info.loaders = LoaderType::Forge;
        is_core = true;
    }

    // **没识别出 core mod 就不要 push 这条 info**：`META-INF/MANIFEST.MF` 几乎每个 jar 都有，
    // 无条件 push 会多出一条加载器为默认值 `Normal` 的空条目，界面上每个模组都会多出「原版」
    if is_core {
        mod_info.info.push(info);
    }
    // 原先这里无条件置 true，而 MANIFEST.MF 几乎每个 Forge / NeoForge jar 都有 ——
    // 结果界面上**每个**模组都挂着"核心"徽标。core 只表示"用老式 core mod 机制加载"，
    // 没有上面两个标记就不是
    mod_info.core = is_core;

    Ok(())
}

/// 扫描内置的模组，即 jar-in-jar
///
/// **会递归**：内置 jar 里还能再套内置 jar，`parse_mod_archive` 又会调回这里，层数不限。
///
/// - `archive`: 当前这一层的压缩包
/// - `mod_info`: 解析结果追加到其中
/// - `base`: **当前层在父包里的路径链**，顶层为空；内置 jar 没有独立文件路径，
///   得靠它拼出唯一身份
///
/// # 返回值
///
/// 成功返回 `Ok(())`；读取失败返回对应错误
fn read_jar_in_jar(
    archive: &mut ZipArchive<impl Read + Seek>,
    mod_info: &mut ModObj,
    base: &Path,
) -> CoreResult<()> {
    // 收集所有 META-INF/jarjar/ 目录下的 .jar 文件
    let jar_entries: Vec<usize> = (0..archive.len())
        .filter_map(|i| {
            archive.by_index(i).ok().and_then(|entry| {
                let name = entry.name().unwrap_or_default();
                if name.ends_with(names::JAR_DOT_EXT)
                    && (name.starts_with(names::MOD_JAR_JAR_DIR)
                        || name.starts_with(names::MOD_JARS_DIR))
                {
                    Some(i)
                } else {
                    None
                }
            })
        })
        .collect();

    for idx in jar_entries {
        let mut entry = archive.by_index(idx).map_err(|err| {
            ErrorType::ArchiveReadError(ErrorData {
                error: err.to_string(),
            })
        })?;
        // 条目名要留着当身份用，先把它的借用结束掉再读内容
        let entry_name = entry.name().unwrap_or_default().to_string();

        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|err| {
            ErrorType::ArchiveReadError(ErrorData {
                error: err.to_string(),
            })
        })?;

        let cursor = Cursor::new(bytes);
        let mut inner_zip = ZipArchive::new(cursor).map_err(|err| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: PathBuf::new(),
                error: err.to_string(),
            })
        })?;

        // 这一层在父包里的路径链：顶层 base 为空 → 就是条目名本身；
        // 再往下一层就是"父条目名/子条目名"，层层累加
        let rel = base.join(&entry_name);

        match parse_mod_archive(&mut inner_zip, &rel) {
            Ok(mut inmod) => {
                // 内置 jar 没有独立文件路径，`file` 存条目名、`uuid` 由路径链生成；
                // 留默认值会让同层内置 jar 的渲染 key 全一样，展开一个就把同层全展开
                inmod.file = PathBuf::from(&entry_name);
                inmod.uuid = gen_mod_uuid(&rel);
                mod_info.jar_in_jar.push(inmod);
            }
            Err(err) => {
                mml_log::error_type(err);
            }
        }
    }

    Ok(())
}

/// 从任意可读的 ZIP 归档中解析模组信息，核心解析逻辑
///
/// - `archive`: 压缩包
/// - `base`: 这个包在**父包里的路径链**，顶层传文件路径，内置 jar 传"父条目名/子条目名"；
///   只用来给内置 jar 生成唯一身份，见 [`read_jar_in_jar`]
///
/// # 返回值
///
/// 返回解析出的模组信息；读取或解析失败返回对应错误
fn parse_mod_archive(
    archive: &mut ZipArchive<impl Read + Seek>,
    base: &Path,
) -> CoreResult<ModObj> {
    let mut mod_info = ModObj::default();

    // mcmod.info 与下面的 mods.toml 同一口径：坏文件只跳过它自己，不牵连整包；
    // 老包的 mcmod.info 手改坏、编码不对的情况并不少见
    if let Ok(item) = archive.by_name(names::MC_MOD_INFO_FILE)
        && let Err(err) = read_forge_json(item, &mut mod_info)
    {
        mml_log::error_type(err);
    }

    // **单个元数据文件坏了不该让整包失败**：真实案例 `mekanism_lasers-1.1.10.3-a.jar`
    // 里两个 toml 坏了一个，旧实现用 `?` 抛出后整包变成"读取失败"、连模组名都看不到
    macro_rules! try_read_file {
        ($archive:expr, $name:expr, $func:expr, $loader:expr) => {
            if let Ok(item) = $archive.by_name($name) {
                if let Err(err) = $func(item, $loader, &mut mod_info) {
                    mml_log::error_type(err);
                }
            }
        };
    }

    try_read_file!(
        archive,
        names::MC_MOD_TOML_FILE,
        read_forge_toml,
        LoaderType::Forge
    );
    try_read_file!(
        archive,
        names::NEO_TOML_FILE,
        read_forge_toml,
        LoaderType::NeoForge
    );
    try_read_file!(
        archive,
        names::NEO_TOML1_FILE,
        read_forge_toml,
        LoaderType::NeoForge
    );

    // 读取 fabric.mod.json / quilt.mod.json，同样是"坏了只跳过它自己"
    if let Ok(item) = archive.by_name(names::FABRIC_MOD_FILE)
        && let Err(err) = read_fabric_json(item, &mut mod_info)
    {
        mml_log::error_type(err);
    }

    if let Ok(item) = archive.by_name(names::QUILT_MOD_FILE)
        && let Err(err) = read_quilt_json(item, &mut mod_info)
    {
        mml_log::error_type(err);
    }

    // 扫描coremod
    read_core_mod(archive, &mut mod_info)?;

    // 扫描 jar-in-jar，会递归：内置 jar 里还能再套内置 jar
    read_jar_in_jar(archive, &mut mod_info, base)?;

    // 读图标：放在最后是因为图标要挂在已经解析出的第一条 info 上
    read_mod_icon(archive, &mut mod_info);

    Ok(mod_info)
}

/// 图标体积上限，单位字节；图标只是装饰，有人塞过整张高清图，不值得读进内存
const MAX_ICON_SIZE: u64 = 512 * 1024;

/// 读模组图标
///
/// 各加载器把图标路径放在不同字段里，见 [`find_mod_icon_path`]；这里再按条目名把
/// **图片字节**读出来，`ModItemObj::icon` 存的是字节，界面那边转 data URL。
///
/// - `archive`: 模组压缩包
/// - `mod_info`: 模组信息，图标写到第一条 info 上
fn read_mod_icon(archive: &mut ZipArchive<impl Read + Seek>, mod_info: &mut ModObj) {
    let Some(path) = find_mod_icon_path(archive) else {
        return;
    };

    let Ok(mut entry) = archive.by_name(&path) else {
        return;
    };
    if entry.size() == 0 || entry.size() > MAX_ICON_SIZE {
        return;
    }

    let mut data = Vec::with_capacity(entry.size() as usize);
    if entry.read_to_end(&mut data).is_err() || data.is_empty() {
        return;
    }

    match mod_info.info.first_mut() {
        Some(info) => info.icon = Some(data),
        // 只认得出图标、没有别的元数据的包：补一条，别把图标丢了
        None => mod_info.info.push(ModItemObj {
            icon: Some(data),
            ..Default::default()
        }),
    }
}

/// 找出图标在包里的条目名
///
/// 支持：Forge / NeoForge 的 `mods.toml` `logoFile`、1.12 及更早 Forge 老包的
/// `mcmod.info` `logoFile`、Fabric / Quilt 的 `icon`，后者是字符串或按尺寸分的
/// 对象如 `{"64": "…png"}`。
///
/// - `archive`: 模组压缩包
///
/// # 返回值
///
/// 返回包内条目名；元数据缺失或没写图标时返回 `None`
fn find_mod_icon_path(archive: &mut ZipArchive<impl Read + Seek>) -> Option<String> {
    // Forge / NeoForge：mods.toml 的 logoFile
    for name in [
        names::MC_MOD_TOML_FILE,
        names::NEO_TOML_FILE,
        names::NEO_TOML1_FILE,
    ] {
        let Ok(mut file) = archive.by_name(name) else {
            continue;
        };
        let mut text = String::new();
        if file.read_to_string(&mut text).is_err() {
            continue;
        }
        if let Some(path) = forge_logo_file(&text) {
            return Some(path);
        }
    }

    // 1.12 及更早的 Forge 老包：mcmod.info 的 logoFile，这类包没有 mods.toml
    if let Some(path) = mcmod_logo_file(archive) {
        return Some(path);
    }

    // Fabric / Quilt：JSON 的 icon，quilt 在 quilt_loader.metadata.icon 下
    for name in [names::FABRIC_MOD_FILE, names::QUILT_MOD_FILE] {
        let Ok(file) = archive.by_name(name) else {
            continue;
        };
        let Ok(obj) = MiniJsonObj::from_stream(file) else {
            continue;
        };
        let Some(root) = obj.as_object() else {
            continue;
        };
        let holder = if name == names::QUILT_MOD_FILE {
            root.get_object("quilt_loader")
                .and_then(|loader| loader.get_object("metadata"))
        } else {
            Some(root)
        };
        let Some(holder) = holder else {
            continue;
        };

        // 字符串形式
        if let Some(value) = holder.get_opt_string("icon") {
            return Some(value);
        }
        // 对象形式按尺寸分档，取第一个非空值够用
        if let Some(map) = holder.get_object("icon") {
            for (_, value) in map.iter() {
                if let Some(value) = value.as_string() {
                    return Some(value);
                }
            }
        }
    }

    None
}

/// 从 mods.toml 文本里取未被注释的 `logoFile`
///
/// Forge 的 mods.toml 模板里 `logoFile` 基本都注释掉了，如 `#logoFile="examplemod.png"`；
/// 照读会去开不存在的条目，所以跳过注释行并去掉行尾注释。
fn forge_logo_file(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("logoFile") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        // 去掉引号与行内注释，如 `logoFile="a.png" # 图标`
        let value = rest.split('#').next().unwrap_or(rest).trim();
        let value = value.trim_matches(|c| c == '"' || c == '\'').trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

/// 从 `mcmod.info` 里取 `logoFile`，只认 1.12 及更早的 Forge 老包
///
/// - `archive`: 模组压缩包
///
/// # 返回值
///
/// 返回包内条目名；没有 `mcmod.info`、读取失败或谁都没写 `logoFile` 时返回 `None`
fn mcmod_logo_file(archive: &mut ZipArchive<impl Read + Seek>) -> Option<String> {
    let mut file = archive.by_name(names::MC_MOD_INFO_FILE).ok()?;
    let mut json = String::new();
    file.read_to_string(&mut json).ok()?;

    mcmod_logo_file_in(&json)
}

/// 从 `mcmod.info` 的**文本**里取 `logoFile`，即 [`mcmod_logo_file`] 的纯文本部分
///
/// 形态与 [`read_forge_json`] 一致：顶层是数组，或带 `modList` 数组的对象。
/// 逐条看 `logoFile`，取第一个非空的 —— 图标是装饰性的，多条目包认第一个就够。
///
/// 容错也照 [`read_forge_json`] 那条路：先直接解析，失败再用 [`sanitize_mcmod_json`]
/// 修一遍单引号 / 数组里未加引号的裸标识符。
fn mcmod_logo_file_in(json: &str) -> Option<String> {
    let obj = match MiniJsonObj::from_str(json) {
        Ok(obj) => obj,
        Err(_) => MiniJsonObj::from_str(&sanitize_mcmod_json(json)).ok()?,
    };

    let values = if obj.is_list() {
        obj.as_list()
    } else {
        obj.as_object().and_then(|map| map.get_list("modList"))
    }?;

    for value in values.iter() {
        let Some(map) = value.as_object() else {
            continue;
        };
        let Some(path) = map.get_opt_string("logoFile") else {
            continue;
        };
        let path = path.trim();
        if !path.is_empty() {
            return Some(path.to_string());
        }
    }

    None
}

/// 读取模组信息
///
/// - `path`: 模组文件路径
///
/// # 返回值
///
/// 返回解析出的模组信息；打开或解析失败返回对应错误
pub fn read_mod_info<P: AsRef<Path>>(path: P) -> CoreResult<ModObj> {
    let file = path_helper::open_read(&path)?;
    let mut zip = ZipArchive::new(file).map_err(|err| {
        ErrorType::ArchiveOpenError(FileSystemErrorData {
            path: path.as_ref().to_path_buf(),
            error: err.to_string(),
        })
    })?;

    // base 传文件路径本身：内置 jar 的路径链就是"文件路径/父条目名/子条目名"，
    // 这样不同模组里的同名内置 jar 也能区分开
    let mut mod_info = parse_mod_archive(&mut zip, path.as_ref())?;
    mod_info.file = path.as_ref().to_path_buf();

    // 从注解扫描 side，仅文件类模组可用
    if let Ok(scan_result) = class_scan::scan_jar(path.as_ref()) {
        for scan_mod in &scan_result.mods {
            if let Some(info) = mod_info
                .info
                .iter_mut()
                .find(|info| info.mod_id == scan_mod.modid)
            {
                info.side = scan_mod.side;
            }
        }
    }

    Ok(mod_info)
}

/// 读模组
///
/// - `path`: 模组文件路径
/// - `sha256`: 是否连 SHA256 一起算，否则只算 SHA1
///
/// # 返回值
///
/// 返回模组信息，含哈希；读取失败返回对应错误
fn read_mod<P: AsRef<Path>>(path: P, sha256: bool) -> CoreResult<ModObj> {
    let sha1 = hash_helper::gen_hash_from_file(HashType::Sha1, path.as_ref())?;

    let hash = if sha256 {
        let sha256 = hash_helper::gen_hash_from_file(HashType::Sha256, path.as_ref())?;
        FileHash::Sha1Sha256(sha1, sha256)
    } else {
        FileHash::Sha1(sha1)
    };

    let mut mod_info = read_mod_info(path)?;
    mod_info.hash = hash;

    Ok(mod_info)
}

/// 模组 uuid 的命名空间，固定值，保证不同启动器数据目录下生成的 uuid 一致
const MOD_UUID_NAMESPACE: Uuid = Uuid::from_u128(0x9d1a_2f3e_4c5b_6a70_8192_a3b4_c5d6_e7f8);

/// 模组的稳定标识：文件完整路径的 uuid v5
///
/// 同一文件每次扫描得到同一个 uuid，启用 / 禁用 / 删除按它定位
///
/// - `path`: 模组文件完整路径
///
/// # 返回值
///
/// 返回模组的稳定标识
///
/// **注意文件名一变 uuid 就变**，它是路径的哈希；启用 / 禁用只是给文件名加减后缀，
/// 所以那两个操作之后 uuid 会变成另一个值，需要同步身份的地方都靠这个函数重算。
pub fn gen_mod_uuid(path: &Path) -> Uuid {
    Uuid::new_v5(&MOD_UUID_NAMESPACE, path.to_string_lossy().as_bytes())
}

/// 扫描文件列表
///
/// - `files`: 文件列表
/// - `process_fn`: 单个文件的处理函数
/// - `gui`: 进度回调，可传 `None`，见 [`ProgressGui`]；每完成一个文件报一次 `(已完成, 总数)`
///
/// # 返回值
///
/// 返回扫描出的模组列表，读取失败的文件标记 `fail`
fn scan_mod_files<F>(files: Vec<PathBuf>, process_fn: F, gui: ProgressGui) -> Vec<ModObj>
where
    F: Fn(&PathBuf) -> CoreResult<ModObj> + Send + Sync,
{
    let list = Mutex::new(Vec::new());

    // 总数只算**真正要处理的** .jar / .disable / .disabled：
    // 目录里可能有 .txt、.DS_Store 之类，拿文件总数当中会导致进度永远到不了 x/x
    let total = files
        .iter()
        .filter(|item| {
            item.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case(names::JAR_EXT)
                    || ext.eq_ignore_ascii_case(names::DISABLE_EXT)
                    || ext.eq_ignore_ascii_case(names::DISABLED_EXT)
            })
        })
        .count();
    // 并行完成数，rayon 里各线程都会加
    let done = AtomicUsize::new(0);

    files.par_iter().for_each(|item| {
        if let Some(ext) = item.extension() {
            let is_jar = ext.eq_ignore_ascii_case(names::JAR_EXT);
            let is_disabled = ext.eq_ignore_ascii_case(names::DISABLE_EXT)
                || ext.eq_ignore_ascii_case(names::DISABLED_EXT);

            if is_jar || is_disabled {
                let disable = is_disabled;
                let result = process_fn(item);

                let mut entry = match result {
                    Ok(mut item) => {
                        item.disable = disable;
                        item
                    }
                    Err(err) => {
                        mml_log::error_type(err);
                        ModObj {
                            fail: true,
                            file: item.clone(),
                            ..Default::default()
                        }
                    }
                };
                entry.uuid = gen_mod_uuid(item);

                list.lock().unwrap().push(entry);

                if let Some(gui) = &gui {
                    // 完成一个报一次，顺序不定，界面只关心计数
                    gui.set_progress_now(done.fetch_add(1, Ordering::Relaxed) + 1, Some(total));
                }
            }
        }
    });

    // 收尾对齐到 x/x：总数只统计了要处理的文件，正常已经在循环里报满；
    // 万一过滤与实际处理有出入，这里保证进度不会停在 x-1/x
    if let Some(gui) = &gui {
        gui.set_progress_now(total, Some(total));
    }

    list.into_inner().unwrap()
}

/// 为文件追加禁用后缀
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
    new_name.push(names::DISABLE_DOT_EXT);
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

fn get_jar_in_jar_list(item: &ModObj, deps: &mut HashMap<String, String>) {
    for item in item.jar_in_jar.iter() {
        for item in item.info.iter() {
            deps.insert(
                item.mod_id.clone(),
                item.version.clone().unwrap_or_default(),
            );
        }

        for item in item.jar_in_jar.iter() {
            get_jar_in_jar_list(item, deps);
        }
    }
}

/// 依赖检测发现的一条问题
#[derive(Debug, Clone)]
pub struct DependantProblem {
    /// 出问题的前置 modid
    pub modid: String,
    /// 问题种类
    pub kind: DependantProblemKind,
    /// 要求的版本范围；`Duplicate` 这一类不带要求，是 [`VersionRange::Any`]
    pub required: VersionRange,
    /// 已装着的版本，多个用 `, ` 连接；`Missing` 这一类为空串
    pub installed: String,
    /// 有哪些模组要求它，取模组的显示名；`Duplicate` 这一类为空
    pub required_by: Vec<String>,
    /// `Duplicate` 那一类里是哪几个文件；其余为空
    pub files: Vec<String>,
}

/// 依赖检测发现的问题种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependantProblemKind {
    /// 缺失：装着的模组里没有这个前置
    Missing,
    /// 版本不符：装着了，但没有一个版本落在要求的范围内
    Mismatch,
    /// 前置被禁用：只有禁用状态的包提供它
    Disabled,
    /// 重复：同一个 modid 有两份及以上处于启用状态的顶层文件
    Duplicate,
}

/// 依赖检测的结果
#[derive(Debug, Clone)]
pub struct DependantTest {
    /// 检测到的问题，按种类与 modid 排序；没有问题时为空
    pub problems: Vec<DependantProblem>,
}

/// 模组的显示名：优先元数据里的名字，退回 modid，再退回文件名
fn mod_display_name(item: &ModObj) -> String {
    item.info
        .iter()
        .map(|info| {
            if !info.name.is_empty() {
                info.name.clone()
            } else {
                info.mod_id.clone()
            }
        })
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| mod_file_name(item))
}

/// 模组条目的文件名，不含目录
fn mod_file_name(item: &ModObj) -> String {
    item.file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 该前置是否由加载器或游戏自身提供，如 Forge 实例里的 `forge` / `minecraft`
///
/// 不排掉这些，每个模组都会报一串"缺 minecraft"。
/// 注意 `fabric` 不排：Fabric API 自己的 modid 就是它，正是最该报出来的那个前置。
fn is_loader_provided(game: &InstanceSettingObj, modid: &str) -> bool {
    let id = modid.to_ascii_lowercase();
    match id.as_str() {
        "minecraft" | "java" | "mcp" | "fml" => true,
        _ => match game.loader {
            LoaderType::Forge => matches!(id.as_str(), "forge" | "minecraftforge"),
            LoaderType::NeoForge => matches!(id.as_str(), "neoforge" | "forge"),
            LoaderType::Fabric => id == "fabricloader",
            LoaderType::Quilt => matches!(id.as_str(), "quilt_loader" | "quilt_base" | "fabricloader"),
            LoaderType::OptiFine => id == "optifine",
            LoaderType::LiteLoader => id == "liteloader",
            LoaderType::Normal | LoaderType::Custom => false,
        },
    }
}

/// 模组依赖检测：按各包声明的强制前置，检查是否齐全、版本是否满足
///
/// 口径：
/// - 只看**启用状态**的顶层模组：禁用包声明的依赖不算，禁用的包也不能顶上依赖；
/// - 内置 jar-in-jar 里的模组算"已提供"，Forge / NeoForge 会把它们加载起来，但不参与"重复"判定；
/// - 加载器与游戏自身提供的前置跳过，见 [`is_loader_provided`]；
/// - 版本范围为空或 `*` 时只查有没有，不比版本；装着的包没写版本号时同样不判版本不符；
/// - 同一前置被多个模组要求时合并成一条，版本范围不同则各算一条；
/// - 推荐安装的前置不报。
///
/// - `game`: 实例设置，只用来判断哪些前置由加载器提供
/// - `mods`: 已经扫描出的模组列表，见 `InstanceSettingObj::read_mod`
///
/// # 返回值
///
/// 见 [`DependantTest`]
pub fn mods_dep_test(game: &InstanceSettingObj, mods: &[ModObj]) -> CoreResult<DependantTest> {
    // 装着的：modid → 各条元数据的（版本，是否启用）
    let mut installed: HashMap<String, Vec<(String, bool)>> = HashMap::new();
    // 内置模组：modid → 版本
    let mut bundled: HashMap<String, String> = HashMap::new();
    // 启用的顶层模组：modid → （文件名，版本），用来找"同一个包装了两份"
    let mut top: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for item in mods.iter().filter(|item| !item.fail) {
        for info in item.info.iter() {
            if info.mod_id.is_empty() {
                continue;
            }
            let version = info.version.clone().unwrap_or_default();
            installed
                .entry(info.mod_id.clone())
                .or_default()
                .push((version.clone(), !item.disable));
            if !item.disable {
                top.entry(info.mod_id.clone())
                    .or_default()
                    .push((mod_file_name(item), version));
            }
        }

        // 禁用的包里那些内置模组不算提供
        if !item.disable {
            get_jar_in_jar_list(item, &mut bundled);
        }
    }

    // 谁要求谁：键取「前置 modid + 版本范围」，同一个前置的不同范围各报一条
    let mut requires: HashMap<(String, String), (VersionRange, Vec<String>)> = HashMap::new();
    for item in mods.iter().filter(|item| !item.fail && !item.disable) {
        let by = mod_display_name(item);
        for info in item.info.iter() {
            for dep in info.dependants.iter() {
                let DependantType::Required(dep) = dep else {
                    continue;
                };
                if dep.modid.is_empty() || is_loader_provided(game, &dep.modid) {
                    continue;
                }
                let entry = requires
                    .entry((dep.modid.clone(), dep.version.to_string()))
                    .or_insert_with(|| (dep.version.clone(), Vec::new()));
                if !entry.1.contains(&by) {
                    entry.1.push(by.clone());
                }
            }
        }
    }

    let mut problems = Vec::new();

    for ((modid, _), (required, mut required_by)) in requires {
        required_by.sort();

        // 能顶上的：启用的顶层条目 + 内置条目
        let mut providers: Vec<String> = installed
            .get(&modid)
            .map(|list| {
                list.iter()
                    .filter(|(_, enable)| *enable)
                    .map(|(version, _)| version.clone())
                    .collect()
            })
            .unwrap_or_default();
        if let Some(version) = bundled.get(&modid) {
            providers.push(version.clone());
        }

        let kind = if providers.is_empty() {
            match installed.get(&modid).and_then(|list| list.first()) {
                // 只有禁用状态的包提供它：与"缺失"分开报，用户才知道该去启用
                Some((_, false)) => DependantProblemKind::Disabled,
                _ => DependantProblemKind::Missing,
            }
        } else if required == VersionRange::Any
            || providers
                .iter()
                .any(|version| version.is_empty() || required.contains(version))
        {
            continue;
        } else {
            DependantProblemKind::Mismatch
        };

        let installed = if providers.is_empty() {
            installed
                .get(&modid)
                .and_then(|list| list.first().map(|(version, _)| version.clone()))
                .unwrap_or_default()
        } else {
            providers.join(", ")
        };

        problems.push(DependantProblem {
            modid,
            kind,
            required,
            installed,
            required_by,
            files: Vec::new(),
        });
    }

    // 重复：同一个 modid 有两份及以上处于启用状态的顶层文件
    for (modid, list) in top {
        if list.len() < 2 {
            continue;
        }
        let mut versions: Vec<String> = list.iter().map(|(_, version)| version.clone()).collect();
        versions.sort();
        versions.dedup();
        let mut files: Vec<String> = list.into_iter().map(|(file, _)| file).collect();
        files.sort();
        problems.push(DependantProblem {
            modid,
            kind: DependantProblemKind::Duplicate,
            required: VersionRange::Any,
            installed: versions.join(", "),
            required_by: Vec::new(),
            files,
        });
    }

    // 排序：种类优先（缺失最要紧），同种类按 modid
    problems.sort_by(|a, b| {
        let rank = |kind: &DependantProblemKind| match kind {
            DependantProblemKind::Missing => 0,
            DependantProblemKind::Disabled => 1,
            DependantProblemKind::Mismatch => 2,
            DependantProblemKind::Duplicate => 3,
        };
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then_with(|| a.modid.cmp(&b.modid))
    });

    Ok(DependantTest { problems })
}

// 本文件的测试：元数据解析 / 依赖范围与依赖检测 / 图标三组放在一起，
// 共用一个 `use super::*`；元数据那组要起日志，见 `boot_log`。
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TextEntry, text_entry};
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    // ==================== 元数据解析 ====================
    //
    // 单个元数据文件坏了不该影响整包：真实案例 `mekanism_lasers-1.1.10.3-a.jar` 里两个 toml
    // 坏了一个，旧实现用 `?` 抛出后整包判为"读取失败"、连名字都看不到；正确行为是坏的那个
    // 跳过并记一条日志，好的照常读。

    /// 起日志系统：解析失败时**故意**要记一条日志，而 `mml_log` 没 start 过就会 panic，
    /// 那是"日志系统还没起来"的前提问题，不是被测逻辑的问题。
    ///
    /// **走 crate 级的共用 boot**，见 [`crate::test_support::boot`]：`mml_log::STREAM` 是
    /// `OnceLock`，各测试模块各写一份就会第二次 `start()` → panic，本模块与
    /// `game_group::tests` 曾经互相踩挂就是这个原因。
    fn boot_log() {
        crate::test_support::boot();
    }

    /// 造一个只含指定条目的 jar
    fn make_jar(entries: &[TextEntry]) -> PathBuf {
        boot_log();
        let path = mml_testutil::temp_dir().join(format!("mml-mod-{}.jar", Uuid::new_v4()));
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for e in entries {
            writer.start_file(e.name, options).unwrap();
            writer.write_all(e.text.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    /// 两个 toml、其中一个非法：整包仍应读出**合法那个**的信息，而不是判失败
    ///
    /// 两个路径都必须是**会被扫描的**那三个之一，即 `META-INF/mods.toml` /
    /// `META-INF/neoforge.mods.toml` / `neoforge.mods.toml`；
    /// 放在别处的 toml 根本不进解析，拿它当样本是测不出问题的。
    #[test]
    fn broken_toml_does_not_fail_whole_jar() {
        let path = make_jar(&[
            text_entry(
                names::NEO_TOML_FILE,
                "modLoader=\"javafml\"\nloaderVersion=\"[1,)\"\nlicense=\"MIT\"\n\
                 [[mods]]\nmodId=\"mekanism_lasers\"\ndisplayName=\"Mekanism Lasers\"\nversion=\"1.1.10.3\"\n",
            ),
            // 非法样本用**根目录的 neoforge.mods.toml**，即第三个扫描路径，`[[mods]` 少一个方括号；
            // 这正是"一个包里两个 toml、坏的那个把整包带崩"的真实形态
            text_entry(names::NEO_TOML1_FILE, "[[mods]\nmodId=\"whatever\"\n"),
        ]);

        let info = read_mod_info(&path).expect("坏 toml 不该让整包解析失败");
        assert!(!info.fail, "不该被判为读取失败");
        let first = info.info.first().expect("应读出合法 toml 的条目");
        assert_eq!(first.mod_id, "mekanism_lasers");
        assert_eq!(first.name, "Mekanism Lasers");
        assert_eq!(first.version.as_deref(), Some("1.1.10.3"));
    }

    /// 只有非法 toml：当作"读不出元数据"，但不 panic、也不报错
    #[test]
    fn only_broken_toml_yields_empty_info() {
        let path = make_jar(&[text_entry(names::MC_MOD_TOML_FILE, "[[mods]\n")]);

        let info = read_mod_info(&path).expect("坏 toml 不该让整包解析失败");
        assert!(info.info.is_empty(), "解析不出条目时 info 应为空");
    }

    /// MANIFEST.MF 里**没有** core mod 标记时，不该多出一条空 info
    ///
    /// `META-INF/MANIFEST.MF` 几乎每个 jar 都有，而 `read_core_mod` 以前无条件 push 一条
    /// info，那条的加载器是枚举默认值 `Normal`；界面改成"汇总**所有**条目的加载器"之后，
    /// 用户就会看到**每个模组都多出一个「原版」**。
    #[test]
    fn manifest_without_core_marker_adds_no_info() {
        let path = make_jar(&[
            text_entry(
                names::FABRIC_MOD_FILE,
                r#"{"id":"demo","name":"Demo","version":"1.0"}"#,
            ),
            text_entry(
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\nCreated-By: Gradle\n",
            ),
        ]);

        let info = read_mod_info(&path).expect("应能解析");
        assert_eq!(info.info.len(), 1, "只该有 fabric 那一条，不该多出空条目");
        assert_eq!(
            info.info[0].loaders.to_string(),
            "fabric",
            "不该是默认的 normal"
        );
        assert!(!info.core, "没有 core mod 标记就不该判成 core");
    }

    /// 真的带 core mod 标记时，那条 info 仍要保留，别把功能一起改掉
    #[test]
    fn manifest_with_core_marker_keeps_info() {
        let path = make_jar(&[text_entry(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\nFMLCorePlugin: com.example.CorePlugin\n",
        )]);

        let info = read_mod_info(&path).expect("应能解析");
        assert!(info.core, "有 FMLCorePlugin 就该判成 core");
        let first = info.info.first().expect("core mod 的 info 不该被丢掉");
        assert_eq!(first.loaders.to_string(), "forge");
    }

    // ==================== 依赖范围与依赖检测 ====================

    /// 造一个装着元数据的模组条目
    ///
    /// - `file`: 文件名，重复安装那一档按它区分
    /// - `modid`: 模组标识
    /// - `version`: 版本号
    /// - `deps`: 前置列表
    fn mod_item(file: &str, modid: &str, version: &str, deps: Vec<DependantType>) -> ModObj {
        ModObj {
            file: PathBuf::from(file),
            info: vec![ModItemObj {
                mod_id: modid.to_string(),
                name: modid.to_string(),
                version: Some(version.to_string()),
                dependants: deps,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// 造一条强制前置，按 1.12.2 的 `modid@版本范围` 写法
    ///
    /// - `spec`: 形如 `b@[1.0,)`；不带 `@` 时整串就是 modid
    fn need(spec: &str) -> DependantType {
        DependantType::Required(DependantInfo::parse_required_name(spec))
    }

    /// 造一个 Forge 实例设置，用来验证"加载器提供的前置不报"
    fn forge_game() -> InstanceSettingObj {
        InstanceSettingObj {
            loader: LoaderType::Forge,
            ..Default::default()
        }
    }

    /// 带 `modid@` 前缀的解析：modid 取出来、范围可用
    #[test]
    fn parse_with_modid_prefix() {
        let d = DependantInfo::parse_required_name("spongeapi@[7.0.0,)");
        assert_eq!(d.modid, "spongeapi");
        assert!(d.version.contains("7.0.0"));
        assert!(!d.version.contains("6.9.9"));

        let d = DependantInfo::parse_required_name("Forge@[9.10.1.850,)");
        assert_eq!(d.modid, "Forge");
        assert!(d.version.contains("9.10.1.850"));

        let d = DependantInfo::parse_required_version("modid", ">=0.19.5");
        assert_eq!(d.modid, "modid");
        assert!(d.version.contains("0.20.0"));
    }

    /// 裸名字整串就是 modid、范围不限：当版本范围解析会让 modid 变空、依赖被丢掉
    #[test]
    fn parse_bare_name_is_modid() {
        let d = DependantInfo::parse_required_name("spongeapi");
        assert_eq!(d.modid, "spongeapi");
        assert_eq!(d.version, VersionRange::Any);
    }

    /// 缺前置：被依赖的包不在列表里 → Missing，并带上"谁要求它"
    #[test]
    fn missing_dep_is_reported() {
        let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("b@[1.0,)")])];

        let test = mods_dep_test(&forge_game(), &mods).unwrap();
        assert_eq!(test.problems.len(), 1);
        assert_eq!(test.problems[0].modid, "b");
        assert_eq!(test.problems[0].kind, DependantProblemKind::Missing);
        assert_eq!(test.problems[0].required_by, vec!["a".to_string()]);
        assert!(test.problems[0].installed.is_empty());
    }

    /// 版本落在范围内 → 不报
    #[test]
    fn satisfied_dep_is_silent() {
        let mods = vec![
            mod_item("a.jar", "a", "1.0", vec![need("b@[1.0,)")]),
            mod_item("b.jar", "b", "1.4", vec![]),
        ];

        assert!(mods_dep_test(&forge_game(), &mods).unwrap().problems.is_empty());
    }

    /// 装着了但版本不在范围内 → Mismatch，并报出实际版本
    #[test]
    fn version_mismatch_is_reported() {
        let mods = vec![
            mod_item("a.jar", "a", "1.0", vec![need("b@[2.0,)")]),
            mod_item("b.jar", "b", "1.4", vec![]),
        ];

        let test = mods_dep_test(&forge_game(), &mods).unwrap();
        assert_eq!(test.problems.len(), 1);
        assert_eq!(test.problems[0].kind, DependantProblemKind::Mismatch);
        assert_eq!(test.problems[0].installed, "1.4");
    }

    /// 加载器与游戏自身提供的前置不报：forge 实例里的 minecraft / forge 都不算缺失
    #[test]
    fn loader_provided_dep_is_silent() {
        let mods = vec![mod_item(
            "a.jar",
            "a",
            "1.0",
            vec![need("minecraft@[1.20,)"), need("forge@[47,)")],
        )];

        assert!(mods_dep_test(&forge_game(), &mods).unwrap().problems.is_empty());
    }

    /// Fabric 的 `fabric` 不排：Fabric API 的 modid 就是它，缺了要报出来
    #[test]
    fn fabric_api_is_not_treated_as_loader() {
        let game = InstanceSettingObj {
            loader: LoaderType::Fabric,
            ..Default::default()
        };
        let mods = vec![
            mod_item("a.jar", "a", "1.0", vec![need("fabric")]),
            // 加载器自己的 id 则要排掉
            mod_item("b.jar", "b", "1.0", vec![need("fabricloader")]),
        ];

        let test = mods_dep_test(&game, &mods).unwrap();
        assert_eq!(test.problems.len(), 1);
        assert_eq!(test.problems[0].modid, "fabric");
    }

    /// 只有禁用状态的包提供它 → Disabled，与"缺失"分开
    #[test]
    fn disabled_dep_is_reported() {
        let mut dep = mod_item("b.jar", "b", "1.4", vec![]);
        dep.disable = true;
        let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("b")]), dep];

        let test = mods_dep_test(&forge_game(), &mods).unwrap();
        assert_eq!(test.problems.len(), 1);
        assert_eq!(test.problems[0].kind, DependantProblemKind::Disabled);
    }

    /// 内置 jar-in-jar 里的模组能顶上依赖，不算缺失
    #[test]
    fn bundled_dep_is_satisfied() {
        let mut a = mod_item("a.jar", "a", "1.0", vec![need("lib@[2.0,)")]);
        a.jar_in_jar = vec![mod_item("lib.jar", "lib", "2.1", vec![])];

        assert!(mods_dep_test(&forge_game(), &vec![a]).unwrap().problems.is_empty());
    }

    /// 同一个 modid 装了两份启用的包 → Duplicate，并列出文件名
    #[test]
    fn duplicate_modids_are_reported() {
        let mods = vec![
            mod_item("sodium-1.jar", "sodium", "0.5", vec![]),
            mod_item("sodium-2.jar", "sodium", "0.6", vec![]),
        ];

        let test = mods_dep_test(&forge_game(), &mods).unwrap();
        assert_eq!(test.problems.len(), 1);
        assert_eq!(test.problems[0].kind, DependantProblemKind::Duplicate);
        assert_eq!(test.problems[0].modid, "sodium");
        assert_eq!(
            test.problems[0].files,
            vec!["sodium-1.jar".to_string(), "sodium-2.jar".to_string()]
        );
    }

    /// 推荐前置不报，禁用包声明的依赖也不报
    #[test]
    fn recommend_and_disabled_requirer_are_silent() {
        let mut disabled = mod_item(
            "c.jar",
            "c",
            "1.0",
            vec![DependantType::Required(DependantInfo::parse_required_name("d"))],
        );
        disabled.disable = true;
        let mods = vec![
            mod_item(
                "a.jar",
                "a",
                "1.0",
                vec![DependantType::Recommend(DependantInfo::parse_required_name("z"))],
            ),
            disabled,
        ];

        assert!(mods_dep_test(&forge_game(), &mods).unwrap().problems.is_empty());
    }

    /// 装了但没写版本号时不判版本不符，避免把"元数据没写"报成问题
    #[test]
    fn empty_installed_version_is_not_mismatch() {
        let mut b = mod_item("b.jar", "b", "", vec![]);
        b.info[0].version = None;
        let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("b@[2.0,)")]), b];

        assert!(mods_dep_test(&forge_game(), &mods).unwrap().problems.is_empty());
    }

    // ==================== 图标 ====================

    /// 模板里被注释掉的 logoFile 必须跳过，这是最常见的形态
    #[test]
    fn logo_file_ignores_comments() {
        let text = "#logoFile=\"examplemod.png\"\nmodId=\"demo\"\n";
        assert_eq!(forge_logo_file(text), None);
    }

    /// 真写上的 logoFile 取得到，引号与行内注释都去掉
    #[test]
    fn logo_file_reads_value() {
        assert_eq!(
            forge_logo_file("modId=\"demo\"\nlogoFile=\"icon/logo.png\"\n").unwrap(),
            "icon/logo.png"
        );
        assert_eq!(
            forge_logo_file("logoFile = 'a.png' # 图标").unwrap(),
            "a.png"
        );
        assert_eq!(forge_logo_file("  logoFile=\"b.png\"  ").unwrap(), "b.png");
    }

    /// mcmod.info 顶层数组形态：取第一条写了 logoFile 的
    #[test]
    fn mcmod_logo_file_reads_array() {
        let json = r#"[
            {"modid": "demo", "name": "Demo"},
            {"modid": "other", "logoFile": "assets/logo.png"}
        ]"#;
        assert_eq!(mcmod_logo_file_in(json).unwrap(), "assets/logo.png");
    }

    /// 带 `modList` 的对象形态，部分老包这么写，空值跳过
    #[test]
    fn mcmod_logo_file_reads_mod_list() {
        let json = r#"{"modListVersion": 2, "modList": [
            {"modid": "demo", "logoFile": ""},
            {"modid": "other", "logoFile": "icon.png"}
        ]}"#;
        assert_eq!(mcmod_logo_file_in(json).unwrap(), "icon.png");
    }

    /// 非法的单引号 JSON 也能读，走 sanitize_mcmod_json 那条容错路径
    #[test]
    fn mcmod_logo_file_sanitizes_single_quotes() {
        let json = "[{'modid': 'demo', 'logoFile': 'logo.png'}]";
        assert_eq!(mcmod_logo_file_in(json).unwrap(), "logo.png");
    }

    /// 没写 logoFile / 结构不认识 / 文本不是 JSON 时返回 None，不 panic
    #[test]
    fn mcmod_logo_file_absent() {
        assert_eq!(mcmod_logo_file_in(r#"[{"modid": "demo"}]"#), None);
        assert_eq!(mcmod_logo_file_in("{}"), None);
        assert_eq!(mcmod_logo_file_in("not a json"), None);
    }
}
