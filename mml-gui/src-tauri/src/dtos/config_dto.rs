//! 配置编辑器 DTO —— 文件条目、NBT 树、区域文件区块表与二进制分段

use serde::{Deserialize, Serialize};

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
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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
    /// 键名；列表与数组的元素是下标字符串，根节点是空串
    pub name: String,
    /// 标签类型，见 [`NbtKindDto`]
    pub kind: NbtKindDto,
    /// 标量的值；复合、列表与数组为空串
    pub value: String,
    /// 元素个数，只对列表与三种数组有意义，其余为 0
    pub length: u32,
    /// 复合的成员，按键名排序
    pub children: Vec<NbtNodeDto>,
    /// 列表与数组的元素，按原顺序；数组超过编辑上限时为空，只有 [`NbtNodeDto::length`] 可用
    pub items: Vec<NbtNodeDto>,
}

/// 文本配置文件的内容与校验值
///
/// 校验值是**全文**的 SHA1，保存时按它核对磁盘上还是不是打开时的这一份，
/// 避免把别的窗口（如资源管理改 options.txt）的改动冲掉。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigTextDto {
    /// 全文
    pub text: String,
    /// 全文的 SHA1，十六进制小写
    pub sha1: String,
}

/// 一次行替换：把原文 `start` 起的 `delete_count` 行换成 `lines`
///
/// 行号一律**相对原文**，各段按 `start` 升序、互不重叠（可以同点插入）。
/// 前端算差异用的是 Monaco 的 LcsDiff（`mml-vue/src/lib/textDiff.ts`），
/// 后端套差异的是 `mml_sys::text_diff`。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHunkDto {
    /// 原文里的起始行号，从 0 起
    pub start: u32,
    /// 要替换掉的行数；0 表示纯插入
    pub delete_count: u32,
    /// 替换成的新行；空表示纯删除
    pub lines: Vec<String>,
}

/// NBT 文件的展示树与校验值
///
/// 校验值是**文件字节**的 SHA1（NBT 是二进制，不能像文本那样按解码后的文本算），
/// 保存时按它核对磁盘上还是不是打开时的这一份。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigNbtDto {
    /// 根节点
    pub node: NbtNodeDto,
    /// 文件字节的 SHA1，十六进制小写
    pub sha1: String,
}

/// NBT 改动的一次操作
///
/// 不加 `rename_all`：serde 与生成器都按变体名输出，与 [`NbtKindDto`] 同一写法。
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NbtOpDto {
    /// 改某个标签的值：标量按它自己的类型解析，数组按逗号分隔的一串数
    Set,
    /// 新建一个标签并挂进父容器：复合按路径最后一段当键名，列表按它当插入下标
    Add,
    /// 改复合成员的键名：`value` 是新键名，整棵子树原样搬过去
    Rename,
    /// 删掉一个标签：复合删成员，列表删元素
    Remove,
}

/// NBT 树的一次改动
///
/// `path` 一律**相对当时的树**：同一批改动按顺序套用，后一条看得到前面几条的结果。
/// 前端每做一步就记一条，保存时按发生顺序整批发过来，与它本地那棵树的结果一致。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NbtEditDto {
    /// 操作，见 [`NbtOpDto`]
    pub op: NbtOpDto,
    /// 从根往下的路径，逐层是键名或列表下标的十进制字符串；空路径就是根标签
    pub path: Vec<String>,
    /// `Add` 时新标签的类型；`Set` / `Rename` / `Remove` 不用，传 `null`
    pub kind: Option<NbtKindDto>,
    /// `Set` / `Add` 的新值文本：数字按目标类型解析，字符串原样，数组是逗号分隔的一串数；
    /// `Rename` 时是新键名；`Remove` 与新建复合 / 列表时传空串
    pub value: String,
}

/// 「查找实体 / 方块」的命中结果，见 `config_find_chunk`
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkFindDto {
    /// 命中的区域文件，来源内的相对路径
    pub file: String,
    /// 命中的区块在文件头里的表项下标，0–1023
    pub index: u32,
    /// 命中节点从区块根往下的路径，逐层是键名或列表下标；没命中时为空
    pub path: Vec<String>,
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
