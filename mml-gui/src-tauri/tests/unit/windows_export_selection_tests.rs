use super::*;

#[cfg(windows)]
#[test]
fn unreadable_indexed_mod_is_listed_for_curseforge_but_fails_for_modrinth() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = crate::test_support::temp_dir("export-locked-download");
    let path = dir.join("mod.jar");
    std::fs::write(&path, b"mod").unwrap();
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    assert!(std::fs::metadata(&path).is_ok());
    let picks = [ExportPickDto {
        path: "mod.jar".into(),
        url: "https://example.invalid/mod.jar".into(),
        mod_id: None,
        file_id: None,
        exclude: false,
    }];
    let online = HashMap::from([(
        "mod.jar".into(),
        OnlineInfoObj {
            modid: "123".into(),
            fileid: "456".into(),
            ..Default::default()
        },
    )]);
    // CurseForge 的清单只写编号，不读文件内容：锁着的模组照样能列进清单
    let result = collect(&dir, picks.iter(), &online, ExportPackType::CurseForge).unwrap();
    assert_eq!(result.mods.len(), 1);
    // Modrinth 的清单要 sha1 / sha512，读不了就报错，别写出空哈希
    assert!(collect(&dir, picks.iter(), &online, ExportPackType::Modrinth).is_err());
    drop(locked);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn overlapping_directories_and_files_are_written_once() {
    let dir = crate::test_support::temp_dir("export-overlap");
    std::fs::create_dir_all(dir.join("config/nested")).unwrap();
    std::fs::write(dir.join("config/nested/settings.toml"), "enabled = true").unwrap();
    let picks: Vec<_> = [
        "config",
        "config/nested",
        "config/nested/settings.toml",
        "config/nested/settings.toml",
    ]
    .into_iter()
    .map(|path| ExportPickDto {
        path: path.into(),
        url: "".into(),
        mod_id: None,
        file_id: None,
        exclude: false,
    })
    .collect();
    let result = collect(&dir, picks.iter(), &HashMap::new(), ExportPackType::HMCL).unwrap();
    assert_eq!(result.local, [dir.join("config/nested/settings.toml")]);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn excluded_entries_come_out_of_checked_directories() {
    let dir = crate::test_support::temp_dir("export-exclude");
    std::fs::create_dir_all(dir.join(".minecraft/mods")).unwrap();
    std::fs::create_dir_all(dir.join(".minecraft/logs")).unwrap();
    std::fs::write(dir.join(".minecraft/mods/a.jar"), b"a").unwrap();
    std::fs::write(dir.join(".minecraft/logs/x.log"), b"x").unwrap();
    std::fs::write(dir.join(".minecraft/logs/keep.log"), b"k").unwrap();
    let picks = [
        ExportPickDto {
            path: ".minecraft".into(),
            url: "".into(),
            mod_id: None,
            file_id: None,
            exclude: false,
        },
        ExportPickDto {
            path: ".minecraft/logs".into(),
            url: "".into(),
            mod_id: None,
            file_id: None,
            exclude: true,
        },
        ExportPickDto {
            path: ".minecraft/logs/keep.log".into(),
            url: "".into(),
            mod_id: None,
            file_id: None,
            exclude: false,
        },
        // 取消勾选的目录在勾完之后被删掉时，不该让整次导出失败
        ExportPickDto {
            path: ".minecraft/gone".into(),
            url: "".into(),
            mod_id: None,
            file_id: None,
            exclude: true,
        },
    ];
    let result = collect(&dir, picks.iter(), &HashMap::new(), ExportPackType::MML).unwrap();
    assert_eq!(
        result.local,
        [
            dir.join(".minecraft/logs/keep.log"),
            dir.join(".minecraft/mods/a.jar"),
        ]
    );
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn edited_project_and_file_ids_override_the_instance_info() {
    let dir = crate::test_support::temp_dir("export-edited-ids");
    std::fs::write(dir.join("mod.jar"), b"mod").unwrap();
    let online = HashMap::from([(
        "mod.jar".into(),
        OnlineInfoObj {
            modid: "111".into(),
            fileid: "222".into(),
            ..Default::default()
        },
    )]);
    // 改成合法编号：清单用改后的
    let picks = [ExportPickDto {
        path: "mod.jar".into(),
        url: "".into(),
        mod_id: Some("333".into()),
        file_id: Some("444".into()),
        exclude: false,
    }];
    let result = collect(&dir, picks.iter(), &online, ExportPackType::CurseForge).unwrap();
    assert_eq!(result.mods.len(), 1);
    let info = result.mods[0].info.as_ref().unwrap();
    assert_eq!((info.modid.as_str(), info.fileid.as_str()), ("333", "444"));
    // 编号非法：这个格式拼不出清单项，退回 overrides
    let picks = [ExportPickDto {
        path: "mod.jar".into(),
        url: "".into(),
        mod_id: Some("abc".into()),
        file_id: Some("444".into()),
        exclude: false,
    }];
    let result = collect(&dir, picks.iter(), &online, ExportPackType::CurseForge).unwrap();
    assert!(result.mods.is_empty());
    assert_eq!(result.local, [dir.join("mod.jar")]);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn mod_pick_with_ids_still_indexes_when_the_file_pick_excludes_it() {
    let dir = crate::test_support::temp_dir("export-mod-excluded");
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(dir.join("mods/a.jar"), b"a").unwrap();
    // 模组页按清单勾上，文件页又报了一条排除：不能两边都落空
    let picks = [
        ExportPickDto {
            path: "mods/a.jar".into(),
            url: "".into(),
            mod_id: Some("123".into()),
            file_id: Some("456".into()),
            exclude: false,
        },
        ExportPickDto {
            path: "mods/a.jar".into(),
            url: "".into(),
            mod_id: None,
            file_id: None,
            exclude: true,
        },
    ];
    let result = collect(
        &dir,
        picks.iter(),
        &HashMap::new(),
        ExportPackType::CurseForge,
    )
    .unwrap();
    assert_eq!(result.mods.len(), 1);
    assert!(result.local.is_empty());
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn indexed_file_is_not_duplicated_in_overrides() {
    let dir = crate::test_support::temp_dir("export-index-overlap");
    std::fs::create_dir(dir.join("mods")).unwrap();
    std::fs::write(dir.join("mods/a.jar"), b"a").unwrap();
    std::fs::write(dir.join("mods/b.jar"), b"b").unwrap();
    let picks: Vec<_> = ["mods", "mods/a.jar", "mods/a.jar"]
        .into_iter()
        .map(|path| ExportPickDto {
            path: path.into(),
            url: "https://example.invalid/a".into(),
            mod_id: None,
            file_id: None,
            exclude: false,
        })
        .collect();
    for picks in [picks.clone(), picks.into_iter().rev().collect()] {
        let result = collect(
            &dir,
            picks.iter(),
            &HashMap::new(),
            ExportPackType::Modrinth,
        )
        .unwrap();
        assert_eq!(result.local, [dir.join("mods/b.jar")]);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].file, dir.join("mods/a.jar"));
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn curseforge_keeps_foreign_or_invalid_ids_as_local_files() {
    let dir = crate::test_support::temp_dir("export-source");
    std::fs::write(dir.join("mod.jar"), b"mod").unwrap();
    let picks = [ExportPickDto {
        path: "mod.jar".into(),
        url: "".into(),
        mod_id: None,
        file_id: None,
        exclude: false,
    }];
    for (project, file) in [
        ("modrinth", "version"),
        ("0", "1"),
        ("1", "0"),
        ("123", ""),
        ("123", "456"),
    ] {
        let online = HashMap::from([(
            "mod.jar".into(),
            OnlineInfoObj {
                modid: project.into(),
                fileid: file.into(),
                ..Default::default()
            },
        )]);
        let result = collect(&dir, picks.iter(), &online, ExportPackType::CurseForge).unwrap();
        let indexed = project == "123" && file == "456";
        assert_eq!(result.mods.len(), usize::from(indexed), "{project}/{file}");
        assert_eq!(
            result.local.len(),
            usize::from(!indexed),
            "{project}/{file}"
        );
        assert!(result.files.is_empty());
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn modrinth_uses_edited_or_saved_urls_and_falls_back_to_local() {
    let dir = crate::test_support::temp_dir("export-downloads");
    std::fs::write(dir.join("mod.jar"), b"mod").unwrap();
    let online = HashMap::from([(
        "mod.jar".into(),
        OnlineInfoObj {
            url: "https://example.invalid/saved".into(),
            ..Default::default()
        },
    )]);
    for url in ["", "  https://example.invalid/edited  "] {
        let picks = [ExportPickDto {
            path: "mod.jar".into(),
            url: url.into(),
            mod_id: None,
            file_id: None,
            exclude: false,
        }];
        let result = collect(&dir, picks.iter(), &online, ExportPackType::Modrinth).unwrap();
        // 只有地址、没有项目号 / 文件号：进 files，清单里的 project 留空
        assert!(result.mods.is_empty());
        assert_eq!(
            result.files[0].url,
            if url.is_empty() {
                "https://example.invalid/saved"
            } else {
                url.trim()
            }
        );
        assert!(result.files[0].info.is_none());
        let local = collect(
            &dir,
            picks.iter(),
            &HashMap::new(),
            ExportPackType::Modrinth,
        )
        .unwrap();
        assert_eq!(local.local.len(), usize::from(url.is_empty()));
        assert_eq!(local.files.len(), usize::from(!url.is_empty()));
        for pack in [ExportPackType::MMC, ExportPackType::HMCL] {
            let whole = collect(&dir, picks.iter(), &online, pack).unwrap();
            assert_eq!(whole.local, [dir.join("mod.jar")]);
            assert!(whole.mods.is_empty());
        }
    }
    crate::test_support::cleanup(&[&dir]);
}

