//! 各加载器的元数据解析
//!
//! Forge 的 `mcmod.info` / `mods.toml`、Fabric 与 Quilt 的 `fabric.mod.json` 各有一套格式。

use std::{collections::HashMap, io::Read};

use crate::error::archive_read_err;
use crate::loader::LoaderType;
use mml_base::{
    range_checker::{RangeSyntax, VersionRange},
    serialize_tools::{MiniJsonObj, MiniTomlMap},
};
use mml_names::i18_items::error_type::CoreResult;

use super::types::{DependantInfo, DependantType, LoadSideType, ModItemObj, ModObj, Quote};

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
pub(super) fn sanitize_mcmod_json(json: &str) -> String {
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
pub(super) fn read_forge_json(mut reader: impl Read, mod_info: &mut ModObj) -> CoreResult<()> {
    let mut json = String::new();
    reader.read_to_string(&mut json).map_err(archive_read_err)?;

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
pub(super) fn read_forge_toml(
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
            let key_str = key.to_string();
            let Some(mod_item) = mod_info
                .info
                .iter_mut()
                .find(|item| item.mod_id.eq_ignore_ascii_case(&key_str))
            else {
                continue;
            };

            let Some(list) = value.as_list() else {
                continue;
            };

            for item in list.iter() {
                let Some(map) = item.as_object() else {
                    continue;
                };
                let Some(modid) = map.get_opt_string("modId") else {
                    continue;
                };

                let version = map.get_opt_string("versionRange");

                if modid.eq_ignore_ascii_case("minecraft") {
                    if let Some(side) = map.get_opt_string("side") {
                        mod_item.side = match side.to_ascii_lowercase().as_str() {
                            "both" => LoadSideType::Both,
                            "client" => LoadSideType::Client,
                            "server" => LoadSideType::Server,
                            _ => mod_item.side,
                        };
                    }
                    if let Some(version) = version {
                        mod_item.dependants.push(DependantType::Required(
                            DependantInfo::parse_required_version(
                                &modid,
                                &version,
                                RangeSyntax::Maven,
                            ),
                        ));
                    }
                } else {
                    if matches!(mod_item.side, LoadSideType::Unknown)
                        && let Some(side) = map.get_opt_string("side")
                    {
                        mod_item.side = match side.to_ascii_lowercase().as_str() {
                            "both" => LoadSideType::Both,
                            "client" => LoadSideType::Client,
                            "server" => LoadSideType::Server,
                            _ => mod_item.side,
                        };
                    }

                    let is_required = map
                        .get_opt_string("type")
                        .map(|s| s.eq_ignore_ascii_case("required"))
                        .unwrap_or(map.get_bool("mandatory"));
                    let version = version.unwrap_or_default();

                    let dep_type = if is_required {
                        DependantType::Required(DependantInfo::parse_required_version(
                            &modid,
                            &version,
                            RangeSyntax::Maven,
                        ))
                    } else {
                        DependantType::Recommend(DependantInfo::parse_required_version(
                            &modid,
                            &version,
                            RangeSyntax::Maven,
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
pub(super) fn read_fabric_json(reader: impl Read, mod_info: &mut ModObj) -> CoreResult<()> {
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
                        RangeSyntax::Semver,
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
                        RangeSyntax::Semver,
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
pub(super) fn read_quilt_json(reader: impl Read, mod_info: &mut ModObj) -> CoreResult<()> {
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
                                RangeSyntax::Semver,
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
pub(super) fn parse_manifest(content: &str) -> HashMap<String, String> {
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
