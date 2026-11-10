//! Modrinth 整合包内文件解析

use super::*;

/// 获取整合包模组信息
///
/// - `path`: 整合包解压目录
/// - `info`: 整合包信息
/// - `pack_gui`: 整合包安装界面回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 返回文件下载项列表与在线信息表；取消返回 `TaskCancel`
pub async fn get_mod_info<P: AsRef<Path>>(
    path: P,
    info: &ModrinthPackObj,
    pack_gui: AddModPackGui,
    cancel: CancellationToken,
) -> CoreResult<DownloadItemRes> {
    let mut list = Vec::new();
    let mut mods = HashMap::new();

    let size = info.files.len();
    let mut now = 0usize;

    let mut hash = HashMap::new();

    for item in info.files.iter() {
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let item1 = make_pack_download_obj(item, &path);
        let url = item
            .downloads
            .iter()
            .filter(|item| item.starts_with(&format!("{}data/", urls::MODRINTH_DOWNLOAD)))
            .next();

        let sha1 = item.hashes.sha1.clone();
        let path = tools::get_path_part(&item.path);
        // 有隐藏信息
        if let Some(data) = &item.project {
            mods.remove(&data.pid);
            mods.insert(
                data.pid.clone(),
                OnlineInfoObj {
                    path: path.parent,
                    name: path.file.clone(),
                    file: path.file,
                    sha1,
                    url: item1.url.clone(),
                    modid: data.pid.clone(),
                    fileid: data.fid.clone(),
                },
            );
        } else if let Some(url) = url {
            // 是modr的标准下载地址
            let modid = tools::get_string(url, "data/", "/ver");
            let fileid = tools::get_string(url, "versions/", "/");

            mods.remove(&modid);
            mods.insert(
                modid.clone(),
                OnlineInfoObj {
                    path: path.parent,
                    name: path.file.clone(),
                    file: path.file,
                    sha1,
                    url: item1.url.clone(),
                    modid,
                    fileid,
                },
            );
        } else {
            // 尝试从hash获取版本信息
            hash.insert(
                item.hashes.sha512.clone(),
                OnlineInfoObj {
                    path: path.parent,
                    name: path.file.clone(),
                    file: path.file,
                    sha1,
                    url: item1.url.clone(),
                    ..Default::default()
                },
            );
        }

        now += 1;

        if let Some(gui) = &pack_gui {
            gui.set_sub_now(now, Some(size));
        }

        list.push(item1);
    }

    if !hash.is_empty() {
        let ids = hash.keys().cloned().collect();
        let data = modrinth_api::get_version_from_sha512(ids).await;
        if let Ok(data) = data {
            for (key, value) in data {
                let mut temp = hash.remove(&key).unwrap();
                temp.modid = value.project_id;
                temp.fileid = value.id;

                mods.remove(&temp.modid);
                mods.insert(temp.modid.clone(), temp);
            }
        }
    }

    Ok(DownloadItemRes { list, online: mods })
}
