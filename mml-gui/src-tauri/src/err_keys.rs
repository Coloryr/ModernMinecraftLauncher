//! 命令返回给前端的错误键
//!
//! 命令的 `Err` 里给的是**键**而不是句子：前端 `tErr()` 拿它查 `zh-CN.ts` / `en-US.ts`
//! 的同名文案，键写错或漏了，界面上就直接显示这串 key。
//! 与两份语言表的一致性由本文末尾的用例盯着。

// ---------- 通用 ----------

/// 无效的实例 UUID
pub const UUID: &str = "err.uuid";
/// 实例 UUID 为空
pub const UUID_EMPTY: &str = "err.uuidEmpty";
/// 实例 UUID 重复
pub const UUID_CONFLICT: &str = "err.uuidConflict";
/// 找不到该实例
pub const GAME_NOT_FOUND: &str = "err.gameNotFound";
/// 实例已在运行中
pub const INSTANCE_RUNNING: &str = "err.instanceRunning";
/// 操作已取消
pub const CANCELLED: &str = "err.cancelled";
/// 枚举取值不认识
pub const VARIANT_INVALID: &str = "err.variantInvalid";
/// 窗口模型未初始化
pub const MODEL_MISSING: &str = "err.modelMissing";
/// 窗口参数不合法
pub const WINDOW_QUERY: &str = "err.windowQuery";
/// 未知的排序方式
pub const SORT_TYPE_NOT_FOUND: &str = "err.sortTypeNotFound";

// ---------- 账户 ----------

/// 没选账户
pub const ACCOUNT_MISSING: &str = "err.accountMissing";
/// 账户名为空
pub const NAME_EMPTY: &str = "err.nameEmpty";
/// 登录方式被锁定
pub const LOGIN_LOCKED: &str = "err.loginLocked";
/// 该账户不是微软登录，刷不了 Token
pub const NOT_OAUTH: &str = "err.notOauth";

// ---------- 服务器与联机 ----------

/// 服务器地址为空
pub const SERVER_EMPTY: &str = "err.serverEmpty";
/// 服务器名称与地址都要填
pub const NAME_IP: &str = "err.nameIp";

// ---------- 资源与下载 ----------

/// 未知的下载源
pub const SOURCE_TYPE: &str = "err.sourceType";
/// 未知的整合包来源
pub const UNKNOWN_SOURCE: &str = "err.unknownSource";
/// 未知的加载器类型
pub const UNKNOWN_LOADER: &str = "err.unknownLoader";
/// 未知的压缩包类型
pub const UNKNOWN_PACK_TYPE: &str = "err.unknownPackType";
/// 未知的资源类型
pub const FILE_TYPE_NOT_FOUND: &str = "err.fileTypeNotFound";
/// 没找到对应版本的文件
pub const FILE_NOT_FOUND: &str = "err.fileNotFound";
/// 无效的文件编号
pub const BAD_FILE_ID: &str = "err.badFileId";
/// 该整合包正在安装
pub const ALREADY_DOWNLOADING: &str = "err.alreadyDownloading";
/// 安装任务不存在
pub const TASK_NOT_FOUND: &str = "err.taskNotFound";
/// Java 下载失败
pub const DOWNLOAD_FAILED: &str = "err.downloadFailed";
/// Java 信息不合法
pub const JAVA_INVALID: &str = "err.javaInvalid";
/// 下载项已失效
pub const JAVA_ITEM_NOT_FOUND: &str = "err.javaItemNotFound";
/// 分组名为空
pub const GROUP_EMPTY: &str = "err.groupEmpty";
/// 分组已存在
pub const GROUP_EXISTS: &str = "err.groupExists";
/// 分组不存在
pub const GROUP_NOT_FOUND: &str = "err.groupNotFound";

// ---------- 配置文件 ----------

/// 无效的文件名
pub const FILE_NAME: &str = "err.fileName";
/// 二进制文件不能用文本方式编辑
pub const FILE_BINARY: &str = "err.fileBinary";
/// 文件太大，打不开
pub const FILE_TOO_LARGE: &str = "err.fileTooLarge";
/// 不是区域文件
pub const NOT_MCA_FILE: &str = "err.notMcaFile";
/// 没找到该存档
pub const SAVE_NOT_FOUND: &str = "err.saveNotFound";
/// 存档名字为空
pub const SAVE_NAME_EMPTY: &str = "err.saveNameEmpty";
/// 地图种子不是整数
pub const SAVE_SEED: &str = "err.saveSeed";

// ---------- 导出 ----------

/// 已有导出任务在跑
pub const EXPORT_RUNNING: &str = "err.exportRunning";
/// 不支持的整合包类型
pub const EXPORT_PACK_TYPE: &str = "err.exportPackType";

// ---------- 自定义主页面 ----------

/// 压缩包里没有 index.html
pub const CUSTOM_HOME_NO_INDEX: &str = "err.customHomeNoIndex";

// ---------- ColorMC 迁移 ----------

/// 这个目录不像 ColorMC 的工作目录
pub const COLORMC_SOURCE: &str = "err.colormcSource";
/// ColorMC 目录就是本启动器的运行目录
pub const COLORMC_SAME_DIR: &str = "err.colormcSameDir";
/// ColorMC 目录嵌套在自己的运行目录里
pub const COLORMC_NESTED: &str = "err.colormcNested";
/// 迁移任务正在跑
pub const COLORMC_BUSY: &str = "err.colormcBusy";
/// 没选到要迁移的东西
pub const COLORMC_EMPTY: &str = "err.colormcEmpty";
/// 迁移模式不合法
pub const COLORMC_MODE: &str = "err.colormcMode";
/// 迁移任务不存在
pub const COLORMC_TASK: &str = "err.colormcTask";

// ---------- 拖入与图片 ----------

/// 拖入的东西里没有文件
pub const DROP_NO_FILE: &str = "err.dropNoFile";
/// 不是 PNG
pub const NOT_PNG: &str = "err.notPng";
/// 图片数据不合法
pub const IMAGE_INVALID: &str = "err.imageInvalid";
/// 找不到皮肤 / 披风纹理
pub const TEXTURE_MISSING: &str = "err.textureMissing";

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    /// 本文里的键：逐行取 `pub const NAME: &str = "键";`
    fn keys() -> Vec<String> {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/err_keys.rs"))
            .expect("读不到 err_keys.rs");

        text.lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix("pub const ")?;
                let (_, after) = rest.split_once(": &str = \"")?;
                let (key, _) = after.split_once('"')?;
                Some(key.to_string())
            })
            .collect()
    }

    /// 每个键都要在两份语言表里存在，漏了界面上就直接显示这串 key
    #[test]
    fn keys_exist_in_locales() {
        let list = keys();
        assert!(list.len() > 40, "常量没解析出来，检查 keys()");

        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mml-vue/src/lib/i18n/locales");
        for file in ["zh-CN.ts", "en-US.ts"] {
            let text = std::fs::read_to_string(dir.join(file)).expect("读不到语言表");
            for key in &list {
                assert!(text.contains(&format!("\"{key}\":")), "{file} 缺少 {key}");
            }
        }
    }
}
