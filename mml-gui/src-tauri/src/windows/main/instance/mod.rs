//! 主窗口的实例与分组写操作：新建 / 改名 / 改配置 / 删除 / 移动
//!
//! 从 `main/mod.rs` 拆出来的，光 `main_update_instance` 一个函数就一百多行。命令带
//! 分组标记，值填原始组名 main，把组键钉回 `main`，见 AGENTS.md §4。

pub(crate) mod edit;
pub(crate) mod group;
pub(crate) mod shortcut;

/// 渲染器在 options.txt 里的键，对应游戏内的图形后端选择
pub(crate) const RENDERER_KEY: &str = "preferredGraphicsBackend";

/// 该版本能不能选渲染器：26.2 起游戏才提供这个选项
///
/// - `version`: 游戏版本号
///
/// # 返回值
///
/// 支持时返回 `true`；版本无法解析时返回 `false`
pub(crate) fn supports_renderer(version: &str) -> bool {
    mml_base::version_parse::is_game_version_greater_equal(version, "26.2")
}

/// 把渲染器取值收敛到游戏认的三档
///
/// - `value`: options.txt 里读到的值或前端提交的值
///
/// # 返回值
///
/// opengl / vulkan 原样返回，其余一律按 default
pub(crate) fn renderer_of(value: &str) -> &'static str {
    match value {
        "opengl" => "opengl",
        "vulkan" => "vulkan",
        _ => "default",
    }
}
