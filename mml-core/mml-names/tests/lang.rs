/// 打印当前系统语言，供人工核对语言识别结果
///
/// 默认被忽略，需要时用 `--ignored` 单独跑。
#[test]
#[ignore = "skip"]
fn lang_locale() {
    let locale = mml_names::get_current_locale();

    println!("locale: {}", locale);
}
