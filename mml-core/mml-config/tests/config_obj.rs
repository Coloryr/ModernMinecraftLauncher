//! 配置数据结构序列化测试，纯函数，不涉及任何全局状态

use mml_config::config_obj::{
    ConfigObj, DnsObj, GCType, GameCheckObj, HttpObj, JvmConfigObj, ProxyState, ProxyType,
    RunArgObj, SourceLocal, WindowSettingObj,
};
use mml_names::VERSION;

/// 默认配置序列化后能完整还原
#[test]
fn config_roundtrip() {
    let obj = ConfigObj::default();
    let json = serde_json::to_string(&obj).unwrap();
    let back: ConfigObj = serde_json::from_str(&json).unwrap();

    assert_eq!(back.version, *VERSION);
    assert!(back.java_list.is_empty());
    assert_eq!(back.http.source, SourceLocal::Offical);
    assert_eq!(back.http.download_thread, 5);
    assert_eq!(back.http.proxy_port, 7890);
    assert_eq!(back.http.check_file, true);
    assert_eq!(back.http.auto_download, true);
    assert_eq!(back.dns.enable, false);
    assert_eq!(back.jvm_arg.gc_mode, Some(GCType::Auto));
    assert_eq!(back.window.width, Some(1280));
    assert_eq!(back.check.core, true);
    assert_eq!(back.check.mod_sha1, true);
}

/// 缺字段的 JSON 应由 `#[serde(default)]` 补全默认值
#[test]
fn partial_json_fills_defaults() {
    let obj: ConfigObj = serde_json::from_str(r#"{"Version":"0.0.0"}"#).unwrap();
    assert_eq!(obj.version, "0.0.0");
    assert_eq!(obj.http.download_thread, 5);
    assert_eq!(obj.http.source, SourceLocal::Offical);
    assert_eq!(obj.dns.enable, false);
    assert_eq!(obj.check.lib, true);
    // Window 缺失时取 ConfigObj::default 里的 WindowSettingObj::new()，即 1280x720 窗口模式，
    // 而不是 WindowSettingObj::default() 的全 None
    assert_eq!(obj.window.full_screen, Some(false));
    assert_eq!(obj.window.width, Some(1280));
    assert_eq!(obj.window.height, Some(720));
    assert_eq!(obj.window.game_title, None);
}

/// 下载源枚举序列化为数字，serde_repr
#[test]
fn source_local_repr() {
    assert_eq!(serde_json::to_string(&SourceLocal::Offical).unwrap(), "0");
    assert_eq!(serde_json::to_string(&SourceLocal::Bmclapi).unwrap(), "1");
    let v: SourceLocal = serde_json::from_str("1").unwrap();
    assert_eq!(v, SourceLocal::Bmclapi);
}

/// 代理策略与代理类型枚举序列化为数字
#[test]
fn proxy_repr() {
    assert_eq!(serde_json::to_string(&ProxyState::Auto).unwrap(), "0");
    assert_eq!(serde_json::to_string(&ProxyState::None).unwrap(), "1");
    assert_eq!(serde_json::to_string(&ProxyState::User).unwrap(), "2");
    assert_eq!(serde_json::to_string(&ProxyType::Http).unwrap(), "0");
    assert_eq!(serde_json::to_string(&ProxyType::Sock4).unwrap(), "1");
    assert_eq!(serde_json::to_string(&ProxyType::Sock5).unwrap(), "2");
    let v: ProxyType = serde_json::from_str("2").unwrap();
    assert_eq!(v, ProxyType::Sock5);
}

/// GC 类型枚举序列化为数字
#[test]
fn gc_type_repr() {
    assert_eq!(serde_json::to_string(&GCType::Auto).unwrap(), "0");
    assert_eq!(serde_json::to_string(&GCType::G1GC).unwrap(), "1");
    assert_eq!(serde_json::to_string(&GCType::ZGC).unwrap(), "2");
    assert_eq!(serde_json::to_string(&GCType::None).unwrap(), "3");
    let v: GCType = serde_json::from_str("1").unwrap();
    assert_eq!(v, GCType::G1GC);
}

/// Java 配置字段名应为 PascalCase，即 Name / Local
#[test]
fn jvm_config_field_names() {
    let j = JvmConfigObj {
        name: String::from("17"),
        local: String::from("java/bin"),
    };
    let json = serde_json::to_string(&j).unwrap();
    assert!(json.contains(r#""Name""#), "字段名应为 Name: {json:?}");
    assert!(json.contains(r#""Local""#), "字段名应为 Local: {json:?}");

    // 缺字段的 JSON 走 Default
    let back: JvmConfigObj = serde_json::from_str("{}").unwrap();
    assert_eq!(back.name, "");
    assert_eq!(back.local, "");
}

/// 子对象各自的默认值
#[test]
fn sub_obj_defaults() {
    let http = HttpObj::default();
    assert_eq!(http.download_thread, 5);
    assert_eq!(http.proxy_ip, "127.0.0.1");
    assert_eq!(http.work_proxy, ProxyState::Auto);
    assert_eq!(http.work_proxy_type, ProxyType::Http);
    assert_eq!(http.login_proxy, ProxyState::Auto);
    assert_eq!(http.login_proxy_type, ProxyType::Http);

    let dns = DnsObj::default();
    assert_eq!(dns.enable, false);
    assert!(dns.https.is_empty());
    assert_eq!(dns.http_proxy, false);

    let check = GameCheckObj::default();
    assert!(check.core && check.lib && check.assets && check.game_mod);
    assert!(check.core_sha1 && check.lib_sha1 && check.assets_sha1 && check.mod_sha1);
}

/// RunArgObj::new 应给全部字段赋 Some 值
#[test]
fn run_arg_new_all_some() {
    let arg = RunArgObj::new();
    assert_eq!(arg.remove_jvm_arg, Some(false));
    assert_eq!(arg.remove_game_arg, Some(false));
    assert_eq!(arg.jvm_args, Some(String::new()));
    assert_eq!(arg.game_args, Some(String::new()));
    assert_eq!(arg.jvm_env, Some(String::new()));
    assert_eq!(arg.gc_mode, Some(GCType::Auto));
    assert!(arg.max_memory.is_some(), "max_memory 应有默认值");
    assert!(arg.min_memory.is_some(), "min_memory 应有默认值");
    assert_eq!(arg.colorasm, Some(false));
    assert_eq!(arg.launch_pre_run, Some(false));
    assert_eq!(arg.pre_run_with_game, Some(true));
    assert_eq!(arg.launch_post_run, Some(false));
    assert_eq!(arg.pre_run_arg, Some(String::new()));
    assert_eq!(arg.post_run_arg, Some(String::new()));
    // 注意：new() 中 max_memory=4096、min_memory=512，此处只断言两者都有默认值，不校验大小关系
}

/// RunArgObj 的 Default 与 new 的区别：Default 全部为 None
#[test]
fn run_arg_default_all_none() {
    let arg = RunArgObj::default();
    assert_eq!(arg.max_memory, None);
    assert_eq!(arg.gc_mode, None);
    assert_eq!(arg.jvm_args, None);
}

/// WindowSettingObj::new 的默认窗口参数
#[test]
fn window_setting_new() {
    let w = WindowSettingObj::new();
    assert_eq!(w.full_screen, Some(false));
    assert_eq!(w.width, Some(1280));
    assert_eq!(w.height, Some(720));
    assert_eq!(w.game_title, None);
    assert_eq!(w.edit_title, None);
    assert_eq!(w.random_title, None);
    assert_eq!(w.cycle_title, None);
    assert_eq!(w.title_delay, None);
}

/// WindowSettingObj 反序列化缺字段时补 None
#[test]
fn window_setting_partial() {
    let w: WindowSettingObj = serde_json::from_str(r#"{"Width":800}"#).unwrap();
    assert_eq!(w.width, Some(800));
    assert_eq!(w.height, None);
}
