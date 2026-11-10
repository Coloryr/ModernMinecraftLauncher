//! Java 选择与调试参数
//!
//! 挑 Java、按版本调整启动参数，以及 JDWP 与调试器包装。

use std::path::PathBuf;

use mml_names::i18_items::error_type::ErrorType;

use crate::{
    game_arg::GameLaunchObj, game_log::GameLog, gui_hook::LaunchGui,
    launcher::instance_setting_obj::InstanceSettingObj,
};

impl InstanceSettingObj {
    /// 获取启动使用的 Java
    ///
    /// - `obj`: 启动配置
    /// - `gui`: 启动界面回调
    ///
    /// # 返回值
    ///
    /// 返回 Java 路径；找不到可用的 Java 返回 `None`
    pub(super) fn get_java(&self, obj: &GameLaunchObj, gui: LaunchGui) -> Option<PathBuf> {
        if mml_jvms::get_all_java().is_empty() {
            mml_jvms::scan_java();
        }

        // 选了具体 Java 就按名字取，custom 才用自定义路径。
        // 老配置里两项都有的，以名字为准 —— 否则改过 Java 的实例还会从旧路径启动
        match self.jvm_name.as_deref() {
            Some(name) if name != "custom" => {
                if let Some(data) = mml_jvms::get_java_info(name) {
                    if data.path.exists() {
                        return Some(data.path.clone());
                    }
                }
                // 名字失效（被删 / 没扫到）：直接按版本要求自动选，不退回旧的自定义路径
                crate::add_game_log_item(&self.uuid, GameLog::JavaLocalRedirect);
            }
            Some("custom") => {
                if let Some(data) = &self.jvm_local {
                    let path = PathBuf::from(data);

                    if path.exists() {
                        return Some(path);
                    }

                    crate::add_game_log_item(&self.uuid, GameLog::JavaLocalRedirect);
                }
            }
            // 自动：旧配置里残留的自定义路径不再生效，按版本要求重新挑
            _ => {}
        }

        // 从小往大取：声明的兼容集合是一个范围，取里面最低的那个最保守。
        // 反过来的话，像 GTNH 声明 [17,21,23,24,25] 的包会挑到 25，
        // 而 Forge 1.7.10 的 FMLTweaker 要设 SecurityManager —— 24 起已被移除，必崩
        let mut list: Vec<_> = obj.java_versions.clone().into_iter().collect();
        list.sort_unstable();
        for item in list.iter() {
            // Java 8 没有跨大版本兼容：launchwrapper 在 9+ 上会
            // ClassCastException（AppClassLoader 不是 URLClassLoader），
            // 所以 8 必须精确命中，不能像 17+ 那样取更高版本
            let jvm = mml_jvms::get_java(*item, *item > 8);
            if let Some(jvm) = jvm {
                if jvm.path.exists() {
                    return Some(jvm.path.clone());
                }
            }
        }

        if let Some(gui) = &gui {
            gui.no_java(list[0]);
        }

        None
    }

    /// 老版本 Forge 的 FMLTweaker 要设 SecurityManager，Java 18 起默认禁止设置，得显式放行
    ///
    /// 只有 Java 24 以下认这个开关：24 起它本身是致命错误（VM 启动阶段直接失败，`System.initPhase3`）。
    /// 所以按**实际选中的** Java 版本决定，取不到版本就不加 —— 宁可让老 Forge 报它自己的错，
    /// 也不能把新 Java 弄得起不来
    ///
    /// - `obj`: 启动内容，`jvm_args` 在此补一项
    /// - `java`: 实际要用的 Java 路径
    pub(super) fn allow_security_manager(obj: &mut GameLaunchObj, java: &std::path::Path) {
        if obj
            .jvm_args
            .iter()
            .any(|arg| arg.starts_with("-Djava.security.manager"))
        {
            return;
        }

        // 只有 Java 18–23 需要显式放行：8 没有这个开关，24 起 SecurityManager 已被移除
        if java_major_of(java).is_some_and(|major| (18..=23).contains(&major)) {
            obj.jvm_args
                .push("-Djava.security.manager=allow".to_string());
        }
    }

    /// 按实际选中的 Java 版本补 / 去 ZGC 的分代开关
    ///
    /// 声明的兼容版本只决定走不走 ZGC；真正跑的是 `get_java` 挑出来的那份。
    /// 21 / 22 的 ZGC 默认不分代，要显式加 `-XX:+ZGenerational`；23 起分代是默认，
    /// 该选项已废弃，26 起 JVM 直接报 Unrecognized VM option。取不到版本时一律不加。
    ///
    /// - `obj`: 启动内容，`jvm_args` 在此就地修正
    /// - `java`: 实际要用的 Java 路径
    pub(super) fn fix_zgc_generational(obj: &mut GameLaunchObj, java: &std::path::Path) {
        fix_zgc_args(&mut obj.jvm_args, java_major_of(java));
    }
}

/// 捕获文件名里能安全使用的实例名片段
///
/// 实例名可以带空格、斜杠、中文，直接当文件名会出问题；这里只留 ASCII 字母数字、
/// 短横线与下划线，其余换成下划线。
///
/// - `name`: 实例名
///
/// # 返回值
///
/// 文件名片段；实例名为空时返回 `instance`
#[cfg(any(windows, target_os = "linux"))]
pub(super) fn capture_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        String::from("instance")
    } else {
        stem
    }
}

/// 建调试器捕获目录失败时的错误
///
/// - `err`: 建目录时的 IO 错误
///
/// # 返回值
///
/// 统一转成进程错误
pub(super) fn debugger_dir_error(err: std::io::Error) -> ErrorType {
    ErrorType::ProcessError(mml_names::i18_items::error_type::ErrorData::from_error(err))
}

/// 拼出 JVM 远程调试的 JDWP 参数
///
/// Java 9 起 `address` 只写端口时仅监听 localhost，要写成 `*:port` 才能被别的机器连上；
/// Java 8 不认 `*:` 前缀，所以两边各按各的写法。
///
/// - `java`: 实际要用的 Java 路径
/// - `port`: 调试端口
///
/// # 返回值
///
/// 可直接放进 JVM 参数的 `-agentlib:jdwp=...` 文本
pub(super) fn jdwp_arg(java: &std::path::Path, port: u16) -> String {
    jdwp_arg_for(java_major_of(java), port)
}

/// 按 Java 大版本拼 JDWP 参数
///
/// - `major`: Java 大版本；取不到按 Java 8 的写法处理
/// - `port`: 调试端口
///
/// # 返回值
///
/// `-agentlib:jdwp=...` 文本
pub(super) fn jdwp_arg_for(major: Option<i32>, port: u16) -> String {
    let address = match major {
        Some(major) if major >= 9 => format!("*:{port}"),
        _ => port.to_string(),
    };
    format!("-agentlib:jdwp=transport=dt_socket,server=y,suspend=n,address={address}")
}

/// 实际选中的 Java 大版本；不在已扫描的 Java 列表里返回 `None`
///
/// - `java`: 实际要用的 Java 路径
///
/// # 返回值
///
/// 主版本号，如 21 / 26
pub(super) fn java_major_of(java: &std::path::Path) -> Option<i32> {
    mml_jvms::get_all_java()
        .iter()
        .find(|item| item.path.as_path() == java)
        .map(|item| item.major_version)
}

/// 按 Java 大版本修正 ZGC 分代开关
///
/// - `args`: 已生成的 JVM 参数
/// - `major`: 实际 Java 大版本；`None` 表示取不到
///
/// 只有 21 / 22 需要 `-XX:+ZGenerational`；23 起分代是默认、该选项已废弃，
/// 26 起 JVM 不再认识它，所以其它版本（以及取不到版本时）一律不带。
pub(super) fn fix_zgc_args(args: &mut Vec<String>, major: Option<i32>) {
    // 先摘掉：23 起它是默认、已废弃，26 起 JVM 直接不认；取不到版本时也按不加处理
    args.retain(|arg| arg != "-XX:+ZGenerational");
    if major.is_some_and(|major| (21..23).contains(&major))
        && args.iter().any(|arg| arg == "-XX:+UseZGC")
    {
        args.push("-XX:+ZGenerational".to_string());
    }
}
