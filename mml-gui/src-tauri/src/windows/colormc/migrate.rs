//! 迁移引擎：顶层条目的复制 / 移动，含跨盘回退，以及失败收集
//!
//! 从 `colormc/mod.rs` 拆出来的。**原样文件夹搬运**，不做字段级转换；单个文件失败不中断
//! 整次迁移，记进报告的 `failed` 里。

use std::fs;
use std::io;
use std::path::Path;

use mml_sys::path_helper;
use tauri::AppHandle;

use crate::dtos::ColorMcReportDto;

use super::check::check;
use super::detect::{auth_file, list_instances, looks_like_colormc, top_entries};
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
        return Err(String::from("err.colormcSource"));
    }
    if src == dst {
        return Err(String::from("err.colormcSameDir"));
    }
    // 运行目录在源目录里面：复制会递归地把自己再拷一遍，直接拒绝
    if dst.starts_with(src) {
        return Err(String::from("err.colormcNested"));
    }

    // 实例名要在搬运前收集：移动模式下源目录随后就没了
    let src_instances = list_instances(src);
    let src_entries = top_entries(src);
    if src_entries.is_empty() {
        return Err(String::from("err.colormcEmpty"));
    }

    mml_log::info(format!(
        "colormc migrate start: {} -> {} (mode={})",
        src.display(),
        dst.display(),
        if moved { "move" } else { "copy" }
    ));

    // ---- 1. 搬运 ----
    // `failed` 里可能有**嵌套**的失败项，比如某个文件被占用，所以成功条目数单独数，
    // 不能用"总数 - failed.len()"倒推
    let mut failed: Vec<String> = Vec::new();
    let mut ok_entries: u32 = 0;

    if moved {
        // 移动：按顶层条目重命名，跨盘时回退成复制 + 删源
        let total = src_entries.len() as u32;
        progress(app, "move", 0, total, "");
        let mut done: u32 = 0;
        for name in &src_entries {
            let from = src.join(name);
            let to = dst.join(name);
            match move_entry(app, &from, &to, name, &mut done, total) {
                Ok(()) => ok_entries += 1,
                Err(err) => {
                    mml_log::error(format!("colormc move failed: {name} ({err})"));
                    failed.push(name.clone());
                }
            }
        }
        // 全部条目都搬完、源目录空了才删掉它；有失败就留着
        let empty = fs::read_dir(src)
            .map(|mut reader| reader.next().is_none())
            .unwrap_or(false);
        if empty {
            if let Err(err) = fs::remove_dir_all(src) {
                mml_log::error(format!("colormc remove source failed: {err}"));
            }
        } else {
            mml_log::info(String::from("colormc move: source kept (not empty)"));
        }
    } else {
        // 复制：先数一遍文件数供进度用，再逐文件复制
        progress(app, "scan", 0, 0, "");
        let total = count_files(src);
        progress(app, "copy", 0, total, "");
        let mut done: u32 = 0;
        for name in &src_entries {
            let from = src.join(name);
            let to = dst.join(name);
            match copy_entry(app, "copy", &from, &to, name, &mut done, total, &mut failed) {
                Ok(()) => ok_entries += 1,
                Err(err) => {
                    mml_log::error(format!("colormc copy failed: {name} ({err})"));
                    failed.push(name.clone());
                }
            }
        }
    }

    // ---- 2. 兼容性检测 ----
    progress(app, "check", 0, src_instances.len() as u32, "");
    let checked = check(app, &src_entries, &src_instances, dst);

    mml_log::info(format!(
        "colormc migrate done: entries={ok_entries} failed={} instances={}/{}",
        failed.len(),
        checked.instances_ok.len(),
        checked.instances_ok.len() + checked.instances_bad.len()
    ));

    Ok(ColorMcReportDto {
        moved,
        source: src.display().to_string(),
        entries: ok_entries,
        failed,
        instances_ok: checked.instances_ok,
        instances_bad: checked.instances_bad,
        instances_legacy_gui: checked.instances_legacy_gui,
        compat: checked.compat,
        auth_outside: auth_file().is_some(),
    })
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
                mml_log::error(format!("colormc copy skip: {child_rel} ({err})"));
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
