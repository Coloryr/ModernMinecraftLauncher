//! 配置编辑器 DTO —— 文件条目、NBT 树、区域文件区块表与二进制分段

use serde::Serialize;

/// 配置文件类型，前端据此选编辑器的语言、判断该走哪种视图
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileItemType {
    /// JSON：`.json` / `.jsonc` / `.mcmeta`
    Json,
    /// 带注释的 JSON：`.json5`
    Json5,
    /// TOML：`.toml`
    Toml,
    /// YAML：`.yaml` / `.yml`
    Yaml,
    /// 键值配置：`.properties` / `.cfg` / `.conf` / `.ini`
    Properties,
    /// XML：`.xml`
    Xml,
    /// 脚本：`.js` / `.mjs` / `.cjs`，KubeJS 用得多
    JavaScript,
    /// 文本形式的 NBT：`.snbt`
    Snbt,
    /// NBT 二进制：`.dat` / `.dat_old` / `.rio` / `.nbt`，走 NBT 树，只读
    Nbt,
    /// 区域文件：`.mca`，走区块表，只读
    Mca,
    /// 图片：`.png` / `.jpg` / `.jpeg` / `.gif` / `.webp`，走二进制视图，只读
    Image,
    /// 压缩包：`.zip` / `.jar` / `.7z` / `.tar` / `.gz` / `.tgz` / `.txz` / `.mrpack`，走二进制视图，只读
    Archive,
    /// 认不出来的扩展名与无扩展名，按纯文本处理
    Text,
}

/// 一个配置文件条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItemDto {
    /// 相对实例游戏目录（`.minecraft`）的路径，分隔符统一为 `/`
    pub name: String,
    /// 文件类型，见 [`FileItemType`]
    pub file_type: FileItemType,
}

/// 二进制文件里的一段字节
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BytesSliceDto {
    /// 这一段的起始偏移，单位字节；越界请求会被夹到文件末尾
    pub offset: u64,
    /// 这一段的字节数，即 `data` 解出来的长度；到文件末尾时可能不足请求的长度
    pub length: u32,
    /// 文件总大小，单位字节；前端据此算总页数与滑块位置
    pub total: u64,
    /// 这一段字节，标准 Base64
    pub data: String,
}

/// NBT 标签类型
///
/// 不加 `rename_all`：serde 与生成器都按变体名输出，与 `gui_config.rs` 的 `Theme` 同一写法。
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NbtKindDto {
    /// TAG_End，只在文件头作结束标记
    End,
    /// TAG_Byte，8 位整数
    Byte,
    /// TAG_Short，16 位整数
    Short,
    /// TAG_Int，32 位整数
    Int,
    /// TAG_Long，64 位整数
    Long,
    /// TAG_Float，单精度浮点
    Float,
    /// TAG_Double，双精度浮点
    Double,
    /// TAG_Byte_Array，字节数组
    ByteArray,
    /// TAG_String，UTF-8 字符串
    String,
    /// TAG_List，同类型标签的列表
    List,
    /// TAG_Compound，键值对复合结构
    Compound,
    /// TAG_Int_Array，32 位整数数组
    IntArray,
    /// TAG_Long_Array，64 位整数数组
    LongArray,
}

/// NBT 树上的一个节点
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NbtNodeDto {
    /// 键名；列表元素是下标字符串，根节点是空串
    pub name: String,
    /// 标签类型，见 [`NbtKindDto`]
    pub kind: NbtKindDto,
    /// 标量的值；复合、列表与数组为空串
    pub value: String,
    /// 元素个数，只对列表与三种数组有意义，其余为 0
    pub length: u32,
    /// 复合的成员，按键名排序
    pub children: Vec<NbtNodeDto>,
    /// 列表的元素，按原顺序
    pub items: Vec<NbtNodeDto>,
}

/// 区域文件里的一项区块
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McaChunkDto {
    /// 文件头表项下标，0–1023
    pub index: u32,
    /// 区域内的区块 X 坐标，0–31；表项下标就是 `(x & 31) + (z & 31) * 32`
    pub x: i32,
    /// 区域内的区块 Z 坐标，0–31
    pub z: i32,
    /// 数据起始偏移，单位字节
    pub pos: u32,
    /// 占用的扇区数，单位 4 KiB
    pub sectors: u8,
    /// 最后写入时间，Unix 秒
    pub time: u32,
}
