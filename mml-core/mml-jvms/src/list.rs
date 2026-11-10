//! Java 列表的增删查与配置同步

use super::*;

/// 从配置文件加载 Java 列表，异步逐个测试有效性
///
/// 清空内存列表后按配置重建：有效的 Java 记录真实信息，
/// 无效的保留占位条目，`major_version` 为 -1，加载完成后触发变更事件。
pub fn load() {
    let config = mml_config::read_config();
    let config = &config.java_list;

    add_list(config);
}

/// 根据名称获取 Java 信息
///
/// - `key`: Java 名称
///
/// # 返回值
///
/// 找到则返回 `Arc<JavaInfoObj>` 的克隆，未找到返回 `None`
pub fn get_java_info(key: &str) -> Option<Arc<JavaInfoObj>> {
    let list = JVMS.read().ok()?;
    let item = list.get(key)?;
    Some(item.clone())
}

/// 删除指定名称的 Java
///
/// 同时从内存列表和配置文件中的 Java 列表中移除。
///
/// - `name`: Java 名称
pub fn remove(name: &str) {
    let removed = {
        let mut list = JVMS.write().unwrap();
        list.remove(name).is_some()
    };
    // 事件回调可能读取 JVMS，须在写锁释放后触发
    if removed {
        invoke_jvm_change();
    }

    // 先释放写锁再保存：save() 内部要拿 CONFIG 读锁，同线程写锁未释放时重入会死锁
    let mut find = false;
    {
        let mut config = mml_config::write_config();
        let javas = &mut config.java_list;
        javas.retain(|item| {
            if item.name.eq_ignore_ascii_case(name) {
                find = true;
                false
            } else {
                true
            }
        });
    }

    if find {
        mml_config::save();
    }
}

/// 删除所有 Java 并保存配置
pub fn remove_all() {
    let mut list = JVMS.write().unwrap();
    list.clear();

    drop(list);

    // 先释放写锁再保存：save() 内部要拿 CONFIG 读锁，同线程写锁未释放时重入会死锁
    {
        let mut config = mml_config::write_config();
        config.java_list.clear();
    }
    mml_config::save();
}

/// 添加一个 Java 运行时
///
/// 测试 Java 可执行文件是否有效，有效则加入列表并保存配置。
///
/// - `name`: Java 显示名称
/// - `file`: Java 可执行文件路径
///
/// # 返回值
///
/// 添加成功返回该 Java 的信息，可执行文件无效时返回 `None`
pub fn add_item(name: Option<String>, file: String) -> Option<Arc<JavaInfoObj>> {
    let dir = mml_base::get_base_dir();
    let local = if file.starts_with(dir.to_str().unwrap()) {
        String::from(&file[dir.to_str().unwrap().len()..])
    } else {
        file
    };
    let path = if local.starts_with(names::JAVA_DIR) {
        dir.join(&local)
    } else {
        Path::new(&local).to_path_buf()
    };

    let info = java_helper::test_java(&path);

    match info {
        None => None,
        Some(info) => {
            // 名字是列表的唯一键，回填进条目里，界面上显示的才是这个名字
            let info = JavaInfoObj {
                name: name.unwrap_or_else(|| info.name.clone()),
                ..info
            };
            let name = info.name.clone();

            // 先移除同名旧条目
            remove(&name);

            let info = Arc::new(info);

            let mut list = JVMS.write().unwrap();
            list.insert(name.clone(), info.clone());

            drop(list);

            invoke_jvm_change();

            {
                let mut config = mml_config::write_config();
                let javas = &mut config.java_list;
                javas.push(JvmConfigObj {
                    name: name,
                    local: local.clone(),
                });
            }
            mml_config::save();

            Some(info.clone())
        }
    }
}

/// 直接添加一条已知的 Java 信息，跳过可执行文件有效性测试
///
/// - `info`: Java 信息
///
/// # 返回值
///
/// 存入全局列表后的 Java 信息
pub fn add_info_item(info: JavaInfoObj) -> Arc<JavaInfoObj> {
    let mut added = add_info_items(vec![info]);
    added.pop().unwrap()
}

/// 批量添加已知的 Java 信息，跳过可执行文件有效性测试
///
/// 名称与已有条目重名但路径不同时，用 Java 目录路径做后缀区分，避免互相覆盖。
///
/// - `list`: 待添加的 Java 信息
///
/// # 返回值
///
/// 存入全局列表后的 Java 信息，顺序与入参一致
pub fn add_info_items(list: Vec<JavaInfoObj>) -> Vec<Arc<JavaInfoObj>> {
    let mut added = Vec::with_capacity(list.len());

    // 配置写锁必须在 save() 之前释放：save() 要拿同一把锁的读侧，同线程先写后读会自锁
    {
        let mut jvms = JVMS.write().unwrap();
        let mut config = mml_config::write_config();

        for item in list {
            let name = unique_java_name(&jvms, &item);

            jvms.remove(&name);
            config
                .java_list
                .retain(|saved| !saved.name.eq_ignore_ascii_case(&name));

            let info = Arc::new(JavaInfoObj {
                name: name.clone(),
                ..item
            });
            jvms.insert(name.clone(), info.clone());
            config.java_list.push(JvmConfigObj {
                name,
                local: info.path.to_string_lossy().to_string(),
            });
            added.push(info);
        }
    }

    // 事件回调会读 JVMS，save() 要拿配置读锁，都得在锁释放后触发
    invoke_jvm_change();
    mml_config::save();

    added
}

/// 取一个不与列表里其它路径冲突的名称
///
/// - `jvms`: 当前 Java 列表
/// - `info`: 待添加的 Java 信息
///
/// # 返回值
///
/// 与已有条目同路径时原样返回，即覆盖导入；否则加目录路径或序号区分
fn unique_java_name(jvms: &HashMap<String, Arc<JavaInfoObj>>, info: &JavaInfoObj) -> String {
    let taken = |name: &str| match jvms.get(name) {
        Some(exists) => exists.path != info.path,
        None => false,
    };

    if !taken(&info.name) {
        return info.name.clone();
    }

    // 用 Java 目录的尾部路径区分，越短越好：jdk-17 再用 openjdk-jdk-17
    let parts = java_home_parts(&info.path);
    for start in (0..parts.len()).rev() {
        let name = format!("{}-{}", info.name, parts[start..].join("-"));
        if !taken(&name) {
            return name;
        }
    }

    // 兜底：尾部路径也撞上时追加序号
    let mut index = 2;
    loop {
        let name = format!("{}-{}", info.name, index);
        if !taken(&name) {
            return name;
        }
        index += 1;
    }
}

/// Java 可执行文件所在目录的路径分段
///
/// - `path`: Java 可执行文件路径，形如 `Program Files/Java/openjdk/jdk-17/bin/javaw.exe`
///
/// # 返回值
///
/// 去掉 `bin` 之后的各段目录名，如 `["Program Files", "Java", "openjdk", "jdk-17"]`
fn java_home_parts(path: &Path) -> Vec<String> {
    let Some(home) = path.parent().and_then(|bin| bin.parent()) else {
        return Vec::new();
    };

    home.components()
        .filter_map(|part| match part {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

/// 从配置列表批量测试并添加 Java
///
/// - `list`: 配置文件中保存的 Java 列表
fn add_list(list: &Vec<JvmConfigObj>) {
    let dir = mml_base::get_base_dir();
    let list_cloned = list.clone();

    let mut list1 = JVMS.write().unwrap();
    list1.clear();

    drop(list1);

    // 在异步任务中逐个测试 Java
    tokio::task::spawn(async move {
        let mut empty: bool = false;
        for item in list_cloned.iter() {
            let path = item.local.clone();
            let path = if path.starts_with(names::JAVA_DIR) {
                dir.join(path)
            } else {
                PathBuf::from(path)
            };

            let info = java_helper::test_java(&path);
            let mut list1 = JVMS.write().unwrap();
            list1.remove(&item.name);

            if info.is_none() {
                // Java 无效，保留占位条目
                list1.insert(
                    item.name.clone(),
                    Arc::new(JavaInfoObj {
                        name: item.name.clone(),
                        path,
                        version: String::new(),
                        major_version: -1,
                        java_type: String::new(),
                        arch: ArchEnum::Unknown,
                    }),
                );
            } else {
                let mut info = info.unwrap();
                info.name = item.name.clone();
                list1.insert(item.name.clone(), Arc::new(info));
            }

            empty = false;
        }

        // 配置加载完成，含无效占位条目，通知 UI 刷新列表
        invoke_jvm_change();

        if empty {
            scan_java();
        }
    });
}

/// 根据版本需求查找匹配的 Java
///
/// - `version`: 所需的主版本号，如 17、21
/// - `over`: `true` 允许返回更高版本的 Java，`false` 要求精确匹配
///
/// # 返回值
///
/// 找到则返回匹配的 Java 信息，未找到返回 `None`
pub fn get_java(version: i32, over: bool) -> Option<Arc<JavaInfoObj>> {
    let list = JVMS.read().ok()?;
    let system_arch = mml_sys::get_system_info().system_arch;

    let mut filtered: Vec<&Arc<JavaInfoObj>> = list
        .iter()
        .filter(|item| {
            if over {
                item.1.major_version >= version
            } else {
                item.1.major_version == version
            }
        })
        .filter(|item| item.1.arch == system_arch)
        .map(|item| item.1)
        .collect();

    // 按版本号降序排列，优先选择最新版本
    filtered.sort_by(|a, b| b.major_version.cmp(&a.major_version));

    filtered.first().map(|&info| info.clone())
}

/// 获取所有已配置的 Java 运行时列表
///
/// # 返回值
///
/// 所有已配置的 Java 运行时；一个都没有时为空列表
pub fn get_all_java() -> Vec<Arc<JavaInfoObj>> {
    let read = JVMS.read().unwrap();
    let mut vec = Vec::new();

    for (_, value) in read.iter() {
        vec.push(value.clone());
    }

    vec
}
