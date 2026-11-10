//! 注解判定
//!
//! 读 class 上的注解，认出 `@Mod` / `@SideOnly` 与参数级注解，得出模组的加载侧。

use super::const_pool::{cp_get, read_u16};
use super::{ClassAnnotationInfo, ModAnnotation};
use crate::game_mods::LoadSideType;

/// 常量池：按常量池索引存储 UTF8 字符串，索引从 1 开始
/// index 0 保留不用，index 1..pool_count 对应常量池条目
type ConstantPool = Vec<Option<String>>;

/// 从常量池索引获取 UTF8 字符串引用
///
/// - `cp`: 常量池
/// - `idx`: 常量池索引
///
/// # 返回值
///
/// 返回对应的字符串引用；索引无效返回 `None`

/// 检查注解
///
/// - `data`: 注解属性数据
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回 `@SideOnly` / `@Mod` 注解的扫描结果；无相关注解返回 `None`
pub(super) fn check_annotations(
    data: &[u8],
    cp: &ConstantPool,
) -> Result<Option<ClassAnnotationInfo>, Box<dyn std::error::Error>> {
    if data.len() < 2 {
        return Ok(None);
    }

    let num_annotations = read_u16(data, 0)? as usize;
    let mut offset = 2;
    let mut side = LoadSideType::Unknown;
    let mut mod_info = None;

    for _ in 0..num_annotations {
        if offset + 2 > data.len() {
            break;
        }

        let type_idx = read_u16(data, offset)? as usize;
        offset += 2;

        if let Some(type_name) = cp_get(cp, type_idx) {
            match type_name {
                "Lnet/minecraftforge/fml/relauncher/SideOnly;" => {
                    if let Some(s) = parse_side_only(data, &mut offset, cp)? {
                        side = s;
                    }
                }
                "Lnet/minecraftforge/fml/common/Mod;" => {
                    if let Some(info) = parse_mod_annotation(data, &mut offset, cp)? {
                        mod_info = Some(info);
                    }
                }
                _ => {
                    // 未知注解类型，跳过
                    skip_annotation_pairs(data, &mut offset)?;
                }
            }
        } else {
            // type_idx 无效，跳过
            skip_annotation_pairs(data, &mut offset)?;
        }
    }

    if side == LoadSideType::Unknown && mod_info.is_none() {
        Ok(None)
    } else {
        Ok(Some(ClassAnnotationInfo { side, mod_info }))
    }
}

/// 跳过注解的 num_pairs 和所有元素对
///
/// - `data`: 注解属性数据
/// - `offset`: 当前读取偏移，跳过后推进到末尾
///
/// # 返回值
///
/// 成功返回 `Ok(())`；解析失败返回对应错误
pub(super) fn skip_annotation_pairs(
    data: &[u8],
    offset: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if *offset + 2 > data.len() {
        return Ok(());
    }
    let num_pairs = read_u16(data, *offset)? as usize;
    *offset += 2;

    for _ in 0..num_pairs {
        if *offset + 2 > data.len() {
            break;
        }
        *offset += 2; // 跳过元素名索引

        if *offset >= data.len() {
            break;
        }
        let tag = data[*offset] as char;
        *offset += 1;

        skip_annotation_value(data, offset, tag)?;
    }

    Ok(())
}

/// 解析 @SideOnly 注解
///
/// - `data`: 注解属性数据
/// - `offset`: 当前读取偏移，解析后推进到注解末尾
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回物理端类型；未能解析返回 `None`
pub(super) fn parse_side_only(
    data: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<Option<LoadSideType>, Box<dyn std::error::Error>> {
    if *offset + 2 > data.len() {
        return Ok(None);
    }
    let num_pairs = read_u16(data, *offset)? as usize;
    *offset += 2;

    for _ in 0..num_pairs {
        if *offset + 2 > data.len() {
            break;
        }
        *offset += 2; // 跳过元素名

        if *offset >= data.len() {
            break;
        }
        let tag = data[*offset] as char;
        *offset += 1;

        if tag == 'e' {
            if *offset + 4 > data.len() {
                break;
            }
            let const_name_idx = read_u16(data, *offset + 2)? as usize;
            *offset += 4;

            if let Some(const_name) = cp_get(cp, const_name_idx) {
                return Ok(match const_name {
                    "CLIENT" => Some(LoadSideType::Client),
                    "SERVER" => Some(LoadSideType::Server),
                    _ => Some(LoadSideType::Unknown),
                });
            }
        } else {
            skip_annotation_value(data, offset, tag)?;
        }
    }

    Ok(None)
}

/// 解析 @Mod 注解
///
/// - `data`: 注解属性数据
/// - `offset`: 当前读取偏移，解析后推进到注解末尾
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回 `@Mod` 注解信息；无 `modid` 返回 `None`
pub(super) fn parse_mod_annotation(
    data: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<Option<ModAnnotation>, Box<dyn std::error::Error>> {
    if *offset + 2 > data.len() {
        return Ok(None);
    }
    let num_pairs = read_u16(data, *offset)? as usize;
    *offset += 2;

    let mut modid = String::new();
    let mut client_only = false;
    let mut server_only = false;

    for _ in 0..num_pairs {
        if *offset + 2 > data.len() {
            break;
        }
        let name_idx = read_u16(data, *offset)? as usize;
        *offset += 2;

        if *offset >= data.len() {
            break;
        }
        let tag = data[*offset] as char;
        *offset += 1;

        if let Some(name) = cp_get(cp, name_idx) {
            match name {
                "modid" if tag == 's' => {
                    if *offset + 2 > data.len() {
                        break;
                    }
                    let val_idx = read_u16(data, *offset)? as usize;
                    *offset += 2;
                    if let Some(val) = cp_get(cp, val_idx) {
                        modid = val.to_string();
                    }
                }
                "clientSideOnly" if tag == 'Z' => {
                    if *offset + 2 > data.len() {
                        break;
                    }
                    client_only = read_u16(data, *offset)? != 0;
                    *offset += 2;
                }
                "serverSideOnly" if tag == 'Z' => {
                    if *offset + 2 > data.len() {
                        break;
                    }
                    server_only = read_u16(data, *offset)? != 0;
                    *offset += 2;
                }
                _ => {
                    skip_annotation_value(data, offset, tag)?;
                }
            }
        } else {
            // name_idx 无效，跳过该值
            skip_annotation_value(data, offset, tag)?;
        }
    }

    if modid.is_empty() {
        Ok(None)
    } else {
        let side = if client_only && !server_only {
            LoadSideType::Client
        } else if server_only && !client_only {
            LoadSideType::Server
        } else {
            LoadSideType::Unknown
        };

        Ok(Some(ModAnnotation { modid, side }))
    }
}

/// 跳过注解值
///
/// - `data`: 注解属性数据
/// - `offset`: 当前读取偏移，跳过后推进到值末尾
/// - `tag`: 值类型标记
///
/// # 返回值
///
/// 成功返回 `Ok(())`；解析失败返回对应错误
pub(super) fn skip_annotation_value(
    data: &[u8],
    offset: &mut usize,
    tag: char,
) -> Result<(), Box<dyn std::error::Error>> {
    match tag {
        'B' | 'C' | 'S' | 's' | 'I' | 'J' | 'F' | 'D' | 'c' | 'Z' => *offset += 2,
        'e' => *offset += 4,
        '@' => {
            if *offset + 2 > data.len() {
                return Ok(());
            }
            *offset += 2; // 跳过 type_idx
            let num_vals = read_u16(data, *offset)? as usize;
            *offset += 2;
            for _ in 0..num_vals {
                if *offset + 2 > data.len() {
                    break;
                }
                *offset += 2; // 跳过名称索引
                if *offset >= data.len() {
                    break;
                }
                let sub_tag = data[*offset] as char;
                *offset += 1;
                skip_annotation_value(data, offset, sub_tag)?;
            }
        }
        '[' => {
            if *offset + 2 > data.len() {
                return Ok(());
            }
            let num_vals = read_u16(data, *offset)? as usize;
            *offset += 2;
            for _ in 0..num_vals {
                if *offset >= data.len() {
                    break;
                }
                let sub_tag = data[*offset] as char;
                *offset += 1;
                skip_annotation_value(data, offset, sub_tag)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// 检查参数注解
///
/// - `data`: 注解属性数据
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回 `@SideOnly` 注解的扫描结果；无相关注解返回 `None`
pub(super) fn check_parameter_annotations(
    data: &[u8],
    cp: &ConstantPool,
) -> Result<Option<ClassAnnotationInfo>, Box<dyn std::error::Error>> {
    if data.len() < 1 {
        return Ok(None);
    }

    let num_parameters = data[0] as usize;
    let mut offset = 1;

    for _ in 0..num_parameters {
        if offset + 2 > data.len() {
            break;
        }
        let num_annotations = read_u16(data, offset)? as usize;
        offset += 2;

        for _ in 0..num_annotations {
            if offset + 2 > data.len() {
                break;
            }
            let type_idx = read_u16(data, offset)? as usize;
            offset += 2;

            if let Some(type_name) = cp_get(cp, type_idx) {
                if type_name == "Lnet/minecraftforge/fml/relauncher/SideOnly;" {
                    if let Some(info) = check_annotations(&data[offset - 2..], cp)? {
                        return Ok(Some(info));
                    }
                }
            }

            // 跳过该注解的剩余数据
            skip_annotation_pairs(data, &mut offset)?;
        }
    }

    Ok(None)
}
