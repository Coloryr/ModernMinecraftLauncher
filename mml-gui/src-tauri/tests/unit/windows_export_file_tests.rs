use super::*;
use std::io::Write;

#[tokio::test]
async fn async_export_failure_preserves_archive_and_success_replaces_it() {
    let dir = crate::test_support::temp_dir("archive-export-commit");
    let path = dir.join("instance.mrpack");
    fs::write(&path, "previous").unwrap();
    assert!(
        write_async(&path, |temp| async move {
            fs::write(temp, "partial").unwrap();
            Err("source disappeared".into())
        })
        .await
        .is_err()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "previous");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    write_async(&path, |temp| async move {
        fs::write(temp, "complete").map_err(|e| e.to_string())
    })
    .await
    .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "complete");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    crate::test_support::cleanup(&[&dir]);
}

#[tokio::test]
async fn cancelled_archive_task_cleans_its_partial_file() {
    let dir = crate::test_support::temp_dir("archive-export-cancelled");
    let path = dir.join("instance.zip");
    fs::write(&path, "previous").unwrap();
    let target = path.clone();
    let (ready, started) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        write_async(&target, |temp| async move {
            fs::write(temp, "partial").unwrap();
            ready.send(()).unwrap();
            std::future::pending::<Result<(), String>>().await
        })
        .await
    });
    started.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(fs::read_to_string(&path).unwrap(), "previous");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn successful_export_replaces_existing_text_without_bom() {
    let dir = crate::test_support::temp_dir("text-export-replace");
    let path = dir.join("中文.json");
    fs::write(&path, "previous").unwrap();
    write(&path, |file| {
        file.write_all("[\"中文\"]".as_bytes())
            .map_err(|e| e.to_string())
    })
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), "[\"中文\"]".as_bytes());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn failed_or_cancelled_write_keeps_previous_file_and_removes_partial_output() {
    let dir = crate::test_support::temp_dir("text-export-failed");
    let path = dir.join("mods.csv");
    fs::write(&path, "previous").unwrap();
    for reason in ["write failed", crate::err_keys::CANCELLED] {
        assert_eq!(
            write(&path, |file| {
                file.write_all(b"partial").unwrap();
                Err(reason.into())
            })
            .unwrap_err(),
            reason
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn failed_new_export_leaves_no_destination_or_temporary_file() {
    let dir = crate::test_support::temp_dir("text-export-new-failed");
    assert!(write(&dir.join("mods.txt"), |_| Err("failed".into())).is_err());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    assert!(write(&dir.join("missing/mods.txt"), |_| Ok(())).is_err());
    crate::test_support::cleanup(&[&dir]);
}

#[test]
fn rename_failure_preserves_existing_directory_and_cleans_temporary_file() {
    let dir = crate::test_support::temp_dir("text-export-rename");
    let path = dir.join("mods.json");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("keep"), "previous").unwrap();
    assert!(
        write(&path, |file| file
            .write_all(b"new")
            .map_err(|e| e.to_string()))
        .is_err()
    );
    assert_eq!(fs::read_to_string(path.join("keep")).unwrap(), "previous");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    crate::test_support::cleanup(&[&dir]);
}

