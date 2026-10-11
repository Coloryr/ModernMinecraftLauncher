//! 资源管理窗口的拖拽导入：把拖进来的文件放进**当前分类**对应的目录
//!
//! 落点就是当前面板那一类，见 [`dir_of`]：模组 / 资源包 / 光影包 / 截图 / 服务器 /
//! 结构文件都是"把文件复制进那个目录"；存档页收的是存档压缩包，走内核 `import_save`
//! 解压成一个存档目录，目录名由压缩包内结构决定，不由调用方指定。
//!
//! 同名冲突按"每次问我"处理：先以 `mode = "check"` 预检拿冲突清单，用户在前端弹窗里
//! 选完再带 `overwrite` / `skip` / `rename` 回来真正导入 —— 判定与落盘都在这一个命令里，
//! 免得"预检"和"导入"两处对同名口径各写一遍。
//!
//! 目录一律跳过：拖进来的东西是目录时不导入、也不报错，前端据此不显示导入遮罩，
//! 见 useFileDrop 的 accept。

use std::path::{Path, PathBuf};

use mml_game::GameInstance;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_sys::path_helper;

use crate::dtos::ResourceDropDto;

use super::{KIND_DATAPACKS, KIND_SAVES, datapack_dir, dir_of, parse_instance};

/// 只预检冲突，不落盘
const MODE_CHECK: &str = "check";
/// 冲突项跳过，保留盘上已有的那个
const MODE_SKIP: &str = "skip";
/// 冲突项自动加 ` (1)` 序号
const MODE_RENAME: &str = "rename";

/// 拖拽导入：把拖入的文件放进当前分类对应的目录
///
/// - `uuid`: 实例 uuid
/// - `kind`: 分类，与窗口左侧分类对应的后端 kind 串一致，如 `shaders` 页传 `shaderpacks`
/// - `paths`: 拖入项的绝对路径，目录会被跳过
/// - `mode`: 同名冲突的处理方式，取值 `check` / `overwrite` / `skip` / `rename`；
///   `check` 只预检不落盘
/// - `parent`: 只有数据包那一页要传，即存档目录名，落点 `saves/<parent>/datapacks`
///
/// # 返回值
///
/// 本次结果：`imported` / `skipped` / `conflicts` 三个名单，`check` 模式下只有
/// `conflicts`；拖入项全是目录时返回 `err.dropNoFile`
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_import_files(
    uuid: String,
    kind: String,
    paths: Vec<String>,
    mode: String,
    parent: Option<String>,
) -> Result<ResourceDropDto, String> {
    let instance = parse_instance(&uuid)?;
    // 复制 / 解压都是磁盘活：放阻塞线程，别占住主线程，与 add_import_* 同一条路子
    tauri::async_runtime::spawn_blocking(move || import(instance, &kind, &paths, &mode, parent))
        .await
        .map_err(|err| err.to_string())?
}

/// 导入实现，同步执行，在阻塞线程里跑
///
/// - `instance`: 实例句柄
/// - `kind`: 分类
/// - `paths`: 拖入项的绝对路径
/// - `mode`: 同名冲突的处理方式
/// - `parent`: 数据包页的存档目录名
///
/// # 返回值
///
/// 与 [`resource_import_files`] 同一份结果；没有可用文件时返回 `err.dropNoFile`
fn import(
    instance: GameInstance,
    kind: &str,
    paths: &[String],
    mode: &str,
    parent: Option<String>,
) -> Result<ResourceDropDto, String> {
    // 只认文件：拖进来的目录一律跳过，前端也不会为目录显示导入遮罩，这里是兜底
    let files: Vec<PathBuf> = paths
        .iter()
        .map(PathBuf::from)
        .filter(|item| item.is_file())
        .collect();
    if files.is_empty() {
        return Err(String::from("err.dropNoFile"));
    }

    // 取一份设置快照后**立刻放锁**，与 mod.rs 的 block_on_instance 同一口径
    let game = instance.read().unwrap().clone();
    match kind {
        KIND_SAVES => import_saves(&game, &files, mode),
        // 数据包挂在某个存档下：落点要配合 parent 这个存档目录名，没传就报错
        KIND_DATAPACKS => {
            let dir = datapack_dir(&game, parent.as_deref().unwrap_or_default())?;
            copy_into(&dir, &files, mode)
        }
        // 其余分类按目录名取，不认识的在 dir_of 里报错
        _ => {
            let dir = dir_of(&game, kind)?;
            copy_into(&dir, &files, mode)
        }
    }
}

/// 把文件复制进指定目录，与实例无关，便于单测
///
/// - `dir`: 目标目录，不存在时会创建
/// - `files`: 待复制的文件路径
/// - `mode`: 同名冲突的处理方式
///
/// # 返回值
///
/// 本次结果：`imported` / `skipped` / `conflicts` 三个名单
///
/// 只认文件：传进来的目录一律跳过，前端不会为目录显示导入遮罩，这里是兜底。
fn copy_into(dir: &Path, files: &[PathBuf], mode: &str) -> Result<ResourceDropDto, String> {
    let mut result = ResourceDropDto::default();
    if mode != MODE_CHECK {
        path_helper::create_dir_all(dir).map_err(|err| err.to_string())?;
    }

    for file in files.iter().filter(|item| item.is_file()) {
        let Some(name) = file_name(file) else {
            continue;
        };
        let target = dir.join(&name);
        let exists = target.exists();
        if mode == MODE_CHECK {
            if exists {
                result.conflicts.push(name);
            }
            continue;
        }
        if exists {
            match mode {
                MODE_SKIP => {
                    result.skipped.push(name);
                    continue;
                }
                MODE_RENAME => {
                    let (new_name, new_target) = unique_name(dir, &name);
                    path_helper::copy_file(file, &new_target).map_err(|err| err.to_string())?;
                    result.imported.push(new_name);
                    continue;
                }
                // 剩下的都直接覆盖，含 overwrite 与没传清楚的兜底
                _ => {}
            }
        }
        path_helper::copy_file(file, &target).map_err(|err| err.to_string())?;
        result.imported.push(name);
    }
    Ok(result)
}

/// 存档导入：压缩包解压成一个存档目录，口径与内核 `import_save` 一致
///
/// - `game`: 实例设置本体
/// - `files`: 待导入的存档压缩包
/// - `mode`: 同名冲突的处理方式
///
/// # 返回值
///
/// 本次结果：`imported` / `skipped` / `conflicts` 三个名单
///
/// 目录名由压缩包内结构决定，`import_save` 不接受指定名字，所以"重命名"在这里的含义是
/// **把已存在的那个存档目录先挪成 ` (1)`**，再把新存档解压到原名 —— 结果是
/// "老的没丢、新的用原名"，与复制型的重命名是同一个意图。
fn import_saves(
    game: &InstanceSettingObj,
    files: &[PathBuf],
    mode: &str,
) -> Result<ResourceDropDto, String> {
    let dir = game.get_saves_path();
    let mut result = ResourceDropDto::default();

    for file in files {
        let name = save_dir_name(file);
        if name.is_empty() {
            continue;
        }
        let target = dir.join(&name);
        if mode == MODE_CHECK {
            if target.exists() {
                result.conflicts.push(name);
            }
            continue;
        }
        if target.exists() {
            match mode {
                MODE_SKIP => {
                    result.skipped.push(name);
                    continue;
                }
                MODE_RENAME => {
                    let (_, moved) = unique_name(&dir, &name);
                    std::fs::rename(&target, &moved).map_err(|err| err.to_string())?;
                }
                _ => {}
            }
        }
        game.import_save(file).map_err(|err| err.to_string())?;
        result.imported.push(name);
    }
    Ok(result)
}

/// 存档压缩包会解压成哪个目录名
///
/// - `file`: 存档压缩包路径
///
/// # 返回值
///
/// 解压后的目录名；推不出来时是空串
///
/// 口径与 `mml_game` 的 `import_save` 保持一致：包内只有一个顶层目录就用它，
/// 否则用压缩包文件名。
fn save_dir_name(file: &Path) -> String {
    if let Ok(archive) = mml_base::archives::BaseArchive::open(file)
        && let Some(top) = archive.single_top_dir()
    {
        return top.to_string();
    }
    file.file_stem()
        .map(|stem| {
            // 二次去后缀，处理 .tar.gz 这类双后缀，与内核 import_save 同款
            stem.to_string_lossy().trim_end_matches(".tar").to_string()
        })
        .unwrap_or_default()
}

/// 取路径的文件名
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 文件名；路径以 `..` 结尾等取不到的情况返回 `None`
fn file_name(file: &Path) -> Option<String> {
    file.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// 在 `dir` 里给 `name` 找一个不撞的名字：`a.jar` → `a (1).jar`
///
/// - `dir`: 目标目录
/// - `name`: 原始文件名
///
/// # 返回值
///
/// 新的文件名与它的完整路径；999 个候选都撞时退回原名
///
/// 试到 999 还撞就退回原名，宁可覆盖，也不要在这里无限找下去。
fn unique_name(dir: &Path, name: &str) -> (String, PathBuf) {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|item| item.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|item| format!(".{}", item.to_string_lossy()))
        .unwrap_or_default();

    for index in 1..1000 {
        let candidate = format!("{stem} ({index}){ext}");
        let target = dir.join(&candidate);
        if !target.exists() {
            return (candidate, target);
        }
    }
    (name.to_string(), dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{cleanup, ensure_boot, temp_dir};

    /// 在 `dir` 下造一个内容为 `name` 的文件
    fn write(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, name.as_bytes()).unwrap();
        path
    }

    /// 在 `dir` 下造一个**指定文件名与内容**的文件，覆盖 / 跳过 / 重命名那几个用例要区分这两者
    fn write_as(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, text.as_bytes()).unwrap();
        path
    }

    /// 预检：只报冲突，一个字节都不落盘，也不建目标目录
    #[test]
    fn check_reports_conflicts_without_writing() {
        ensure_boot();
        let root = temp_dir("import-check");
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let a = write(&src, "a.jar");
        let b = write(&src, "b.jar");
        // 目标目录都还不存在：预检不该顺手把它建出来
        let dst = root.join("mods");

        let result = copy_into(&dst, &[a, b], MODE_CHECK).unwrap();
        assert!(result.conflicts.is_empty());
        assert!(result.imported.is_empty());
        assert!(!dst.exists());

        cleanup(&[&root]);
    }

    /// 预检撞上同名：只报那一个，且不动盘上的东西
    #[test]
    fn check_reports_conflict_names() {
        ensure_boot();
        let root = temp_dir("import-conflict");
        let src = root.join("src");
        let dst = root.join("mods");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        let a = write(&src, "a.jar");
        let b = write(&src, "b.jar");
        let old = write(&dst, "a.jar");

        let result = copy_into(&dst, &[a, b], MODE_CHECK).unwrap();
        assert_eq!(result.conflicts, vec![String::from("a.jar")]);
        // 盘上那条还是老内容，预检不是"先删后报"
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "a.jar");

        cleanup(&[&root]);
    }

    /// 没有冲突时直接导入，目标目录不存在就建出来
    #[test]
    fn import_creates_dir_and_copies() {
        ensure_boot();
        let root = temp_dir("import-copy");
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let a = write(&src, "a.jar");
        let dst = root.join("mods");
        assert!(!dst.exists());

        let result = copy_into(&dst, &[a], "overwrite").unwrap();
        assert_eq!(result.imported, vec![String::from("a.jar")]);
        assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "a.jar");

        cleanup(&[&root]);
    }

    /// 覆盖：老内容被新的顶掉，名字不变
    #[test]
    fn import_overwrites_same_name() {
        ensure_boot();
        let root = temp_dir("import-overwrite");
        let src = root.join("src");
        let dst = root.join("mods");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        let a = write_as(&src, "a.jar", "new");
        std::fs::write(dst.join("a.jar"), b"old").unwrap();

        let result = copy_into(&dst, &[a], "overwrite").unwrap();
        assert_eq!(result.imported, vec![String::from("a.jar")]);
        assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "new");

        cleanup(&[&root]);
    }

    /// 跳过：盘上那条原封不动，结果里记一笔
    #[test]
    fn import_skips_same_name() {
        ensure_boot();
        let root = temp_dir("import-skip");
        let src = root.join("src");
        let dst = root.join("mods");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        let a = write_as(&src, "a.jar", "new");
        std::fs::write(dst.join("a.jar"), b"old").unwrap();

        let result = copy_into(&dst, &[a], MODE_SKIP).unwrap();
        assert_eq!(result.skipped, vec![String::from("a.jar")]);
        assert!(result.imported.is_empty());
        assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "old");

        cleanup(&[&root]);
    }

    /// 重命名：老的留着，新的按 ` (n)` 递增找空位，`a (1)` 占了就给 `a (2)`
    #[test]
    fn import_renames_with_next_free_index() {
        ensure_boot();
        let root = temp_dir("import-rename");
        let src = root.join("src");
        let dst = root.join("mods");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        let a = write_as(&src, "a.jar", "new");
        std::fs::write(dst.join("a.jar"), b"old").unwrap();
        std::fs::write(dst.join("a (1).jar"), b"older").unwrap();

        let result = copy_into(&dst, &[a], MODE_RENAME).unwrap();
        assert_eq!(result.imported, vec![String::from("a (2).jar")]);
        assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "old");
        assert_eq!(
            std::fs::read_to_string(dst.join("a (2).jar")).unwrap(),
            "new"
        );

        cleanup(&[&root]);
    }

    /// 一次拖入**多个文件**要全部落盘，不是只导第一个，冲突也逐个判
    #[test]
    fn import_multiple_files_at_once() {
        ensure_boot();
        let root = temp_dir("import-multi");
        let src = root.join("src");
        let dst = root.join("mods");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        let a = write_as(&src, "a.jar", "A");
        let b = write_as(&src, "b.jar", "B");
        let c = write_as(&src, "c.jar", "C");
        // b.jar 的目标位置已有同名：选"跳过"只该跳它一个
        std::fs::write(dst.join("b.jar"), b"old").unwrap();

        let result = copy_into(&dst, &[a, b, c], MODE_SKIP).unwrap();
        assert_eq!(
            result.imported,
            vec![String::from("a.jar"), String::from("c.jar")]
        );
        assert_eq!(result.skipped, vec![String::from("b.jar")]);
        assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "A");
        assert_eq!(std::fs::read_to_string(dst.join("c.jar")).unwrap(), "C");
        assert_eq!(std::fs::read_to_string(dst.join("b.jar")).unwrap(), "old");

        cleanup(&[&root]);
    }

    /// 目录一律跳过：不导入、不报错，这是"拖目录不显示导入标志"那条的兜底
    #[test]
    fn directories_are_skipped() {
        ensure_boot();
        let root = temp_dir("import-dir");
        let dst = root.join("mods");
        std::fs::create_dir_all(&dst).unwrap();
        let folder = root.join("some-pack");
        std::fs::create_dir_all(&folder).unwrap();
        write(&folder, "inside.jar");

        let result = copy_into(&dst, &[folder], "overwrite").unwrap();
        assert!(result.imported.is_empty());
        assert!(result.skipped.is_empty());
        assert_eq!(std::fs::read_dir(&dst).unwrap().count(), 0);

        cleanup(&[&root]);
    }

    /// 没扩展名 / 多个点的文件名不被 ` (n)` 规则弄坏
    #[test]
    fn rename_keeps_name_without_extension() {
        ensure_boot();
        let root = temp_dir("import-rename-noext");
        let src = root.join("src");
        let dst = root.join("mods");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        let a = write(&src, "servers.dat");
        std::fs::write(dst.join("servers.dat"), b"old").unwrap();

        let result = copy_into(&dst, &[a], MODE_RENAME).unwrap();
        assert_eq!(result.imported, vec![String::from("servers (1).dat")]);

        cleanup(&[&root]);
    }
}
