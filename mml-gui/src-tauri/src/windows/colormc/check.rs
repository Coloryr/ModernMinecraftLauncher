//! 迁移后的**兼容性检测**：哪些能直接用、哪些有偏差、哪些 ColorMC 有而 M²L 读不懂
//!
//! 从 `colormc/mod.rs` 拆出来的。结论表 `COMPAT_TABLE` 与判定逻辑放在一起，
//! 改口径时只动这一个文件。

use std::path::Path;

use mml_base::serialize_tools;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_names::names;
use tauri::AppHandle;

use crate::dtos::ColorMcCompatDto;

use super::detect::{COLORMC_GUI_SETTING_FILE, instance_root};
use super::progress;

/// 兼容性检测：把"搬过来的东西 M²L 到底能不能用"逐条判定
///
/// - `app`: 窗口句柄，`None` 表示不报进度
/// - `src_entries`: 源工作目录的顶层条目名
/// - `src_instances`: 源目录里的实例目录名
/// - `dst`: 目标目录，实例就在这里逐个验
///
/// 顶层条目查 [`COMPAT_TABLE`]，没列进去的按 `extra` 处理。
/// 实例逐个用 M²L 的解析器读 `game.json` —— 这是真正的判据：M²L 只认"目录里有能解析的
/// `game.json`"的实例，所以解析失败就等于"搬过来了也看不见"。
/// 实例里还留着 ColorMC 的 `guisetting.json` 时单独记一笔：M²L 读的是自己的
/// `gui_setting.json`，见 `crate::gui_setting`，所以 ColorMC 那份里的模组分组 / 备注 /
/// 日志设置**不会跟着过来**，文件本身留着，将来要转换还有原数据。
///
/// # 返回值
///
/// 实例三分类结论与顶层条目逐条结论，见 [`Checked`]
pub(super) fn check(
    app: Option<&AppHandle>,
    src_entries: &[String],
    src_instances: &[String],
    dst: &Path,
) -> Checked {
    let mut checked = Checked {
        instances_ok: Vec::new(),
        instances_bad: Vec::new(),
        instances_legacy_gui: Vec::new(),
        compat: Vec::new(),
    };

    // 逐条给结论
    for name in src_entries {
        let (level, note) = COMPAT_TABLE
            .iter()
            .find(|(entry, _, _)| entry == name)
            .map(|(_, level, note)| (*level, *note))
            .unwrap_or(("extra", "colormc.compat.extra"));
        checked.compat.push(ColorMcCompatDto {
            name: name.clone(),
            level: level.to_string(),
            note: note.to_string(),
        });
    }

    // 实例逐个验：能不能被 M²L 读出来
    let root = instance_root(dst);
    let total = src_instances.len() as u32;
    for (index, name) in src_instances.iter().enumerate() {
        let dir = root.join(name);
        let game = dir.join(names::GAME_FILE);
        if serialize_tools::json_from_file::<InstanceSettingObj>(&game).is_ok() {
            if dir.join(COLORMC_GUI_SETTING_FILE).is_file() {
                checked.instances_legacy_gui.push(name.clone());
            }
            checked.instances_ok.push(name.clone());
        } else {
            checked.instances_bad.push(name.clone());
        }
        progress(app, "check", index as u32 + 1, total, name);
    }

    checked
}

/// 兼容性对照表：ColorMC 工作目录的**顶层条目** → 级别与说明 i18n key
///
/// 级别：`ok` 能直接用 / `partial` 能用但有偏差或会丢东西 / `extra` M²L 读不懂，留着不碍事。
/// 结论来自逐项对照 ColorMC 源码 `E:\code\ColorMC` 与本仓库代码：
/// - `minecraft/` **同构**：实例、资源、版本库、依赖库都在同样的位置，能直接用；
/// - `collect.json` 结构一致：M²L 的 `CollectObj` 少了 ColorMC 的 4 个开关，未知字段被忽略；
/// - `config.json` 能读，靠的是 `#[serde(default)]`，但 ColorMC 独有的 `SafeLog4j` 会丢、
///   代理用 3 个 bool 表达而 M²L 用"策略 + 类型"两组枚举 —— 语义不同，会回落成默认值；
/// - `count.dat` 是游戏统计，读不了就从零开始；`logs.log` 会盖掉 M²L 自己那份日志；
/// - `java/` 是 ColorMC 的运行时目录，合过去 M²L 不会自动认那些 JRE；
/// - 其余目录 / 文件 M²L 根本不读。
pub(super) const COMPAT_TABLE: &[(&str, &str, &str)] = &[
    ("minecraft", "ok", "colormc.compat.minecraft"),
    ("collect.json", "ok", "colormc.compat.collect"),
    ("config.json", "partial", "colormc.compat.config"),
    ("count.dat", "partial", "colormc.compat.count"),
    ("logs.log", "partial", "colormc.compat.logs"),
    ("java", "partial", "colormc.compat.java"),
    ("download", "extra", "colormc.compat.download"),
    ("block", "extra", "colormc.compat.block"),
    ("image", "extra", "colormc.compat.extra"),
    ("tools", "extra", "colormc.compat.extra"),
    ("frpc", "extra", "colormc.compat.extra"),
    ("inputs", "extra", "colormc.compat.extra"),
    ("gui.json", "extra", "colormc.compat.gui"),
    ("window.json", "extra", "colormc.compat.gui"),
    ("cloud.json", "extra", "colormc.compat.extra"),
    ("frp.json", "extra", "colormc.compat.extra"),
    ("lock", "extra", "colormc.compat.lock"),
];

/// 兼容性检测的结果
///
/// 字段给同级的 migrate 模块读，故都是 `pub(super)`
pub(super) struct Checked {
    /// 能被 M²L 解析出 `game.json` 的实例目录名
    pub(super) instances_ok: Vec<String>,
    /// 解析不出 `game.json` 的实例目录名，搬过来了 M²L 也看不见
    pub(super) instances_bad: Vec<String>,
    /// 还带着 ColorMC 界面设置文件的实例目录名
    pub(super) instances_legacy_gui: Vec<String>,
    /// 顶层条目逐条结论，含级别与说明的 i18n 键
    pub(super) compat: Vec<ColorMcCompatDto>,
}
