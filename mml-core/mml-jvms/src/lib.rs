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
    {
        let mut list = JVMS.write().unwrap();
        list.clear();
    }

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
            let name = name.unwrap_or(info.name.clone());

            // 先移除同名旧条目
            remove(&name);

            let info = Arc::new(info);

            {
                let mut list = JVMS.write().unwrap();
                list.insert(name.clone(), info.clone());
            }

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
    let name = info.name.clone();

    // 先移除同名旧条目
    remove(&name);

    let info = Arc::new(info);

    {
        let mut list = JVMS.write().unwrap();
        list.insert(name.clone(), info.clone());
    }

    invoke_jvm_change();

    {
        let mut config = mml_config::write_config();
        let javas = &mut config.java_list;
        javas.push(JvmConfigObj {
            name: name.clone(),
            local: info.path.to_string_lossy().to_string(),
        });
    }
    mml_config::save();

    info.clone()
}

/// 从配置列表批量测试并添加 Java
///
/// - `list`: 配置文件中保存的 Java 列表
fn add_list(list: &Vec<JvmConfigObj>) {
    let dir = mml_base::get_base_dir();
    let list_cloned = list.clone();

    {
        let mut list1 = JVMS.write().unwrap();
        list1.clear();
    }

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
    let name = name.unwrap_or(
        path.as_ref()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string(),
    );

    let archive = ArchiveType::try_from_path(&path);
    let Some(arch) = archive else {
        return Err(ErrorType::InvalidOperation);
    };

    let dir = JAVA_DIR.get().unwrap().join(&name);

    archives::decompress(arch, path.as_ref(), &dir, gui)?;

    let Some(data) = find_java_from_path(dir) else {
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

    let Some(data) = java_helper::test_java(file) else {
        return Ok(None);
    };

    Ok(Some(add_info_item(data)))
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, Once, OnceLock};

    use mml_config::config_obj::JvmConfigObj;
    use mml_sys::ArchEnum;

    use super::*;

    /// 涉及全局 Java 列表 / 全局配置的用例串行化，避免相互干扰
    static JVMS_LOCK: Mutex<()> = Mutex::new(());

    /// 测试运行根目录与一次性初始化
    static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();
    static INIT: Once = Once::new();

    /// 初始化配置系统与后台保存线程等全局依赖，返回运行根目录
    fn ensure_env() -> PathBuf {
        INIT.call_once(|| {
            let dir =
                mml_testutil::temp_dir().join(format!("mml-jvms-unit-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();

            mml_base::init(&dir);
            mml_config::init(&dir).unwrap();
            // remove/remove_all 会调用 save()，需要后台保存线程已启动
            mml_config::config_save::start();

            RUN_DIR.set(dir).unwrap();
        });

        RUN_DIR.get().unwrap().clone()
    }

    /// 构造测试用 Java 信息
    fn java_info(name: &str, major: i32, arch: ArchEnum) -> JavaInfoObj {
        JavaInfoObj {
            name: name.to_string(),
            path: PathBuf::from(format!("/fake/{name}/bin/java")),
            version: format!("{major}.0.0"),
            major_version: major,
            java_type: "OpenJDK".to_string(),
            arch,
        }
    }

    /// 向全局列表插入测试条目，返回清理函数，避免污染其他用例
    fn insert(entries: Vec<JavaInfoObj>) -> impl FnOnce() {
        {
            let mut list = JVMS.write().unwrap();
            for info in entries {
                list.insert(info.name.clone(), Arc::new(info));
            }
        }
        || {
            let mut list = JVMS.write().unwrap();
            // 移除所有 ut- 前缀的测试条目
            list.retain(|name, _| !name.starts_with("ut-"));
        }
    }

    /// 与当前系统架构不同的架构，用于架构过滤测试
    fn other_arch() -> ArchEnum {
        if mml_sys::get_system_info().system_arch == ArchEnum::X86_64 {
            ArchEnum::AArch64
        } else {
            ArchEnum::X86_64
        }
    }

    /// get_java：精确匹配与更高版本匹配
    #[test]
    fn get_java_exact_and_over() {
        ensure_env();
        let _guard = JVMS_LOCK.lock().unwrap();

        let sys_arch = mml_sys::get_system_info().system_arch;
        let cleanup = insert(vec![
            java_info("ut-j17", 17, sys_arch),
            java_info("ut-j21", 21, sys_arch),
        ]);

        // 精确匹配
        let found = get_java(17, false).expect("应找到 major=17 的 Java");
        assert_eq!(found.major_version, 17);

        // over=true 时取更高版本，降序取最新
        let found = get_java(17, true).expect("应找到 major>=17 的 Java");
        assert_eq!(found.major_version, 21);

        // 精确匹配不存在的版本
        assert!(get_java(99, false).is_none());

        cleanup();
    }

    /// get_java：过滤与当前系统架构不一致的 Java
    #[test]
    fn get_java_filters_arch() {
        ensure_env();
        let _guard = JVMS_LOCK.lock().unwrap();

        let sys_arch = mml_sys::get_system_info().system_arch;
        let cleanup = insert(vec![
            java_info("ut-j21", 21, sys_arch),
            java_info("ut-j30-bad-arch", 30, other_arch()),
        ]);

        // 架构不一致的高版本应被过滤掉
        let found = get_java(17, true).expect("应找到架构匹配的 Java");
        assert_eq!(found.major_version, 21);
        assert!(get_java(30, true).is_none(), "架构不匹配的 Java 不应返回");

        cleanup();
    }

    /// get_java_info / get_all_java 的读取行为
    #[test]
    fn get_java_info_and_all() {
        let _guard = JVMS_LOCK.lock().unwrap();

        let cleanup = insert(vec![java_info("ut-info", 8, ArchEnum::X86_64)]);

        let info = get_java_info("ut-info").expect("应能按名称取到 Java 信息");
        assert_eq!(info.major_version, 8);
        assert_eq!(info.java_type, "OpenJDK");

        // 不存在的名称返回 None
        assert!(get_java_info("no-such-java").is_none());

        // 全量列表包含插入条目
        assert!(get_all_java().iter().any(|item| item.name == "ut-info"));

        cleanup();
        // 清理后取不到
        assert!(get_java_info("ut-info").is_none());
    }

    /// remove：同时从内存列表和配置文件移除
    #[test]
    fn remove_deletes_from_memory_and_config() {
        ensure_env();
        let _guard = JVMS_LOCK.lock().unwrap();

        let cleanup = insert(vec![java_info("ut-remove", 11, ArchEnum::X86_64)]);

        // 预置配置条目
        {
            let mut config = mml_config::write_config();
            config.java_list.push(JvmConfigObj {
                name: "ut-remove".to_string(),
                local: "java/fake".to_string(),
            });
        }

        remove("ut-remove");

        // 内存中已移除
        assert!(get_java_info("ut-remove").is_none());
        // 配置中已移除
        let config = mml_config::read_config();
        assert!(
            !config.java_list.iter().any(|item| item.name == "ut-remove"),
            "配置中的 Java 条目应被移除"
        );

        // 移除不存在的名称不应 panic
        drop(config);
        remove("no-such-java");

        cleanup();
    }

    /// remove_all：清空内存列表与配置
    #[test]
    fn remove_all_clears_everything() {
        ensure_env();
        let _guard = JVMS_LOCK.lock().unwrap();

        let cleanup = insert(vec![
            java_info("ut-all-1", 8, ArchEnum::X86_64),
            java_info("ut-all-2", 17, ArchEnum::X86_64),
        ]);

        {
            let mut config = mml_config::write_config();
            config.java_list.push(JvmConfigObj {
                name: "ut-all-1".to_string(),
                local: "java/fake".to_string(),
            });
        }

        remove_all();

        assert!(get_all_java().is_empty(), "remove_all 应清空内存列表");
        let config = mml_config::read_config();
        assert!(config.java_list.is_empty(), "remove_all 应清空配置列表");

        cleanup();
    }

    /// 变更事件：注册 / 触发 / 注销
    #[test]
    fn jvm_change_event_subscribe_and_unsubscribe() {
        let counter = Arc::new(AtomicU64::new(0));

        let counter_clone = counter.clone();
        let id = add_jvm_change(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        invoke_jvm_change();
        assert_eq!(counter.load(Ordering::SeqCst), 1, "触发后应回调一次");

        invoke_jvm_change();
        assert_eq!(counter.load(Ordering::SeqCst), 2, "再次触发应再次回调");

        // 注销后不再回调
        remove_jvm_change(id);
        invoke_jvm_change();
        assert_eq!(counter.load(Ordering::SeqCst), 2, "注销后不应再回调");
    }

    /// scan_java：在未安装 Java 的环境下也应安全返回，有 Java 则正常入列
    #[test]
    fn scan_java_does_not_panic() {
        let _guard = JVMS_LOCK.lock().unwrap();

        // 记录扫描前已有条目，扫描后清掉新增条目，
        // 避免本机真实 JVM 残留干扰其他用例，如 get_java 的版本匹配断言
        let before: std::collections::HashSet<String> =
            JVMS.read().unwrap().keys().cloned().collect();

        scan_java();

        // 无论是否找到 Java，列表内容都是合法的
        for item in get_all_java() {
            assert!(!item.version.is_empty() || item.major_version > 0);
        }

        JVMS.write()
            .unwrap()
            .retain(|name, _| before.contains(name));
    }
}
