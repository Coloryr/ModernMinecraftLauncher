//! Java 运行时全局状态：存放目录、内存列表与变更事件

use super::*;

/// Java 运行时存放目录，即 `{运行目录}/java/`
pub(crate) static JAVA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 全局 Java 运行时列表，按名称索引
pub(crate) static JVMS: LazyLock<RwLock<HashMap<String, Arc<JavaInfoObj>>>> =
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
