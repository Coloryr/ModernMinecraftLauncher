/// 扫描本机 Java 并逐条打印，供人工查看
#[test]
fn get_java_list() {
    mml_jvms::scan_java();
    let list2 = mml_jvms::get_all_java();
    for item in list2 {
        println!(
            "find java {}: {} {} {} {}",
            item.major_version,
            item.name,
            item.version,
            item.arch,
            item.path.to_string_lossy()
        );
    }
}
