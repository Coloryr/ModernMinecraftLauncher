//! 从 ColorMC 迁移的跨 IPC 传输对象：探测结果 / 迁移进度 / 兼容性报告

use serde::Serialize;

/// ColorMC 工作目录探测结果，只用于"首次启动问一次"那个弹窗
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorMcInfoDto {
    /// 探测到的工作目录
    pub path: String,
    /// 命中来源：`run` = `%LOCALAPPDATA%\ColorMC\run` 记的路径 / `app` = `colormc://`
    /// 协议注册的安装位置 / `default` = 平台默认位置 / `fallback` = ColorMC 的数据目录
    pub from: String,
    /// 该目录里的实例数，即 `minecraft/instances` 下的子目录数
    pub instances: u32,
    /// 工作目录根下的**顶层条目名**，有序；弹窗用它列出"将搬入什么"
    pub entries: Vec<String>,
    /// 账户文件 `auth.json` 的实际位置，**在工作目录之外**，所以不会被迁移；
    /// 本机没有时为空串
    pub auth_path: String,
}

/// 迁移进度，长任务期间推送；`stage` 由前端翻成文字
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorMcProgressDto {
    /// 阶段：`scan` = 统计待搬条目 / `copy` / `move` / `check` = 兼容性检测
    pub stage: String,
    /// 已处理数
    pub done: u32,
    /// 总数，未知为 0
    pub total: u32,
    /// 当前正在处理的条目，相对路径；没有则为空串
    pub text: String,
}

/// 一条兼容性结论
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorMcCompatDto {
    /// 条目，相对 ColorMC 工作目录，如 `config.json` / `minecraft` / `java`
    pub name: String,
    /// 级别：`ok` 能直接用 / `partial` 能用但有偏差或会丢东西 / `extra` 对方读不懂，留着不影响
    pub level: String,
    /// 说明的 i18n key，前端用 `t()` 翻译，都是 `colormc.compat.*`
    pub note: String,
}

/// 迁移结果 + 兼容性检测报告
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorMcReportDto {
    /// `true` = 移动，源目录已删；`false` = 复制，源目录保留
    pub moved: bool,
    /// 源目录
    pub source: String,
    /// 实际搬运成功的顶层条目数
    pub entries: u32,
    /// 没搬成功的条目，相对路径；占用 / 无权限等，单个失败不影响其它条目
    pub failed: Vec<String>,
    /// 实例：M²L **能识别**的，即 `game.json` 解析通过
    pub instances_ok: Vec<String>,
    /// 实例：**识别不了**的，缺 `game.json` 或解析失败——文件搬过来了，但 M²L 看不到它
    pub instances_bad: Vec<String>,
    /// 实例：还带着 ColorMC 的 `guisetting.json` 的。M²L 读的是自己的 `gui_setting.json`，
    /// 两边 `Groups` 结构不同，文件名故意不一样，所以这些实例的模组分组 / 备注 /
    /// 日志设置**没跟过来**，ColorMC 那份文件留着，不会被覆盖
    pub instances_legacy_gui: Vec<String>,
    /// 逐条兼容性结论，按搬过来的顶层条目组织
    pub compat: Vec<ColorMcCompatDto>,
    /// 从 ColorMC 的 `auth.json` 新合并进来的账户数；没有账户文件或全都已存在时为 0
    pub accounts: u32,
}
