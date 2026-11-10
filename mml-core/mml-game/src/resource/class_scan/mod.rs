//! 模组 Side 扫描
//!
//! 读模组 jar 里的 class 常量池与注解，判断模组是仅客户端、仅服务端还是两边都要。
//! 常量池解析在 `const_pool`，class 结构遍历在 `class`，注解判定在 `annotation`。

use std::{fs::File, io::Read, path::Path};

use zip::ZipArchive;

use crate::game_mods::LoadSideType;

mod annotation;

use self::class::scan_class_for_annotations;
mod class;
mod const_pool;

/// Mod 信息
#[derive(Debug, Clone)]
pub struct ModInfo {
    /// Mod 标识，取自 @Mod 注解；缺失时回退成 jar 文件名
    pub modid: String,
    /// 物理端类型，没有 @SideOnly 时是 `Both`
    pub side: LoadSideType,
}

/// 单个 jar 的扫描结果
#[derive(Debug, Default)]
pub struct JarScanResult {
    /// 该 jar 中扫出的所有 Mod
    pub mods: Vec<ModInfo>,
    /// 扫描过程中发现的警告
    pub warnings: Vec<String>,
}

/// 类的注解扫描结果
pub(super) struct ClassAnnotationInfo {
    /// 这个类上的物理端类型
    side: LoadSideType,
    /// 类级 `@Mod` 注解信息
    mod_info: Option<ModAnnotation>,
}

/// @Mod 注解信息
pub(super) struct ModAnnotation {
    /// Mod 标识
    modid: String,
    /// 由 clientSideOnly / serverSideOnly 推出的物理端
    side: LoadSideType,
}

/// 扫描单个模组 jar 文件，返回所有 Mod 的信息
///
/// - `path`: jar 文件路径
///
/// # 返回值
///
/// 返回扫描结果；打开或读取失败返回对应错误
pub fn scan_jar<P: AsRef<Path>>(path: P) -> Result<JarScanResult, Box<dyn std::error::Error>> {
    let file = File::open(path.as_ref())?;
    let mut archive = ZipArchive::new(file)?;

    let mut result = JarScanResult::default();
    let mut mod_annotations = Vec::new();

    // 先收集所有 class 文件的索引和名称
    let entries: Vec<(usize, String)> = (0..archive.len())
        .filter_map(|i| {
            archive.by_index(i).ok().and_then(|entry| {
                let name = entry.name().unwrap_or_default();
                if name.ends_with(".class") && !name.contains("META-INF") {
                    Some((i, name.to_string()))
                } else {
                    None
                }
            })
        })
        .collect();

    // 遍历所有 class 文件
    for (idx, _class_name) in entries {
        let mut entry = archive.by_index(idx)?;
        let mut class_bytes = Vec::new();
        entry.read_to_end(&mut class_bytes)?;

        if let Some(class_info) = scan_class_for_annotations(&class_bytes)? {
            if let Some(mut mod_info) = class_info.mod_info {
                // @SideOnly 优先，这是 Forge 的原生机制
                if class_info.side != LoadSideType::Unknown {
                    mod_info.side = class_info.side;
                }
                mod_annotations.push(mod_info);
            }
        }
    }

    // 处理结果
    for mut mod_info in mod_annotations {
        if mod_info.modid.is_empty() {
            mod_info.modid = path
                .as_ref()
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
        }

        // 没有 SideOnly 则认为是 Both
        let side = if mod_info.side == LoadSideType::Unknown {
            LoadSideType::Both
        } else {
            mod_info.side
        };

        result.mods.push(ModInfo {
            modid: mod_info.modid,
            side,
        });
    }

    Ok(result)
}
