//! mixin 类的注解解析
//!
//! 从 class 文件里读出 `@Mixin` 声明的目标类，以及各注入注解改写的目标方法。
//! 只认 mixin 自己那几个注解，其余注解按值跳过。

use super::const_pool::{cp_get, parse_constant_pool, read_u16, read_u32};

/// 常量池：按常量池索引存储 UTF8 字符串，索引从 1 开始
type ConstantPool = Vec<Option<String>>;

/// 注解里的一个元素值
#[derive(Debug, Clone)]
pub(super) enum ElementValue {
    /// 字符串，标签 `s`
    Str(String),
    /// 数组，标签 `[`
    List(Vec<ElementValue>),
    /// 类字面量，标签 `c`，存点分类名
    Class(String),
    /// 嵌套注解、枚举、数值、布尔这类这里用不到的值，只为把偏移推过去
    Other,
}

/// 一个注解
#[derive(Debug, Clone)]
pub(super) struct Annotation {
    /// 注解描述符，如 `Lorg/spongepowered/asm/mixin/Mixin;`
    pub(super) desc: String,
    /// 元素名与值，按注解里的声明顺序
    pub(super) values: Vec<(String, ElementValue)>,
}

/// 一条注入
#[derive(Debug, Clone)]
pub(crate) struct Injection {
    /// 注入方式 id，见 `MixinTarget::kind`
    pub(crate) kind: &'static str,
    /// 目标方法名，注解里带 owner 或描述符时只留方法名
    pub(crate) methods: Vec<String>,
}

/// mixin 类的解析结果
#[derive(Debug, Default)]
pub(crate) struct MixinClass {
    /// `@Mixin` 声明的目标类，点分
    pub(crate) targets: Vec<String>,
    /// 方法上的注入注解
    pub(crate) injections: Vec<Injection>,
}

/// 解析一个 mixin 类
///
/// - `bytes`: class 文件字节流
///
/// # 返回值
///
/// 目标类与方法级注入；不是 class 文件或结构非法时返回错误
pub(crate) fn parse_mixin(bytes: &[u8]) -> Result<MixinClass, Box<dyn std::error::Error>> {
    if bytes.len() < 10 || bytes[0..4] != [0xCA, 0xFE, 0xBA, 0xBE] {
        return Err("不是 class 文件".into());
    }

    let parsed = parse_constant_pool(bytes)?;
    let cp = parsed.pool;
    let mut offset = parsed.end_offset;

    // 访问标志、本类、父类
    offset += 6;
    let interfaces = read_u16(bytes, offset)? as usize;
    offset += 2 + interfaces * 2;

    // 字段：只看注解的话可以整段跳过，但要按属性表长度走准
    let fields = read_u16(bytes, offset)? as usize;
    offset += 2;
    for _ in 0..fields {
        offset += 6;
        read_attributes(bytes, &mut offset, &cp)?;
    }

    let methods_count = read_u16(bytes, offset)? as usize;
    offset += 2;
    let mut methods = Vec::with_capacity(methods_count);
    for _ in 0..methods_count {
        let name_idx = read_u16(bytes, offset + 2)? as usize;
        offset += 6;
        let annotations = read_attributes(bytes, &mut offset, &cp)?;
        let name = cp_get(&cp, name_idx).unwrap_or_default().to_string();
        methods.push((name, annotations));
    }

    let annotations = read_attributes(bytes, &mut offset, &cp)?;

    Ok(MixinClass {
        targets: mixin_targets(&annotations),
        injections: mixin_injections(&methods),
    })
}

/// 取 `@Mixin` 声明的目标类
///
/// `targets` 写字符串，`value` 写类字面量，两种都认。
fn mixin_targets(annotations: &[Annotation]) -> Vec<String> {
    let mut targets = Vec::new();
    for annotation in annotations {
        if annotation.desc != "Lorg/spongepowered/asm/mixin/Mixin;" {
            continue;
        }
        for (name, value) in &annotation.values {
            let values = match (name.as_str(), value) {
                ("targets", ElementValue::Str(value)) => vec![value.replace('/', ".")],
                ("targets", ElementValue::List(list)) => list
                    .iter()
                    .filter_map(|item| match item {
                        ElementValue::Str(value) => Some(value.replace('/', ".")),
                        _ => None,
                    })
                    .collect(),
                ("value", ElementValue::List(list)) => list
                    .iter()
                    .filter_map(|item| match item {
                        ElementValue::Class(value) => Some(value.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => continue,
            };
            for value in values {
                if !value.is_empty() && !targets.contains(&value) {
                    targets.push(value);
                }
            }
        }
    }
    targets
}

/// 收方法级注入注解
fn mixin_injections(methods: &[(String, Vec<Annotation>)]) -> Vec<Injection> {
    let mut injections = Vec::new();
    for (method, annotations) in methods {
        for annotation in annotations {
            let Some(kind) = injection_kind(&annotation.desc) else {
                continue;
            };
            let methods = match kind {
                // @Overwrite 直接顶替同名方法，注解里不写方法名
                "overwrite" => vec![method.clone()],
                // @Accessor / @Invoker 改的是字段或方法，名字写在 value 里，没写就是自己
                "accessor" | "invoker" => match annotation_string(annotation, "value") {
                    Some(value) => vec![value],
                    None => vec![method.clone()],
                },
                _ => annotation_strings(annotation, "method")
                    .iter()
                    .map(|value| method_of(value))
                    .collect(),
            };
            if methods.is_empty() {
                continue;
            }
            injections.push(Injection { kind, methods });
        }
    }
    injections
}

/// 注解描述符对应的注入方式
fn injection_kind(desc: &str) -> Option<&'static str> {
    const PREFIX: &str = "Lorg/spongepowered/asm/mixin/";
    let name = desc.strip_prefix(PREFIX)?.strip_suffix(';')?;
    Some(match name {
        "injection/Inject" => "inject",
        "injection/Redirect" => "redirect",
        "injection/ModifyArg" => "modify_arg",
        "injection/ModifyArgs" => "modify_args",
        "injection/ModifyVariable" => "modify_variable",
        "injection/ModifyConstant" => "modify_constant",
        "injection/ModifyExpressionValue" => "modify_expression_value",
        "injection/WrapOperation" => "wrap_operation",
        "injection/WrapWithCondition" => "wrap_with_condition",
        "Overwrite" => "overwrite",
        "gen/Accessor" => "accessor",
        "gen/Invoker" => "invoker",
        _ => return None,
    })
}

/// 目标方法名：注解里可能写成 `Lowner;name(desc)V`，只取方法名
fn method_of(raw: &str) -> String {
    let head = raw.split('(').next().unwrap_or(raw);
    head.rsplit(';').next().unwrap_or(head).trim().to_string()
}

/// 取注解里的单个字符串
fn annotation_string(annotation: &Annotation, name: &str) -> Option<String> {
    annotation
        .values
        .iter()
        .find(|(key, _)| key == name)
        .and_then(|(_, value)| match value {
            ElementValue::Str(value) => Some(value.clone()),
            _ => None,
        })
}

/// 取注解里的字符串或字符串数组
fn annotation_strings(annotation: &Annotation, name: &str) -> Vec<String> {
    let Some((_, value)) = annotation.values.iter().find(|(key, _)| key == name) else {
        return Vec::new();
    };
    match value {
        ElementValue::Str(value) => vec![value.clone()],
        ElementValue::List(list) => list
            .iter()
            .filter_map(|item| match item {
                ElementValue::Str(value) => Some(value.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// 读一段属性表，返回其中的注解
fn read_attributes(
    bytes: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<Vec<Annotation>, Box<dyn std::error::Error>> {
    let count = read_u16(bytes, *offset)? as usize;
    *offset += 2;

    let mut annotations = Vec::new();
    for _ in 0..count {
        let name_idx = read_u16(bytes, *offset)? as usize;
        let len = read_u32(bytes, *offset + 2)? as usize;
        *offset += 6;
        if *offset + len > bytes.len() {
            return Err("属性长度越界".into());
        }

        if matches!(
            cp_get(cp, name_idx),
            Some("RuntimeVisibleAnnotations" | "RuntimeInvisibleAnnotations")
        ) {
            annotations.extend(parse_annotations(&bytes[*offset..*offset + len], cp)?);
        }
        *offset += len;
    }

    Ok(annotations)
}

/// 读注解表
fn parse_annotations(
    data: &[u8],
    cp: &ConstantPool,
) -> Result<Vec<Annotation>, Box<dyn std::error::Error>> {
    let count = read_u16(data, 0)? as usize;
    let mut offset = 2;
    let mut annotations = Vec::with_capacity(count);
    for _ in 0..count {
        annotations.push(parse_annotation(data, &mut offset, cp)?);
    }
    Ok(annotations)
}

/// 读一个注解
fn parse_annotation(
    data: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<Annotation, Box<dyn std::error::Error>> {
    let type_idx = read_u16(data, *offset)? as usize;
    *offset += 2;
    let desc = cp_get(cp, type_idx).unwrap_or_default().to_string();

    let count = read_u16(data, *offset)? as usize;
    *offset += 2;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let name_idx = read_u16(data, *offset)? as usize;
        *offset += 2;
        let name = cp_get(cp, name_idx).unwrap_or_default().to_string();
        let value = parse_value(data, offset, cp)?;
        values.push((name, value));
    }

    Ok(Annotation { desc, values })
}

/// 读一个元素值
fn parse_value(
    data: &[u8],
    offset: &mut usize,
    cp: &ConstantPool,
) -> Result<ElementValue, Box<dyn std::error::Error>> {
    if *offset >= data.len() {
        return Err("注解值越界".into());
    }
    let tag = data[*offset] as char;
    *offset += 1;

    Ok(match tag {
        's' => {
            let idx = read_u16(data, *offset)? as usize;
            *offset += 2;
            ElementValue::Str(cp_get(cp, idx).unwrap_or_default().to_string())
        }
        'c' => {
            let idx = read_u16(data, *offset)? as usize;
            *offset += 2;
            ElementValue::Class(class_of_descriptor(cp_get(cp, idx).unwrap_or_default()))
        }
        'e' => {
            *offset += 4;
            ElementValue::Other
        }
        '@' => {
            parse_annotation(data, offset, cp)?;
            ElementValue::Other
        }
        '[' => {
            let count = read_u16(data, *offset)? as usize;
            *offset += 2;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                list.push(parse_value(data, offset, cp)?);
            }
            ElementValue::List(list)
        }
        'B' | 'C' | 'S' | 'I' | 'J' | 'F' | 'D' | 'Z' => {
            *offset += 2;
            ElementValue::Other
        }
        _ => return Err(format!("未知的注解值标签: {tag}").into()),
    })
}

/// 类字面量的描述符转点分类名，`[Lnet/minecraft/Foo;` 这种数组取元素类型
fn class_of_descriptor(desc: &str) -> String {
    let desc = desc.trim_start_matches('[');
    let desc = desc.strip_prefix('L').unwrap_or(desc);
    let desc = desc.strip_suffix(';').unwrap_or(desc);
    desc.replace('/', ".")
}
