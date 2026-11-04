//! 实例启动参数保存：Java 名字与自定义路径只能留一项

use super::*;

/// 选具体 Java 时清掉旧路径，custom 才保存路径，自动则两项都清空
#[test]
fn java_selection_keeps_one_source() {
    let mut inst = InstanceSettingObj::default();
    let mut args = InstanceArgsDto::default();

    args.java_name = "Java 21".to_string();
    args.java_path = "C:/old/java.exe".to_string();
    apply_args_to_core(&mut inst, &args);
    assert_eq!(inst.jvm_name.as_deref(), Some("Java 21"));
    assert!(inst.jvm_local.is_none(), "选了具体 Java 不该再留着旧路径");

    args.java_name = "custom".to_string();
    args.java_path = "C:/new/java.exe".to_string();
    apply_args_to_core(&mut inst, &args);
    assert_eq!(inst.jvm_name.as_deref(), Some("custom"));
    assert_eq!(inst.jvm_local.as_deref(), Some("C:/new/java.exe"));

    args.java_name = String::new();
    apply_args_to_core(&mut inst, &args);
    assert!(inst.jvm_name.is_none());
    assert!(inst.jvm_local.is_none());
}
