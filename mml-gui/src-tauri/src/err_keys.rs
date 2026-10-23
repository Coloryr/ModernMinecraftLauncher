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
/// 右键「进入存档」时那个存档已经不在了（被删或改名）
pub const WORLD_GONE: &str = "err.worldGone";
/// 实例正在下载或处理任务，暂时不能删
pub const INSTANCE_BUSY: &str = "err.instanceBusy";
/// 操作已取消
pub const CANCELLED: &str = "err.cancelled";
/// 日期文本格式不对，如 `YYYY-MM-DD` 解析失败
pub const DATE_INVALID: &str = "err.dateInvalid";
/// 枚举取值不认识
pub const VARIANT_INVALID: &str = "err.variantInvalid";
/// 窗口模型未初始化
pub const MODEL_MISSING: &str = "err.modelMissing";
/// 模组列表还没加载，没有可用的扫描数据
pub const MODS_NOT_LOADED: &str = "err.modsNotLoaded";
/// 实例不是从平台安装的整合包，或实例里没记项目 / 文件编号，查不了整合包更新
pub const NOT_ONLINE_MODPACK: &str = "err.notOnlineModpack";
/// 窗口参数不合法
pub const WINDOW_QUERY: &str = "err.windowQuery";
/// 没给实例 uuid，日志窗口定不下是哪一个
pub const WINDOW_INSTANCE_MISSING: &str = "err.windowInstanceMissing";
/// 没有可上传的日志内容
pub const LOG_EMPTY: &str = "err.logEmpty";
/// 日志上传失败
pub const LOG_UPLOAD: &str = "err.logUpload";
/// 未知的排序方式
pub const SORT_TYPE_NOT_FOUND: &str = "err.sortTypeNotFound";

// ---------- 账户 ----------

/// 没选账户
pub const ACCOUNT_MISSING: &str = "err.accountMissing";
/// 没登录过正版（微软）账户，不允许用离线账户启动
pub const OFFLINE_NEED_PREMIUM: &str = "err.offlineNeedPremium";
/// 账户名为空
pub const NAME_EMPTY: &str = "err.nameEmpty";
/// 登录方式被锁定
pub const LOGIN_LOCKED: &str = "err.loginLocked";
/// 账户正被其它运行中的实例占用
pub const ACCOUNT_IN_USE: &str = "err.accountInUse";
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
/// 存档下载只支持 CurseForge 源
pub const SAVE_SOURCE_LIMIT: &str = "addResource.saveSourceLimit";
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
/// 模组更新下载失败
pub const MOD_UPDATE_FAILED: &str = "err.modUpdateFailed";
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
/// 重命名时新名字已被占用
pub const NAME_TAKEN: &str = "err.nameTaken";
/// 二进制文件不能用文本方式编辑
pub const FILE_BINARY: &str = "err.fileBinary";
/// 文件太大，打不开
pub const FILE_TOO_LARGE: &str = "err.fileTooLarge";
/// 磁盘上的那一份已经被改过，本次保存基于的版本过期了
pub const FILE_CHANGED: &str = "err.fileChanged";
/// 文件是只读的，写不回去
pub const FILE_READ_ONLY: &str = "err.fileReadOnly";
/// 不是区域文件
pub const NOT_MCA_FILE: &str = "err.notMcaFile";

/// 区域文件里这一项没有区块
pub const MCA_CHUNK_EMPTY: &str = "err.mcaChunkEmpty";

/// 区域文件为空或已损坏，连 8 KiB 文件头都不够
pub const MCA_FILE_EMPTY: &str = "err.mcaFileEmpty";
/// 不是图片文件
pub const NOT_IMAGE_FILE: &str = "err.notImageFile";
/// 没找到该存档
pub const SAVE_NOT_FOUND: &str = "err.saveNotFound";
/// NBT 改动指向的路径对不上（结构变了，或指向的不是标量）
pub const NBT_PATH: &str = "err.nbtPath";
/// NBT 的新值按目标标签类型解析不了
pub const NBT_VALUE: &str = "err.nbtValue";
/// 存档名字为空
pub const SAVE_NAME_EMPTY: &str = "err.saveNameEmpty";
/// 地图种子不是整数
pub const SAVE_SEED: &str = "err.saveSeed";

// ---------- 导出 ----------

/// 已有导出任务在跑
/// 模组清单尚未扫描或已变化。
pub const MOD_EXPORT_SCAN: &str = "err.modExportScan";
/// 导出字段无效。
/// 导出路径不能位于实例内。
pub const EXPORT_DESTINATION: &str = "err.exportDestination";
pub const SERVER_PACK_INVALID: &str = "err.serverPackInvalid";
pub const SERVER_PACK_PLATFORM: &str = "err.serverPackPlatform";
pub const SERVER_PACK_BUSY: &str = "err.serverPackBusy";
/// 实例内的链接不能打包。
pub const EXPORT_LINK: &str = "err.exportLink";
pub const EXPORT_FIELDS: &str = "err.exportFields";
pub const EXPORT_RUNNING: &str = "err.exportRunning";
/// 不支持的整合包类型
pub const EXPORT_PACK_TYPE: &str = "err.exportPackType";

// ---------- 主题包 ----------

/// 主题包声明的类型与导入入口不匹配
pub const THEME_TYPE_MISMATCH: &str = "err.themeTypeMismatch";
/// 没有清单且入口签名分不出 Home 与 Page
pub const THEME_TYPE_UNKNOWN: &str = "err.themeTypeUnknown";
/// theme.json 不是合法 JSON、类型不认识或 api 不支持
pub const THEME_MANIFEST_INVALID: &str = "err.themeManifestInvalid";
/// 缺该类型必需的入口文件
pub const THEME_MISSING_ENTRY: &str = "err.themeMissingEntry";
/// 不是合法 zip，或条目读不出来
pub const THEME_ZIP_INVALID: &str = "err.themeZipInvalid";
/// 包里含加密条目
pub const THEME_ZIP_ENCRYPTED: &str = "err.themeZipEncrypted";
/// 条目路径绝对、越界或含控制字符
pub const THEME_ZIP_ENTRY_PATH: &str = "err.themeZipEntryPath";
/// 条目重名，含只差大小写与规范化后重名
pub const THEME_ZIP_DUPLICATE: &str = "err.themeZipDuplicate";
/// 条目类型不支持，如符号链接、设备文件
pub const THEME_ZIP_UNSUPPORTED: &str = "err.themeZipUnsupported";
/// 压缩比异常，疑似 zip 炸弹
pub const THEME_ZIP_RATIO: &str = "err.themeZipRatio";

// ---------- 自定义界面 ----------

/// 整页包条目太多或解压后太大，按 zip 炸弹拒绝
pub const PAGE_ARCHIVE_LIMIT: &str = "err.pageArchiveLimit";
/// 自定义界面条目 id 不合法
pub const CUSTOM_UI_ID: &str = "err.customUiId";

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

// ---------- 本地文件 ----------

/// 清理本地文件失败，可能有文件正被占用
pub const LOCAL_FILE_CLEAR: &str = "err.localFileClear";
/// 本地文件项 id 不认识
pub const LOCAL_FILE_UNKNOWN: &str = "err.localFileUnknown";
/// 这一项不允许清理
pub const LOCAL_FILE_LOCKED: &str = "err.localFileLocked";

// ---------- 背景图 ----------

/// 背景图格式不支持
pub const BG_UNSUPPORTED: &str = "err.bgUnsupported";
/// 配置目录还没初始化
pub const BG_CONFIG_DIR: &str = "err.bgConfigDir";
/// 当前没有设置背景图
pub const BG_NONE: &str = "err.bgNone";

pub const LOBBY_ACCOUNT: &str = "err.lobbyAccount";
pub const LOBBY_ACCOUNT_LOCKED: &str = "err.lobbyAccountLocked";
pub const LOBBY_CLOSED: &str = "err.lobbyClosed";
pub const LOBBY_PLATFORM: &str = "err.lobbyPlatform";
pub const LOBBY_DOWNLOAD: &str = "err.lobbyDownload";
pub const LOBBY_START: &str = "err.lobbyStart";
pub const LOBBY_STATE: &str = "err.lobbyState";
pub const LOBBY_MAPPING: &str = "err.lobbyMapping";

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
