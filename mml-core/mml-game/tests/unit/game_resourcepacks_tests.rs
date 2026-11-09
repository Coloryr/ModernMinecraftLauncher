use super::{ResourcepackObj, flatten_text, is_pack_lang_file, parse_pack_meta};
use crate::test_support::{TextEntry, text_entry};
use mml_base::serialize_tools::MiniJsonObj;
use std::collections::HashMap;

/// 从 JSON 文本造一个 [`MiniJsonObj`]；`from_value` 是私有的，只能走字符串
fn mini(text: &str) -> MiniJsonObj {
    MiniJsonObj::from_str(text).unwrap()
}

/// 空语言表：只看 `fallback` / key 这条兜底链
fn no_lang() -> HashMap<String, String> {
    HashMap::new()
}

/// 造一张语言表
fn lang(list: &[TextEntry]) -> HashMap<String, String> {
    list.iter()
        .map(|e| (e.name.to_string(), e.text.to_string()))
        .collect()
}

/// 字符串 / 数组 / 标量：按形状取值，**不许**漏出 `"null"` 这种字面量
#[test]
fn flatten_plain_shapes() {
    assert_eq!(flatten_text(&mini(r#""一句话""#), &no_lang()), "一句话");
    assert_eq!(flatten_text(&mini(r#"["A","B"]"#), &no_lang()), "AB");
    assert_eq!(flatten_text(&mini("null"), &no_lang()), "");
    assert_eq!(flatten_text(&mini("1"), &no_lang()), "");
    assert_eq!(flatten_text(&mini("true"), &no_lang()), "");
}

/// 对象：`text` 优先，数组按顺序拼接
#[test]
fn flatten_text_field() {
    assert_eq!(
        flatten_text(&mini(r#"{"text":"一句话"}"#), &no_lang()),
        "一句话"
    );
    assert_eq!(
        flatten_text(&mini(r#"[{"text":"A"},{"text":"B"}]"#), &no_lang()),
        "AB"
    );
}

/// `translate`：语言表命中就用译文，**压过** `fallback`；没命中才退回兜底链
#[test]
fn flatten_translate_resolves_from_lang() {
    let table = lang(&[text_entry("some.key", "译文")]);

    // 命中：译文优先于 `fallback`
    assert_eq!(
        flatten_text(
            &mini(r#"{"translate":"some.key","fallback":"兜底文字"}"#),
            &table
        ),
        "译文"
    );
    // 表里没这个 key：用 `fallback`
    assert_eq!(
        flatten_text(
            &mini(r#"{"translate":"other.key","fallback":"兜底文字"}"#),
            &table
        ),
        "兜底文字"
    );
    // 连 `fallback` 都没有：把 key 原样给出来
    assert_eq!(
        flatten_text(&mini(r#"{"translate":"other.key"}"#), &table),
        "other.key"
    );
}

/// 只写了 `fallback`，没有 `text` / `translate`，也要认
#[test]
fn flatten_bare_fallback() {
    assert_eq!(
        flatten_text(&mini(r#"{"fallback":"兜底文字"}"#), &no_lang()),
        "兜底文字"
    );
}

/// `extra` 追加在主文字之后，可以嵌多层；里面的 `translate` 查同一张表
#[test]
fn flatten_extra_is_appended() {
    assert_eq!(
        flatten_text(&mini(r#"{"text":"A","extra":[{"text":"B"}]}"#), &no_lang()),
        "AB"
    );
    assert_eq!(
        flatten_text(
            &mini(r#"{"text":"A","extra":[{"text":"B","extra":[{"text":"C"}]}]}"#),
            &no_lang()
        ),
        "ABC"
    );
    assert_eq!(
        flatten_text(
            &mini(r#"{"text":"A","extra":[{"translate":"k"}]}"#),
            &lang(&[text_entry("k", "译文")])
        ),
        "A译文"
    );
}

/// 只有客户端渲染得出来的组件：不给文字，但它们的 `extra` 照样处理
#[test]
fn flatten_client_only_components_keep_extra() {
    assert_eq!(
        flatten_text(&mini(r#"{"keybind":"key.jump"}"#), &no_lang()),
        ""
    );
    assert_eq!(
        flatten_text(
            &mini(r#"{"score":{"name":"a","objective":"b"}}"#),
            &no_lang()
        ),
        ""
    );
    assert_eq!(
        flatten_text(
            &mini(r#"{"keybind":"key.jump","extra":[{"text":"X"}]}"#),
            &no_lang()
        ),
        "X"
    );
}

/// 走一遍 `pack.mcmeta` 的解析：简介与版本区间的各种写法都要收得下
#[test]
fn parse_pack_meta_shapes() {
    /// 从 JSON 文本造一个资源包信息，只填 pack.mcmeta 相关字段
    ///
    /// - `meta`: `pack.mcmeta` 的 JSON 文本
    ///
    /// # 返回值
    ///
    /// 返回解析出的资源包信息，其余字段取缺省值
    fn parse(meta: &str) -> ResourcepackObj {
        parse_pack_meta(mini(meta), &no_lang()).expect("pack 段是对象就应解析成功")
    }

    // 简介：字符串 / 数组 / 对象
    assert_eq!(
        parse(r#"{"pack":{"description":"一句话"}}"#).description,
        "一句话"
    );
    // 数组形式是当初"整包读取失败"的元凶：必须解析得出来
    assert_eq!(
        parse(r#"{"pack":{"description":[{"text":"一句话"},{"text":"！"}]}}"#).description,
        "一句话！"
    );
    assert_eq!(
        parse(r#"{"pack":{"description":{"translate":"k","fallback":"兜底文字"}}}"#).description,
        "兜底文字"
    );
    // 简介缺失 / 给 null / 给数字：兜空串，**不算读取失败**
    assert_eq!(parse(r#"{"pack":{}}"#).description, "");
    assert_eq!(parse(r#"{"pack":{"description":null}}"#).description, "");
    assert_eq!(parse(r#"{"pack":{"description":123}}"#).description, "");

    // `supported_formats` 的三种写法：对象 / 数组 / 单个数字
    let obj = parse(r#"{"pack":{"supported_formats":{"min_inclusive":5,"max_inclusive":75}}}"#);
    assert_eq!((obj.min_format, obj.max_format), (5, 75));
    let obj = parse(r#"{"pack":{"supported_formats":[8,9999]}}"#);
    assert_eq!((obj.min_format, obj.max_format), (8, 9999));
    let obj = parse(r#"{"pack":{"supported_formats":20}}"#);
    assert_eq!((obj.min_format, obj.max_format), (20, 0));

    // 显式给的 `min_format` / `max_format` 优先于 `supported_formats`
    let obj = parse(r#"{"pack":{"min_format":11,"max_format":22,"supported_formats":[5,75]}}"#);
    assert_eq!((obj.min_format, obj.max_format), (11, 22));

    // 版本字段写成区间数组，Mod Menu Helper 的真实写法：min 取小、max 取大
    let obj = parse(r#"{"pack":{"pack_format":18,"min_format":[97,1],"max_format":[97,1]}}"#);
    assert_eq!(obj.pack_format, 18);
    assert_eq!((obj.min_format, obj.max_format), (1, 97));

    // `pack` 段根本不存在 / 不是对象 —— 这才算读不出来
    assert!(parse_pack_meta(mini(r#"{"pack":"x"}"#), &no_lang()).is_none());
    assert!(parse_pack_meta(mini("{}"), &no_lang()).is_none());
}

/// 语言表条目名的判定：正好四段、命名空间非空、只认 `.json`
#[test]
fn pack_lang_file_shape() {
    assert!(is_pack_lang_file("assets/fo/lang/zh_cn.json", "zh_cn"));
    assert!(is_pack_lang_file(
        "assets/minecraft/lang/en_us.json",
        "en_us"
    ));

    // 语言代码不同、层数不对、后缀不对 —— 都不算
    assert!(!is_pack_lang_file("assets/fo/lang/zh_cn.json", "en_us"));
    assert!(!is_pack_lang_file("assets/fo/lang/zh_cn.json.bak", "zh_cn"));
    assert!(!is_pack_lang_file("assets/fo/lang/sub/zh_cn.json", "zh_cn"));
    assert!(!is_pack_lang_file("assets/lang/zh_cn.json", "zh_cn"));
    assert!(!is_pack_lang_file("fo/lang/zh_cn.json", "zh_cn"));
    assert!(!is_pack_lang_file("assets/fo/zh_cn.json", "zh_cn"));
    // zip 里的目录条目也不是语言表，真实包里就有 `assets/fo/lang/` 这么一条
    assert!(!is_pack_lang_file("assets/fo/lang/", "zh_cn"));
}
