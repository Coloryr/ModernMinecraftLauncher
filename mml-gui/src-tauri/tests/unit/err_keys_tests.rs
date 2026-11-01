use std::path::PathBuf;

/// 本文里的键：逐行取 `pub const NAME: &str = "键";`
fn keys() -> Vec<String> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/err_keys.rs"))
        .expect("读不到 err_keys.rs");

    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("pub const ")?;
            let (_, after) = rest.split_once(": &str = \"")?;
            let (key, _) = after.split_once('"')?;
            Some(key.to_string())
        })
        .collect()
}

/// 每个键都要在两份语言表里存在，漏了界面上就直接显示这串 key
#[test]
fn keys_exist_in_locales() {
    let list = keys();
    assert!(list.len() > 40, "常量没解析出来，检查 keys()");

    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mml-vue/src/lib/i18n/locales");
    for file in ["zh-CN.ts", "en-US.ts"] {
        let text = std::fs::read_to_string(dir.join(file)).expect("读不到语言表");
        for key in &list {
            assert!(text.contains(&format!("\"{key}\":")), "{file} 缺少 {key}");
        }
    }
}

