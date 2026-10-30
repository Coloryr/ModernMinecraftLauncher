//! 启动参数 DTO
use serde::{Deserialize, Serialize};

/// 附加环境变量，键值对
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarLineDto {
    /// 变量名
    pub key: String,
    /// 变量值
    pub value: String,
}

/// 实例启动参数：内存 / 窗口 / Java，加上扩展参数、自定义执行与代理
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceArgsDto {
    /// 最大内存，单位 MB
    pub memory: i64,
    /// 最小内存，单位 MB
    pub min_memory: i64,
    /// 是否全屏
    pub fullscreen: bool,
    /// 窗口宽度
    pub width: i64,
    /// 窗口高度
    pub height: i64,
    /// 内存是否跟随全局设置
    ///
    /// 实例没单独设最大 / 最小内存时为 true：这时 [`Self::memory`] 与 [`Self::min_memory`]
    /// 填的是**全局的生效值**，界面上那两个框只作展示、不可编辑。
    pub memory_inherit: bool,
    /// 窗口大小是否跟随全局设置
    ///
    /// 实例没单独设全屏 / 宽 / 高时为 true，[`Self::fullscreen`] 等同理是全局生效值。
    pub window_inherit: bool,
    /// 使用的 Java 名；"custom" 表示自定义路径
    pub java_name: String,
    /// 自定义 Java 路径
    pub java_path: String,
    /// GC 回收器：auto / g1gc / zgc / none / custom
    pub gc: String,
    /// 自定义 GC 参数，gc 为 custom 时使用
    pub gc_custom: String,
    /// 自定义主类
    pub main_class: String,
    /// 附加 JVM 参数
    pub jvm_args: Vec<String>,
    /// 附加游戏参数
    pub game_args: Vec<String>,
    /// 附加 classpath
    pub class_path: Vec<String>,
    /// 附加环境变量
    pub env_vars: Vec<EnvVarLineDto>,
    /// 是否把 HOME / APPDATA / XDG 等系统目录隔离到实例目录
    pub isolate_home: bool,
    /// 游戏内语言
    pub lang: String,
    /// 日志编码：utf8 / gbk
    pub log_encoding: String,
    /// 启动前执行
    pub pre_enabled: bool,
    /// 启动前执行的命令
    pub pre_cmd: String,
    /// 启动后执行
    pub post_enabled: bool,
    /// 启动后执行的命令
    pub post_cmd: String,
    /// 游戏内代理
    pub proxy_ip: String,
    /// 代理端口
    pub proxy_port: i64,
    /// 代理用户名
    pub proxy_user: String,
    /// 代理密码
    pub proxy_pass: String,
    /// 自动加入服务器
    pub server_ip: String,
    /// 服务器端口
    pub server_port: i64,
    /// 是否自动加入服务器
    pub join_server: bool,
}

impl Default for InstanceArgsDto {
    /// 默认启动参数，不是全零：内存 4096 / 窗口 854×480 这些是可用初值
    ///
    /// # 返回值
    ///
    /// 返回新建实例可直接使用的启动参数
    fn default() -> Self {
        Self {
            memory: 4096,
            min_memory: 1024,
            fullscreen: false,
            width: 854,
            height: 480,
            // 新实例默认跟随全局：内存与窗口都不单独设，启动时用 mml_config 里那份
            memory_inherit: true,
            window_inherit: true,
            java_name: String::new(),
            java_path: String::new(),
            gc: "auto".into(),
            gc_custom: String::new(),
            main_class: String::new(),
            jvm_args: Vec::new(),
            game_args: Vec::new(),
            class_path: Vec::new(),
            env_vars: Vec::new(),
            isolate_home: true,
            lang: "zh_cn".into(),
            log_encoding: "utf8".into(),
            pre_enabled: false,
            pre_cmd: String::new(),
            post_enabled: false,
            post_cmd: String::new(),
            proxy_ip: String::new(),
            proxy_port: 0,
            proxy_user: String::new(),
            proxy_pass: String::new(),
            server_ip: String::new(),
            server_port: 0,
            join_server: false,
        }
    }
}
