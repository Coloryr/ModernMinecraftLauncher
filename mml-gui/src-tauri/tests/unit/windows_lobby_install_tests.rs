use super::*;
use flate2::{Compression, write::GzEncoder};
use tokio_util::sync::CancellationToken;

/// 造一个只含单文件的 tar.gz，`link` 决定那一条是不是符号链接
fn fixture(link: bool) -> Vec<u8> {
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let mut header = tar::Header::new_gnu();
    header.set_mode(0o755);
    header.set_size(if link { 0 } else { 7 });
    if link {
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_link_name("../outside").unwrap();
    }
    header.set_cksum();
    tar.append_data(
        &mut header,
        "terracotta-test.exe",
        if link { &b""[..] } else { &b"program"[..] },
    )
    .unwrap();
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
fn installer_rejects_links_and_cancelled_sessions_before_marking_verified() {
    let root = crate::test_support::temp_dir("terracotta-installer");
    let token = CancellationToken::new();
    assert!(
        unpack(
            &fixture(true),
            &root.join("link"),
            "terracotta-test.exe",
            "digest",
            &token
        )
        .is_err()
    );
    assert!(!root.join("link/verified.sha256").exists());
    token.cancel();
    assert!(
        unpack(
            &fixture(false),
            &root.join("cancelled"),
            "terracotta-test.exe",
            "digest",
            &token
        )
        .is_err()
    );
    assert!(!root.join("cancelled/verified.sha256").exists());
    unpack(
        &fixture(false),
        &root.join("valid"),
        "terracotta-test.exe",
        "digest",
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(root.join("valid/terracotta-test.exe")).unwrap(),
        b"program"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("valid/verified.sha256")).unwrap(),
        "digest"
    );
    crate::test_support::cleanup(&[&root]);
}

