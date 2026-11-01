use super::java_dir_name;

/// 运行时名称要当包内目录名：非法字符换成下划线，结尾的点与空格去掉
#[test]
fn java_dir_name_cleans_path_chars() {
    assert_eq!(
        java_dir_name("OpenJDK-17.0.1-x86_64"),
        "OpenJDK-17.0.1-x86_64"
    );
    assert_eq!(java_dir_name("Java 17 (x64)"), "Java 17 (x64)");
    assert_eq!(java_dir_name("jdk21. "), "jdk21");
    assert!(super::format::safe_component(&java_dir_name("a/b:c*?")));
}

