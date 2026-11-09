//! 打印当前系统语言的识别结果，供人工核对
//!
//! 运行：`cargo run -p mml-names --example lang_locale`

fn main() {
    println!("locale: {}", mml_names::get_current_locale());
}
