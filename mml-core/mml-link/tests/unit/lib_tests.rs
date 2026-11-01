use super::*;

/// 公共文件名保留真实文件名，只清掉路径片段和非法字符
#[test]
fn file_name_keeps_real_name() {
    assert_eq!(file_name("Demo.JAR").as_deref(), Some("Demo.JAR"));
    assert_eq!(file_name("dir/foo.jar").as_deref(), Some("foo.jar"));
    assert_eq!(file_name("bad<name>.jar").as_deref(), Some("bad_name_.jar"));
    assert!(file_name("").is_none());
}

/// 非 SHA1 不接受
#[test]
fn hash_must_be_sha1() {
    assert!(normalize_hash("").is_none());
    assert!(normalize_hash("abc").is_none());
    assert!(normalize_hash(&"g".repeat(40)).is_none());
    assert_eq!(
        normalize_hash(&"A".repeat(40)).as_deref(),
        Some("a".repeat(40).as_str())
    );
}

/// 映射记录落在实例目录，不落在启动器运行目录
#[test]
fn link_info_is_written_inside_instance() {
    let dir = mml_testutil::temp_path(format!("mml-link-{}", Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    mml_downloader::init(&dir).unwrap();
    init(&dir).unwrap();
    set_enabled(true);
    config_save::start();

    let hash = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
    let source = download_path(ResourceType::Mod, hash, "demo.jar").unwrap();
    assert_eq!(
        source.file_name().and_then(|name| name.to_str()),
        Some("demo.jar")
    );
    fs::write(&source, b"hello").unwrap();
    add_download_info(
        ResourceType::Mod,
        ResourceInfoObj {
            pid: "p".to_string(),
            name: "Demo".to_string(),
            hash: hash.to_string(),
            file: "demo.jar".to_string(),
            url: "https://example.com/demo.jar".to_string(),
        },
        "p".to_string(),
    );

    let instance = Uuid::new_v4();
    let base = dir.join("instance");
    let target = base.join(".minecraft").join("mods").join("demo.jar");
    assert!(
        try_link(
            ResourceType::Mod,
            instance,
            &base,
            &target,
            "p",
            "f",
            "Demo",
            hash,
            "https://example.com/demo.jar"
        )
        .unwrap()
    );
    assert!(target.is_file());

    // 第二个实例命中同一份公共文件，直接链接，不再下载
    let instance2 = Uuid::new_v4();
    let base2 = dir.join("instance2");
    let target2 = base2.join(".minecraft").join("mods").join("demo.jar");
    assert!(
        try_link(
            ResourceType::Mod,
            instance2,
            &base2,
            &target2,
            "p",
            "f",
            "Demo",
            hash,
            "https://example.com/demo.jar"
        )
        .unwrap()
    );
    assert!(target2.is_file());

    // 旧的哈希文件名要原地改成真实文件名，同时把实例里的文件搬进去
    let world_hash = "7c211433f02071597741e6ff5a8ea34789abbf43";
    let old_central = get_mod_download_path().join(format!("{world_hash}.jar"));
    fs::write(&old_central, b"world").unwrap();
    add_download_info(
        ResourceType::Mod,
        ResourceInfoObj {
            pid: "p3".to_string(),
            name: "World".to_string(),
            hash: world_hash.to_string(),
            file: format!("{world_hash}.jar"),
            url: "https://example.com/world.jar".to_string(),
        },
        "p3".to_string(),
    );
    let instance3 = Uuid::new_v4();
    let base3 = dir.join("instance3");
    let target3 = base3.join(".minecraft").join("mods").join("world.jar");
    fs::create_dir_all(target3.parent().unwrap()).unwrap();
    fs::write(&target3, b"world").unwrap();
    map_existing(
        ResourceType::Mod,
        instance3,
        &base3,
        &target3,
        "p3",
        "f3",
        "World",
        world_hash,
        "https://example.com/world.jar",
    )
    .unwrap();
    assert!(get_mod_download_path().join("world.jar").is_file());
    assert!(!old_central.exists());
    assert_eq!(fs::read(&target3).unwrap(), b"world",);

    config_save::stop();

    assert!(!dir.join(names::LINK_INFO_FILE).exists());
    let file = base.join(names::LINK_INFO_FILE);
    let data: Vec<LinkInfoObj> =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].path, ".minecraft/mods/demo.jar");
    assert!(data[0].is_link);

    // 索引落在键值库里：库目录在、且有内容；记录本身从内存表查
    let index_dir = dir.join(names::DOWNLOAD_INDEX_DIR);
    assert!(index_dir.is_dir());
    assert!(fs::read_dir(&index_dir).unwrap().next().is_some());
    assert_eq!(
        get_download_info(ResourceType::Mod, "p").map(|item| item.url),
        Some("https://example.com/demo.jar".to_string())
    );

    let _ = fs::remove_dir_all(&dir);
}

