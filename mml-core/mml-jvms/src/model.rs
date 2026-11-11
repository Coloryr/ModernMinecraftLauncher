//! Java 运行时信息类型

use super::*;

/// Java 运行时信息
#[derive(Debug, Clone)]
pub struct JavaInfoObj {
    /// Java 显示名称，如 "OpenJDK-17.0.1-x86_64"
    pub name: String,
    /// Java 可执行文件的完整路径
    pub path: PathBuf,
    /// Java 完整版本号字符串，如 "17.0.1"
    pub version: String,
    /// Java 主版本号，如 8、11、17、21；占位条目为 -1 表示不可用
    pub major_version: i32,
    /// Java 启动器首词，取自版本行，如 "openjdk"、"java"
    pub java_type: String,
    /// 发行商，如 Oracle / Zulu / Temurin；认不出时回落为 OpenJDK / Java
    pub vendor: String,
    /// CPU 架构，与系统架构一致才会被版本匹配选中
    pub arch: ArchEnum,
}
