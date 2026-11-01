use super::*;

/// 真的写一个 .lnk 出来：COM 这一路没有别的办法验证
#[cfg(windows)]
#[test]
fn writes_a_real_link() {
    let dir = crate::test_support::temp_dir("shortcut-link");
    let path = platform::write_link(
        &dir,
        "All the Mods 10-8.3",
        "0b5c1f5e-0000-0000-0000-000000000000",
        None,
    )
    .expect("应能写出 .lnk");
    assert!(path.is_file(), "快捷方式应已落盘：{}", path.display());
    assert!(std::fs::metadata(&path).unwrap().len() > 100);
    crate::test_support::cleanup(&[&dir]);
}

/// 实例图标（png）就地转成 .ico，第二次直接复用
#[test]
fn instance_icon_becomes_ico() {
    let dir = crate::test_support::temp_dir("shortcut-ico");
    let png = dir.join("icon.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 30, 30, 255]))
        .save(&png)
        .unwrap();

    let ico = ico_of(&png).expect("应能转出 ico");
    assert_eq!(ico, dir.join("icon.ico"));
    let bytes = std::fs::read(&ico).unwrap();
    assert_eq!(&bytes[0..6], &[0, 0, 1, 0, 1, 0], "ICO 头部不对");
    assert_eq!(&bytes[22..30], b"\x89PNG\r\n\x1a\n", "条目里应是 PNG");

    // 转好的比原图新：再调一次不重写
    assert_eq!(ico_of(&png).unwrap(), ico);
    // 没有图标就没有 ico
    assert!(ico_of(&dir.join("none.png")).is_none());

    crate::test_support::cleanup(&[&dir]);
}

/// 非法字符换成下划线，纯空白退回默认名
#[test]
fn shortcut_name_is_filesystem_safe() {
    assert_eq!(shortcut_name("All the Mods 10-8.3"), "All the Mods 10-8.3");
    assert_eq!(shortcut_name("a/b:c*d?e\"f<g>h|i"), "a_b_c_d_e_f_g_h_i");
    assert_eq!(shortcut_name("   "), "M²L");
}

