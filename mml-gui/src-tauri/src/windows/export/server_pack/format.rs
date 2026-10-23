//! 在任何覆盖之前校验清单与完整条目表。

use std::{collections::HashSet, fs::File, io::{Read, Seek}, path::Path};
use zip::ZipArchive;
use crate::err_keys;
use super::{CONFIG, GUI_CONFIG, INSTANCES, LAUNCHER, MANIFEST, Manifest};

pub(super) fn safe_component(value: &str) -> bool {
    !value.is_empty() && value != "." && value != ".."
        && !value.ends_with(['.', ' '])
        && !value.chars().any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
        && !matches!(value.split('.').next().unwrap_or("").to_ascii_uppercase().as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9")
}

pub(crate) fn is_server_pack(path: &Path) -> bool {
    File::open(path).ok().and_then(|f| ZipArchive::new(f).ok())
        .is_some_and(|mut zip| zip.by_name(MANIFEST).is_ok())
}

pub(super) fn read_json<T: serde::de::DeserializeOwned, R: Read + Seek>(
    zip: &mut ZipArchive<R>, name: &str,
) -> Result<T, String> {
    let file = zip.by_name(name).map_err(|_| err_keys::SERVER_PACK_INVALID.to_string())?;
    if file.size() > 4 * 1024 * 1024 { return Err(err_keys::SERVER_PACK_INVALID.into()); }
    serde_json::from_reader(file).map_err(|_| err_keys::SERVER_PACK_INVALID.into())
}

pub(super) fn validate<R: Read + Seek>(zip: &mut ZipArchive<R>) -> Result<Manifest, String> {
    let bad = || err_keys::SERVER_PACK_INVALID.to_string();
    let manifest: Manifest = read_json(zip, MANIFEST)?;
    if manifest.format != "mml-server-pack" || manifest.version != 1 || manifest.instances.is_empty()
        || manifest.instances.len() > 256 || zip.len() > 200_000 { return Err(bad()); }
    let mut ids = HashSet::new();
    let mut dirs = HashSet::new();
    for instance in &manifest.instances {
        if instance.uuid.is_nil() || !ids.insert(instance.uuid) || !safe_component(&instance.directory)
            || !dirs.insert(instance.directory.to_lowercase()) { return Err(bad()); }
        let game: mml_game::launcher::instance_setting_obj::InstanceSettingObj =
            read_json(zip, &format!("{INSTANCES}{}/game.json", instance.directory))?;
        if game.uuid != instance.uuid || game.dir != instance.directory { return Err(bad()); }
    }
    let mut entries = HashSet::new();
    let mut total = 0u64;
    for index in 0..zip.len() {
        let file = zip.by_index(index).map_err(|_| bad())?;
        let name = file.name().map_err(|_| bad())?;
        let name = name.as_ref();
        if !name.trim_end_matches('/').split('/').all(safe_component)
            || !entries.insert(name.trim_end_matches('/').to_lowercase())
            || file.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) { return Err(bad()); }
        let instance_file = manifest.instances.iter().any(|i| name.starts_with(&format!("{INSTANCES}{}/", i.directory)));
        let directory = file.is_dir() && (INSTANCES.starts_with(name)
            || manifest.instances.iter().any(|i| format!("{INSTANCES}{}/", i.directory).starts_with(name)));
        if !(instance_file || directory || matches!(name, MANIFEST | CONFIG | GUI_CONFIG)
            || (name == LAUNCHER && manifest.launcher)) { return Err(bad()); }
        total = total.checked_add(file.size()).ok_or_else(bad)?;
        if total > 128 * 1024 * 1024 * 1024 { return Err(bad()); }
    }
    if manifest.launcher != entries.contains(&LAUNCHER.to_lowercase()) { return Err(bad()); }
    Ok(manifest)
}
