//! 迁移引擎：顶层条目的复制 / 移动，含跨盘回退，以及失败收集
//!
//! 从 `colormc/mod.rs` 拆出来的。**原样文件夹搬运**，不做字段级转换；单个文件失败不中断
//! 整次迁移，记进报告的 `failed` 里。
//!
//! 唯一的例外是 `minecraft/instances`：实例不能原样覆盖，见 [`super::import`]。
//! 重名时导入的那个改名、两份都留；uuid 冲突只给导入的那个换新；
//! ColorMC 的组名并进 `group_save.json`。

use std::fs;
use std::io;
use std::path::Path;

use mml_game::game_group::ImportedGroup;
use mml_names::names;
use mml_sys::path_helper;
use tauri::AppHandle;

use crate::dtos::ColorMcReportDto;
use crate::err_keys;
use crate::log_text::LogText;

use super::check::check;
use super::detect::{instance_root, looks_like_colormc, top_entries};
use super::import::{self, InstancePlan};
use super::progress;

/// 迁移主流程，同步执行，由调用方放到阻塞线程里跑
///
/// 目标目录是**参数**，命令传运行目录、测试传自己的临时目录 —— 引擎里不去摸全局单例，
/// 既能测，也避免了"引擎偷偷决定搬到哪"。
///
/// - `app`: 窗口句柄，`None` 表示不报进度
/// - `src`: 源目录，必须是能识别的 ColorMC 工作目录
/// - `dst`: 目标目录，通常是运行目录
/// - `moved`: `true` 走移动、`false` 走复制
///
/// # 返回值
///
/// 迁移与兼容性检测的报告；源目录不合法、与目标相同、目标嵌在源里或源为空时
/// 返回 `Err`，内容为 i18n 键
pub(super) fn migrate(
    app: Option<&AppHandle>,
    src: &Path,
    dst: &Path,
    moved: bool,
) -> Result<ColorMcReportDto, String> {
    // 源目录必须是"真的 ColorMC 工作目录"：调用方传来的路径不能当数
    if !src.is_dir() || !looks_like_colormc(src) {
        return Err(String::from(err_keys::COLORMC_SOURCE));
    }
    if src == dst {
        return Err(String::from(err_keys::COLORMC_SAME_DIR));
    }
    // 运行目录在源目录里面：复制会递归地把自己再拷一遍，直接拒绝
    if dst.starts_with(src) {
        return Err(String::from(err_keys::COLORMC_NESTED));
    }

    let src_entries = top_entries(src);
    if src_entries.is_empty() {
        return Err(String::from(err_keys::COLORMC_EMPTY));
    }

    // 实例计划必须在搬运前算：移动模式下源目录随后就没了，而且重名改名、uuid 换新
    // 都要在落盘前定下来
    let plans = import::plan_instances(&instance_root(src), &instance_root(dst));

    mml_log::info(
        LogText::ColormcMigrateStart {
            source: src.display().to_string(),
            target: dst.display().to_string(),
            mode: String::from(if moved { "move" } else { "copy" }),
        }
        .text(),
    );

    // ---- 1. 搬运 ----
    // `failed` 里可能有**嵌套**的失败项，比如某个文件被占用，所以成功条目数单独数，
    // 不能用"总数 - failed.len()"倒推
    let mut failed: Vec<String> = Vec::new();
    let mut ok_entries: u32 = 0;

    // 复制的进度总量按文件数算；移动按顶层条目算，但 `minecraft` 是拆成子条目搬的，
    // 要把它换成"一个占位 + 实际子条目数"，否则搬完 done 会超过 total
    let total = if moved {
        src_entries.len() as u32 + minecraft_children(src)
    } else {
        progress(app, "scan", 0, 0, "");
        count_files(src)
    };
    progress(app, if moved { "move" } else { "copy" }, 0, total, "");

    let mut done: u32 = 0;
    for name in &src_entries {
        let from = src.join(name);
        let to = dst.join(name);
        let result = if name == names::MINECRAFT_DIR {
            migrate_minecraft(
                app,
                &from,
                &to,
                moved,
                &plans,
                &mut done,
                total,
                &mut failed,
            )
        } else if moved {
            move_entry(app, &from, &to, name, &mut done, total)
        } else {
            copy_entry(app, "copy", &from, &to, name, &mut done, total, &mut failed)
        };

        match result {
            Ok(()) => ok_entries += 1,
            Err(err) => {
                mml_log::error(entry_failed(moved, name, err).text());
                failed.push(name.clone());
            }
        }
    }

    // 移动：全部条目都搬完、源目录空了才删掉它；有失败就留着
    if moved {
        if is_empty(src) {
            if let Err(err) = fs::remove_dir_all(src) {
                mml_log::error(
                    LogText::ColormcRemoveSourceFailed {
                        error: err.to_string(),
                    }
                    .text(),
                );
            }
        } else {
            mml_log::info(LogText::ColormcSourceKept.text());
        }
    }

    // ---- 2. 兼容性检测 ----
    // 检测按**落地后**的目录名走：重名实例已经改过名，拿源目录名会验错位置
    let targets: Vec<String> = plans.iter().map(|plan| plan.dir.clone()).collect();
    progress(app, "check", 0, targets.len() as u32, "");
    let checked = check(app, &src_entries, &targets, dst);

    mml_log::info(
        LogText::ColormcMigrateDone {
            ok: ok_entries as usize,
            failed: failed.len(),
            instances: checked.instances_ok.len(),
            total: checked.instances_ok.len() + checked.instances_bad.len(),
        }
        .text(),
    );

    Ok(ColorMcReportDto {
        moved,
        source: src.display().to_string(),
        entries: ok_entries,
        failed,
        instances_ok: checked.instances_ok,
        instances_bad: checked.instances_bad,
        instances_legacy_gui: checked.instances_legacy_gui,
        compat: checked.compat,
        accounts: 0,
    })
}

/// 搬 `minecraft/`：普通子条目照旧覆盖，`instances` 走实例导入
///
/// 子条目级别的失败只记进 `failed` 并继续，返回 `Ok(())` 表示这个顶层条目处理完了。
///
/// - `app`: 窗口句柄，`None` 表示不报进度
/// - `from`: 源 `minecraft` 目录
/// - `to`: 目标 `minecraft` 目录
/// - `moved`: `true` 走移动、`false` 走复制
/// - `plans`: 实例导入计划
/// - `done`: 已完成数，按引用累加
/// - `total`: 进度总量，`0` 表示未知
/// - `failed`: 失败项收集表
///
/// # 返回值
///
/// 源目录读不到时返回 `Err`，内容为错误原文
#[allow(clippy::too_many_arguments)]
fn migrate_minecraft(
    app: Option<&AppHandle>,
    from: &Path,
    to: &Path,
    moved: bool,
    plans: &[InstancePlan],
    done: &mut u32,
    total: u32,
    failed: &mut Vec<String>,
) -> Result<(), String> {
    let reader = fs::read_dir(from).map_err(|err| err.to_string())?;
    path_helper::create_dir_all(to).map_err(|err| err.to_string())?;

    let mut entries: Vec<String> = reader
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    entries.sort();

    for name in entries {
        let from = from.join(&name);
        let to = to.join(&name);
        let rel = format!("{}/{}", names::MINECRAFT_DIR, name);

        if name == names::INSTANCE_DIR {
            if let Err(err) = migrate_instances(app, &from, &to, moved, plans, done, total, failed)
            {
                mml_log::error(LogText::ColormcInstancesFailed { error: err }.text());
                failed.push(rel);
            }
            continue;
        }

        let result = if moved {
            move_entry(app, &from, &to, &rel, done, total)
        } else {
            copy_entry(app, "copy", &from, &to, &rel, done, total, failed)
        };
        if let Err(err) = result {
            mml_log::error(entry_failed(moved, &rel, err).text());
            failed.push(rel);
        }
    }

    // 移动：子条目都搬走了才删源目录，有失败就留着
    if moved {
        if is_empty(from)
            && let Err(err) = fs::remove_dir(from)
        {
            mml_log::error(
                LogText::ColormcRemoveSourceFailed {
                    error: err.to_string(),
                }
                .text(),
            );
        }
        *done += 1;
        progress(app, "move", *done, total, names::MINECRAFT_DIR);
    }

    Ok(())
}

/// 搬实例目录：按计划改名落位，写回 `game.json`，最后合并分组
///
/// - `app`: 窗口句柄，`None` 表示不报进度
/// - `from`: 源 `minecraft/instances`
/// - `to`: 目标 `minecraft/instances`
/// - `moved`: `true` 走移动、`false` 走复制
/// - `plans`: 实例导入计划
/// - `done`: 已完成数，按引用累加
/// - `total`: 进度总量，`0` 表示未知
/// - `failed`: 失败项收集表
///
/// # 返回值
///
/// 源目录读不到时返回 `Err`，内容为错误原文
#[allow(clippy::too_many_arguments)]
fn migrate_instances(
    app: Option<&AppHandle>,
    from: &Path,
    to: &Path,
    moved: bool,
    plans: &[InstancePlan],
    done: &mut u32,
    total: u32,
    failed: &mut Vec<String>,
) -> Result<(), String> {
    if !from.is_dir() {
        return Ok(());
    }
    path_helper::create_dir_all(to).map_err(|err| err.to_string())?;

    let mut groups: Vec<ImportedGroup> = Vec::new();

    for plan in plans {
        let source = from.join(&plan.source);
        let target = to.join(&plan.dir);
        let rel = format!(
            "{}/{}/{}",
            names::MINECRAFT_DIR,
            names::INSTANCE_DIR,
            plan.source
        );

        let result = if moved {
            move_entry(app, &source, &target, &rel, done, total)
        } else {
            copy_entry(app, "copy", &source, &target, &rel, done, total, failed)
        };

        match result {
            Ok(()) => match import::write_back(&target, plan) {
                // 写回成功（或本来就不用改）才登记分组：uuid 没落进 game.json 的话，
                // 分组表里这一条会在下次启动 prune 时被清掉
                Ok(true) => {
                    if plan.group.is_some() {
                        groups.push(ImportedGroup {
                            uuid: plan.uuid,
                            group: plan.group.clone(),
                        });
                    }
                }
                Ok(false) => {}
                Err(err) => {
                    mml_log::error(
                        LogText::ColormcInstancePatchFailed {
                            name: rel.clone(),
                            error: err,
                        }
                        .text(),
                    );
                    failed.push(format!("{rel}/{}", names::GAME_FILE));
                }
            },
            Err(err) => {
                mml_log::error(entry_failed(moved, &rel, err).text());
                failed.push(rel);
            }
        }
    }

    // 实例根目录下的零碎文件；分组表是 M²L 自己的，哪怕源里也有也不能盖
    let reader = fs::read_dir(from).map_err(|err| err.to_string())?;
    let mut files: Vec<String> = reader
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name != names::GROUP_FILE && name != names::LEGACY_GROUP_FILE)
        .collect();
    files.sort();

    for name in files {
        let source = from.join(&name);
        let target = to.join(&name);
        let rel = format!("{}/{}/{}", names::MINECRAFT_DIR, names::INSTANCE_DIR, name);
        let result = if moved {
            move_entry(app, &source, &target, &rel, done, total)
        } else {
            copy_entry(app, "copy", &source, &target, &rel, done, total, failed)
        };
        if let Err(err) = result {
            mml_log::error(entry_failed(moved, &rel, err).text());
            failed.push(rel);
        }
    }

    if let Err(err) = import::merge_groups(to, &groups) {
        mml_log::error(LogText::ColormcGroupMergeFailed { error: err }.text());
        failed.push(format!(
            "{}/{}/{}",
            names::MINECRAFT_DIR,
            names::INSTANCE_DIR,
            names::GROUP_FILE
        ));
    }

    if moved {
        if is_empty(from)
            && let Err(err) = fs::remove_dir(from)
        {
            mml_log::error(
                LogText::ColormcRemoveSourceFailed {
                    error: err.to_string(),
                }
                .text(),
            );
        }
    }

    Ok(())
}

/// 构造"某个条目搬运失败"的日志文案
///
/// - `moved`: `true` 表示移动、`false` 表示复制
/// - `name`: 条目名，相对迁移根目录
/// - `error`: 错误原文
///
/// # 返回值
///
/// 移动 / 复制各自对应的日志文案
fn entry_failed(moved: bool, name: &str, error: String) -> LogText {
    if moved {
        LogText::ColormcMoveFailed {
            name: name.to_string(),
            error,
        }
    } else {
        LogText::ColormcCopyFailed {
            name: name.to_string(),
            error,
        }
    }
}

/// `minecraft/` 下的子条目数，移动模式的进度总量用它替换掉 `minecraft` 那一个占位
///
/// - `src`: 源工作目录
///
/// # 返回值
///
/// 子条目数；目录读不到返回 0
fn minecraft_children(src: &Path) -> u32 {
    fs::read_dir(src.join(names::MINECRAFT_DIR))
        .map(|reader| reader.flatten().count() as u32)
        .unwrap_or(0)
}

/// 目录是否为空；读不到按"不空"处理，宁可留着也不误删
///
/// - `dir`: 要检查的目录
///
/// # 返回值
///
/// 目录存在且没有任何条目时返回 `true`
fn is_empty(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|mut reader| reader.next().is_none())
        .unwrap_or(false)
}

/// 递归数文件，复制模式拿它当进度总量
///
/// - `dir`: 要数的目录
///
/// # 返回值
///
/// 目录下的文件总数；目录读不到返回 0
pub(super) fn count_files(dir: &Path) -> u32 {
    let Ok(reader) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in reader.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += count_files(&path);
        } else {
            total += 1;
        }
    }
    total
}

/// 删掉一个条目，文件或目录都行；移动前清掉目标位置的同名条目，因为 Windows 的 rename 不覆盖
///
/// - `path`: 要删除的路径
///
/// # 返回值
///
/// 删除成功或路径本就不存在返回 `Ok(())`，其余返回 io 错误
pub(super) fn remove_entry(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else if path.is_file() {
        fs::remove_file(path)
    } else {
        Ok(())
    }
}

/// 复制一个条目：目录递归、文件直接复制，同名一律覆盖
///
/// 单个文件失败只记进 `failed` 并继续：占用 / 无权限不该毁掉整次迁移。
///
/// - `app`: 窗口句柄，`None` 表示不报进度
/// - `stage`: 进度事件里报的阶段名；移动的跨盘回退也走这里，那时报 `move`
/// - `from`: 源路径
/// - `to`: 目标路径
/// - `rel`: 相对顶层条目的路径，报进度用
/// - `done`: 已完成文件数，按引用累加
/// - `total`: 总文件数，`0` 表示未知
/// - `failed`: 失败项收集表，同样按相对路径记
///
/// # 返回值
///
/// 成功返回 `Ok(())`；目录内部的单个文件失败不影响返回，只在顶层条目本身失败时
/// 返回 `Err`，内容为错误原文
#[allow(clippy::too_many_arguments)]
pub(super) fn copy_entry(
    app: Option<&AppHandle>,
    stage: &str,
    from: &Path,
    to: &Path,
    rel: &str,
    done: &mut u32,
    total: u32,
    failed: &mut Vec<String>,
) -> Result<(), String> {
    if from.is_dir() {
        path_helper::create_dir_all(to).map_err(|err| err.to_string())?;
        let reader = fs::read_dir(from).map_err(|err| err.to_string())?;
        for entry in reader.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let child_rel = format!("{rel}/{name}");
            if let Err(err) = copy_entry(
                app,
                stage,
                &entry.path(),
                &to.join(&name),
                &child_rel,
                done,
                total,
                failed,
            ) {
                mml_log::error(
                    LogText::ColormcCopySkip {
                        path: child_rel.clone(),
                        error: err,
                    }
                    .text(),
                );
                failed.push(child_rel.clone());
            }
        }
        return Ok(());
    }

    let result = path_helper::copy_file(from, to);
    *done += 1;
    progress(app, stage, *done, total, rel);
    result.map_err(|err| err.to_string())
}

/// 移动一个顶层条目
///
/// 先 `rename`，同盘瞬时完成；跨盘时回退成"复制 + 删源"。[`path_helper::move_file`] 也是这个
/// 策略，但这里要按文件报进度，所以自己走一遍 [`copy_entry`]。
///
/// - `app`: 窗口句柄，`None` 表示不报进度
/// - `from`: 源路径
/// - `to`: 目标路径
/// - `rel`: 相对顶层条目的路径，报进度用
/// - `done`: 已完成条目数，按引用累加
/// - `total`: 总条目数，`0` 表示未知
///
/// # 返回值
///
/// 成功返回 `Ok(())`；删旧目标、rename 或跨盘复制失败时返回 `Err`，内容为错误原文
pub(super) fn move_entry(
    app: Option<&AppHandle>,
    from: &Path,
    to: &Path,
    rel: &str,
    done: &mut u32,
    total: u32,
) -> Result<(), String> {
    // "一律覆盖"：目标已存在就先删掉它
    if to.exists() {
        remove_entry(to).map_err(|err| err.to_string())?;
    }

    match fs::rename(from, to) {
        Ok(()) => {
            *done += 1;
            progress(app, "move", *done, total, rel);
            Ok(())
        }
        Err(err) if err.kind() == io::ErrorKind::CrossesDevices => {
            let mut failed: Vec<String> = Vec::new();
            let mut files: u32 = 0;
            // 跨盘只能真的复制：进度总量未知，0 表示未知，阶段名仍报 move
            copy_entry(app, "move", from, to, rel, &mut files, 0, &mut failed)?;
            remove_entry(from).map_err(|err| err.to_string())?;
            *done += 1;
            progress(app, "move", *done, total, rel);
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}
