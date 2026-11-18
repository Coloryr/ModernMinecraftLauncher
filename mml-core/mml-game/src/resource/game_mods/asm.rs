//! ASM 变压器扫描
//!
//! 1.13 以后的 Forge / NeoForge 用 `META-INF/coremods.json` 声明 coremod，变压器是一份 JS，
//! 里头明写了目标类与方法；1.7 / 1.12 的用 `MANIFEST.MF` 的 `FMLCorePlugin` 声明插件类，
//! 目标要读插件代码才知道，这里只记插件类名。

use std::{
    io::{Read, Seek},
    sync::LazyLock,
};

use mml_base::serialize_tools::MiniJsonObj;
use regex::Regex;
use zip::ZipArchive;

use super::meta::parse_manifest;
use super::types::{AsmTarget, ModObj};

/// coremod JS 里的目标类：`'class': 'net/minecraft/...'`
static RE_JS_CLASS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)['\"]class['\"]\s*:\s*['\"]([^'\"]+)['\"]"#).unwrap()
});

/// coremod JS 里的目标方法：`'methodName': 'getAllEnchantments'`
static RE_JS_METHOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)['\"]methodName['\"]\s*:\s*['\"]([^'\"]+)['\"]"#).unwrap()
});

/// `META-INF/coremods.json` 声明的 coremod
pub const ASM_SOURCE_COREMODS: &str = "coremods.json";

/// `MANIFEST.MF` 的 `FMLCorePlugin`，1.7 / 1.12 的老式 coremod 插件
pub const ASM_SOURCE_PLUGIN: &str = "FMLCorePlugin";

/// modlauncher 的转换服务，列在这里的类可以返回 transformer，但不一定真的改字节码
pub const ASM_SOURCE_SERVICE: &str = "ITransformationService";

/// modlauncher 的转换服务声明文件
const TRANSFORMATION_SERVICE: &str =
    "META-INF/services/cpw.mods.modlauncher.api.ITransformationService";

/// 读一个模组 jar 里的 ASM 变压器
///
/// - `archive`: 模组压缩包
/// - `mod_info`: 模组信息，结果追加到 `asm`
pub(super) fn read_asm(archive: &mut ZipArchive<impl Read + Seek>, mod_info: &mut ModObj) {
    read_coremods(archive, mod_info);
    read_manifest_plugin(archive, mod_info);
    read_services(archive, mod_info);
}

/// `META-INF/coremods.json` 声明的 coremod
fn read_coremods(archive: &mut ZipArchive<impl Read + Seek>, mod_info: &mut ModObj) {
    let Some(text) = read_text(archive, "META-INF/coremods.json") else {
        return;
    };
    let Some(map) = MiniJsonObj::from_str(&text)
        .ok()
        .and_then(|json| json.as_object())
    else {
        return;
    };

    for (name, value) in map.iter() {
        let Some(path) = value.as_string() else {
            continue;
        };
        // JS 变压器把目标写在 initializeCoreMod 里，类变压器没有可读的声明
        let (target, method) = if path.to_ascii_lowercase().ends_with(".js") {
            js_target(archive, &path)
        } else {
            (String::new(), String::new())
        };
        mod_info.asm.push(AsmTarget {
            source: ASM_SOURCE_COREMODS.to_string(),
            transformer: name.clone(),
            target,
            method,
        });
    }
}

/// 从 coremod 的 JS 里取目标类与方法
fn js_target(archive: &mut ZipArchive<impl Read + Seek>, path: &str) -> (String, String) {
    let Some(text) = read_text(archive, path) else {
        return (String::new(), String::new());
    };
    let target = RE_JS_CLASS
        .captures(&text)
        .map(|captures| captures[1].replace('/', "."))
        .unwrap_or_default();
    let method = RE_JS_METHOD
        .captures(&text)
        .map(|captures| captures[1].to_string())
        .unwrap_or_default();
    (target, method)
}

/// 1.7 / 1.12 的 coremod 插件类
fn read_manifest_plugin(archive: &mut ZipArchive<impl Read + Seek>, mod_info: &mut ModObj) {
    let Some(text) = read_text(archive, "META-INF/MANIFEST.MF") else {
        return;
    };
    let manifest = parse_manifest(&text);
    let Some(class) = manifest.get("FMLCorePlugin") else {
        return;
    };
    let class = class.trim();
    if class.is_empty() {
        return;
    }
    mod_info.asm.push(AsmTarget {
        source: ASM_SOURCE_PLUGIN.to_string(),
        transformer: class.to_string(),
        target: String::new(),
        method: String::new(),
    });
}

/// modlauncher 转换服务
fn read_services(archive: &mut ZipArchive<impl Read + Seek>, mod_info: &mut ModObj) {
    let Some(text) = read_text(archive, TRANSFORMATION_SERVICE) else {
        return;
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        mod_info.asm.push(AsmTarget {
            source: ASM_SOURCE_SERVICE.to_string(),
            transformer: line.to_string(),
            target: String::new(),
            method: String::new(),
        });
    }
}

/// 读一个条目成文本
fn read_text(archive: &mut ZipArchive<impl Read + Seek>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    Some(text)
}