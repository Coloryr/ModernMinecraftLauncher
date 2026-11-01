//! 窗口注册表单元测试

use super::family::*;
use super::spec::*;
use super::uuids::*;

use uuid::Uuid;

/// 每个 kind 一条基准 uuid，用于断言"派生 uuid 不撞基准"
const BASE_OF_KIND: [(&str, Uuid); 4] = [
    ("log", LOG_WINDOW_UUID),
    ("resource", RESOURCE_WINDOW_UUID),
    ("add_resource", ADD_RESOURCE_WINDOW_UUID),
    ("config", CONFIG_WINDOW_UUID),
];

/// 测试里直接取族，省得到处 `family_of_kind(..).unwrap()`
fn family(kind: &str) -> &'static WindowFamily {
    family_of_kind(kind).unwrap()
}

/// 窗口身份：同一实例恒定、不同实例不同，且都不是基准 uuid
#[test]
fn family_window_id_is_per_instance() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let id = family_window_id(family("log"), &first);

    assert_eq!(
        id.uuid,
        family_window_id(family("log"), &first).uuid,
        "同一实例应派生同一个窗口"
    );
    assert_ne!(
        id.uuid,
        family_window_id(family("log"), &second).uuid,
        "不同实例应是不同窗口"
    );
    assert_ne!(id.uuid, LOG_WINDOW_UUID, "派生 uuid 不能撞上基准条目");
    assert_eq!(id.label, format!("mml-log-{first}"));
}

/// 四族两两不共用 uuid：同一个实例在每族里都应是不同的窗口
#[test]
fn families_do_not_share_uuid() {
    let instance = Uuid::new_v4();
    let mut seen: Vec<(Uuid, &str)> = Vec::new();

    for (kind, base) in BASE_OF_KIND {
        let id = family_window_id(family(kind), &instance);
        assert_ne!(id.uuid, base, "{kind} 的派生 uuid 撞上了自己的基准 uuid");
        assert_eq!(id.label, format!("mml-{kind}-{instance}"));
        for (other, other_kind) in &seen {
            assert_ne!(id.uuid, *other, "{kind} 与 {other_kind} 派生出同一个窗口");
        }
        seen.push((id.uuid, kind));
    }
}

/// 每族的基准 uuid 都必须是注册表里的静态条目：族内窗口的尺寸与命名空间都从它取，
/// 写错一个字母就会静默退化成"没有最小尺寸"、而且派生出的窗口与别的族撞不上也认不出
#[test]
fn family_bases_are_registered() {
    for (kind, base) in BASE_OF_KIND {
        assert_eq!(
            family(kind).base,
            base,
            "{kind} 的族表基准 uuid 与它的常量对不上"
        );
        let size = min_size_of(&base);
        assert!(size.is_some(), "{kind} 的基准 uuid 没登记进 WINDOWS_INFO");
        assert_eq!(family_min_size(family(kind)), size.unwrap());
        assert!(size.unwrap().width > 0.0 && size.unwrap().height > 0.0);
    }
}

/// 族内窗口的尺寸跟着**基准条目的常量**走
///
/// 踩过：族表里另存了一份尺寸，改了 `ADD_RESOURCE_MIN_WIDTH` 而真实窗口纹丝不动
#[test]
fn family_size_follows_base_constants() {
    let instance = Uuid::new_v4();
    let id = family_window_id(family("add_resource"), &instance);

    assert_eq!(
        min_size_of_label(&id.label),
        Some(MinSize {
            width: ADD_RESOURCE_MIN_WIDTH,
            height: ADD_RESOURCE_MIN_HEIGHT,
        })
    );
}

/// 标签 → 实例 / uuid / 尺寸三条反查都要认得出每一族
#[test]
fn family_label_resolves_back() {
    let instance = Uuid::new_v4();

    for (kind, _) in BASE_OF_KIND {
        let family = family(kind);
        let id = family_window_id(family, &instance);
        assert_eq!(
            family_of_label(&id.label).map(|hit| (hit.family.kind, hit.instance)),
            Some((kind, instance))
        );
        assert_eq!(uuid_of_label(&id.label), Some(id.uuid));
        assert_eq!(min_size_of_label(&id.label), Some(family_min_size(family)));
    }
}

/// 静态窗口的标签解析不受窗口族影响
#[test]
fn static_labels_still_resolve() {
    assert_eq!(uuid_of_label("mml-download"), Some(DOWNLOAD_WINDOW_UUID));
    assert_eq!(
        min_size_of_label("mml-download"),
        min_size_of(&DOWNLOAD_WINDOW_UUID)
    );
    for (kind, base) in BASE_OF_KIND {
        // 基准条目本身不是窗口族的标签：后面没有实例 uuid
        let label = format!("mml-{kind}");
        assert!(family_of_label(&label).is_none());
        assert_eq!(uuid_of_label(&label), Some(base));
        assert_eq!(
            min_size_of_label(&label),
            Some(family_min_size(family(kind)))
        );
        // 后缀不是 uuid 的同类标签同样不认
        let bad = format!("mml-{kind}-not-a-uuid");
        assert!(family_of_label(&bad).is_none());
        assert_eq!(uuid_of_label(&bad), None);
    }
    // 不是窗口族的 kind 取不到族，不会误把静态窗口当族
    assert!(family_of_kind("download").is_none());
    assert!(family_of_kind("log").is_some());
}
