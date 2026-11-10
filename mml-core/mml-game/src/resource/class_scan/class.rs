//! class 结构遍历
//!
//! 按 class 文件格式走字段、方法、属性表，把注解交给 `annotation` 判定。

use super::ClassAnnotationInfo;
use super::annotation::{check_annotations, check_parameter_annotations};
use super::const_pool::{cp_get, parse_constant_pool, read_u16, read_u32};
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

/// 扫描单个 class 文件
///
/// - `bytes`: class 文件字节流
///
/// # 返回值
///
/// 返回注解扫描结果；非 class 文件或无相关注解返回 `None`
pub(super) fn scan_class_for_annotations(
    bytes: &[u8],
) -> Result<Option<ClassAnnotationInfo>, Box<dyn std::error::Error>> {
    if bytes.len() < 8 || &bytes[0..4] != &[0xCA, 0xFE, 0xBA, 0xBE] {
        return Ok(None);
    }

    let parsed_pool = parse_constant_pool(bytes)?;
    let cp = parsed_pool.pool;
    let mut offset = parsed_pool.end_offset;

    // 跳过访问标志、this_class、super_class
    offset += 2 + 2 + 2;

    // 跳过 interfaces
    let interfaces_count = read_u16(bytes, offset)? as usize;
    offset += 2 + interfaces_count * 2;

    let mut class_side = LoadSideType::Unknown;
    let mut mod_info = None;

    // 扫描字段
    if let Some(info) = scan_fields(bytes, &mut offset, &cp)? {
        class_side = info.side;
        if info.mod_info.is_some() {
            mod_info = info.mod_info;
        }
    }

    // 扫描方法
    if let Some(info) = scan_methods(bytes, &mut offset, &cp)? {
        if class_side == LoadSideType::Unknown {
            class_side = info.side;
        }
        if mod_info.is_none() && info.mod_info.is_some() {
            mod_info = info.mod_info;
        }
    }

    // 扫描类属性，含类级别的 @Mod 与 @SideOnly
    if let Some(info) = scan_class_attributes(bytes, &mut offset, &cp)? {
        if class_side == LoadSideType::Unknown {
            class_side = info.side;
        }
        if mod_info.is_none() && info.mod_info.is_some() {
            mod_info = info.mod_info;
        }
    }

    if class_side == LoadSideType::Unknown && mod_info.is_none() {
        Ok(None)
    } else {
        Ok(Some(ClassAnnotationInfo {
            side: class_side,
            mod_info,
        }))
    }
}

/// 扫描字段
///
/// - `bytes`: class 文件字节流
/// - `offset`: 当前读取偏移，扫描后推进到字段区末尾
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回注解扫描结果；无相关注解返回 `None`
pub(super) fn scan_fields(
    bytes: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<Option<ClassAnnotationInfo>, Box<dyn std::error::Error>> {
    let fields_count = read_u16(bytes, *offset)? as usize;
    *offset += 2;

    let mut result = ClassAnnotationInfo {
        side: LoadSideType::Unknown,
        mod_info: None,
    };

    for _ in 0..fields_count {
        *offset += 6; // 访问标志 + 名字索引 + 描述索引
        let attrs_count = read_u16(bytes, *offset)? as usize;
        *offset += 2;

        for _ in 0..attrs_count {
            if let Some(info) = process_attribute(bytes, offset, cp)? {
                if result.side == LoadSideType::Unknown {
                    result.side = info.side;
                }
                if result.mod_info.is_none() {
                    result.mod_info = info.mod_info;
                }
            }
        }
    }

    if result.side == LoadSideType::Unknown && result.mod_info.is_none() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

/// 扫描方法
///
/// - `bytes`: class 文件字节流
/// - `offset`: 当前读取偏移，扫描后推进到方法区末尾
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回注解扫描结果；无相关注解返回 `None`
pub(super) fn scan_methods(
    bytes: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<Option<ClassAnnotationInfo>, Box<dyn std::error::Error>> {
    let methods_count = read_u16(bytes, *offset)? as usize;
    *offset += 2;

    let mut result = ClassAnnotationInfo {
        side: LoadSideType::Unknown,
        mod_info: None,
    };

    for _ in 0..methods_count {
        *offset += 6; // 访问标志 + 名字索引 + 描述索引
        let attrs_count = read_u16(bytes, *offset)? as usize;
        *offset += 2;

        for _ in 0..attrs_count {
            if let Some(info) = process_attribute(bytes, offset, cp)? {
                if result.side == LoadSideType::Unknown {
                    result.side = info.side;
                }
                if result.mod_info.is_none() {
                    result.mod_info = info.mod_info;
                }
            }
        }
    }

    if result.side == LoadSideType::Unknown && result.mod_info.is_none() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

/// 扫描类属性
///
/// - `bytes`: class 文件字节流
/// - `offset`: 当前读取偏移，扫描后推进到属性区末尾
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回注解扫描结果；无相关注解返回 `None`
pub(super) fn scan_class_attributes(
    bytes: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<Option<ClassAnnotationInfo>, Box<dyn std::error::Error>> {
    let attrs_count = read_u16(bytes, *offset)? as usize;
    *offset += 2;

    let mut result = ClassAnnotationInfo {
        side: LoadSideType::Unknown,
        mod_info: None,
    };

    for _ in 0..attrs_count {
        if let Some(info) = process_attribute(bytes, offset, cp)? {
            if result.side == LoadSideType::Unknown {
                result.side = info.side;
            }
            if result.mod_info.is_none() {
                result.mod_info = info.mod_info;
            }
        }
    }

    if result.side == LoadSideType::Unknown && result.mod_info.is_none() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

/// 处理单个属性
///
/// - `bytes`: class 文件字节流
/// - `offset`: 当前读取偏移，处理后推进到该属性末尾
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回注解扫描结果；非注解属性返回 `None`
pub(super) fn process_attribute(
    bytes: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<Option<ClassAnnotationInfo>, Box<dyn std::error::Error>> {
    let attr_name_idx = read_u16(bytes, *offset)? as usize;
    let attr_len = read_u32(bytes, *offset + 2)? as usize;
    *offset += 6;

    // 安全检查：确保 attr_len 不会越界
    if *offset + attr_len > bytes.len() {
        *offset += attr_len;
        return Ok(None);
    }

    let result = if let Some(name) = cp_get(cp, attr_name_idx) {
        match name {
            "RuntimeVisibleAnnotations" | "RuntimeInvisibleAnnotations" => {
                check_annotations(&bytes[*offset..*offset + attr_len], cp)?
            }
            "RuntimeVisibleParameterAnnotations" => {
                check_parameter_annotations(&bytes[*offset..*offset + attr_len], cp)?
            }
            _ => None,
        }
    } else {
        None
    };

    *offset += attr_len;
    Ok(result)
}
