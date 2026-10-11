//! 主窗口的实例启动参数：核心配置 ↔ 前端 DTO 的双向转换
//!
//! 从 `main.rs` 拆出来的，那边只剩命令与窗口模型。这里的字段是一对一搬运，
//! 但要对着 `InstanceSettingObj` 看语义，单独放一处免得埋在命令堆里。

use crate::dtos::{EnvVarLineDto, InstanceArgsDto};
use mml_config::config_obj::{GCType, RunArgObj, WindowSettingObj};
use mml_game::GameInstance;
use mml_game::launcher::instance_setting_obj::{
    AdvanceJvmObj, InstanceSettingObj, ProxyHostObj, ServerObj,
};
use uuid::Uuid;

/// 解析核心实例配置 -> 前端启动参数 DTO
///
/// - `inst`: 核心实例配置，各字段都是 `Option`，缺省项保留 DTO 的默认值
///
/// # 返回值
///
/// 前端启动参数 DTO；核心侧缺省的内存 / 全屏等沿用 `InstanceArgsDto` 默认值
pub(super) fn args_from_core(inst: &InstanceSettingObj) -> InstanceArgsDto {
    let mut a = InstanceArgsDto::default();
    if let Some(jvm) = &inst.jvm_arg {
        if let Some(v) = jvm.max_memory {
            a.memory = v as i64;
        }
        if let Some(v) = jvm.min_memory {
            a.min_memory = v as i64;
        }
        a.gc = match jvm.gc_mode {
            Some(GCType::G1GC) => "g1gc",
            Some(GCType::ZGC) => "zgc",
            Some(GCType::None) => "none",
            _ => "auto",
        }
        .into();
        if let Some(s) = &jvm.jvm_args {
            a.jvm_args = s.lines().map(String::from).collect();
        }
        if let Some(s) = &jvm.game_args {
            a.game_args = s.lines().map(String::from).collect();
        }
        if let Some(s) = &jvm.jvm_env {
            a.env_vars = s
                .lines()
                .filter_map(|l| {
                    l.split_once('=').map(|(k, v)| EnvVarLineDto {
                        key: k.to_string(),
                        value: v.to_string(),
                    })
                })
                .collect();
        }
        if let Some(v) = jvm.launch_pre_run {
            a.pre_enabled = v;
        }
        if let Some(s) = &jvm.pre_run_arg {
            a.pre_cmd = s.clone();
        }
        if let Some(v) = jvm.launch_post_run {
            a.post_enabled = v;
        }
        if let Some(s) = &jvm.post_run_arg {
            a.post_cmd = s.clone();
        }
    }
    if let Some(w) = &inst.window {
        if let Some(v) = w.full_screen {
            a.fullscreen = v;
        }
        if let Some(v) = w.width {
            a.width = v as i64;
        }
        if let Some(v) = w.height {
            a.height = v as i64;
        }
    }
    a.java_name = inst.jvm_name.clone().unwrap_or_default();
    a.java_path = inst.jvm_local.clone().unwrap_or_default();
    if let Some(adv) = &inst.advance_jvm {
        a.main_class = adv.main_class.clone().unwrap_or_default();
        if let Some(s) = &adv.class_path {
            a.class_path = s
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
        }
    }
    if let Some(p) = &inst.proxy_host {
        a.proxy_ip = p.ip.clone().unwrap_or_default();
        a.proxy_port = p.port.unwrap_or(0) as i64;
        a.proxy_user = p.user.clone().unwrap_or_default();
        a.proxy_pass = p.password.clone().unwrap_or_default();
    }
    if let Some(s) = &inst.start_server {
        a.join_server = s.enable;
        a.server_ip = s.ip.clone().unwrap_or_default();
        a.server_port = s.port.unwrap_or(0) as i64;
    }
    a
}

/// 前端启动参数 DTO -> 写入核心实例配置，不保存，由调用方 save
///
/// - `inst`: 待写入的核心实例配置，缺失的子对象就地创建
/// - `a`: 前端提交的启动参数
pub(super) fn apply_args_to_core(inst: &mut InstanceSettingObj, a: &InstanceArgsDto) {
    let jvm = inst.jvm_arg.get_or_insert_with(RunArgObj::default);
    jvm.max_memory = Some(a.memory.max(0) as u32);
    jvm.min_memory = Some(a.min_memory.max(0) as u32);
    jvm.gc_mode = Some(match a.gc.as_str() {
        "g1gc" => GCType::G1GC,
        "zgc" => GCType::ZGC,
        "none" => GCType::None,
        _ => GCType::Auto,
    });
    // 自定义 GC 参数没有独立字段：并入附加 JVM 参数一起下发
    let mut jvm_lines: Vec<String> = a.jvm_args.clone();
    if a.gc == "custom" && !a.gc_custom.trim().is_empty() {
        jvm_lines.extend(a.gc_custom.lines().map(String::from));
    }
    jvm.jvm_args = Some(jvm_lines.join("\n"));
    jvm.game_args = Some(a.game_args.join("\n"));
    jvm.jvm_env = Some(
        a.env_vars
            .iter()
            .map(|v| format!("{}={}", v.key, v.value))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    jvm.launch_pre_run = Some(a.pre_enabled);
    jvm.pre_run_arg = Some(a.pre_cmd.clone());
    jvm.launch_post_run = Some(a.post_enabled);
    jvm.post_run_arg = Some(a.post_cmd.clone());

    let win = inst.window.get_or_insert_with(WindowSettingObj::default);
    win.full_screen = Some(a.fullscreen);
    win.width = Some(a.width.clamp(0, u16::MAX as i64) as u16);
    win.height = Some(a.height.clamp(0, u16::MAX as i64) as u16);

    inst.jvm_name = (!a.java_name.is_empty()).then(|| a.java_name.clone());
    inst.jvm_local = (!a.java_path.is_empty()).then(|| a.java_path.clone());

    let adv = inst.advance_jvm.get_or_insert_with(AdvanceJvmObj::default);
    adv.main_class = (!a.main_class.is_empty()).then(|| a.main_class.clone());
    adv.class_path = (!a.class_path.is_empty()).then(|| a.class_path.join(";"));

    let proxy = inst.proxy_host.get_or_insert_with(ProxyHostObj::default);
    proxy.ip = (!a.proxy_ip.is_empty()).then(|| a.proxy_ip.clone());
    proxy.port = (a.proxy_port > 0).then_some(a.proxy_port as u16);
    proxy.user = (!a.proxy_user.is_empty()).then(|| a.proxy_user.clone());
    proxy.password = (!a.proxy_pass.is_empty()).then(|| a.proxy_pass.clone());

    let server = inst.start_server.get_or_insert_with(ServerObj::default);
    server.enable = a.join_server && !a.server_ip.trim().is_empty();
    server.ip = (!a.server_ip.is_empty()).then(|| a.server_ip.clone());
    server.port = (a.server_port > 0).then_some(a.server_port as u16);
}

/// 已解析出来的核心实例：uuid 与实例句柄
///
/// 两者一起返回，是因为调用方几乎总要拿 uuid 去发事件或清缓存，而 `GameInstance` 自身
/// 只有设置内容、没有 uuid。
pub(crate) struct CoreInstance {
    /// 实例 uuid，由调用方传入的字符串解析而来
    pub(crate) uuid: Uuid,
    /// 实例句柄，与 `mml_game::get_instance` 返回的是同一份
    pub(crate) instance: GameInstance,
}

/// 取核心实例，含 uuid 解析与存在性检查
///
/// - `uuid`: 实例 uuid 字符串，可能非法
///
/// # 返回值
///
/// 见 [`CoreInstance`]；uuid 非法或实例不存在返回 `None`
pub(crate) fn core_instance(uuid: &str) -> Option<CoreInstance> {
    Uuid::parse_str(uuid).ok().and_then(|id| {
        mml_game::get_instance(&id).map(|inst| CoreInstance {
            uuid: id,
            instance: inst,
        })
    })
}
