//! 主窗口 DTO：事件负载 + 实例更新补丁，前端 wire，camelCase
//!
//! 从 `../windows/main.rs` 挪出：这些类型只用于跨 Tauri IPC，即事件 / 命令入参，
//! 无业务方法、不参与磁盘持久化，归入 DTO 层。

use serde::{Deserialize, Serialize};

/// 双层 Option 反序列化：区分"没传"与"清空"
///
/// 配合 `#[serde(default)]`：字段缺失 -> `None` 表示不改，
/// `null` -> `Some(None)` 表示清空，有值 -> `Some(Some(v))` 表示更新。
///
/// - `de`: serde 反序列化器
///
/// # 返回值
///
/// 反序列化结果；成功时外层恒为 `Some`，`null` 与"有值"由内层区分
fn double_option<'de, T, D>(de: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(de).map(Some)
}

/// 游戏内语言条目，供语言设置下拉使用
///
/// `name` 是给人看的显示名，取自该语言文件里的 `language.name`，如“简体中文”；
/// 资源没下全时它就是 `code`，所以前端直接用 `name` 显示即可。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceLangDto {
    /// 语言代码，如 zh_cn
    pub code: String,
    /// 显示名，如 简体中文
    pub name: String,
}

/// 本机内存，单位 MiB，供启动参数里的内存设置作参考值
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMemoryDto {
    /// 物理内存总量；查询失败为 0，前端不显示
    pub total: u64,
    /// 当前可用内存；查询失败为 0，前端不显示
    pub free: u64,
}

/// 核心数据加载完成事件
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadState {
    /// 是否加载成功
    pub ok: bool,
    /// 失败文案，成功时为 None
    pub error: Option<String>,
}

/// Minecraft 新闻条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    /// 条目 ID
    pub id: i64,
    /// 标题
    pub title: String,
    /// 日期
    pub date: String,
    /// 分类标签
    pub tag: String,
    /// 配图地址
    pub image: String,
    /// 原文链接，点击卡片时用系统浏览器打开
    pub url: String,
}

/// 游戏日志事件
///
/// thread / level / category 由核心的日志解析填充，与 InstanceRuntimeLog 同一套正则，
/// 解析不出的行为空串，前端筛选器据此显示"全部"
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    /// 实例 UUID
    pub uuid: String,
    /// 时间
    pub time: String,
    /// 日志原文
    pub text: String,
    /// 线程名，解析不出为空串
    pub thread: String,
    /// 级别，解析不出为空串
    pub level: String,
    /// 分类，解析不出为空串
    pub category: String,
    /// 是否清空日志，前端清屏
    pub clear: bool,
}

/// 游戏日志行，即 `log_get_runtime` 返回的历史条目，字段同 LogEvent 去掉事件头
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    /// 时间
    pub time: String,
    /// 日志原文
    pub text: String,
    /// 线程名，解析不出为空串
    pub thread: String,
    /// 级别，解析不出为空串
    pub level: String,
    /// 分类，解析不出为空串
    pub category: String,
}

/// 启动状态事件
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateEvent {
    /// 实例 UUID
    pub uuid: String,
    /// 状态标识
    pub state: String,
    /// 启动进度百分比，0–100；阶段由后端 LaunchState 定，前端不自算。
    /// 无进度语义的状态如 launching 为 None，前端走滚动条
    pub progress: Option<u32>,
}

/// 游戏退出事件
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitEvent {
    /// 实例 UUID
    pub uuid: String,
    /// 进程退出码
    pub code: i32,
}

/// 启动错误事件
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvent {
    /// 实例 UUID，启动前错误为 None
    pub uuid: Option<String>,
    /// 错误文案
    pub message: String,
}

/// 实例变更事件 instance-change
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceChangeEvent {
    /// 变更类型：add / edit / remove / group
    pub r#type: String,
}

/// 实例更新补丁，即前端 Partial<InstanceInfoDto> 的 IPC 形态
///
/// 字段缺失 = 不改；`null` = 清空，仅标注 `double_option` 的字段支持。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePatch {
    /// 分组 uuid，`null` = 默认分组
    ///
    /// `Option<Option<String>>`：外层区分"改不改"，内层是"改成什么"。
    /// **必须带 `double_option`** —— 否则前端发 `{ group: null }` 表示移到默认分组
    /// 会被 serde 解成外层 `None`，被当成"这个字段不修改"，切换分组静默失效。
    /// 删分组时前端正是逐个把实例的 group 置 null，缺了它整段等于没执行。
    #[serde(default, deserialize_with = "double_option")]
    pub group: Option<Option<String>>,
    /// 实例名
    pub name: Option<String>,
    /// 游戏版本号
    pub version: Option<String>,
    /// 游戏版本类型
    pub version_type: Option<String>,
    /// 加载器类型
    pub loader: Option<String>,
    /// 加载器版本号
    #[serde(default, deserialize_with = "double_option")]
    pub loader_version: Option<Option<String>>,
    /// 整合包平台
    #[serde(default, deserialize_with = "double_option")]
    pub modpack_type: Option<Option<String>>,
    /// 整合包项目 ID
    #[serde(default, deserialize_with = "double_option")]
    pub pid: Option<Option<String>>,
    /// 整合包文件 ID
    #[serde(default, deserialize_with = "double_option")]
    pub fid: Option<Option<String>>,
    /// 在线整合包地址
    #[serde(default, deserialize_with = "double_option")]
    pub server_url: Option<Option<String>>,
    /// 游戏内语言
    pub lang: Option<String>,
    /// 日志编码：utf8 / gbk
    pub log_encoding: Option<String>,
}

/// 方块列表条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockItemDto {
    /// 方块ID，如 minecraft:stone
    pub id: String,
    /// 显示名，按请求语言翻译，miss 时回退 id 尾段
    pub name: String,
    /// 创造分组尾段，如 buildingBlocks / natural / …
    pub cat: String,
    /// 贴图地址，mml-image 完整 URL，带 ?v= 版本参数
    pub image: String,
}

/// 图标裁剪的源图预览，供"修改图标"的截图弹窗使用
///
/// `src` 是**降采样后**的预览图 data URL，只用于选范围；而 `width` / `height` 是**原图**
/// 尺寸：选区坐标按原图算，后端裁的也是原图，所以这里不能给预览尺寸。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconSourceDto {
    /// 预览图 data URL，形如 `data:image/png;base64,`，可直接放进 `<img src>`
    pub src: String,
    /// 原图宽，单位 px
    pub width: u32,
    /// 原图高，单位 px
    pub height: u32,
}

/// 方块贴图渲染状态
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStatusDto {
    /// 已有渲染结果，即 blocks() 非空
    pub rendered: bool,
    /// 已渲染的游戏版本，未渲染为空串
    pub version: String,
    /// 正在渲染
    pub running: bool,
    /// 进度：已处理数
    pub now: u32,
    /// 进度：总数
    pub total: u32,
    /// 进度文字，渲染中才有
    pub text: Option<String>,
    /// 上次渲染失败的错误信息
    pub error: Option<String>,
}
