//! Minecraft 1.12.2 模组 Side 扫描器
//!
//! 用于扫描模组 jar 包中的 class 文件，检测每个 Mod 的 Side 信息

use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

use crate::game_mods::LoadSideType;

/// 常量池标签
mod constant_tags {
    pub const UTF8: u8 = 1;
    pub const INTEGER: u8 = 3;
    pub const FLOAT: u8 = 4;
    pub const LONG: u8 = 5;
    pub const DOUBLE: u8 = 6;
    pub const CLASS: u8 = 7;
    pub const STRING: u8 = 8;
    pub const FIELDREF: u8 = 9;
    pub const METHODREF: u8 = 10;
    pub const INTERFACE_METHODREF: u8 = 11;
    pub const NAME_AND_TYPE: u8 = 12;
    pub const METHOD_HANDLE: u8 = 15;
    pub const METHOD_TYPE: u8 = 16;
    pub const DYNAMIC: u8 = 17;
    pub const INVOKE_DYNAMIC: u8 = 18;
    pub const MODULE: u8 = 19;
    pub const PACKAGE: u8 = 20;
}

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
fn cp_get<'a>(cp: &'a ConstantPool, idx: usize) -> Option<&'a str> {
    cp.get(idx).and_then(|opt| opt.as_deref())
}

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
struct ClassAnnotationInfo {
    /// 这个类上的物理端类型
    side: LoadSideType,
    /// 类级 `@Mod` 注解信息
    mod_info: Option<ModAnnotation>,
}

/// @Mod 注解信息
struct ModAnnotation {
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
                // @SideOnly 优先（Forge 原生机制）
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

/// 扫描单个 class 文件
///
/// - `bytes`: class 文件字节流
///
/// # 返回值
///
/// 返回注解扫描结果；非 class 文件或无相关注解返回 `None`
fn scan_class_for_annotations(
    bytes: &[u8],
) -> Result<Option<ClassAnnotationInfo>, Box<dyn std::error::Error>> {
    if bytes.len() < 8 || &bytes[0..4] != &[0xCA, 0xFE, 0xBA, 0xBE] {
        return Ok(None);
    }

    let (cp, offset_after_pool) = parse_constant_pool(bytes)?;
    let mut offset = offset_after_pool;

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

    // 扫描类属性（包括类级别的 @Mod 和 @SideOnly）
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

/// 将 Java Modified UTF-8 转换为标准 UTF-8
/// Java 的 Modified UTF-8 与标准 UTF-8 有两个区别：
/// 1. U+0000 被编码为 0xC0 0x80，而不是 0x00
/// 2. U+10000 以上的字符用代理对编码后分别 UTF-8
///
/// - `data`: Modified UTF-8 字节流
///
/// # 返回值
///
/// 返回转换后的字符串；编码非法返回对应错误
fn from_modified_utf8(data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let byte = data[i];
        if byte == 0xC0 && i + 1 < data.len() && data[i + 1] == 0x80 {
            // Modified UTF-8 对 null 的编码 → 转为标准 0x00
            result.push(0x00);
            i += 2;
        } else if byte & 0x80 == 0 {
            // ASCII 单字节
            result.push(byte);
            i += 1;
        } else if byte & 0xE0 == 0xC0 {
            // 双字节序列
            if i + 1 >= data.len() {
                break;
            }
            result.push(byte);
            result.push(data[i + 1]);
            i += 2;
        } else if byte & 0xF0 == 0xE0 {
            // 三字节序列，标准 UTF-8
            if i + 2 >= data.len() {
                break;
            }
            // 检查是否为代理对，Modified UTF-8 特有
            if byte == 0xED
                && i + 5 < data.len()
                && (data[i + 1] & 0xF0) == 0xA0
                && data[i + 3] == 0xED
                && (data[i + 4] & 0xF0) == 0xB0
            {
                // 解码代理对并编码为 4 字节 UTF-8
                let high = (((data[i + 1] & 0x0F) as u32) << 6) | ((data[i + 2] & 0x3F) as u32);
                let low = (((data[i + 4] & 0x0F) as u32) << 6) | ((data[i + 5] & 0x3F) as u32);
                let codepoint = 0x10000 + ((high << 10) | low);
                // 编码为 4 字节 UTF-8
                result.push(0xF0 | ((codepoint >> 18) & 0x07) as u8);
                result.push(0x80 | ((codepoint >> 12) & 0x3F) as u8);
                result.push(0x80 | ((codepoint >> 6) & 0x3F) as u8);
                result.push(0x80 | (codepoint & 0x3F) as u8);
                i += 6;
            } else {
                result.push(byte);
                result.push(data[i + 1]);
                result.push(data[i + 2]);
                i += 3;
            }
        } else {
            // 无效字节，原样保留
            result.push(byte);
            i += 1;
        }
    }
    Ok(String::from_utf8(result)?)
}

/// 解析常量池，返回按常量池索引直接访问的 UTF8 字符串数组
///
/// # 参数
///
/// - `bytes`: class 文件字节流
///
/// # 返回值
///
/// 返回常量池与其结束偏移；解析越界返回对应错误
fn parse_constant_pool(bytes: &[u8]) -> Result<(ConstantPool, usize), Box<dyn std::error::Error>> {
    let pool_count = read_u16(bytes, 8)? as usize;
    // 索引 0 保留，实际条目从 1 到 pool_count-1
    let mut cp: ConstantPool = vec![None; pool_count];
    let mut offset = 10;

    let mut i = 1;
    while i < pool_count {
        if offset >= bytes.len() {
            return Err("常量池解析越界".into());
        }
        let tag = bytes[offset];
        offset += 1;

        match tag {
            constant_tags::UTF8 => {
                let len = read_u16(bytes, offset)? as usize;
                offset += 2;
                if offset + len > bytes.len() {
                    return Err("UTF8字符串越界".into());
                }
                let s = from_modified_utf8(&bytes[offset..offset + len])?;
                cp[i] = Some(s);
                offset += len;
            }
            constant_tags::INTEGER | constant_tags::FLOAT => {
                offset += 4;
            }
            constant_tags::LONG | constant_tags::DOUBLE => {
                // Long 和 Double 占用常量池的两个条目
                offset += 8;
                i += 1; // 跳过下一个不可用的池条目
            }
            constant_tags::CLASS
            | constant_tags::STRING
            | constant_tags::MODULE
            | constant_tags::PACKAGE
            | constant_tags::METHOD_TYPE => {
                offset += 2;
            }
            constant_tags::FIELDREF
            | constant_tags::METHODREF
            | constant_tags::INTERFACE_METHODREF
            | constant_tags::NAME_AND_TYPE => {
                offset += 4;
            }
            constant_tags::METHOD_HANDLE => {
                offset += 3;
            }
            constant_tags::DYNAMIC | constant_tags::INVOKE_DYNAMIC => {
                offset += 4;
            }
            _ => {}
        }
        i += 1;
    }

    Ok((cp, offset))
}

/// 扫描字段
///
/// # 参数
///
/// - `bytes`: class 文件字节流
/// - `offset`: 当前读取偏移（扫描后推进到字段区末尾）
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回注解扫描结果；无相关注解返回 `None`
fn scan_fields(
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
/// # 参数
///
/// - `bytes`: class 文件字节流
/// - `offset`: 当前读取偏移（扫描后推进到方法区末尾）
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回注解扫描结果；无相关注解返回 `None`
fn scan_methods(
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
/// # 参数
///
/// - `bytes`: class 文件字节流
/// - `offset`: 当前读取偏移（扫描后推进到属性区末尾）
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回注解扫描结果；无相关注解返回 `None`
fn scan_class_attributes(
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
/// # 参数
///
/// - `bytes`: class 文件字节流
/// - `offset`: 当前读取偏移（处理后推进到该属性末尾）
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回注解扫描结果；非注解属性返回 `None`
fn process_attribute(
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

/// 检查注解
///
/// # 参数
///
/// - `data`: 注解属性数据
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回 `@SideOnly` / `@Mod` 注解的扫描结果；无相关注解返回 `None`
fn check_annotations(
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
/// # 参数
///
/// - `data`: 注解属性数据
/// - `offset`: 当前读取偏移（跳过后推进到末尾）
///
/// # 返回值
///
/// 成功返回 `Ok(())`；解析失败返回对应错误
fn skip_annotation_pairs(
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
/// # 参数
///
/// - `data`: 注解属性数据
/// - `offset`: 当前读取偏移（解析后推进到注解末尾）
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回物理端类型；未能解析返回 `None`
fn parse_side_only(
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
/// # 参数
///
/// - `data`: 注解属性数据
/// - `offset`: 当前读取偏移（解析后推进到注解末尾）
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回 `@Mod` 注解信息；无 `modid` 返回 `None`
fn parse_mod_annotation(
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
/// # 参数
///
/// - `data`: 注解属性数据
/// - `offset`: 当前读取偏移（跳过后推进到值末尾）
/// - `tag`: 值类型标记
///
/// # 返回值
///
/// 成功返回 `Ok(())`；解析失败返回对应错误
fn skip_annotation_value(
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
/// # 参数
///
/// - `data`: 注解属性数据
/// - `cp`: 常量池
///
/// # 返回值
///
/// 返回 `@SideOnly` 注解的扫描结果；无相关注解返回 `None`
fn check_parameter_annotations(
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

/// 读取 u16（大端）
///
/// # 参数
///
/// - `bytes`: 字节流
/// - `offset`: 读取偏移
///
/// # 返回值
///
/// 返回读取的值；越界返回对应错误
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Box<dyn std::error::Error>> {
    if offset + 1 >= bytes.len() {
        return Err(format!("read_u16超出边界: offset={offset}, len={}", bytes.len()).into());
    }
    Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

/// 读取 u32（大端）
///
/// # 参数
///
/// - `bytes`: 字节流
/// - `offset`: 读取偏移
///
/// # 返回值
///
/// 返回读取的值；越界返回对应错误
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Box<dyn std::error::Error>> {
    if offset + 3 >= bytes.len() {
        return Err(format!("read_u32超出边界: offset={offset}, len={}", bytes.len()).into());
    }
    Ok(u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}
