use super::*;

/// 同一个实例只放行一次补全，结束后可以再来一次
#[test]
fn only_one_prepare_per_instance() {
    let id = Uuid::new_v4();
    assert!(!is_preparing(&id));
    assert_eq!(phase_of(&id), None);

    let _token = begin(id).expect("第一次登记");
    assert!(begin(id).is_none());
    assert!(is_preparing(&id));
    assert_eq!(phase_of(&id), Some("check"));

    finish(&id);
    assert!(!is_preparing(&id));
    assert_eq!(phase_of(&id), None);

    // 结束后能重新开始，比如用户手动重试
    let _token = begin(id).expect("重新登记");
    finish(&id);
}

/// 阶段只从检查推进到下载，重复推进不产生第二次刷新
#[test]
fn download_phase_advances_once() {
    let id = Uuid::new_v4();
    let _token = begin(id).expect("登记补全");

    assert!(set_download(&id));
    assert_eq!(phase_of(&id), Some("download"));
    assert!(!set_download(&id));
    assert_eq!(phase_of(&id), Some("download"));

    finish(&id);
}

/// 取消只对正在补全的实例生效
#[test]
fn cancel_only_while_preparing() {
    let id = Uuid::new_v4();
    assert!(!cancel(&id));

    let token = begin(id).expect("登记补全");
    assert!(!token.is_cancelled());
    assert!(cancel(&id));
    assert!(token.is_cancelled());

    finish(&id);
    assert!(!cancel(&id));
}
