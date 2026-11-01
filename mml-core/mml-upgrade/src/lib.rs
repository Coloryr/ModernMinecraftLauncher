//! 启动时的数据升级：按"从哪个版本升级"分模块
//!
//! 每个步骤一个模块，模块名就是它处理的数据版本：`up0_1_5` 表示
//! 0.1.5 及更早版本写下的数据要升级到当前版本。加新步骤时照抄一个模块，
//! 在 [`STEPS`] 末尾登记即可 —— 启动时按顺序跑，界面上是一条"数据更新"进度，
//! 中途可以停下，下次启动接着跑。

use std::path::Path;

use mml_link::Store;

pub mod up0_1_5;

/// 一次升级的汇总结果
#[derive(Debug, Default)]
pub struct Report {
    /// 导入的模组记录数
    pub mods: usize,
    /// 导入的资源包记录数
    pub resourcepacks: usize,
    /// 记录不完整、跳过的条数
    pub failed: usize,
    /// 是否真的处理了旧数据
    pub migrated: bool,
    /// 是否被中途叫停
    pub cancelled: bool,
}

impl Report {
    /// 有没有真的处理过东西
    ///
    /// # 返回值
    ///
    /// 处理过返回 `true`
    pub fn did_work(&self) -> bool {
        self.migrated
    }
}

/// 一个升级步骤
pub struct Step {
    /// 这步处理的数据版本；`0.1.5` 表示 0.1.5 及更早版本写下的数据
    pub from: &'static str,
    /// 模块名，排查日志时对齐用
    pub name: &'static str,
    /// 这步还有没有要做的事
    pub pending: fn(&Path) -> bool,
    /// 执行这步；进度回调返回 `false` 表示请求停止
    pub run: for<'a> fn(&'a Path, &'a Store, &'a mut dyn FnMut(usize, usize) -> bool) -> Report,
}

/// 全部升级步骤，按版本从旧到新
///
/// 顺序就是执行顺序：一步停下（被叫停或出错）后面的不再跑
pub static STEPS: &[Step] = &[Step {
    from: "0.1.5",
    name: "up0_1_5",
    pending: up0_1_5::pending,
    run: up0_1_5::run,
}];

/// 是否还有待升级的数据
///
/// - `root`: 启动器运行目录
///
/// # 返回值
///
/// 任一步骤判定有活要干就返回 `true`
pub fn pending(root: &Path) -> bool {
    STEPS.iter().any(|step| (step.pending)(root))
}

/// 按顺序跑还能跑的步骤
///
/// - `root`: 启动器运行目录
/// - `store`: mml-link 的索引库
/// - `on_progress`: 进度回调，参数是当前步骤的已处理条数与总条数
///
/// # 返回值
///
/// 见 [`Report`]，各步骤的结果合并在一起
pub fn migrate(
    root: &Path,
    store: &Store,
    mut on_progress: impl FnMut(usize, usize) -> bool,
) -> Report {
    let mut report = Report::default();
    for step in STEPS {
        if !(step.pending)(root) {
            continue;
        }
        let progress: &mut dyn FnMut(usize, usize) -> bool = &mut on_progress;
        let step_report = (step.run)(root, store, progress);
        report.mods += step_report.mods;
        report.resourcepacks += step_report.resourcepacks;
        report.failed += step_report.failed;
        report.migrated |= step_report.migrated;
        report.cancelled = step_report.cancelled;
        // 被叫停就别开下一步了
        if step_report.cancelled {
            break;
        }
    }
    report
}
