//! 界面回调接口
//!
//! 内核通过这些 trait 与界面交互，用于询问与汇报进度，
//! 由桌面壳实现后传入；全部以 `Option<Arc<...>>` 形式使用，可为空，无界面时跳过交互。

use std::sync::Arc;

use async_trait::async_trait;
use mml_auth::LoginObj;

use crate::{GameInstance, launcher::instance_setting_obj::InstanceSettingObj};

/// 启动界面回调
pub type LaunchGui = Option<Arc<dyn ILaunchGui>>;
/// 实例创建界面回调
pub type AddInstanceGui = Option<Arc<dyn IAddInstanceGui>>;
/// 整合包安装界面回调
pub type AddModPackGui = Option<Arc<dyn IAddModPackGui>>;
/// 进度条界面回调
pub type ProgressGui = Option<Arc<dyn IProgressGui>>;

/// 项目安装状态，按变体顺序从 `DownloadPack` 推进到 `Done`
pub enum AddModPackState {
    /// 下载整合包
    DownloadPack,
    /// 读取整合包信息
    ReadInfo,
    /// 获取文件信息
    GetInfo,
    /// 下载文件
    DownloadFile,
    /// 解压文件
    Extract,
    /// 完成
    Done,
}

/// 实例创建界面回调
#[async_trait]
pub trait IAddInstanceGui: Send + Sync {
    /// 是否同意替换名字
    ///
    /// - `name`: 重名的实例名
    ///
    /// # 返回值
    ///
    /// 返回 `true` 表示同意自动改名后继续创建
    async fn name_replace(&self, name: &str) -> bool;
    /// 是否同意覆盖
    ///
    /// - `obj`: 已存在的重名实例
    ///
    /// # 返回值
    ///
    /// 返回 `true` 表示覆盖该实例
    async fn overwrite(&self, obj: GameInstance) -> bool;
}

/// 整合包安装界面回调
pub trait IAddModPackGui: Send + Sync {
    /// 设置整合包安装状态
    ///
    /// - `state`: 新的安装状态
    fn set_state(&self, state: AddModPackState);
    /// 设置当前进度
    ///
    /// - `value`: 当前进度
    /// - `all`: 总进度，未知为 `None`
    fn set_now(&self, value: usize, all: Option<usize>);
    /// 子进度文字
    ///
    /// - `text`: 文字内容，`None` 表示清除
    fn set_sub_text(&self, text: Option<String>);
    /// 子进度
    ///
    /// - `value`: 当前进度
    /// - `all`: 总进度，未知为 `None`
    fn set_sub_now(&self, value: usize, all: Option<usize>);
}

/// 进度条界面回调
pub trait IProgressGui: Send + Sync {
    /// 显示文字
    ///
    /// - `text`: 文字内容，`None` 表示清除
    fn set_progress_text(&self, text: Option<String>);
    /// 进度
    ///
    /// - `value`: 当前进度
    /// - `all`: 总进度，未知为 `None`
    fn set_progress_now(&self, value: usize, all: Option<usize>);
}

/// 实例启动状态，按变体顺序推进，`progress` 依赖该顺序
pub enum LaunchState {
    /// 登陆账户
    Login,
    /// 检查文件
    Check,
    /// 读取信息
    ReadInfo,
    /// 下载文件
    Download,
    /// 准备启动参数
    Jvm,
    /// 启动前运行
    Pre,
    /// 启动后运行
    Post,
    /// 结束
    End,
    /// 加载服务器整合包
    LoadServerPack,
    /// 检查服务器整合包
    CheckServerPack,
    /// 下载服务器整合包
    DownloadServerPack,
}

impl LaunchState {
    /// 启动进度百分比，取值 0–100，按阶段等权近似
    ///
    /// 阶段集合与顺序以本枚举为准，GUI 侧不再自持阶段表；
    /// 新增变体时这里的 match 会因非穷尽而编译失败，强制同步
    ///
    /// # 返回值
    ///
    /// 返回该状态对应的百分比
    pub fn progress(&self) -> u32 {
        let idx = match self {
            LaunchState::Login => 0,
            LaunchState::Check => 1,
            LaunchState::ReadInfo => 2,
            LaunchState::Download => 3,
            LaunchState::Jvm => 4,
            LaunchState::Pre => 5,
            LaunchState::Post => 6,
            LaunchState::End => 7,
            LaunchState::LoadServerPack => 8,
            LaunchState::CheckServerPack => 9,
            LaunchState::DownloadServerPack => 10,
        };
        (idx + 1) * 100 / 11
    }
}

/// 进程运行时机
pub enum ProcessRunType {
    /// 启动前运行
    PreLaunch,
    /// 启动后运行
    PostLaunch,
}

/// 启动界面回调
#[async_trait]
pub trait ILaunchGui: Send + Sync {
    /// 启动状态修改
    ///
    /// - `setting`: 目标实例设置
    /// - `state`: 新的启动状态
    fn update_state(&self, setting: &InstanceSettingObj, state: LaunchState);
    /// 登陆失败
    ///
    /// - `auth`: 登陆信息
    ///
    /// # 返回值
    ///
    /// 返回 `true` 表示用原用户名重新登陆重试
    async fn login_fail(&self, auth: &LoginObj) -> bool;
    /// 请求是否要下载文件
    ///
    /// # 返回值
    ///
    /// 返回 `true` 表示同意下载缺失文件
    async fn request_download_file(&self) -> bool;
    /// 没有合适的java
    ///
    /// - `java`: 所需的 Java 版本
    fn no_java(&self, java: i32);
    /// 是否运行启动其他进程
    ///
    /// - `run_type`: 进程运行时机
    ///
    /// # 返回值
    ///
    /// 返回 `true` 表示允许运行
    fn launch_process(&self, run_type: ProcessRunType) -> bool;
}
