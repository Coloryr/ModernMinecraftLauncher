//! ColorMC 工作目录的识别与信息统计：候选目录 / 是不是 ColorMC / 实例与顶层条目
//!
//! 从 `colormc/mod.rs` 拆出来的。"问过了"的标记 `colormc_migrate.json` 也在这里 ——
//! 它表达的是"这个运行目录已经问过了"，删掉就能重新触发询问。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use mml_base::{get_base_dir, serialize_tools};
use mml_names::names;
use mml_sys::path_helper;

use crate::dtos::ColorMcInfoDto;

/// 标记文件路径
///
/// # 返回值
///
/// 运行目录下的 `colormc_migrate.json` 路径
pub(super) fn marker_file() -> PathBuf {
    get_base_dir().join(names::COLORMC_MIGRATE_FILE)
}

/// 是否已经问过，标记文件存在即算问过，任何选择都不再问
///
/// # 返回值
///
/// 标记文件存在即为 `true`
pub(super) fn asked() -> bool {
    marker_file().is_file()
}

/// 记下"已问过"以及当时的选择
///
/// - `choice`: 当时的选择，取 `copy` / `move` / `skip`
///
/// # 返回值
///
/// 写成功返回 `Ok(())`；写盘失败返回 `Err`，内容为错误原文
pub(super) fn mark(choice: &str) -> Result<(), String> {
    let obj = MigrateMarker {
        choice: choice.to_string(),
        time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    serialize_tools::json_to_file(&obj, marker_file()).map_err(|err| err.to_string())
}

/// 这个目录**像不像** ColorMC 的工作目录
///
/// 不能只看"目录存在"：只要 ColorMC 跑过一次，`%LOCALAPPDATA%\ColorMC\` 就会存在
/// 里面是账户 `auth.json` 与方块解锁状态 `block.json`，与工作目录在哪无关，
/// 拿它当命中会误报。这里按 ColorMC 真正会写的东西判定：`minecraft/instances`、
/// `minecraft/`、`config.json`、`collect.json` 命中其一即可。
///
/// - `dir`: 待判定的目录
///
/// # 返回值
///
/// 命中任一判据即为 `true`
pub(super) fn looks_like_colormc(dir: &Path) -> bool {
    let mc = dir.join(names::MINECRAFT_DIR);
    mc.join(names::INSTANCE_DIR).is_dir()
        || mc.is_dir()
        || dir.join(names::CONFIG_FILE).is_file()
        || dir.join(names::COLLECT_FILE).is_file()
}

/// 按 ColorMC 自己的顺序给出候选工作目录
///
/// 照 `Program.cs` 的 `Main()`：`%LOCALAPPDATA%\ColorMC\run` 记的路径 → 平台默认
/// Windows 是 `<exe 目录>\colormc\`、Linux 是 `~/.ColorMC/`、macOS 是 `/Users/shared/ColorMC/`
/// → `%APPDATA%\ColorMC\`，ColorMC 在默认位置不可写时会退到这里。
///
/// 与 ColorMC 的差别：**不创建任何目录、也不试写**，ColorMC 会建目录再写 `test` 文件
/// 来探可写性。这里只是探测，不动用户的东西；代价是"默认位置不存在但 %APPDATA% 有数据"
/// 这种退化情况靠**三个候选挨个查**来覆盖，而不是靠试写。
///
/// # 返回值
///
/// 候选目录与来源标记，按可信度从高到低排列
pub(super) fn candidates() -> Vec<Candidate> {
    let mut list: Vec<Candidate> = Vec::new();

    // 1. run 文件里记的路径
    if let Some(local) = dirs::data_local_dir() {
        let run = local.join(COLORMC_DIR).join(COLORMC_RUN_FILE);
        if run.is_file()
            && let Ok(text) = path_helper::read_text(&run)
        {
            let text = text.trim();
            if !text.is_empty() {
                list.push(Candidate {
                    dir: PathBuf::from(text),
                    from: "run",
                });
            }
        }
    }

    // 2. 平台默认位置
    #[cfg(target_os = "windows")]
    let default = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(COLORMC_INNER_DIR)));
    #[cfg(target_os = "linux")]
    let default = dirs::home_dir().map(|home| home.join(".ColorMC"));
    #[cfg(target_os = "macos")]
    let default = Some(PathBuf::from("/Users/shared/ColorMC"));
    if let Some(dir) = default {
        list.push(Candidate {
            dir,
            from: "default",
        });
    }

    // 3. 兜底位置
    if let Some(data) = dirs::data_dir() {
        list.push(Candidate {
            dir: data.join(COLORMC_DIR),
            from: "fallback",
        });
    }

    list
}

/// 一个 ColorMC 工作目录候选
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    /// 候选目录
    pub(super) dir: PathBuf,
    /// 来源标记，取 `run` / `default` / `fallback`，可信度从高到低
    pub(super) from: &'static str,
}

/// 探测本机的 ColorMC 工作目录，取第一个"存在且像 ColorMC 目录"的候选
///
/// # 返回值
///
/// 命中的目录与来源标记；一个都没命中返回 `None`
pub(super) fn detect() -> Option<Candidate> {
    candidates()
        .into_iter()
        .find(|c| c.dir.is_dir() && looks_like_colormc(&c.dir))
}

/// ColorMC 的账户文件位置，**在工作目录之外**，所以永远不会被迁移
///
/// # 返回值
///
/// 账户文件路径；文件不存在返回 `None`
pub fn auth_file() -> Option<PathBuf> {
    let file = dirs::data_local_dir()?
        .join(COLORMC_DIR)
        .join(names::AUTH_FILE);
    file.is_file().then_some(file)
}

/// 实例根目录，即工作目录下的 `minecraft/instances`，两边布局一致
///
/// - `root`: ColorMC 工作目录
///
/// # 返回值
///
/// 实例根目录路径，不检查是否存在
pub(super) fn instance_root(root: &Path) -> PathBuf {
    root.join(names::MINECRAFT_DIR).join(names::INSTANCE_DIR)
}

/// 数实例，只数子目录、不读内容，便宜
///
/// - `root`: ColorMC 工作目录
///
/// # 返回值
///
/// 实例根目录下的子目录数；目录读不到返回 0
pub(super) fn count_instances(root: &Path) -> u32 {
    let Ok(reader) = fs::read_dir(instance_root(root)) else {
        return 0;
    };
    reader
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .count() as u32
}

/// 实例目录名，已排序
///
/// - `root`: ColorMC 工作目录
///
/// # 返回值
///
/// 排好序的实例目录名；目录读不到返回空表
pub(super) fn list_instances(root: &Path) -> Vec<String> {
    let Ok(reader) = fs::read_dir(instance_root(root)) else {
        return Vec::new();
    };
    let mut list: Vec<String> = reader
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    list.sort();
    list
}

/// 工作目录根下的顶层条目名，已排序；弹窗用它列出"将搬入什么"
///
/// - `root`: ColorMC 工作目录
///
/// # 返回值
///
/// 排好序的顶层条目名；目录读不到返回空表
pub(super) fn top_entries(root: &Path) -> Vec<String> {
    let Ok(reader) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut list: Vec<String> = reader
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    list.sort();
    list
}

/// 组装探测结果
///
/// - `path`: 探测到的 ColorMC 工作目录
/// - `from`: 候选来源标记，取 `run` / `default` / `fallback`
///
/// # 返回值
///
/// 弹窗要展示的信息，含实例数 / 顶层条目 / 账户文件路径 / 来源 / 路径
pub(super) fn info(path: PathBuf, from: &'static str) -> ColorMcInfoDto {
    ColorMcInfoDto {
        instances: count_instances(&path),
        entries: top_entries(&path),
        auth_path: auth_file()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        from: from.to_string(),
        path: path.display().to_string(),
    }
}

/// ColorMC 的数据目录名，在 `%LOCALAPPDATA%\<它>` 与 `%APPDATA%\<它>` 下
pub(super) const COLORMC_DIR: &str = "ColorMC";

/// ColorMC 在 exe 旁边的默认工作目录名，Windows 上是 `<exe 目录>\colormc\`
pub(super) const COLORMC_INNER_DIR: &str = "colormc";

/// ColorMC 记"运行路径"的文件名，内容就是路径字符串，放在 `%LOCALAPPDATA%\ColorMC\` 下
pub(super) const COLORMC_RUN_FILE: &str = "run";

/// ColorMC 的实例界面设置文件名
///
/// 注意与 [`names::GUI_SETTING_FILE`] 即 M²L 自己的 `gui_setting.json` **不是同一个文件**：
/// 两边的 `Groups` 结构不同，同名会让 M²L 整份读不了。这里只用它判断"这个实例还带着
/// ColorMC 的界面设置"，好在报告里说明那部分数据没跟过来。
pub(super) const COLORMC_GUI_SETTING_FILE: &str = "guisetting.json";

/// 迁移标记，落在运行目录下的 `colormc_migrate.json`
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct MigrateMarker {
    /// 用户的选择，取 `copy` / `move` / `skip`
    #[serde(rename = "Choice")]
    choice: String,
    /// 记录时间，格式 `%Y-%m-%d %H:%M:%S`，仅用于排查
    #[serde(rename = "Time")]
    time: String,
}
