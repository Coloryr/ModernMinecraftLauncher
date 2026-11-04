//! Java 运行时管理模块
//!
//! 本模块负责管理启动器中配置的 Java 运行时环境，即 JRE / JDK。
//!
//! # 核心功能
//!
//! - **自动扫描** — Windows 查系统注册表，Linux / macOS 查标准路径
//! - **手动添加** — 用户可手动指定 Java 可执行文件路径
//! - **版本匹配** — 根据 Minecraft 版本自动选择兼容的 Java 版本
//! - **架构匹配** — 自动过滤与系统架构一致的 Java，即 x86_64 / aarch64
//! - **变更通知** — 通过事件回调通知 UI 层 Java 列表已变更
//!
//! # 数据结构
//!
//! [`JavaInfoObj`] 包含 Java 的名称、路径、版本、主版本号、类型和架构信息。

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, OnceLock, RwLock},
};

use mml_base::{
    archives::{self, ArchiveType, BaseArchiveGui},
    events::EventHandler,
};
use mml_config::config_obj::JvmConfigObj;
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
use mml_sys::{ArchEnum, Os, java_scan_helper, path_helper};

pub mod java_helper;

/// Java 运行时信息
#[derive(Debug, Clone)]
pub struct JavaInfoObj {
    /// Java 显示名称，如 "OpenJDK-17.0.1-x86_64"
    pub name: String,
    /// Java 可执行文件的完整路径
    pub path: PathBuf,
    /// Java 完整版本号字符串，如 "17.0.1"
    pub version: String,
    /// Java 主版本号，如 8、11、17、21；占位条目为 -1 表示不可用
    pub major_version: i32,
    /// Java 发行版类型，如 "OpenJDK"、"Oracle"
    pub java_type: String,
    /// CPU 架构，与系统架构一致才会被版本匹配选中
    pub arch: ArchEnum,
}

/// Java 运行时存放目录，即 `{运行目录}/java/`
static JAVA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 全局 Java 运行时列表，按名称索引
static JVMS: LazyLock<RwLock<HashMap<String, Arc<JavaInfoObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Java 列表变更事件，用于通知 UI 刷新
static JVM_CHANGE_EVENT: LazyLock<EventHandler> = LazyLock::new(|| EventHandler::new());

/// 注册 Java 列表变更回调
///
/// - `handler`: 回调函数
///
/// # 返回值
///
/// 返回回调 ID，可用于 [`remove_jvm_change`] 取消注册
pub fn add_jvm_change<F>(handler: F) -> u64
where
    F: Fn() + Send + Sync + 'static,
{
    JVM_CHANGE_EVENT.add_handler(handler)
}

/// 移除 Java 列表变更回调
///
/// - `id`: 回调注册时返回的 ID
pub fn remove_jvm_change(id: u64) {
    JVM_CHANGE_EVENT.remove_handle(id);
}

/// 触发 Java 列表变更事件，仅供 crate 内部调用
pub(crate) fn invoke_jvm_change() {
    JVM_CHANGE_EVENT.emit();
}

/// 获取 Java 运行时根目录
///
/// # 返回值
///
/// `<运行目录>/java`；未初始化会 panic
pub fn get_java_dir() -> PathBuf {
    JAVA_DIR.get().unwrap().clone()
}

/// 初始化 Java 运行时管理
///
/// 确定 Java 存放目录并在缺失时创建，仅在程序启动阶段调用一次。
///
/// - `dir`: 程序运行根目录
///
/// # 返回值
///
/// 目录创建失败时返回相应错误
pub fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    let dir = JAVA_DIR.get_or_init(|| dir.as_ref().join(names::JAVA_DIR));
    if !dir.exists() {
        path_helper::create_dir_all(dir)?;
    }

    Ok(())
}

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

    drop(jvms);

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

/// 解压 Java 压缩包并注册其中的 Java
///
/// - `name`: Java 显示名称，`None` 时取压缩包文件名
/// - `path`: 压缩包路径
/// - `gui`: 解压进度回调
///
/// # 返回值
///
/// 解压并注册成功返回该 Java 的信息；格式不支持时返回 `Err`，其中找不到 Java 或测试不通过时返回 `None`
pub async fn unzip_java<P: AsRef<Path>>(
    name: Option<String>,
    path: P,
    gui: BaseArchiveGui,
) -> CoreResult<Option<Arc<JavaInfoObj>>> {
    let dir = extract_java(name, path, gui).await?;
    register_java_dir(dir).await
}

/// 只解压 Java 压缩包到 `java/<名字>`，不识别也不注册
///
/// 下载器只负责把 zip 拿到本地，解压单独作为一步，界面按阶段显示进度。
///
/// - `name`: 解压目录名；`None` 时取压缩包文件名
/// - `path`: 压缩包路径
/// - `gui`: 解压进度回调
///
/// # 返回值
///
/// 解压出来的目录路径；格式不支持时返回 `Err`
pub async fn extract_java<P: AsRef<Path>>(
    name: Option<String>,
    path: P,
    gui: BaseArchiveGui,
) -> CoreResult<PathBuf> {
    let path = path.as_ref().to_path_buf();
    // 解压是阻塞操作：丢到阻塞线程池，避免占住异步 worker
    tokio::task::spawn_blocking(move || extract_java_blocking(name, path, gui))
        .await
        .map_err(|_| ErrorType::InvalidOperation)?
}

/// 从已解压目录识别并注册 Java
///
/// - `dir`: 解压出来的目录
///
/// # 返回值
///
/// 注册成功的 Java；目录里找不到 Java 时返回 `Ok(None)`
pub async fn register_java_dir(dir: PathBuf) -> CoreResult<Option<Arc<JavaInfoObj>>> {
    // 探测要跑一次 java -version，同样是阻塞操作
    tokio::task::spawn_blocking(move || register_java_dir_blocking(dir))
        .await
        .map_err(|_| ErrorType::InvalidOperation)?
}

/// 解压并注册 Java 的阻塞实现，调用方负责把它放到阻塞线程上
/// 解压的阻塞实现，调用方负责把它放到阻塞线程上
fn extract_java_blocking(
    name: Option<String>,
    path: PathBuf,
    gui: BaseArchiveGui,
) -> CoreResult<PathBuf> {
    let name = name.unwrap_or(
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
    );

    let Some(arch) = ArchiveType::try_from_path(&path) else {
        return Err(ErrorType::InvalidOperation);
    };

    let dir = JAVA_DIR.get().unwrap().join(&name);
    archives::decompress(arch, &path, &dir, gui)?;
    Ok(dir)
}

/// 识别并注册的阻塞实现，调用方负责把它放到阻塞线程上
fn register_java_dir_blocking(dir: PathBuf) -> CoreResult<Option<Arc<JavaInfoObj>>> {
    let Some(data) = find_java_from_path(&dir) else {
        return Ok(None);
    };

    let file = if data.to_string_lossy().contains("jre")
        && let Some(path) = data
            .parent()
            .and_then(|data| data.parent())
            .map(|data| data.join("bin").join(data.file_name().unwrap()))
        && path.exists()
    {
        path
    } else {
        data
    };

    let Some(data) = java_helper::test_java(&file) else {
        return Ok(None);
    };

    Ok(Some(add_info_item(data)))
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
