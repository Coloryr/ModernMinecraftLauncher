use super::*;

/// varint 编码后再解回来应与原值一致
#[test]
fn test_write_read_varint() {
    for v in [0, 1, 127, 128, 255, 754, 65535, 0x7FFFFFFF] {
        let mut buf = Vec::new();
        write_varint(&mut buf, v);
        let mut pos = 0;
        assert_eq!(read_varint_from(&buf, &mut pos), v);
        assert_eq!(pos, buf.len());
    }
}

/// § 颜色码切段：文字、颜色与样式都要对上
#[test]
fn test_chat_from_plain() {
    let chat = chat_from_plain("§aHello§r world\n§lBold");
    let segs = chat_to_segments(&chat);
    let text: String = segs.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(text, "Hello world\nBold");
    let first = &segs[0];
    assert_eq!(first.color, "#55FF55");
    assert!(segs.iter().any(|s| s.bold));
}

/// 对象形态：子段落继承父段落颜色，也能覆盖加粗
#[test]
fn test_chat_from_value_object() {
    let v: serde_json::Value =
        serde_json::from_str(r#"{"text":"A","color":"gold","extra":[{"text":"B","bold":true}]}"#)
            .unwrap();
    let chat = chat_from_value(&v);
    let segs = chat_to_segments(&chat);
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].color, "#FFAA00");
    // 子段落继承颜色，覆盖加粗
    assert_eq!(segs[1].color, "#FFAA00");
    assert!(segs[1].bold);
}

/// `extra` 里混**裸字符串**
///
/// 真实样本就是 2b2t 的 `{"text":"","extra":[{"text":"2B "},"\n",{"text":"2T "}, …]}`：
/// 以前整份交给 serde，遇到字符串元素就整体失败、`unwrap_or_default()` 成空组件 ——
/// MOTD 一个字都不显示，而同一张卡片上人数照常，看着就是"连不上但有人数"。
#[test]
fn test_chat_from_value_extra_with_plain_strings() {
    let v: serde_json::Value = serde_json::from_str(
        r#"{"text":"","extra":[{"bold":true,"color":"gray","text":"2B "},{"color":"gold","text":"Updated"},"\n",{"color":"green","text":"2b2t.org"}]}"#,
    )
    .unwrap();

    let segs = chat_to_segments(&chat_from_value(&v));
    let text: String = segs.iter().map(|s| s.text.as_str()).collect();

    assert_eq!(text, "2B Updated\n2b2t.org");
    // 颜色名照旧翻译，子段落继承父段落的样式
    assert!(segs.iter().any(|s| s.color == "#AAAAAA"));
    assert!(segs.iter().any(|s| s.bold));
}

/// 顶层写成**数组**：整段就是一串子组件，数组里的裸字符串是字面量
#[test]
fn test_chat_from_value_array() {
    let v: serde_json::Value =
        serde_json::from_str(r#"[{"text":"A","color":"red"},"B",{"text":"C"}]"#).unwrap();

    let segs = chat_to_segments(&chat_from_value(&v));
    let text: String = segs.iter().map(|s| s.text.as_str()).collect();

    assert_eq!(text, "ABC");
    assert_eq!(segs[0].color, "#FF5555");
}

/// `translate`：没有 `text` 时退到 `fallback`，连 `fallback` 都没有就把 key 原样显示
#[test]
fn test_chat_from_value_translate() {
    let flatten = |json: &str| -> String {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        chat_to_segments(&chat_from_value(&v))
            .iter()
            .map(|s| s.text.as_str())
            .collect()
    };

    assert_eq!(
        flatten(r#"{"translate":"some.key","fallback":"兜底文字"}"#),
        "兜底文字"
    );
    assert_eq!(flatten(r#"{"translate":"some.key"}"#), "some.key");
}
