//! Java 运行时 DTO
use serde::{Deserialize, Serialize};

/// Java 运行时信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfoDto {
    /// 显示名
    pub name: String,
    /// 可执行文件路径
    pub path: String,
    /// 完整版本号
    pub version: String,
    /// 主版本号
    pub major: i32,
    /// 发行类型：JDK / JRE
    pub java_type: String,
    /// 位数：x86 / x64
    pub arch: String,
}

/// Java 压缩包导入进度，事件名 `settings-java-progress`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaImportProgressDto {
    /// 当前阶段 ID，如 extract 解包 / detect 识别
    pub state: String,
    /// 当前进度
    pub now: u32,
    /// 总量
    pub total: u32,
    /// 说明文本
    pub sub_text: Option<String>,
}
