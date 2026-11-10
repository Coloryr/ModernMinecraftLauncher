//! 系统 Java 扫描：查找可执行文件并登记到列表

use super::*;

/// 在指定目录中查找 Java 可执行文件
///
/// - `dir`: 查找路径
///
/// # 返回值
///
/// Windows 找 `javaw.exe`，Linux / macOS 找 `java`；找不到返回 `None`
pub fn find_java_from_path<P: AsRef<Path>>(dir: P) -> Option<PathBuf> {
    let sys = mml_sys::get_system_info();
    match sys.os {
        Os::Windows => path_helper::search_file(dir, names::JAVAW_FILE),
        Os::Linux | Os::MacOS => path_helper::search_file(dir, names::JAVA_FILE),
        _ => None,
    }
}

/// 目录树递归深度上限，够覆盖 `Java/jdk-17/bin` 这类布局
const JAVA_SCAN_MAX_DEPTH: usize = 8;

/// 平台对应的 Java 可执行文件名
///
/// # 返回值
///
/// Windows 为 `javaw.exe`，Linux / macOS 为 `java`；其它系统返回 `None`
fn java_exe_name() -> Option<&'static str> {
    let sys = mml_sys::get_system_info();
    match sys.os {
        Os::Windows => Some(names::JAVAW_FILE),
        Os::Linux | Os::MacOS => Some(names::JAVA_FILE),
        _ => None,
    }
}

/// 在目录树里查找全部 Java 可执行文件
///
/// 目录本身、或任一子目录的 `bin` 下有 Java 可执行文件即算一个 Java。
/// 命中后不再深入该目录，免得把 JDK 自带的 jre 重复算一个。
///
/// - `dir`: 要扫描的根目录
///
/// # 返回值
///
/// 排序去重后的可执行文件路径；目录里没有 Java 时为空列表
pub fn find_all_java_from_path<P: AsRef<Path>>(dir: P) -> Vec<PathBuf> {
    let Some(exe) = java_exe_name() else {
        return Vec::new();
    };

    let mut found = Vec::new();
    collect_java_exe(dir.as_ref(), exe, 0, &mut found);
    found.sort();
    found.dedup();
    found
}

/// 递归收集 Java 可执行文件，命中即停
///
/// - `dir`: 当前目录
/// - `exe`: Java 可执行文件名
/// - `depth`: 当前深度，超过上限即返回
/// - `out`: 收集结果
fn collect_java_exe(dir: &Path, exe: &str, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > JAVA_SCAN_MAX_DEPTH {
        return;
    }

    // 用户可能直接选中了 bin 目录
    let direct = dir.join(exe);
    if direct.is_file() {
        out.push(direct);
        return;
    }

    let home = dir.join("bin").join(exe);
    if home.is_file() {
        out.push(home);
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            collect_java_exe(&path, exe, depth + 1, out);
        }
    }
}

/// 从系统注册表或标准路径中搜索已安装的 Java
///
/// 返回去重后的 Java 列表，按路径排序。
fn find_java() -> Option<Vec<JavaInfoObj>> {
    let mut java_paths = HashSet::new();

    java_scan_helper::find_java_inner(&mut java_paths);

    if java_paths.is_empty() {
        return None;
    }

    // 获取详细信息
    let mut java_list = Vec::new();
    for path in java_paths {
        if let Some(info) = java_helper::test_java(&path) {
            java_list.push(info);
        }
    }

    // 去重，基于路径
    java_list.sort_by(|a, b| a.path.cmp(&b.path));
    java_list.dedup_by(|a, b| a.path == b.path);

    if java_list.is_empty() {
        None
    } else {
        Some(java_list)
    }
}

/// 扫描系统中已安装的 Java 并添加到列表
///
/// 此函数执行系统级的 Java 搜索：注册表、常见路径等。
pub fn scan_java() {
    if let Some(list) = find_java() {
        // 一个都没扫到：列表没变化，不必广播
        if list.is_empty() {
            return;
        }

        let mut list1 = JVMS.write().unwrap();
        let mut config = mml_config::write_config();

        for (_, item) in list.into_iter().enumerate() {
            let name = item.name.clone();
            // 先移除同名旧条目
            config.java_list.retain(|item| {
                if item.name.eq_ignore_ascii_case(&name) {
                    false
                } else {
                    true
                }
            });
            config.java_list.push(JvmConfigObj {
                name: name.clone(),
                local: item.path.to_string_lossy().to_string(),
            });
            list1.insert(item.name.clone(), Arc::new(item));
        }

        drop(config);
        // 放掉列表锁再广播：事件回来时界面会立刻读这份列表
        drop(list1);

        mml_config::save();

        // 扫描会把扫到的 Java 写进列表：广播一次，界面才能跟着刷新。
        // 少了这一句，设置窗口扫完，主窗口实例设置里的 Java 下拉仍是旧的
        invoke_jvm_change();
    }
}
