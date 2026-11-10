//! 启动参数与运行句柄的数据结构

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use mml_auth::LoginObj;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    game_saves::SaveObj,
    launcher::{LogEncoding, instance_setting_obj::ServerObj},
};

pub enum AutoJoinType {
    /// 无自动操作
    None,
    /// 进入指定存档，值为存档信息
    Save(Arc<SaveObj>),
    /// 进入指定服务器，值为服务器配置
    Server(Arc<ServerObj>),
}

/// ColorASM 注入所需的 IPC 连接参数
#[derive(Clone, Debug)]
pub struct MixinArg {
    /// IPC 服务端口
    pub port: u16,
    /// 连接验证密钥，启动时由 IPC 服务生成
    pub key: String,
}

/// 游戏启动所使用的参数
pub struct GameLaunchArg {
    /// 登录账户，含玩家名与令牌
    pub auth: LoginObj,
    /// 自动进入的操作
    pub auto: AutoJoinType,
    /// 是否以管理员方式启动
    pub admin: bool,
    /// ColorASM IPC 连接参数，未启用时为 `None`
    pub mixin: Option<MixinArg>,
    /// 启动时是否跳过服务器包同步；用户在升级弹窗里选了「直接启动」时为真
    pub skip_server_pack: bool,
    /// 这次同步里先不更新的文件，用户在选择弹窗里去掉的可选模组，取清单里的相对路径
    pub server_pack_skip: Vec<String>,
    /// 服务器包同步自己的取消令牌：界面上点「跳过更新」时只取消它，启动继续
    pub server_pack_cancel: Option<CancellationToken>,
    /// 这次用哪个图形调试器包一层
    pub debugger: LaunchDebugger,
    /// JVM 远程调试端口；`None` 表示不加 JDWP 参数
    pub jvm_debug: Option<u16>,
}

/// 启动时包在 Java 外面的图形调试器
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchDebugger {
    /// 直接起 Java，不注入
    None,
    /// RenderDoc：`renderdoccmd capture` 包住 Java，首次会先下载工具
    RenderDoc,
    /// Nsight Graphics：它的 `ngfx-capture` 包住 Java，路径是本机安装的命令行工具
    Nsight(PathBuf),
}

/// 游戏实例处理完成后的参数
pub struct GameRunObj {
    /// 游戏实例 uuid
    pub uuid: Uuid,
    /// 日志编码
    pub encoding: LogEncoding,
    /// 登陆的账户
    pub auth: LoginObj,
    /// 工作目录，即游戏根目录
    pub path: PathBuf,
    /// 实际启动的可执行文件；调试器启动时是调试器的命令行工具
    pub java: PathBuf,
    /// 实际启动参数；调试器启动时是它拉起 Java 的参数
    pub args: Vec<String>,
    /// 环境变量
    pub env: HashMap<String, String>,
    /// 是否管理员方式启动
    pub admin: bool,
    /// 实际包在 Java 外面的调试器；不是调试器启动时为 `None`
    pub debugger: LaunchDebugger,
}

/// 启动命令的参数集合
pub struct LaunchCmd {
    /// Java 可执行文件路径
    pub java: PathBuf,
    /// 工作目录
    pub dir: PathBuf,
    /// 启动参数
    pub arg: Vec<String>,
    /// 环境变量
    pub env: HashMap<String, String>,
}
