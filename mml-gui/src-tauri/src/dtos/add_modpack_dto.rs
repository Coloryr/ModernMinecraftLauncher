//! 下载整合包窗口 DTO：安装任务总览，既是多任务进度条的事件负载，也是查询返回

use serde::Serialize;

/// 单个整合包安装任务的状态
///
/// state 为安装阶段 ID，与前端 i18n 键一一对应，取值 downloadPack / readInfo /
/// getInfo / downloadFile / extract / done，见 add.rs 的 `pack_state_id`；
/// done / failed / cancelled 为终态标记，三者互斥。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPackTaskDto {
    /// 任务 ID，安装开始时生成，与最终实例 uuid 无关
    pub uuid: String,
    /// 整合包来源平台 ID
    pub source: String,
    /// 整合包项目 ID
    pub pid: String,
    /// 整合包文件 ID
    pub fid: String,
    /// 显示名，取自整合包列表缓存
    pub name: String,
    /// 这次任务在干什么：`install` 装新实例、`upgrade` 把已有实例升到新版本
    pub kind: String,
    /// 当前安装阶段 ID
    pub state: String,
    /// 当前阶段进度
    pub now: u32,
    /// 当前阶段总量
    pub total: u32,
    /// 子进度说明文本
    pub sub_text: Option<String>,
    /// 子进度当前值
    pub sub_now: u32,
    /// 子进度总量
    pub sub_total: u32,
    /// 是否安装完成
    pub done: bool,
    /// 是否安装失败
    pub failed: bool,
    /// 是否已被用户取消
    pub cancelled: bool,
    /// 失败原因，failed 时有值
    pub error: Option<String>,
    /// 安装成功的新实例 uuid，done 时有值，前端用它切换选中实例
    pub instance_uuid: Option<String>,
}

/// 整合包安装任务总览，`add-modpack-status` 事件负载与 `add_modpack_status` 返回共用
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPackStatusDto {
    /// 下载整合包窗口是否开着，决定进度条显示在整合包窗口还是主窗口
    pub window_open: bool,
    /// 全部安装任务
    pub tasks: Vec<ModPackTaskDto>,
}
