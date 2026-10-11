//! `version_name_to_path` 的纯函数测试，不涉及网络与配置。
//!
//! 转换规则：
//! - `"group:artifact:version"` → `"group/artifact/version/artifact-version.jar"`
//! - `"group:artifact:version:ext"` → `"group/artifact/version/artifact-version-ext.jar"`
//! - 少于 3 段：`"name.name"` → `"name/name.jar"`

use mml_net::maven_utils::version_name_to_path;

/// 标准三段坐标。
#[test]
fn three_part_coordinate() {
    assert_eq!(
        version_name_to_path("com.example:artifact:1.0"),
        "com/example/artifact/1.0/artifact-1.0.jar"
    );
    assert_eq!(
        version_name_to_path("net.minecraftforge:forge:1.20.4"),
        "net/minecraftforge/forge/1.20.4/forge-1.20.4.jar"
    );
    assert_eq!(
        version_name_to_path("org.lwjgl:lwjgl:3.3.1"),
        "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar"
    );
}

/// 四段坐标，带 classifier
#[test]
fn four_part_coordinate() {
    assert_eq!(
        version_name_to_path("com.example:artifact:1.0:sources"),
        "com/example/artifact/1.0/artifact-1.0-sources.jar"
    );
    assert_eq!(
        version_name_to_path("org.lwjgl:lwjgl:3.3.1:natives-windows"),
        "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
    );
}

/// 少于 3 段的非坐标名称
#[test]
fn non_coordinate_name() {
    assert_eq!(version_name_to_path("plain.name"), "plain/name.jar");
    assert_eq!(version_name_to_path("solo"), "solo.jar");
    assert_eq!(version_name_to_path("a.b.c"), "a/b/c.jar");
    assert_eq!(version_name_to_path(""), ".jar");
}
