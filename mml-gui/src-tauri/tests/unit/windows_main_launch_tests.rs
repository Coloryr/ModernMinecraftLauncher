use super::*;

/// 实例没配启动命令时回落到全局设置，开关或命令为空都算不执行
#[test]
fn effective_commands_falls_back_to_global() {
    let mut global = RunArgObj::new();
    global.launch_pre_run = Some(true);
    global.pre_run_arg = Some("echo pre".into());
    global.launch_post_run = Some(true);
    global.post_run_arg = Some("echo post".into());

    // 实例整块没配：两条都用全局的
    let inst = InstanceSettingObj::default();
    let cmds = effective_commands(&inst, &global);
    assert_eq!(cmds.pre.as_deref(), Some("echo pre"));
    assert_eq!(cmds.post.as_deref(), Some("echo post"));

    // 实例自己关了启动前开关：连全局那条也不跑
    let mut inst = InstanceSettingObj::default();
    let mut own = RunArgObj::new();
    own.launch_pre_run = Some(false);
    own.launch_post_run = None;
    own.post_run_arg = Some("echo own".into());
    inst.jvm_arg = Some(own);
    let cmds = effective_commands(&inst, &global);
    assert!(cmds.pre.is_none());
    // 启动后开关没写：回落到全局的开关，命令用实例自己那条
    assert_eq!(cmds.post.as_deref(), Some("echo own"));

    // 开关开着但命令是空串：不算会执行
    let mut inst = InstanceSettingObj::default();
    let mut own = RunArgObj::new();
    own.launch_pre_run = Some(true);
    own.pre_run_arg = Some(String::new());
    inst.jvm_arg = Some(own);
    assert!(effective_commands(&inst, &global).pre.is_none());
}

#[test]
fn offline_launch_requires_a_premium_login() {
    assert!(offline_needs_premium(AuthType::Offline, false));
    assert!(!offline_needs_premium(AuthType::Offline, true));
    assert!(!offline_needs_premium(AuthType::OAuth, false));
    assert!(!offline_needs_premium(AuthType::Nide8, false));
}


/// 脚本测试用的那份命令：路径带空格、目录带单引号、环境变量带百分号
fn script_parts() -> LaunchCmdParts {
    LaunchCmdParts {
        java: "C:\\Program Files\\Java\\bin\\java.exe".into(),
        dir: "D:\\Games\\It's Mine".into(),
        args: vec!["-Xmx2G".into(), "-Dname=a b".into()],
        env: vec![
            "JAVA_HOME=C:\\Program Files\\Java".into(),
            "RATE=50%".into(),
        ],
    }
}



#[test]
fn bat_script_quotes_paths_and_escapes_percent() {
    let script = script_bat(&script_parts());
    assert!(script.starts_with("@echo off\r\nchcp 65001 >nul\r\nsetlocal\r\ncd /d \"D:\\Games\\It's Mine\"\r\n"));
    assert!(script.contains("set \"JAVA_HOME=C:\\Program Files\\Java\"\r\n"));
    // 批处理里百分号是变量展开符，字面量要写成两个
    assert!(script.contains("set \"RATE=50%%\"\r\n"));
    assert!(script.contains("\"C:\\Program Files\\Java\\bin\\java.exe\" -Xmx2G \"-Dname=a b\""));
    assert!(script.ends_with("endlocal\r\n"));
}

#[test]
fn ps1_script_doubles_single_quotes() {
    let script = script_ps1(&script_parts());
    // 开头有 BOM，PowerShell 5.1 才按 UTF-8 读
    assert!(script.starts_with("\u{feff}Set-Location -LiteralPath 'D:\\Games\\It''s Mine'\r\n"));
    assert!(script.contains("$env:JAVA_HOME = 'C:\\Program Files\\Java'\r\n"));
    assert!(script.contains("$env:RATE = '50%'\r\n"));
    assert!(script.contains("& 'C:\\Program Files\\Java\\bin\\java.exe' '-Xmx2G' '-Dname=a b'\r\n"));
}

#[test]
fn sh_script_escapes_single_quotes() {
    let script = script_sh(&script_parts());
    assert!(script.starts_with("#!/bin/sh\n"));
    assert!(script.contains("cd 'D:\\Games\\It'\\''s Mine' || exit 1\n"));
    assert!(script.contains("export JAVA_HOME='C:\\Program Files\\Java'\n"));
    assert!(script.contains("export RATE='50%'\n"));
    assert!(script.contains("exec 'C:\\Program Files\\Java\\bin\\java.exe' '-Xmx2G' '-Dname=a b'\n"));
}
