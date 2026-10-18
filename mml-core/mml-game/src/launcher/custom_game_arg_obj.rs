//! 自定义加载器的游戏启动配置 DTO

use serde::{Deserialize, Serialize};

use crate::mojang::game_arg_obj::{GameArgObj, GameLibrariesObj};

/// 自定义游戏启动配置
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct CustomGameArgObj {
    /// 基础版本 JSON 字段，按扁平方式合并进本结构
    #[serde(flatten)]
    pub base: GameArgObj,
    /// 兼容的 Java 主版本列表
    #[serde(rename = "compatibleJavaMajors")]
    pub compatible_java_majors: Option<Vec<i32>>,
    /// 名称
    pub name: String,
    /// 排序序号
    pub order: i32,
    /// 组件 UID
    pub uid: String,
    /// 注入的 Tweaker 类
    #[serde(rename = "+tweakers")]
    pub add_tweakers: Option<Vec<String>>,
    /// 附加 JVM 参数
    #[serde(rename = "+jvmArgs")]
    pub add_jvm_args: Option<Vec<String>>,
    /// 版本号
    pub version: String,
    /// 这次使用的客户端 jar，替换语义
    #[serde(rename = "mainJar")]
    pub main_jar: GameLibrariesObj,
    /// MMC 的加号写法：追加进运行库，不替换客户端 jar
    #[serde(rename = "+mainJar")]
    pub add_main_jar: Option<GameLibrariesObj>,
    /// 对应的 Minecraft 版本
    #[serde(rename = "_minecraftVersion")]
    pub minecraft_version: Option<String>,
}

impl Default for CustomGameArgObj {
    fn default() -> Self {
        Self {
            base: Default::default(),
            compatible_java_majors: Default::default(),
            name: Default::default(),
            order: Default::default(),
            uid: Default::default(),
            add_tweakers: Default::default(),
            add_jvm_args: Default::default(),
            version: Default::default(),
            main_jar: Default::default(),
            add_main_jar: Default::default(),
            minecraft_version: Default::default(),
        }
    }
}
