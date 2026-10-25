//! 实例与分组的查询
//!
//! 列表、启动参数、语言都是实例配置的只读口径；
//! 写回启动参数只改这一份配置，改完推一次实例变更。

use std::{fs, path::Path};
use tauri::WebviewWindow;
use uuid::Uuid;

use mml_game::launcher::LogEncoding;
use mml_game::loader::LoaderType;

use crate::dtos::main_dto::{LoadState};
use crate::dtos::{
    GroupDto, InstanceArgsDto, InstanceInfoDto, InstanceLangDto, JavaInfoDto, LaunchPrecheckDto,
};

use crate::windows::main::args::{apply_args_to_core, args_from_core, core_instance};
use crate::windows::main::model;


/// 获取实例列表，对接 mml-game::get_instances 并合并运行状态
///
/// # 返回值
///
/// 全部核心实例的 DTO 列表，分组归属与组内次序取自内核分组表
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_get_instances() -> Vec<InstanceInfoDto> {
    mml_game::get_instances()
        .into_iter()
        // 正在删除的实例不列出来：删除要挪回收站，界面在删除开始时就当它没了
        .filter(|inst| !crate::windows::main::deleting::is_deleting(&inst.read().unwrap().uuid))
        .map(|inst| {
            let inst = inst.read().unwrap();
            InstanceInfoDto {
                uuid: inst.uuid.to_string(),
                name: inst.name.clone(),
                // 分组归属与组内次序都在 mml-game 的分组表 group_save.json 里：
                // 实例配置 game.json 与 gui_setting.json 都没有这两个字段了
                group: mml_game::get_instance_group(&inst.uuid).map(|g| g.to_string()),
                version: inst.version.clone(),
                version_type: Some(inst.game_type.id().to_string()),
                quick_play: mml_base::version_parse::is_game_version_120(&inst.version),
                auto_save: inst.auto_save.clone(),
                loader: String::from(inst.loader.id()),
                loader_version: inst.loader_version.clone(),
                dir: inst.dir.clone(),
                running: mml_game::is_running(&inst.uuid),
                modpack_type: inst.is_modpack.then(|| inst.modpack_type.to_string()),
                pid: inst.pid.clone(),
                fid: inst.fid.clone(),
                server_url: inst.server_url.clone(),
                lang: None,
                log_encoding: Some(
                    if matches!(inst.encoding, LogEncoding::GBK) {
                        "gbk"
                    } else {
                        "utf8"
                    }
                    .to_string(),
                ),
                source: None,
                // 组内次序 = 分组表里的 `order`，键为实例 uuid；不在任何分组里时排到最后
                order: mml_game::get_instance_order(&inst.uuid).unwrap_or(i32::MAX),
                // 这里正拿着实例读锁，只能查不锁实例的那些占用
                busy: crate::windows::main::instance::group::busy_of(
                    &inst.uuid,
                    Path::new(&inst.dir),
                )
                .is_some(),
            }
        })
        .collect()
}

/// 获取分组列表，含空分组
///
/// 返回 uuid + 组名：分组以 uuid 为身份、组名只是显示数据，
/// 前端要用 uuid 回传，用于移动实例 / 删组 / 调序。
///
/// # 返回值
///
/// 按分组表顺序排列的分组列表，含没有实例的空分组
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_get_groups() -> Vec<GroupDto> {
    mml_game::get_group_list()
        .into_iter()
        .map(|g| GroupDto {
            uuid: g.uuid.to_string(),
            name: g.name,
        })
        .collect()
}

/// 获取实例的游戏内语言列表，从资源索引查 minecraft/lang/*.json，资源未下载时为空
///
/// 每项带显示名：名字取自对应语言文件里的 `language.name`，如 zh_cn → 简体中文，
/// 读不到时回落为语言代码，见 `mml_game::get_instance_langs`。
///
/// 查询要读整份资源索引 JSON、再逐个读语言文件，是同步重活：丢到阻塞线程执行，
/// 不要占住 async runtime —— 否则同期的其它 IPC 与图片协议请求会被一起拖住。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 语言项列表，含代码与显示名；uuid 非法或资源未下载时为空列表
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub async fn main_get_instance_langs(uuid: String) -> Vec<InstanceLangDto> {
    let res = tauri::async_runtime::spawn_blocking(move || {
        let Ok(id) = Uuid::parse_str(&uuid) else {
            return Vec::new();
        };
        mml_game::get_instance_langs(&id)
            .into_iter()
            .map(|item| InstanceLangDto {
                code: item.code,
                name: item.name,
            })
            .collect()
    })
    .await;

    res.unwrap_or_default()
}

/// 获取实例启动参数，核心实例读配置，遗留数据读内存缓存
///
/// - `window`: 调用方窗口，用于取本窗口模型里的遗留参数
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 启动参数 DTO；两处都取不到时返回默认值
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_get_instance_args(window: WebviewWindow, uuid: String) -> InstanceArgsDto {
    if let Some(core) = core_instance(&uuid) {
        let obj = core.instance.read().unwrap();
        return args_from_core(&obj);
    }
    if let Ok(store) = model(&window)
        && let Some(a) = store.lock().unwrap().args.get(&uuid)
    {
        return a.clone();
    }
    InstanceArgsDto::default()
}

/// 启动前检查：模组数 / 加载器 / 最大内存，以及两项检查开关
///
/// 内存阈值沿用 ColorMC：最大内存 ≤ 4096 MB 且模组 > 150，或 ≤ 8192 MB 且模组 > 300。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 实例不存在时 `None`；否则返回检查所需的原始值与开关
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_check_launch(uuid: String) -> Option<LaunchPrecheckDto> {
    let core = core_instance(&uuid)?;
    let (mods_path, loader_normal, max) = {
        let inst = core.instance.read().unwrap();
        (
            inst.get_mods_path(),
            inst.loader == LoaderType::Normal,
            inst.jvm_arg.as_ref().and_then(|jvm| jvm.max_memory),
        )
    };
    let mods = count_enabled_mods(&mods_path);
    let max_memory = max
        .or(mml_config::read_config().jvm_arg.max_memory)
        .unwrap_or(4096);
    let check = crate::gui_config::get().launch_check;
    Some(LaunchPrecheckDto {
        mods,
        loader_normal,
        max_memory,
        memory_low: memory_low(mods, max_memory),
        check_loader: check.check_loader,
        check_memory: check.check_memory,
    })
}

/// 内存是否偏低：最大内存 ≤ 4 GB 且模组超过 150，或 ≤ 8 GB 且超过 300
///
/// - `mods`: 启用中的模组数
/// - `max_memory`: 当前最大内存，单位 MB
///
/// # 返回值
///
/// 判定偏低返回 `true`
fn memory_low(mods: u32, max_memory: u32) -> bool {
    (max_memory <= 4096 && mods > 150) || (max_memory <= 8192 && mods > 300)
}

/// 数 `mods` 目录里启用的 jar；`.jar.disabled` 这类扩展名不是 jar，自然不会算进去
///
/// - `dir`: 模组目录
///
/// # 返回值
///
/// 启用中的 jar 数量
fn count_enabled_mods(dir: &Path) -> u32 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
        })
        .count() as u32
}

/// 更新实例启动参数，核心实例写配置并保存，遗留数据只更新内存缓存
///
/// - `window`: 调用方窗口，用于取本窗口模型存遗留参数
/// - `uuid`: 实例 uuid
/// - `args`: 前端提交的启动参数，整份覆盖
///
/// # 返回值
///
/// 写入成功返回 `true`；模型缺失返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_update_instance_args(
    window: WebviewWindow,
    uuid: String,
    args: InstanceArgsDto,
) -> Result<bool, String> {
    if let Some(core) = core_instance(&uuid) {
        {
            let mut obj = core.instance.write().unwrap();
            apply_args_to_core(&mut obj, &args);
            obj.save();
        }
        // 这里**不发** instance-change：启动参数不进实例列表，也不需要刷新图标。
        // 发了的后果是每改一次设置，图标就换 URL 重拉一次、列表也白刷一遍
        return Ok(true);
    }
    let store = model(&window)?;
    store.lock().unwrap().args.insert(uuid, args);
    Ok(true)
}

/// 核心加载状态，前端兜底：启动太快时 load-done 事件会先于页面监听发出而被错过
///
/// # 返回值
///
/// 加载状态：`ok` 表示核心是否已就绪，`error` 恒为 `None`，失败详情走事件
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_load_state() -> LoadState {
    LoadState {
        ok: mml_core::get_state(),
        error: None,
    }
}

/// 从 mml_jvms 读取 Java 列表，配置加载 / 扫描异步进行，未完成时为空
///
/// # 返回值
///
/// Java 项列表；主版本号未知时为 0
pub(crate) fn java_list() -> Vec<JavaInfoDto> {
    mml_jvms::get_all_java()
        .iter()
        .map(|j| JavaInfoDto {
            name: j.name.clone(),
            path: j.path.to_string_lossy().to_string(),
            version: j.version.clone(),
            // 遗留占位条目即 Java 失效的，主版本号为 -1，前端用 0 表示未知
            major: j.major_version.max(0),
            java_type: j.java_type.clone(),
            arch: j.arch.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只数启用中的 jar：大写扩展名算，`.jar.disabled`、目录与非 jar 文件不算
    #[test]
    fn counts_enabled_jars_only() {
        let dir = crate::test_support::temp_dir("launch-memory-mods");
        fs::write(dir.join("a.jar"), b"").unwrap();
        fs::write(dir.join("b.JAR"), b"").unwrap();
        fs::write(dir.join("c.jar.disabled"), b"").unwrap();
        fs::write(dir.join("d.txt"), b"").unwrap();
        fs::create_dir(dir.join("e.jar")).unwrap();
        assert_eq!(count_enabled_mods(&dir), 2);
        crate::test_support::cleanup(&[&dir]);
    }

    /// 阈值沿用 ColorMC：4 GB 档超过 150、8 GB 档超过 300
    #[test]
    fn memory_low_thresholds() {
        assert!(!memory_low(150, 4096));
        assert!(memory_low(151, 4096));
        assert!(!memory_low(300, 8192));
        assert!(memory_low(301, 8192));
        // 超过 8 GB 不再按这两档提醒
        assert!(!memory_low(301, 8193));
    }
}
