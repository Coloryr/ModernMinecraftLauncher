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

