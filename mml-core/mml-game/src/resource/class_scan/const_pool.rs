//! 常量池解析
//!
//! class 文件里的字符串都在常量池里，这里把它解成按索引访问的数组。

/// 常量池标签
pub(super) mod constant_tags {
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

/// 从常量池索引获取 UTF8 字符串引用
///
/// - `cp`: 常量池
/// - `idx`: 常量池索引
///
/// # 返回值
///
/// 返回对应的字符串引用；索引无效返回 `None`
pub(super) fn cp_get<'a>(cp: &'a ConstantPool, idx: usize) -> Option<&'a str> {
    cp.get(idx).and_then(|opt| opt.as_deref())
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
pub(super) fn from_modified_utf8(data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
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

/// 常量池解析结果：池本身与它占用的字节区间
pub(super) struct ParsedConstantPool {
    /// 按常量池索引直接访问的 UTF8 字符串数组，见 [`ConstantPool`]
    pub(super) pool: ConstantPool,
    /// 常量池结束后的字节偏移，后续的字段 / 方法解析从这里接着读
    pub(super) end_offset: usize,
}

/// 解析常量池，返回按常量池索引直接访问的 UTF8 字符串数组
///
/// - `bytes`: class 文件字节流
///
/// # 返回值
///
/// 见 [`ParsedConstantPool`]；解析越界返回对应错误
pub(super) fn parse_constant_pool(
    bytes: &[u8],
) -> Result<ParsedConstantPool, Box<dyn std::error::Error>> {
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

    Ok(ParsedConstantPool {
        pool: cp,
        end_offset: offset,
    })
}

/// 读取 u16，大端序
///
/// - `bytes`: 字节流
/// - `offset`: 读取偏移
///
/// # 返回值
///
/// 返回读取的值；越界返回对应错误
pub(super) fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Box<dyn std::error::Error>> {
    if offset + 1 >= bytes.len() {
        return Err(format!("read_u16超出边界: offset={offset}, len={}", bytes.len()).into());
    }
    Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

/// 读取 u32，大端序
///
/// - `bytes`: 字节流
/// - `offset`: 读取偏移
///
/// # 返回值
///
/// 返回读取的值；越界返回对应错误
pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Box<dyn std::error::Error>> {
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
