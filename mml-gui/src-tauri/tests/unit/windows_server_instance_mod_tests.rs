use super::*;
use crate::dtos::server_instance_dto::{ServerInstanceConfigDto, ServerInstancePickDto};
use mml_game::{
    launcher::file_online_info_obj::OnlineInfoObj, loader::LoaderType,
    other_launcher::hmcl_obj::HMCLServerObj, serverpack::serverpack_obj::ServerPackObj,
};

/// 勾选一组模组路径
fn picks(paths: &[&str]) -> Vec<ServerInstancePickDto> {
    paths
        .iter()
        .map(|path| ServerInstancePickDto {
            path: path.to_string(),
            exclude: false,
            url: String::new(),
            pid: String::new(),
            fid: String::new(),
            custom: false,
            edited: false,
        })
        .collect()
}

/// 测试用的生成选项
fn options(
    format: &str,
    mods: &[&str],
    configs: &[(&str, &str)],
    packs: &[&str],
) -> ServerInstanceOptionsDto {
    ServerInstanceOptionsDto {
        format: format.into(),
        folder: String::new(),
        url: "https://example.invalid/pack".into(),
        name: "服务器".into(),
        version: "1.0.0".into(),
        text: "说明".into(),
        mods: picks(mods),
        configs: configs
            .iter()
            .map(|(dir, mode)| ServerInstanceConfigDto {
                dir: dir.to_string(),
                mode: mode.to_string(),
                delete_old: true,
            })
            .collect(),
        base_files: Vec::new(),
        resource_packs: packs
            .iter()
            .map(|path| ServerInstancePickDto {
                path: path.to_string(),
                exclude: false,
                url: String::new(),
                pid: String::new(),
                fid: String::new(),
                custom: true,
                edited: false,
            })
            .collect(),
    }
}

/// 按选项展开勾选，等价于 generate 里的那一步
fn contents(base: &Path, options: &ServerInstanceOptionsDto) -> Contents {
    Contents {
        mods: files::mods(base, &options.mods).unwrap(),
        configs: options
            .configs
            .iter()
            .map(|item| ConfigDir {
                dir: item.dir.clone(),
                mode: ConfigMode::parse(&item.mode).unwrap(),
                delete_old: item.delete_old,
            })
            .collect(),
        packs: options
            .resource_packs
            .iter()
            .map(|pick| PackPick {
                path: files::file(base, &pick.path).unwrap(),
                url: pick.url.clone(),
                pid: pick.pid.clone(),
                fid: pick.fid.clone(),
                custom: pick.custom,
                edited: pick.edited,
            })
            .collect(),
    }
}

/// 一个原版 Forge 实例
fn game() -> InstanceSettingObj {
    InstanceSettingObj {
        name: "服务器".into(),
        version: "1.21.1".into(),
        loader: LoaderType::Forge,
        loader_version: Some("52.0.1".into()),
        ..Default::default()
    }
}

/// 游戏的 SHA1，用来核对清单里写出来的值
fn sha1(bytes: &[u8]) -> String {
    mml_base::hash_helper::gen_hash(HashType::Sha1, bytes)
}

/// 一个模组的在线来源
fn online(file: &str, pid: &str, fid: &str) -> OnlineInfoObj {
    OnlineInfoObj {
        file: file.into(),
        modid: pid.into(),
        fileid: fid.into(),
        url: format!("https://example.invalid/{file}"),
        ..Default::default()
    }
}

#[test]
fn config_dir_allows_only_one_level() {
    assert_eq!(config_dir("config").unwrap(), "config");
    assert_eq!(config_dir("config\\setting").unwrap(), "config/setting");
    for bad in [
        "",
        "config/",
        "/config",
        "config//setting",
        "config/setting/deep",
        ".",
        "..",
        "config/..",
    ] {
        assert!(config_dir(bad).is_err(), "{bad}");
    }
}

#[test]
fn normalize_url_requires_http_and_drops_trailing_slash() {
    assert_eq!(
        normalize_url(" https://example.invalid/pack/ ").unwrap(),
        "https://example.invalid/pack"
    );
    assert_eq!(
        normalize_url("http://127.0.0.1:8080").unwrap(),
        "http://127.0.0.1:8080"
    );
    for bad in [
        "",
        "  ",
        "example.invalid",
        "ftp://example.invalid",
        "https://",
    ] {
        assert!(normalize_url(bad).is_err(), "{bad}");
    }
}

#[test]
fn prepare_folder_rejects_instance_directory() {
    let instance = crate::test_support::temp_dir("server-instance-prepare");
    let inside = instance.join("out");
    assert_eq!(
        prepare_folder(&instance, inside.to_str().unwrap()).unwrap_err(),
        err_keys::SERVER_INSTANCE_FOLDER
    );

    let outside = crate::test_support::temp_dir("server-instance-prepare-out");
    assert_eq!(
        prepare_folder(&instance, outside.to_str().unwrap()).unwrap(),
        outside.canonicalize().unwrap()
    );
    assert!(prepare_folder(&instance, "  ").is_err());

    crate::test_support::cleanup(&[&instance, &outside]);
}

/// 模组可以整个目录勾，也能单独取消其中一个
#[test]
fn mods_follow_selection_and_exclusions() {
    let base = crate::test_support::temp_dir("server-instance-mods");
    fs::create_dir_all(base.join("mods/sub")).unwrap();
    fs::write(base.join("mods/a.jar"), b"a").unwrap();
    fs::write(base.join("mods/sub/b.jar"), b"b").unwrap();

    let paths = |list: Vec<files::ModPick>| -> Vec<PathBuf> {
        list.into_iter().map(|item| item.path).collect()
    };

    let list = files::mods(&base, &picks(&["mods"])).unwrap();
    assert_eq!(
        paths(list),
        vec![PathBuf::from("mods/a.jar"), PathBuf::from("mods/sub/b.jar")]
    );

    let mut exclude = picks(&["mods"]);
    exclude.push(ServerInstancePickDto {
        path: "mods/sub/b.jar".into(),
        exclude: true,
        url: String::new(),
        pid: String::new(),
        fid: String::new(),
        custom: false,
        edited: false,
    });
    assert_eq!(
        paths(files::mods(&base, &exclude).unwrap()),
        vec![PathBuf::from("mods/a.jar")]
    );

    assert!(files::mods(&base, &[]).unwrap().is_empty());

    crate::test_support::cleanup(&[&base]);
}

/// 目录同步方式：压缩包与逐文件各写各的
#[test]
fn mml_pack_writes_manifest_sidecar_and_archive() {
    // 打压缩包会打日志，日志系统每进程只 boot 一次
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-mml");
    let out = crate::test_support::temp_dir("server-instance-mml-out");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::create_dir_all(base.join("config")).unwrap();
    fs::write(base.join("mods/online.jar"), b"online").unwrap();
    fs::write(base.join("mods/local.jar"), b"local").unwrap();
    fs::write(base.join("config/a.toml"), b"a = 1").unwrap();

    let online =
        HashMap::from([("online.jar".to_string(), online("online.jar", "123", "456"))]);
    let options = options("mml", &["mods"], &[("config", "archive")], &[]);
    let contents = contents(&base, &options);
    let result = generate_mml(
        &game(),
        &online,
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let manifest = fs::read(out.join("server.json")).unwrap();
    let pack: ServerPackObj = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(pack.online_list.len(), 2);
    let online_jar = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/online.jar")
        .unwrap();
    assert_eq!(online_jar.pid.as_deref(), Some("123"));
    assert_eq!(online_jar.sha1.as_deref(), Some(sha1(b"online").as_str()));
    // 有在线来源的不复制进文件夹
    assert!(!out.join("mods/online.jar").exists());

    // 没有在线来源的由我们托管：复制进文件夹并给托管地址
    let local = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/local.jar")
        .unwrap();
    assert!(local.custom);
    assert!(local.pid.as_deref().unwrap_or_default().starts_with("custom-p-"));
    assert!(local.fid.as_deref().unwrap_or_default().starts_with("custom-f-"));
    assert_eq!(
        local.url.as_deref(),
        Some("https://example.invalid/pack/mods/local.jar")
    );
    assert_eq!(fs::read(out.join("mods/local.jar")).unwrap(), b"local");
    assert_eq!(pack.archive_list.len(), 1);
    assert_eq!(pack.archive_list[0].file, "config.zip");
    assert_eq!(pack.archive_list[0].dir, "config");
    assert!(pack.archive_list[0].delete_old);
    assert_eq!(
        pack.archive_list[0].url,
        "https://example.invalid/pack/config.zip"
    );
    assert!(out.join("config.zip").is_file());

    // 客户端逐字比对 SHA1，末尾多一个换行就永远对不上
    let sidecar = fs::read_to_string(out.join("server.json.sha1")).unwrap();
    assert_eq!(sidecar, sha1(&manifest));
    assert!(!sidecar.ends_with('\n'));

    assert_eq!(result.manifest, "server.json");
    assert_eq!(result.files, ["config.zip", "mods"]);
    // 三个文件都同步：online.jar、local.jar 与 config/a.toml
    assert_eq!(result.count, 3);

    crate::test_support::cleanup(&[&base, &out]);
}

/// 两个目录各用各的方式：config 打压缩包，kubejs 逐文件
#[test]
fn mml_mixes_archive_and_per_file_dirs() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-mixed");
    let out = crate::test_support::temp_dir("server-instance-mixed-out");
    fs::create_dir_all(base.join("config")).unwrap();
    fs::create_dir_all(base.join("kubejs")).unwrap();
    fs::write(base.join("config/a.toml"), b"a").unwrap();
    fs::write(base.join("kubejs/startup.js"), b"js").unwrap();

    let options = options(
        "mml",
        &[],
        &[("config", "archive"), ("kubejs", "files")],
        &[],
    );
    let contents = contents(&base, &options);
    let result = generate_mml(
        &game(),
        &HashMap::new(),
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    assert_eq!(pack.archive_list.len(), 1);
    assert_eq!(pack.archive_list[0].file, "config.zip");
    // 逐文件目录进清单，文件也复制进输出文件夹
    assert_eq!(pack.online_list.len(), 1);
    assert_eq!(pack.online_list[0].file, "kubejs/startup.js");
    assert_eq!(
        pack.online_list[0].url.as_deref(),
        Some("https://example.invalid/pack/kubejs/startup.js")
    );
    assert_eq!(fs::read(out.join("kubejs/startup.js")).unwrap(), b"js");
    assert_eq!(result.files, ["config.zip", "kubejs"]);
    assert_eq!(result.count, 2);

    crate::test_support::cleanup(&[&base, &out]);
}

/// 手填的下载地址优先于实例记录；既没地址又没编号的只能本地托管
#[test]
fn mods_take_manual_url_over_recorded() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-manual");
    let out = crate::test_support::temp_dir("server-instance-manual-out");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::write(base.join("mods/a.jar"), b"a").unwrap();
    fs::write(base.join("mods/b.jar"), b"b").unwrap();

    let mut manual = picks(&["mods/a.jar", "mods/b.jar"]);
    manual[0].url = " https://typed.example/a.jar ".into();

    let online = HashMap::from([("a.jar".to_string(), online("a.jar", "1", "2"))]);
    let options = options("mml", &[], &[], &[]);
    let contents = Contents {
        mods: files::mods(&base, &manual).unwrap(),
        configs: Vec::new(),
        packs: Vec::new(),
    };
    generate_mml(
        &game(),
        &online,
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    assert_eq!(pack.online_list.len(), 2);

    // a：手填地址盖掉实例记录，编号仍是记录里的，文件不复制
    let a = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/a.jar")
        .unwrap();
    assert_eq!(a.url.as_deref(), Some("https://typed.example/a.jar"));
    assert_eq!(a.pid.as_deref(), Some("1"));
    assert_eq!(a.fid.as_deref(), Some("2"));
    assert!(!out.join("mods/a.jar").exists());

    // b：没有地址也没有编号，复制进文件夹并给托管地址
    let b = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/b.jar")
        .unwrap();
    assert_eq!(
        b.url.as_deref(),
        Some("https://example.invalid/pack/mods/b.jar")
    );
    assert_eq!(fs::read(out.join("mods/b.jar")).unwrap(), b"b");

    crate::test_support::cleanup(&[&base, &out]);
}

/// 资源包有在线来源时走平台下载，不复制进文件夹
#[test]
fn mml_resource_pack_with_source_is_not_copied() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-pack-online");
    let out = crate::test_support::temp_dir("server-instance-pack-online-out");
    fs::create_dir_all(base.join("resourcepacks")).unwrap();
    fs::write(base.join("resourcepacks/p.zip"), b"p").unwrap();

    let online = HashMap::from([(
        "p.zip".to_string(),
        OnlineInfoObj {
            file: "p.zip".into(),
            modid: "111".into(),
            fileid: "222".into(),
            url: "https://cdn.example.invalid/p.zip".into(),
            ..Default::default()
        },
    )]);
    let options = options("mml", &[], &[], &["resourcepacks/p.zip"]);
    let contents = contents(&base, &options);
    let result = generate_mml(
        &game(),
        &online,
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    assert_eq!(pack.online_list.len(), 1);
    assert_eq!(
        pack.online_list[0].url.as_deref(),
        Some("https://cdn.example.invalid/p.zip")
    );
    assert_eq!(pack.online_list[0].pid.as_deref(), Some("111"));
    // 用了在线地址就不复制
    assert!(!out.join("resourcepacks/p.zip").exists());
    assert!(result.files.is_empty());
    assert_eq!(result.count, 1);

    crate::test_support::cleanup(&[&base, &out]);
}

/// 资源包没有在线来源时逐个下发，路径要转义
#[test]
fn mml_resource_packs_are_hosted_per_file() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-packs");
    let out = crate::test_support::temp_dir("server-instance-packs-out");
    fs::create_dir_all(base.join("resourcepacks")).unwrap();
    fs::write(base.join("resourcepacks/中文 1.zip"), b"pack").unwrap();

    let options = options("mml", &[], &[], &["resourcepacks/中文 1.zip"]);
    let contents = contents(&base, &options);
    let result = generate_mml(
        &game(),
        &HashMap::new(),
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    assert_eq!(pack.online_list.len(), 1);
    assert_eq!(
        pack.online_list[0].url.as_deref(),
        Some("https://example.invalid/pack/resourcepacks/%E4%B8%AD%E6%96%87%201.zip")
    );
    assert_eq!(
        pack.online_list[0].sha1.as_deref(),
        Some(sha1(b"pack").as_str())
    );
    assert!(pack.archive_list.is_empty());
    assert_eq!(
        fs::read(out.join("resourcepacks/中文 1.zip")).unwrap(),
        b"pack"
    );
    assert_eq!(result.files, ["resourcepacks"]);
    assert_eq!(result.count, 1);

    crate::test_support::cleanup(&[&base, &out]);
}

#[test]
fn hmcl_pack_copies_files_and_lists_hashes() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-hmcl");
    let out = crate::test_support::temp_dir("server-instance-hmcl-out");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::create_dir_all(base.join("config")).unwrap();
    fs::create_dir_all(base.join("resourcepacks")).unwrap();
    fs::write(base.join("mods/a.jar"), b"mod").unwrap();
    fs::write(base.join("config/x.toml"), b"x = 1").unwrap();
    fs::write(base.join("resourcepacks/p.zip"), b"p").unwrap();

    let options = options(
        "hmcl",
        &["mods"],
        &[("config", "archive")],
        &["resourcepacks/p.zip"],
    );
    let contents = contents(&base, &options);
    let result = generate_hmcl(
        &game(),
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    assert_eq!(fs::read(out.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(fs::read(out.join("config/x.toml")).unwrap(), b"x = 1");
    assert_eq!(fs::read(out.join("resourcepacks/p.zip")).unwrap(), b"p");

    let manifest = fs::read(out.join("server-manifest.json")).unwrap();
    let pack: HMCLServerObj = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(pack.name, "服务器");
    assert_eq!(pack.file_api, "https://example.invalid/pack");
    assert_eq!(pack.addons.len(), 2);
    assert_eq!(pack.files.len(), 3);
    let paths: Vec<&str> = pack.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        ["config/x.toml", "mods/a.jar", "resourcepacks/p.zip"]
    );
    assert_eq!(pack.files[1].hash, sha1(b"mod"));

    assert_eq!(result.manifest, "server-manifest.json");
    assert_eq!(result.files, ["config", "mods", "resourcepacks"]);
    assert_eq!(result.count, 3);

    crate::test_support::cleanup(&[&base, &out]);
}

#[test]
fn hmcl_pack_rejects_loader_it_cannot_express() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-hmcl-loader");
    let out = crate::test_support::temp_dir("server-instance-hmcl-loader-out");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::write(base.join("mods/a.jar"), b"mod").unwrap();

    let mut item = game();
    item.loader = LoaderType::OptiFine;
    let options = options("hmcl", &["mods"], &[], &[]);
    let contents = contents(&base, &options);
    let err = generate_hmcl(
        &item,
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap_err();
    assert_eq!(err, err_keys::SERVER_INSTANCE_LOADER);

    crate::test_support::cleanup(&[&base, &out]);
}


/// 压缩包同步可以关掉「删除原有目录」，清单里的 Over 随之为 false
#[test]
fn mml_archive_delete_old_follows_option() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-config-keep");
    let out = crate::test_support::temp_dir("server-instance-config-keep-out");
    fs::create_dir_all(base.join("config")).unwrap();
    fs::write(base.join("config/a.toml"), b"a").unwrap();

    let mut options = options("mml", &[], &[("config", "archive")], &[]);
    options.configs[0].delete_old = false;
    let contents = contents(&base, &options);
    generate_mml(
        &game(),
        &HashMap::new(),
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    assert_eq!(pack.archive_list.len(), 1);
    assert!(
        !pack.archive_list[0].delete_old,
        "关掉删除后 Over 应为 false"
    );

    crate::test_support::cleanup(&[&base, &out]);
}


/// 选中一级子目录后，父目录的包排除子目录，子目录单独成包并排在父包后面
#[test]
fn mml_splits_selected_subdirectory_into_own_archive() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-nested");
    let out = crate::test_support::temp_dir("server-instance-nested-out");
    fs::create_dir_all(base.join("config").join("setting")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();
    fs::write(base.join("config").join("setting").join("b.toml"), b"b").unwrap();

    let options = options(
        "mml",
        &[],
        &[("config", "archive"), ("config/setting", "archive")],
        &[],
    );
    let contents = contents(&base, &options);
    let result = generate_mml(
        &game(),
        &HashMap::new(),
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    assert_eq!(pack.archive_list.len(), 2);
    assert_eq!(pack.archive_list[0].file, "config.zip");
    assert_eq!(pack.archive_list[0].dir, "config");
    assert_eq!(pack.archive_list[1].file, "config.setting.zip");
    assert_eq!(pack.archive_list[1].dir, "config/setting");

    let parent =
        mml_base::archives::BaseArchive::open_readonly(out.join("config.zip")).unwrap();
    let names: Vec<String> = parent
        .entries()
        .iter()
        .map(|item| item.name.clone())
        .collect();
    assert!(names.iter().any(|name| name == "a.toml"), "{names:?}");
    assert!(
        !names.iter().any(|name| name == "setting/b.toml"),
        "父包不该包含子目录文件: {names:?}"
    );

    let child =
        mml_base::archives::BaseArchive::open_readonly(out.join("config.setting.zip")).unwrap();
    assert!(
        child.entries().iter().any(|item| item.name == "b.toml"),
        "子目录包应包含自己的文件"
    );
    assert_eq!(result.count, 2);

    crate::test_support::cleanup(&[&base, &out]);
}


/// 实例根目录下的文件打成固定 base.zip，清单里写 Base
#[test]
fn mml_packs_base_files_outside_minecraft() {
    crate::test_support::ensure_boot();
    let dir = format!("server-instance-base-{}", uuid::Uuid::new_v4());
    // 实例目录在 run/minecraft/instances 下，get_base_path 按实例根目录拼
    let instance = mml_base::get_base_dir()
        .join("minecraft")
        .join("instances")
        .join(&dir);
    let game_dir = instance.join(".minecraft");
    fs::create_dir_all(&game_dir).unwrap();
    fs::create_dir_all(instance.join("libraries")).unwrap();
    fs::create_dir_all(instance.join("patches")).unwrap();
    fs::write(instance.join("server.properties"), b"motd=test").unwrap();
    fs::write(instance.join("icon.png"), b"icon").unwrap();
    fs::write(game_dir.join("options.txt"), b"lang:zh_cn").unwrap();
    fs::write(instance.join("libraries").join("foo.jar"), b"lib").unwrap();
    fs::write(instance.join("patches").join("a.json"), b"patch").unwrap();

    let out = crate::test_support::temp_dir("server-instance-base-out");
    let mut game = game();
    game.dir = dir;
    game.icon = Some(names::ICON_FILE.to_string());
    let mut options = options("mml", &[], &[], &[]);
    options.base_files = vec![
        "server.properties".to_string(),
        ".minecraft".to_string(),
        "libraries".to_string(),
        "patches".to_string(),
    ];
    let contents = contents(&game_dir, &options);
    let result = generate_mml(
        &game,
        &HashMap::new(),
        &game_dir,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    let base = pack.base.expect("应写出基础文件包");
    assert_eq!(base.file, "base.zip");
    assert_eq!(base.url, "https://example.invalid/pack/base.zip");
    let archive =
        mml_base::archives::BaseArchive::open_readonly(out.join("base.zip")).unwrap();
    assert!(
        archive.entries().iter().any(|item| item.name == "server.properties"),
        "基础包应包含选中的文件"
    );
    assert!(
        archive
            .entries()
            .iter()
            .any(|item| item.name == ".minecraft/options.txt"),
        "基础包应包含 .minecraft 里的文件"
    );
    assert!(
        archive
            .entries()
            .iter()
            .any(|item| item.name == "libraries/foo.jar"),
        "基础包应包含 libraries 里的文件"
    );
    assert!(
        archive
            .entries()
            .iter()
            .any(|item| item.name == "patches/a.json"),
        "基础包应包含 patches 里的文件"
    );
    assert!(
        !archive.entries().iter().any(|item| item.name == "icon.png"),
        "实例图标不进基础包"
    );
    assert_eq!(result.count, 4);
    assert!(result.files.iter().any(|name| name == "base.zip"));
    assert!(result.files.iter().any(|name| name == names::ICON_FILE));

    let _ = fs::remove_dir_all(&instance);
    crate::test_support::cleanup(&[&out]);
}

/// 基础文件不接受 .minecraft 里的文件与启动器元数据
#[test]
fn base_files_allow_minecraft_but_reject_metadata() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-base-reject");
    fs::create_dir_all(base.join(".minecraft")).unwrap();
    fs::create_dir_all(base.join("libraries")).unwrap();
    fs::create_dir_all(base.join("patches")).unwrap();
    fs::write(base.join("server.properties"), b"x").unwrap();
    fs::write(base.join(".minecraft").join("a.txt"), b"x").unwrap();
    fs::write(base.join(".minecraft").join("server.json"), b"{}").unwrap();
    fs::write(base.join(".minecraft").join("server.json.sha1"), b"x").unwrap();
    fs::write(base.join("libraries").join("foo.jar"), b"x").unwrap();
    fs::write(base.join("patches").join("a.json"), b"x").unwrap();
    fs::write(base.join("game.json"), b"{}").unwrap();

    assert_eq!(
        files::base_files(&base, &["server.properties".to_string()]).unwrap(),
        vec![PathBuf::from("server.properties")]
    );
    assert_eq!(
        files::base_files(&base, &[".minecraft".to_string()]).unwrap(),
        vec![PathBuf::from(".minecraft/a.txt")]
    );
    assert_eq!(
        files::base_files(&base, &[".minecraft/a.txt".to_string()]).unwrap(),
        vec![PathBuf::from(".minecraft/a.txt")]
    );
    assert_eq!(
        files::base_files(&base, &["libraries".to_string()]).unwrap(),
        vec![PathBuf::from("libraries/foo.jar")]
    );
    assert_eq!(
        files::base_files(&base, &["patches".to_string()]).unwrap(),
        vec![PathBuf::from("patches/a.json")]
    );
    assert!(files::base_files(&base, &["game.json".to_string()]).is_err());
    assert!(files::base_files(&base, &[".minecraft/server.json".to_string()]).is_err());

    crate::test_support::cleanup(&[&base]);
}


/// 本地模组 / 资源包没有平台编号时写自定义编号并打 Custom 标记
#[test]
fn mml_marks_generated_ids_as_custom() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-custom-id");
    let out = crate::test_support::temp_dir("server-instance-custom-id-out");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::create_dir_all(base.join("resourcepacks")).unwrap();
    fs::write(base.join("mods").join("local.jar"), b"jar").unwrap();
    fs::write(base.join("resourcepacks").join("pack.zip"), b"pack").unwrap();

    let options = options(
        "mml",
        &["mods/local.jar"],
        &[],
        &["resourcepacks/pack.zip"],
    );
    let contents = contents(&base, &options);
    let _ = generate_mml(
        &game(),
        &HashMap::new(),
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    let local = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/local.jar")
        .expect("应有本地模组");
    assert!(local.custom);
    assert!(local.pid.as_deref().unwrap_or_default().starts_with("custom-p-"));
    assert!(local.fid.as_deref().unwrap_or_default().starts_with("custom-f-"));

    let resource = pack
        .online_list
        .iter()
        .find(|item| item.file == "resourcepacks/pack.zip")
        .expect("应有资源包");
    assert!(resource.custom);

    crate::test_support::cleanup(&[&base, &out]);
}


/// 再次生成时按映射复用项目编号、更新文件号，并把变化的模组 / 资源包 / 配置目录标出来
#[test]
fn mml_reuses_ids_and_reports_changed_entries() {
    crate::test_support::ensure_boot();
    let dir = format!("server-instance-map-{}", uuid::Uuid::new_v4());
    let instance = mml_base::get_base_dir()
        .join("minecraft")
        .join("instances")
        .join(&dir);
    let game_dir = instance.join(".minecraft");
    fs::create_dir_all(game_dir.join("mods")).unwrap();
    fs::create_dir_all(game_dir.join("resourcepacks")).unwrap();
    fs::create_dir_all(game_dir.join("config")).unwrap();
    fs::write(game_dir.join("mods").join("local.jar"), b"v1").unwrap();
    fs::write(game_dir.join("resourcepacks").join("pack.zip"), b"p1").unwrap();
    fs::write(game_dir.join("config").join("a.toml"), b"c1").unwrap();

    let out1 = crate::test_support::temp_dir("server-instance-map-out-1");
    let out2 = crate::test_support::temp_dir("server-instance-map-out-2");
    let mut game = game();
    game.dir = dir;
    let options = options(
        "mml",
        &["mods/local.jar"],
        &[("config", "archive")],
        &["resourcepacks/pack.zip"],
    );
    let contents = contents(&game_dir, &options);

    let first = generate_mml(
        &game,
        &HashMap::new(),
        &game_dir,
        &out1,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();
    assert!(first.changed.is_empty(), "第一次生成不该有变化");
    assert!(instance.join(names::SERVER_INSTANCE_ID_FILE).is_file());

    let pack1: ServerPackObj =
        serde_json::from_slice(&fs::read(out1.join("server.json")).unwrap()).unwrap();
    let mod1 = pack1
        .online_list
        .iter()
        .find(|item| item.file == "mods/local.jar")
        .unwrap();

    // 三个都换内容，再生成一次
    fs::write(game_dir.join("mods").join("local.jar"), b"v2").unwrap();
    fs::write(game_dir.join("resourcepacks").join("pack.zip"), b"p2").unwrap();
    fs::write(game_dir.join("config").join("a.toml"), b"c2").unwrap();
    let second = generate_mml(
        &game,
        &HashMap::new(),
        &game_dir,
        &out2,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    assert!(second.changed.iter().any(|path| path == "mods/local.jar"));
    assert!(second.changed.iter().any(|path| path == "resourcepacks/pack.zip"));
    assert!(second.changed.iter().any(|path| path == "config"));

    let pack2: ServerPackObj =
        serde_json::from_slice(&fs::read(out2.join("server.json")).unwrap()).unwrap();
    let mod2 = pack2
        .online_list
        .iter()
        .find(|item| item.file == "mods/local.jar")
        .unwrap();
    assert_eq!(mod1.pid, mod2.pid, "同一 modid / 路径应复用项目编号");
    assert_ne!(mod1.fid, mod2.fid, "内容变了文件号要更新");

    let saved = mapping::IdMap::load(&instance);
    let meta = saved.meta().expect("应保存生成选项");
    assert_eq!(meta.name, options.name);
    assert_eq!(meta.version, options.version);
    assert_eq!(meta.text, options.text);
    assert_eq!(meta.url, "https://example.invalid/pack");
    assert_eq!(meta.format, "mml");
    assert_eq!(meta.folder, options.folder);

    let _ = fs::remove_dir_all(&instance);
    crate::test_support::cleanup(&[&out1, &out2]);
}


/// 服务器实例本身不允许再生成服务器实例
#[test]
fn server_pack_cannot_generate() {
    crate::test_support::ensure_boot();
    let mut game = game();
    game.modpack_type = ModPackType::ServerPack;
    let options = options("mml", &[], &[], &[]);
    let result = generate(&game, &options, &|_, _, _| {});
    assert_eq!(
        result.err().as_deref(),
        Some(err_keys::SERVER_INSTANCE_FROM_SERVER)
    );
}


/// 自定义编号被用户改过后，生成时优先用手填值，并且仍然按本地托管
#[test]
fn mml_uses_edited_custom_ids() {
    crate::test_support::ensure_boot();
    let dir = format!("server-instance-edit-{}", uuid::Uuid::new_v4());
    let instance = mml_base::get_base_dir()
        .join("minecraft")
        .join("instances")
        .join(&dir);
    let base = instance.join(".minecraft");
    let out = crate::test_support::temp_dir("server-instance-edit-id-out");
    let out2 = crate::test_support::temp_dir("server-instance-edit-id-out-2");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::write(base.join("mods").join("local.jar"), b"jar").unwrap();

    let mut game = game();
    game.dir = dir;
    let mut options = options("mml", &["mods/local.jar"], &[], &[]);
    options.mods[0].pid = "my-project".to_string();
    options.mods[0].fid = "my-file".to_string();
    options.mods[0].custom = true;
    options.mods[0].edited = true;
    let first_contents = contents(&base, &options);
    let _ = generate_mml(
        &game,
        &HashMap::new(),
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &first_contents,
        &|_, _, _| {},
    )
    .unwrap();

    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    let item = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/local.jar")
        .unwrap();
    assert_eq!(item.pid.as_deref(), Some("my-project"));
    assert_eq!(item.fid.as_deref(), Some("my-file"));
    assert!(item.custom);
    assert_eq!(
        item.url.as_deref(),
        Some("https://example.invalid/pack/mods/local.jar")
    );
    assert!(out.join("mods/local.jar").is_file());

    // 已生成过的自定义项目编号锁定，改项目号无效；文件号仍可改
    let mut second_options = options.clone();
    second_options.mods[0].pid = "other-project".to_string();
    second_options.mods[0].fid = "other-file".to_string();
    let second_contents = contents(&base, &second_options);
    let _ = generate_mml(
        &game,
        &HashMap::new(),
        &base,
        &out2,
        "https://example.invalid/pack",
        &second_options,
        &second_contents,
        &|_, _, _| {},
    )
    .unwrap();
    let pack2: ServerPackObj =
        serde_json::from_slice(&fs::read(out2.join("server.json")).unwrap()).unwrap();
    let item2 = pack2
        .online_list
        .iter()
        .find(|item| item.file == "mods/local.jar")
        .unwrap();
    assert_eq!(item2.pid.as_deref(), Some("my-project"));
    assert_eq!(item2.fid.as_deref(), Some("other-file"));

    let _ = fs::remove_dir_all(&instance);
    crate::test_support::cleanup(&[&out, &out2]);
}

/// 实例自带图标时复制进输出文件夹，没有图标时返回 None
#[test]
fn copy_icon_follows_instance_icon() {
    crate::test_support::ensure_boot();
    let dir = crate::test_support::temp_dir("icon-src");
    let out = crate::test_support::temp_dir("icon-out");

    let mut game = InstanceSettingObj {
        // 绝对路径：脱离测试运行目录，直接指向临时目录本身
        dir: dir.to_string_lossy().to_string(),
        icon: Some(names::ICON_FILE.to_string()),
        ..Default::default()
    };
    assert_eq!(copy_icon(&game, &out).unwrap(), None, "没有图标文件时不带");

    let bytes = b"fake png bytes";
    std::fs::write(dir.join(names::ICON_FILE), bytes).unwrap();
    assert_eq!(
        copy_icon(&game, &out).unwrap(),
        Some(names::ICON_FILE.to_string())
    );
    assert_eq!(std::fs::read(out.join(names::ICON_FILE)).unwrap(), bytes);

    // 图标名被清掉时按默认 icon.png 找
    game.icon = None;
    assert!(copy_icon(&game, &out).unwrap().is_some());

    crate::test_support::cleanup(&[&dir, &out]);
}

