use super::*;
use crate::dtos::server_instance_dto::{
    ServerInstanceConfigDto, ServerInstanceCustomDto, ServerInstancePickDto,
};
use mml_config::config_obj::RunArgObj;
use mml_game::{
    launcher::file_online_info_obj::OnlineInfoObj, loader::LoaderType,
    other_launcher::mmc_obj::MMCObj,
    serverpack::serverpack_obj::{ServerArchiveAction, ServerPackObj},
};

/// 勾选一组模组路径
fn picks(paths: &[&str]) -> Vec<ServerInstancePickDto> {
    paths
        .iter()
        .map(|path| ServerInstancePickDto {
            path: path.to_string(),
            exclude: false,
            url: String::new(),
            text: String::new(),
            pid: String::new(),
            fid: String::new(),
            custom: false,
            edited: false,
            type_changed: false,
            optional: false,
        })
        .collect()
}

/// 测试用的生成选项
fn options(
    mods: &[&str],
    configs: &[(&str, &str)],
    packs: &[&str],
) -> ServerInstanceOptionsDto {
    ServerInstanceOptionsDto {
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
                exclude: Vec::new(),
                delete: Vec::new(),
                delete_old: None,
            })
            .collect(),
        base_files: Vec::new(),
        resource_packs: packs
            .iter()
            .map(|path| ServerInstancePickDto {
                path: path.to_string(),
                exclude: false,
                url: String::new(),
                text: String::new(),
                pid: String::new(),
                fid: String::new(),
                custom: true,
                edited: false,
                type_changed: false,
                optional: false,
            })
            .collect(),
        custom: Vec::new(),
    }
}

/// 按选项展开勾选，等价于 generate 里的那一步
fn contents(base: &Path, options: &ServerInstanceOptionsDto) -> Contents {
    Contents {
        mods: files::mods(base, &options.mods).unwrap(),
        configs: options
            .configs
            .iter()
            .map(|item| {
                let mode = ConfigMode::parse(&item.mode).unwrap();
                // 与生成时同一套默认：不给就按方式自带的，删除目录恒为真
                let delete_old = matches!(mode, ConfigMode::Delete)
                    || item.delete_old.unwrap_or_else(|| mode.deletes_old());
                ConfigDir {
                    dir: item.dir.clone(),
                    mode,
                    exclude: item.exclude.clone(),
                    delete: item.delete.clone(),
                    delete_old,
                }
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
                type_changed: pick.type_changed,
            })
            .collect(),
        custom: options
            .custom
            .iter()
            .map(|item| {
                let action = if item.action == "add" {
                    CustomAction::Add
                } else {
                    CustomAction::Delete
                };
                let (path, content) = match action {
                    CustomAction::Add => match &item.content {
                        Some(text) => (
                            files::custom_target(base, &item.path).unwrap(),
                            Some(text.clone()),
                        ),
                        None => (files::file(base, &item.path).unwrap(), None),
                    },
                    CustomAction::Delete => (files::custom_target(base, &item.path).unwrap(), None),
                };
                CustomPick {
                    path,
                    action,
                    content,
                }
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
fn config_dir_allows_nested_dirs() {
    assert_eq!(config_dir("config").unwrap(), "config");
    assert_eq!(config_dir("config\\setting").unwrap(), "config/setting");
    assert_eq!(
        config_dir("config/setting/deep").unwrap(),
        "config/setting/deep"
    );
    for bad in [
        "",
        "config/",
        "/config",
        "config//setting",
        ".",
        "..",
        "config/..",
    ] {
        assert!(config_dir(bad).is_err(), "{bad}");
    }
}

/// 「本目录」列文件时不含子目录
#[test]
fn dir_direct_files_skip_subdirectories() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-config-direct");
    fs::create_dir_all(base.join("config").join("sub")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();
    fs::write(base.join("config").join("b.toml"), b"b").unwrap();
    fs::write(base.join("config").join("sub").join("c.toml"), b"c").unwrap();

    assert_eq!(
        files::dir_direct_files(&base, "config").unwrap(),
        vec![
            PathBuf::from("config/a.toml"),
            PathBuf::from("config/b.toml"),
        ],
        "只列当前层的文件"
    );
    assert_eq!(
        files::dir_files(&base, "config").unwrap(),
        vec![
            PathBuf::from("config/a.toml"),
            PathBuf::from("config/b.toml"),
            PathBuf::from("config/sub/c.toml"),
        ],
        "整包 / 整目录才递归"
    );
    assert!(files::dir_direct_files(&base, "missing").is_err());

    crate::test_support::cleanup(&[&base]);
}

/// 「压缩包本目录同步」只打当前层的文件，且不删玩家原有目录
#[test]
fn self_archive_only_packs_direct_files() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-config-self");
    let base = instance.join(names::GAME_DIR);
    let out = crate::test_support::temp_dir("server-instance-config-self-out");
    fs::create_dir_all(base.join("config").join("sub")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();
    fs::write(base.join("config").join("sub").join("c.toml"), b"c").unwrap();

    let mut game = game();
    game.dir = instance.to_string_lossy().to_string();
    let options = options(&[], &[("config", "archiveSelf")], &[]);
    let contents = contents(&base, &options);
    generate_mml(
        &game,
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
    let archive = pack.archive_list.first().expect("应写出配置压缩包");
    assert!(!archive.delete_old, "本目录同步不能删掉子目录");
    let zip = mml_base::archives::BaseArchive::open_readonly(out.join("config.zip")).unwrap();
    let names: Vec<&str> = zip.entries().iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["a.toml"], "压缩包只装当前层的文件");

    crate::test_support::cleanup(&[&instance, &out]);
}

/// 整包同步可以取消勾选单个文件；本目录同步不受更深处的排除影响
#[test]
fn config_exclusions_filter_packed_files() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-config-exclude");
    let base = instance.join(names::GAME_DIR);
    let out = crate::test_support::temp_dir("server-instance-config-exclude-out");
    fs::create_dir_all(base.join("config").join("sub")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();
    fs::write(base.join("config").join("b.toml"), b"b").unwrap();
    fs::write(base.join("config").join("sub").join("c.toml"), b"c").unwrap();

    let mut game = game();
    game.dir = instance.to_string_lossy().to_string();

    // 整包：取消 b.toml 后只打 a.toml 与子目录
    let mut all_options = options(&[], &[("config", "archiveAll")], &[]);
    all_options.configs[0].exclude = vec!["config/b.toml".to_string()];
    let all_contents = contents(&base, &all_options);
    generate_mml(
        &game,
        &HashMap::new(),
        &base,
        &out,
        "https://example.invalid/pack",
        &all_options,
        &all_contents,
        &|_, _, _| {},
    )
    .unwrap();
    assert!(out.join(names::SERVER_FILE).is_file());
    let zip = mml_base::archives::BaseArchive::open_readonly(out.join("config.zip")).unwrap();
    let names: Vec<&str> = zip.entries().iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["a.toml", "sub/c.toml"]);

    // 本目录：b.toml 仍被排除，子目录里的 c.toml 不归它管
    let out2 = crate::test_support::temp_dir("server-instance-config-exclude-out-2");
    let mut self_options = options(&[], &[("config", "archiveSelf")], &[]);
    self_options.configs[0].exclude = vec!["config/b.toml".to_string()];
    let self_contents = contents(&base, &self_options);
    generate_mml(
        &game,
        &HashMap::new(),
        &base,
        &out2,
        "https://example.invalid/pack",
        &self_options,
        &self_contents,
        &|_, _, _| {},
    )
    .unwrap();
    let zip = mml_base::archives::BaseArchive::open_readonly(out2.join("config.zip")).unwrap();
    let names: Vec<&str> = zip.entries().iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["a.toml"]);

    // 整包全取消勾选：不写空包，客户端目录不会被误删
    let out3 = crate::test_support::temp_dir("server-instance-config-exclude-out-3");
    let mut none_options = options(&[], &[("config", "archiveAll")], &[]);
    none_options.configs[0].exclude = vec![
        "config/a.toml".to_string(),
        "config/b.toml".to_string(),
        "config/sub/c.toml".to_string(),
    ];
    let none_contents = contents(&base, &none_options);
    generate_mml(
        &game,
        &HashMap::new(),
        &base,
        &out3,
        "https://example.invalid/pack",
        &none_options,
        &none_contents,
        &|_, _, _| {},
    )
    .unwrap();
    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out3.join("server.json")).unwrap()).unwrap();
    assert!(pack.archive_list.is_empty(), "全取消勾选不能变成删除目录");
    assert!(!out3.join("config.zip").exists());

    crate::test_support::cleanup(&[&instance, &out, &out2, &out3]);
}

/// 「删除目录」写一条 DeletePath 动作的清单项，客户端据此删掉目录
#[test]
fn delete_mode_writes_delete_path() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-config-delete");
    let base = instance.join(names::GAME_DIR);
    let out = crate::test_support::temp_dir("server-instance-config-delete-out");
    fs::create_dir_all(base.join("config")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();

    let mut game = game();
    game.dir = instance.to_string_lossy().to_string();
    let options = options(&[], &[("config", "delete")], &[]);
    let contents = contents(&base, &options);
    let result = generate_mml(
        &game,
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
    let archive = pack.archive_list.first().expect("应写出删除条目");
    assert_eq!(archive.action, ServerArchiveAction::DeletePath);
    assert!(archive.file.is_empty(), "删除条目不写压缩包文件名");
    assert!(archive.url.is_none(), "删除条目不写下载地址");
    assert!(!out.join("config.zip").exists(), "删除目录不再写压缩包");
    assert!(!result.files.iter().any(|name| name == "config.zip"));

    crate::test_support::cleanup(&[&instance, &out]);
}

/// 单文件标记删除：不进包，写一条 DeleteFile 动作的清单项让客户端删掉这个路径
#[test]
fn file_delete_writes_delete_file() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-config-file-delete");
    let base = instance.join(names::GAME_DIR);
    let out = crate::test_support::temp_dir("server-instance-config-file-delete-out");
    fs::create_dir_all(base.join("config")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();
    fs::write(base.join("config").join("b.toml"), b"b").unwrap();

    let mut game = game();
    game.dir = instance.to_string_lossy().to_string();
    let mut options = options(&[], &[("config", "archiveAll")], &[]);
    options.configs[0].delete = vec!["config/b.toml".to_string()];
    let contents = contents(&base, &options);
    generate_mml(
        &game,
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
    assert_eq!(pack.archive_list.len(), 2, "目录包与删除项各一条");
    let dir_archive = pack
        .archive_list
        .iter()
        .find(|item| item.dir == "config")
        .expect("应写出目录压缩包");
    let zip = mml_base::archives::BaseArchive::open_readonly(out.join(&dir_archive.file)).unwrap();
    let names: Vec<&str> = zip.entries().iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["a.toml"], "标记删除的文件不能进目录包");

    let deletion = pack
        .archive_list
        .iter()
        .find(|item| item.dir == "config/b.toml")
        .expect("应写出删除条目");
    assert_eq!(deletion.action, ServerArchiveAction::DeleteFile);
    assert!(deletion.file.is_empty(), "删除条目不写压缩包文件名");
    assert!(deletion.url.is_none(), "删除条目不写下载地址");

    crate::test_support::cleanup(&[&instance, &out]);
}

/// 上级不同步、子目录单独删：只写子目录的 DeletePath 条目，父目录不产出内容
#[test]
fn skipped_parent_allows_child_delete() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-config-skip-child-delete");
    let base = instance.join(names::GAME_DIR);
    let out = crate::test_support::temp_dir("server-instance-config-skip-child-delete-out");
    fs::create_dir_all(base.join("config").join("old")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();
    fs::write(base.join("config").join("old").join("b.toml"), b"b").unwrap();

    let mut game = game();
    game.dir = instance.to_string_lossy().to_string();
    let options = options(&[], &[("config", "none"), ("config/old", "delete")], &[]);
    let contents = contents(&base, &options);
    generate_mml(
        &game,
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
    assert_eq!(pack.archive_list.len(), 1, "只写子目录这一条删除项");
    assert_eq!(pack.archive_list[0].action, ServerArchiveAction::DeletePath);
    assert_eq!(pack.archive_list[0].dir, "config/old");
    assert!(pack.online_list.is_empty(), "不同步的父目录不生成文件条目");

    crate::test_support::cleanup(&[&instance, &out]);
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
        text: String::new(),
        pid: String::new(),
        fid: String::new(),
        custom: false,
        edited: false,
        type_changed: false,
        optional: false,
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
    let options = options(&["mods"], &[("config", "archive")], &[]);
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
        pack.archive_list[0].url.as_deref(),
        Some("https://example.invalid/pack/config.zip")
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
    let options = options(&[], &[], &[]);
    let contents = Contents {
        mods: files::mods(&base, &manual).unwrap(),
        configs: Vec::new(),
        packs: Vec::new(),
        custom: Vec::new(),
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
    let options = options(&[], &[], &["resourcepacks/p.zip"]);
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

    let options = options(&[], &[], &["resourcepacks/中文 1.zip"]);
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

/// 整包同步的父目录下把子目录显式设为不同步：父包排除它，也不为它单独生成条目
#[test]
fn mml_excludes_none_subdirectory_from_parent_archive() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-none-child");
    let out = crate::test_support::temp_dir("server-instance-none-child-out");
    fs::create_dir_all(base.join("config").join("keep")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();
    fs::write(base.join("config").join("keep").join("b.toml"), b"b").unwrap();

    let options = options(
        &[],
        &[("config", "archive"), ("config/keep", "none")],
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
    assert_eq!(pack.archive_list.len(), 1, "被排除的子目录不该有自己的包");
    assert_eq!(pack.archive_list[0].file, "config.zip");

    let parent = mml_base::archives::BaseArchive::open_readonly(out.join("config.zip")).unwrap();
    let names: Vec<String> = parent
        .entries()
        .iter()
        .map(|item| item.name.clone())
        .collect();
    assert!(names.iter().any(|name| name == "a.toml"), "{names:?}");
    assert!(
        !names.iter().any(|name| name == "keep/b.toml"),
        "父包不该包含被排除的子目录: {names:?}"
    );
    assert_eq!(result.count, 1);

    crate::test_support::cleanup(&[&base, &out]);
}

/// 整包同步关掉"解压前删旧目录"后，清单里的 delete_old 跟着变
#[test]
fn mml_honours_delete_old_override() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-delete-old");
    let out = crate::test_support::temp_dir("server-instance-delete-old-out");
    fs::create_dir_all(base.join("config")).unwrap();
    fs::write(base.join("config").join("a.toml"), b"a").unwrap();

    let mut options = options(&[], &[("config", "archive")], &[]);
    options.configs[0].delete_old = Some(false);
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
    assert!(!pack.archive_list[0].delete_old, "关掉后不该再删旧目录");

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
    let mut options = options(&[], &[], &[]);
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
    fs::create_dir_all(base.join(".minecraft").join("mods")).unwrap();
    fs::write(base.join(".minecraft").join("mods").join("a.jar"), b"x").unwrap();
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
        vec![
            PathBuf::from(".minecraft/a.txt"),
            PathBuf::from(".minecraft/mods/a.jar"),
        ]
    );
    assert_eq!(
        files::base_files(&base, &[".minecraft/a.txt".to_string()]).unwrap(),
        vec![PathBuf::from(".minecraft/a.txt")]
    );
    // 展开 .minecraft 后可以只勾里面的子目录
    assert_eq!(
        files::base_files(&base, &[".minecraft/mods".to_string()]).unwrap(),
        vec![PathBuf::from(".minecraft/mods/a.jar")]
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
    fs::write(instance.join("server.properties"), b"p").unwrap();

    let out1 = crate::test_support::temp_dir("server-instance-map-out-1");
    let out2 = crate::test_support::temp_dir("server-instance-map-out-2");
    let mut game = game();
    game.dir = dir;
    let mut options = options(
        &["mods/local.jar"],
        &[("config", "archive")],
        &["resourcepacks/pack.zip"],
    );
    options.base_files = vec!["server.properties".to_string()];
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
    assert!(first.mod_changes.added.is_empty());
    assert!(first.mod_changes.removed.is_empty());
    assert!(first.mod_changes.modified.is_empty());
    assert!(first.pack_changes.added.is_empty());
    assert!(first.pack_changes.removed.is_empty());
    assert!(first.pack_changes.modified.is_empty());
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
    assert!(
        second
            .mod_changes
            .modified
            .iter()
            .any(|path| path == "mods/local.jar")
    );
    assert!(second.mod_changes.added.is_empty());
    assert!(second.mod_changes.removed.is_empty());
    assert!(
        second
            .pack_changes
            .modified
            .iter()
            .any(|path| path == "resourcepacks/pack.zip")
    );
    assert!(second.pack_changes.added.is_empty());
    assert!(second.pack_changes.removed.is_empty());
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
    assert_eq!(meta.folder, options.folder);
    assert_eq!(meta.base_files, options.base_files);

    let _ = fs::remove_dir_all(&instance);
    crate::test_support::cleanup(&[&out1, &out2]);
}


/// 服务器实例本身不允许再生成服务器实例
#[test]
fn server_pack_cannot_generate() {
    crate::test_support::ensure_boot();
    let mut game = game();
    game.modpack_type = ModPackType::ServerPack;
    let options = options(&[], &[], &[]);
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
    let mut options = options(&["mods/local.jar"], &[], &[]);
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

/// 标成可选的模组要在清单里带 Optional，没标的不能带
#[test]
fn mml_marks_optional_mods() {
    crate::test_support::ensure_boot();
    let base = crate::test_support::temp_dir("server-instance-optional");
    let out = crate::test_support::temp_dir("server-instance-optional-out");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::write(base.join("mods").join("opt.jar"), b"opt").unwrap();
    fs::write(base.join("mods").join("plain.jar"), b"plain").unwrap();

    let mut options = options(&["mods/opt.jar", "mods/plain.jar"], &[], &[]);
    options.mods[0].optional = true;
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
    let opt = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/opt.jar")
        .expect("应有可选模组");
    let plain = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/plain.jar")
        .expect("应有普通模组");
    assert!(opt.optional, "标了可选的模组要带 Optional");
    assert!(!plain.optional, "没标的不能带 Optional");

    crate::test_support::cleanup(&[&base, &out]);
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

/// 服务器包只带基础文件，完整包再补上 .minecraft 里的模组与配置
#[test]
fn export_pack_writes_thin_and_complete_zips() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-export-pack");
    let game_dir = instance.join(".minecraft");
    fs::create_dir_all(game_dir.join("mods")).unwrap();
    fs::create_dir_all(game_dir.join("config")).unwrap();
    fs::write(game_dir.join("mods").join("a.jar"), b"mod").unwrap();
    fs::write(game_dir.join("config").join("x.toml"), b"x = 1").unwrap();
    fs::write(game_dir.join("options.txt"), b"lang:zh_cn").unwrap();
    fs::write(instance.join("server.properties"), b"motd=test").unwrap();
    fs::write(instance.join("gtnh_icon.png"), b"icon").unwrap();

    let mut game = game();
    // 绝对路径：get_base_path / get_game_path 直接落在临时目录里
    game.dir = instance.to_string_lossy().to_string();
    game.icon = Some("gtnh_icon.png".to_string());
    game.jvm_name = Some("Java 17".to_string());
    game.jvm_local = Some(instance.join("java.exe").to_string_lossy().to_string());
    game.jvm_arg = Some(RunArgObj {
        max_memory: Some(4096),
        min_memory: Some(1024),
        jvm_args: Some(
            "-Xmx2G\n-javaagent:%GAME_DIR%/mml-server-sync.jar=https://old.invalid/server.json\n-Dfoo=1"
                .to_string(),
        ),
        ..Default::default()
    });

    let mut options = options(&["mods/a.jar"], &[("config", "archive")], &[]);
    options.base_files = vec!["server.properties".to_string()];
    // 服务器包会附带生成好的清单；导出前发布目录里得有它
    let publish = crate::test_support::temp_dir("server-instance-export-pack-publish");
    fs::write(
        publish.join(names::SERVER_FILE),
        br#"{"Name":"server","Version":"1.21.1","PackVersion":"1.0.0","Files":[]}"#,
    )
    .unwrap();
    options.folder = publish.to_string_lossy().to_string();

    let out = crate::test_support::temp_dir("server-instance-export-pack-out");
    let thin = out.join("thin.zip");
    let complete = out.join("complete.zip");
    let quiet: std::sync::Arc<dyn Fn(usize, usize, String) + Send + Sync> =
        std::sync::Arc::new(|_, _, _| {});
    pack::export_pack(&game, &options, &thin, false, quiet.clone()).unwrap();
    pack::export_pack(&game, &options, &complete, true, quiet).unwrap();

    let in_pack = |item: &str| format!("{}/{}", options.name, item);

    // 服务器包：只有基础文件与六个自动生成条目
    let thin_zip = mml_base::archives::BaseArchive::open_readonly(&thin).unwrap();
    let thin_names: Vec<&str> = thin_zip.entries().iter().map(|item| item.name.as_str()).collect();
    for item in [
        "mmc-pack.json",
        "instance.cfg",
        "game.json",
        "server.json",
        ".minecraft/mml-server-sync.jar",
        ".minecraft/server.json",
        "server.properties",
        "gtnh_icon.png",
    ] {
        assert!(
            thin_names.contains(&in_pack(item).as_str()),
            "服务器包缺少 {item}: {thin_names:?}"
        );
    }
    assert!(
        !thin_names.contains(&in_pack(".minecraft/mods/a.jar").as_str()),
        "服务器包不该带模组: {thin_names:?}"
    );
    assert!(
        !thin_names.contains(&in_pack("icon.png").as_str()),
        "图标按原名带，不该再补 icon.png: {thin_names:?}"
    );
    // 包里的清单就是生成出来的那份，不重新拼
    assert_eq!(
        thin_zip.read(&in_pack("server.json")).unwrap(),
        fs::read(publish.join(names::SERVER_FILE)).unwrap()
    );
    assert_eq!(
        thin_zip.read(&in_pack(".minecraft/server.json")).unwrap(),
        fs::read(publish.join(names::SERVER_FILE)).unwrap()
    );

    let mmc: MMCObj = serde_json::from_slice(&thin_zip.read(&in_pack("mmc-pack.json")).unwrap())
        .unwrap();
    assert_eq!(mmc.format_version, 1, "Prism / MultiMC 只认 formatVersion 1");
    let minecraft = mmc
        .components
        .iter()
        .find(|item| item.uid == "net.minecraft")
        .unwrap();
    assert_eq!(minecraft.version, "1.21.1");
    let forge = mmc
        .components
        .iter()
        .find(|item| item.uid == "net.minecraftforge")
        .unwrap();
    assert_eq!(forge.version, "52.0.1");

    let cfg = String::from_utf8(thin_zip.read(&in_pack("instance.cfg")).unwrap()).unwrap();
    assert!(cfg.contains("InstanceType=OneSix"));
    assert!(cfg.contains("iconKey=gtnh_icon"), "{cfg}");
    assert!(cfg.contains("name=服务器"));
    assert!(cfg.contains("MaxMemAlloc=4096"));
    assert!(cfg.contains("MinMemAlloc=1024"));
    assert!(
        cfg.contains(
            "JvmArgs=-Xmx2G -Dfoo=1 -javaagent:mml-server-sync.jar=https://example.invalid/pack/server.json"
        ),
        "{cfg}"
    );

    let saved: InstanceSettingObj =
        serde_json::from_slice(&thin_zip.read(&in_pack("game.json")).unwrap()).unwrap();
    assert_eq!(saved.name, "服务器");
    assert_eq!(saved.dir, "");
    assert!(saved.is_modpack);
    assert_eq!(saved.modpack_type, ModPackType::ServerPack);
    assert_eq!(
        saved.server_url.as_deref(),
        Some("https://example.invalid/pack")
    );
    assert_ne!(saved.uuid, uuid::Uuid::nil());
    assert_eq!(saved.icon.as_deref(), Some("gtnh_icon.png"));
    assert!(saved.jvm_name.is_none());
    assert!(saved.jvm_local.is_none());
    let jvm_args = saved
        .jvm_arg
        .as_ref()
        .and_then(|jvm| jvm.jvm_args.as_deref())
        .unwrap_or_default();
    assert!(!jvm_args.contains("mml-server-sync.jar"), "{jvm_args}");
    assert!(jvm_args.contains("-Dfoo=1"), "{jvm_args}");

    let jar = thin_zip
        .entries()
        .iter()
        .find(|item| item.name == in_pack(".minecraft/mml-server-sync.jar"))
        .unwrap();
    assert!(jar.size > 0);
    assert_eq!(jar.size, pack::SYNC_JAR.len() as u64);

    // 完整包：基础文件 + 勾选的模组与配置目录
    let complete_zip = mml_base::archives::BaseArchive::open_readonly(&complete).unwrap();
    let complete_names: Vec<&str> = complete_zip
        .entries()
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    for item in [
        "server.properties",
        ".minecraft/mods/a.jar",
        ".minecraft/config/x.toml",
    ] {
        assert!(
            complete_names.contains(&in_pack(item).as_str()),
            "完整包缺少 {item}: {complete_names:?}"
        );
    }
    assert!(
        !complete_names.contains(&in_pack(".minecraft/options.txt").as_str()),
        "没勾的文件不该进包: {complete_names:?}"
    );

    crate::test_support::cleanup(&[&instance, &publish, &out]);
}

/// 自定义条目：添加的文件进清单，删除的写 delete 条目
#[test]
fn custom_entries_add_file_and_delete_path() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-custom");
    let base = instance.join(names::GAME_DIR);
    let out = crate::test_support::temp_dir("server-instance-custom-out");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::write(base.join("mods").join("custom.jar"), b"c").unwrap();
    fs::write(base.join("options.txt"), b"x").unwrap();

    let mut game = game();
    game.dir = instance.to_string_lossy().to_string();
    let mut options = options(&[], &[], &[]);
    options.custom = vec![
        ServerInstanceCustomDto {
            path: "mods/custom.jar".into(),
            action: "add".into(),
            content: None,
        },
        ServerInstanceCustomDto {
            path: "config/gone.toml".into(),
            action: "delete".into(),
            content: None,
        },
    ];
    let contents = contents(&base, &options);
    generate_mml(
        &game,
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
    assert!(
        pack.online_list.iter().any(|item| item.file == "mods/custom.jar"),
        "添加的文件应进清单"
    );
    assert!(
        pack.archive_list
            .iter()
            .any(|item| item.action == ServerArchiveAction::DeleteFile
                && item.dir == "config/gone.toml"),
        "删除条目应写进清单"
    );

    crate::test_support::cleanup(&[&instance, &out]);
}

/// 自定义内容：文字直接写成输出文件夹里的文件，清单带地址与 SHA1
#[test]
fn custom_content_writes_hosted_file() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-custom-content");
    let base = instance.join(names::GAME_DIR);
    let out = crate::test_support::temp_dir("server-instance-custom-content-out");
    fs::create_dir_all(&base).unwrap();
    let text = "late = 1\n";

    let mut game = game();
    game.dir = instance.to_string_lossy().to_string();
    let mut options = options(&[], &[], &[]);
    options.custom = vec![ServerInstanceCustomDto {
        path: "config/late.toml".into(),
        action: "add".into(),
        content: Some(text.into()),
    }];
    let contents = contents(&base, &options);
    generate_mml(
        &game,
        &HashMap::new(),
        &base,
        &out,
        "https://example.invalid/pack",
        &options,
        &contents,
        &|_, _, _| {},
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(out.join("config").join("late.toml")).unwrap(),
        text,
        "内容要落到输出文件夹"
    );
    let pack: ServerPackObj =
        serde_json::from_slice(&fs::read(out.join("server.json")).unwrap()).unwrap();
    let item = pack.online_list.first().expect("内容文件应进清单");
    assert_eq!(item.file, "config/late.toml");
    assert!(item.custom, "内容文件走自定义编号");
    assert_eq!(
        item.url.as_deref(),
        Some("https://example.invalid/pack/config/late.toml")
    );
    assert_eq!(item.sha1.as_deref(), Some(sha1(text.as_bytes()).as_str()));

    crate::test_support::cleanup(&[&instance, &out]);
}

/// 自定义内容只能写到文件位置，指向目录要拒绝
#[test]
fn custom_content_rejects_directory_path() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-custom-content-dir");
    let base = instance.join(names::GAME_DIR);
    fs::create_dir_all(base.join("config")).unwrap();
    assert!(files::custom_target(&base, "config").is_err());
    assert!(files::custom_target(&base, "config/late.toml").is_ok());
    crate::test_support::cleanup(&[&instance]);
}

/// 模组说明写进清单的 Text，并随生成选项保存到实例
#[test]
fn mod_text_goes_to_manifest_and_meta() {
    crate::test_support::ensure_boot();
    let instance = crate::test_support::temp_dir("server-instance-mod-text");
    let base = instance.join(names::GAME_DIR);
    let out = crate::test_support::temp_dir("server-instance-mod-text-out");
    fs::create_dir_all(base.join("mods")).unwrap();
    fs::write(base.join("mods").join("a.jar"), b"a").unwrap();

    let mut game = game();
    game.dir = instance.to_string_lossy().to_string();
    let mut options = options(&["mods/a.jar"], &[], &[]);
    options.mods[0].text = "这是说明".to_string();
    let contents = contents(&base, &options);
    generate_mml(
        &game,
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
    let item = pack
        .online_list
        .iter()
        .find(|item| item.file == "mods/a.jar")
        .expect("应写出模组条目");
    assert_eq!(item.text, "这是说明");

    let ids = mapping::IdMap::load(&game.get_base_path());
    let meta = ids.meta().expect("生成后应保存选项");
    assert_eq!(
        meta.mod_texts.get("mods/a.jar").map(String::as_str),
        Some("这是说明")
    );

    crate::test_support::cleanup(&[&instance, &out]);
}

