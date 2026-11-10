//! 实例在线信息的自动标记

use super::*;

impl InstanceSettingObj {
    /// 自动标记模组
    ///
    /// - `over`: 是否覆盖已经标记的模组
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；查询或保存失败返回对应错误
    pub async fn auto_mark(&self, over: bool) -> CoreResult<()> {
        let list = self.read_mod_fast(None).await;
        let mut online = self.read_online_info();

        let hashs: Vec<String> = online
            .values()
            .map(|data| data.sha1.clone().to_ascii_lowercase())
            .collect();

        let mut check = Vec::new();
        for item in list.iter() {
            let sha1 = item.hash.get_sha1().unwrap();
            if hashs.contains(&sha1) && !over {
                continue;
            }

            check.push(sha1);
        }

        if !check.is_empty() {
            let list = modrinth_api::get_version_from_sha1(check).await?;

            for (_, value) in list {
                online.remove(&value.project_id);
                let file = &value.files[0];
                online.insert(
                    value.project_id.clone(),
                    OnlineInfoObj {
                        path: names::GAME_MODS_DIR.to_string(),
                        name: value.name,
                        file: file.filename.clone(),
                        sha1: file.hashes.sha1.clone(),
                        url: file.url.clone(),
                        modid: value.project_id,
                        fileid: value.id,
                    },
                );
            }

            self.save_online_info(&online);
        }

        Ok(())
    }
}
