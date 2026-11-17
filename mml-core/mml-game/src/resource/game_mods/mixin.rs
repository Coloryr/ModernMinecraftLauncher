//! mixin 目标扫描
//!
//! 找出模组里的 mixin 配置，读配置列出的 mixin 类，把"改了哪个类的哪个方法"记进 [`ModObj`]。
//! 配置名按 `*.mixins.json` / `*.mixin.json` 认，清单的 `MixinConfigs` 与
//! `fabric.mod.json` 里另外声明的配置一并补上。

use std::{
    collections::HashSet,
    io::{Read, Seek},
};

use mml_base::serialize_tools::MiniJsonObj;
use zip::ZipArchive;

use crate::class_scan::mixin_class;

use super::meta::parse_manifest;
use super::types::{MixinTarget, ModObj};

/// 读一个模组 jar 里的 mixin 目标
///
/// - `archive`: 模组压缩包
/// - `mod_info`: 模组信息，结果追加到 `mixins`
pub(super) fn read_mixins(archive: &mut ZipArchive<impl Read + Seek>, mod_info: &mut ModObj) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut scanned: HashSet<String> = HashSet::new();

    for config in mixin_configs(archive) {
        let Some(text) = read_text(archive, &config) else {
            continue;
        };
        let Some(map) = MiniJsonObj::from_str(&text)
            .ok()
            .and_then(|json| json.as_object())
        else {
            continue;
        };
        let package = map.get_opt_string("package").unwrap_or_default();
        let mut listed = 0usize;

        // 三条列表给的类名相对于 package，可能再带一层子包；没写 package 时按全名
        for key in ["mixins", "client", "server"] {
            let Some(list) = map.get_list(key) else {
                continue;
            };
            for item in list {
                let Some(name) = item.as_string() else {
                    continue;
                };
                let name = name.trim_matches('.');
                let class = if package.is_empty() {
                    format!("{}.class", name.replace('.', "/"))
                } else {
                    format!(
                        "{}/{}.class",
                        package.trim_matches('.').replace('.', "/"),
                        name.replace('.', "/")
                    )
                };
                listed += 1;
                read_mixin_class(archive, mod_info, &config, &class, &mut seen);
            }
        }

        // GTNH 那批模组把类名放在插件里注册，配置只有 package：按包扫一遍带 @Mixin 的类
        if listed == 0 && !package.is_empty() && scanned.insert(package.clone()) {
            read_package_mixins(archive, mod_info, &config, &package, &mut seen);
        }
    }
}

/// 按 package 扫带 `@Mixin` 的类，只给没写类列表的配置用
fn read_package_mixins(
    archive: &mut ZipArchive<impl Read + Seek>,
    mod_info: &mut ModObj,
    config: &str,
    package: &str,
    seen: &mut HashSet<String>,
) {
    let prefix = format!("{}/", package.trim_matches('.').replace('.', "/"));
    let classes: Vec<String> = (0..archive.len())
        .filter_map(|index| {
            let entry = archive.by_index(index).ok()?;
            Some(entry.name().unwrap_or_default().to_string())
        })
        .filter(|name| {
            name.starts_with(&prefix) && name.ends_with(".class") && !name.contains('$')
        })
        .take(MAX_PACKAGE_CLASSES)
        .collect();

    for class in classes {
        read_mixin_class(archive, mod_info, config, &class, seen);
    }
}

/// 一个配置最多扫多少个类，防止把整个包当 mixin 包时扫太久
const MAX_PACKAGE_CLASSES: usize = 4096;

/// 读一个 mixin 类并落成目标条目
fn read_mixin_class(
    archive: &mut ZipArchive<impl Read + Seek>,
    mod_info: &mut ModObj,
    config: &str,
    class: &str,
    seen: &mut HashSet<String>,
) {
    let Some(bytes) = read_bytes(archive, class) else {
        return;
    };
    let Ok(info) = mixin_class::parse_mixin(&bytes) else {
        return;
    };
    if info.targets.is_empty() {
        return;
    }

    let mixin = class.trim_end_matches(".class").replace('/', ".");
    for target in &info.targets {
        for injection in &info.injections {
            for method in &injection.methods {
                // 同一个 mixin 类可能被多份配置重复注册（如 early / early.j17），只留一份
                let key = format!("{mixin}|{target}|{method}|{}", injection.kind);
                if !seen.insert(key) {
                    continue;
                }
                mod_info.mixins.push(MixinTarget {
                    config: config.to_string(),
                    mixin: mixin.clone(),
                    target: target.clone(),
                    method: method.clone(),
                    kind: injection.kind.to_string(),
                });
            }
        }
    }
}

/// 找出这个包里的 mixin 配置
fn mixin_configs(archive: &mut ZipArchive<impl Read + Seek>) -> Vec<String> {
    let mut configs: Vec<String> = (0..archive.len())
        .filter_map(|index| {
            let entry = archive.by_index(index).ok()?;
            Some(entry.name().unwrap_or_default().to_string())
        })
        .filter(|name| {
            // 配置名各式各样，如 mixins.angelica.early.json；refmap 只是映射表
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".json") && lower.contains("mixin") && !lower.contains("refmap")
        })
        .collect();

    for name in declared_configs(archive) {
        if !configs.iter().any(|item| item == &name) {
            configs.push(name);
        }
    }

    configs
}

/// 清单与 `fabric.mod.json` 里声明的配置名
fn declared_configs(archive: &mut ZipArchive<impl Read + Seek>) -> Vec<String> {
    let mut list = Vec::new();

    if let Some(text) = read_text(archive, "META-INF/MANIFEST.MF") {
        let manifest = parse_manifest(&text);
        if let Some(value) = manifest.get("MixinConfigs") {
            for item in value.split(',') {
                let item = item.trim();
                if !item.is_empty() {
                    list.push(item.to_string());
                }
            }
        }
    }

    if let Some(text) = read_text(archive, "fabric.mod.json")
        && let Some(map) = MiniJsonObj::from_str(&text)
            .ok()
            .and_then(|json| json.as_object())
        && let Some(items) = map.get_list("mixins")
    {
        for item in items {
            if let Some(value) = item.as_string() {
                list.push(value);
            } else if let Some(value) = item
                .as_object()
                .and_then(|map| map.get_opt_string("config"))
            {
                list.push(value);
            }
        }
    }

    list
}

/// 读一个条目成文本
fn read_text(archive: &mut ZipArchive<impl Read + Seek>, name: &str) -> Option<String> {
    let bytes = read_bytes(archive, name)?;
    String::from_utf8(bytes).ok()
}

/// 读一个条目成字节
fn read_bytes(archive: &mut ZipArchive<impl Read + Seek>, name: &str) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name).ok()?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}