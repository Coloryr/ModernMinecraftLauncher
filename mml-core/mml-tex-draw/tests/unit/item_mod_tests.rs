use super::*;

/// minecraft:model 节点解析：模型名去命名空间、tint 表换算
#[test]
fn item_model_parse() {
    let def: ItemDefJson = serialize_tools::json_from_str(
        r#"{"model":{"type":"minecraft:model","model":"minecraft:item/apple",
            "tints":[{"type":"minecraft:constant","value":-1}]}}"#,
    )
    .unwrap();
    let children = resolve_item_models(def.model.as_ref().unwrap()).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].model, "item/apple");
    assert_eq!(children[0].tints, vec![[255, 255, 255]]);
    assert!(children[0].transform.is_none());
}

/// select 节点：优先取 when 含 "gui" 的 case，无则 fallback
#[test]
fn item_model_select() {
    let def: ItemDefJson = serialize_tools::json_from_str(
        r#"{"model":{"type":"minecraft:select","property":"minecraft:display_context",
            "cases":[{"when":["gui","ground","fixed"],"model":{"type":"minecraft:model",
            "model":"minecraft:item/spyglass"}}],
            "fallback":{"type":"minecraft:model","model":"minecraft:item/spyglass_in_hand"}}}"#,
    )
    .unwrap();
    let children = resolve_item_models(def.model.as_ref().unwrap()).unwrap();
    assert_eq!(children[0].model, "item/spyglass");

    // 无 gui case，trim类，取 fallback
    let def: ItemDefJson = serialize_tools::json_from_str(
        r#"{"model":{"type":"minecraft:select","property":"minecraft:trim_material",
            "cases":[{"when":"minecraft:quartz","model":{"type":"minecraft:model",
            "model":"minecraft:item/leather_chestplate_quartz_trim"}}],
            "fallback":{"type":"minecraft:model","model":"minecraft:item/leather_chestplate"}}}"#,
    )
    .unwrap();
    let children = resolve_item_models(def.model.as_ref().unwrap()).unwrap();
    assert_eq!(children[0].model, "item/leather_chestplate");
}

/// composite 子模型带 transformation：矩阵解析 + special/condition 分派
#[test]
fn item_model_misc() {
    // special 跳过
    let def: ItemDefJson = serialize_tools::json_from_str(
        r#"{"model":{"type":"minecraft:special","base":"minecraft:item/chest",
            "wrapper":{"type":"minecraft:chest"}}}"#,
    )
    .unwrap();
    assert!(resolve_item_models(def.model.as_ref().unwrap()).is_none());

    // condition 取 on_false
    let def: ItemDefJson = serialize_tools::json_from_str(
        r#"{"model":{"type":"minecraft:condition","component":"minecraft:lodestone_tracker",
            "on_true":{"type":"minecraft:model","model":"minecraft:item/compass_16"},
            "on_false":{"type":"minecraft:model","model":"minecraft:item/compass"}}}"#,
    )
    .unwrap();
    let children = resolve_item_models(def.model.as_ref().unwrap()).unwrap();
    assert_eq!(children[0].model, "item/compass");

    // composite 子模型 transformation → 含平移的矩阵
    let def: ItemDefJson = serialize_tools::json_from_str(
        r#"{"model":{"type":"minecraft:composite","models":[
            {"type":"minecraft:model","model":"minecraft:block/white_bed_head"},
            {"type":"minecraft:model","model":"minecraft:block/white_bed_foot",
             "transformation":{"translation":[0.0,0.0,1.0],
             "left_rotation":[0.0,0.0,0.0,1.0],"right_rotation":[0.0,0.0,0.0,1.0],
             "scale":[1.0,1.0,1.0]}}]}}"#,
    )
    .unwrap();
    let children = resolve_item_models(def.model.as_ref().unwrap()).unwrap();
    assert_eq!(children.len(), 2);
    let m = children[1].transform.expect("foot应有transformation");
    assert!((m.w_axis.z - 1.0).abs() < 1e-5, "foot z平移1");
}

/// ARGB int 换算：负数，如 -1 白，与普通色值
#[test]
fn argb_rgb_values() {
    assert_eq!(argb_rgb(-1), [255, 255, 255]);
    assert_eq!(argb_rgb(4603950), [0x46, 0x40, 0x2E]);
}

