//! 添加实例窗口 DTO：目录项，由 `add_list_dir` 返回，前端 wire 形态

use serde::Serialize;

/// 目录项，由 list_dir 返回
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    /// 目录 / 文件名
    pub name: String,
    /// 是否为目录
    pub is_dir: bool,
}

/// 扫描文件夹时找到的可导入实例，由 `add_scan_folder` 返回
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInstanceDto {
    /// 实例目录路径
    pub path: String,
    /// 显示名，取目录名，如 `1.20.1`
    pub name: String,
}

/// 加载器支持列表查询进度，前端弹窗用它显示进度条
#[derive(Clone, Serialize)]
pub struct LoaderProgressDto {
    /// 当前步数
    pub step: u32,
    /// 总步数
    pub total: u32,
}

/// 实例重名确认，前端弹窗询问用户
#[derive(Clone, Serialize)]
pub struct NameConflictDto {
    /// 冲突实例 ID
    pub id: u32,
    /// 处理方式：overwrite 覆盖 / rename 自动改名
    pub kind: String,
    /// 冲突的实例名
    pub name: String,
}

/// 压缩包检测结果
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedPackDto {
    /// 压缩包类型 ID
    pub pack_type: String,
    /// 推荐实例名
    pub name: String,
}


/// 在线实例清单摘要，`add_probe_server_pack` 的返回结构
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackProbeDto {
    /// 实际读取的清单地址，已补上 `server.json`
    pub url: String,
    /// 清单里的实例名
    pub name: String,
    /// 游戏版本
    pub version: String,
    /// 加载器 ID，如 `neoforge`
    pub loader: String,
    /// 加载器版本，没有时为空
    pub loader_version: Option<String>,
    /// 包版本
    pub pack_version: String,
    /// 说明文本
    pub text: String,
    /// 清单里的模组条目数
    pub file_count: u64,
    /// 清单里的配置压缩包数
    pub archive_count: u64,
}

/// 整合包安装进度，state 取值：downloadPack / readInfo / getInfo / downloadFile / extract / done
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackProgressDto {
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
}
