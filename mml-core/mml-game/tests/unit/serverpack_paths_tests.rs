use super::super::serverpack_obj::{ServerArchiveItemObj, ServerItemObj};
use super::*;

#[test]
fn rejects_manifest_escape_and_root_deletion() {
    let root = mml_testutil::temp_path(format!("serverpack_paths_{}", uuid::Uuid::new_v4()));
    let game = root.join("game");
    let temp = root.join("temp");
    for bad in [
        "../outside",
        "/outside",
        "C:/outside",
        "..\\outside",
        "",
        ".",
    ] {
        let mut pack = ServerPackObj::default();
        pack.online_list.push(ServerItemObj {
            file: bad.into(),
            ..Default::default()
        });
        assert!(validate_pack(&pack, &game, &temp).is_err(), "{bad}");
        pack.online_list.clear();
        pack.archive_list.push(ServerArchiveItemObj {
            file: "pack.zip".into(),
            dir: bad.into(),
            delete_old: true,
            ..Default::default()
        });
        assert!(validate_pack(&pack, &game, &temp).is_err(), "{bad}");
        pack.archive_list[0].file = bad.into();
        pack.archive_list[0].dir = "config".into();
        assert!(validate_pack(&pack, &game, &temp).is_err(), "{bad}");
    }
    let mut pack = ServerPackObj::default();
    pack.online_list.push(ServerItemObj {
        file: "mods/example.jar".into(),
        ..Default::default()
    });
    pack.archive_list.push(ServerArchiveItemObj {
        file: "config.zip".into(),
        dir: "".into(),
        delete_old: false,
        ..Default::default()
    });
    validate_pack(&pack, &game, &temp).unwrap();
}

