use super::*;

impl InstanceSettingObj {
    /// 替换启动参数中的占位符
    ///
    /// - `auth`: 使用的账户
    /// - `args`: 需要替换的参数列表
    /// - `classpath`: 运行库列表
    /// - `native`: 本地库路径
    /// - `assets`: 资源文件
    pub(super) fn replace_all_value(
        &self,
        auth: &LoginObj,
        args: &mut Vec<String>,
        classpath: &str,
        native: &Path,
        assets: &Option<String>,
    ) {
        let assets_path = assets_path::get_assets_dir();
        let game_dir = self.get_game_path();
        let assets_index = assets.clone().unwrap_or_else(|| "legacy".to_string());

        let version_name = match self.loader {
            LoaderType::Normal => self.version.clone(),
            LoaderType::Custom => "custom".to_string(),
            _ => format!(
                "{}-{}-{}",
                self.loader.id(),
                self.version,
                self.loader_version.as_ref().unwrap()
            ),
        };

        let sep = if mml_sys::get_system_info().os == Os::Windows {
            ';'
        } else {
            ':'
        };

        let token = if auth.access_token.is_empty() {
            "0".to_string()
        } else {
            auth.access_token.clone()
        };

        // 只能给空对象：游戏按版本把它读成不同的类型 —— 1.7.10 是
        // Map<String, Collection<String>>，1.8.x 是 authlib 的 PropertyMap，
        // 塞一个字符串进去会直接 JsonSyntaxException 崩在 Main。
        // 语言走 options.txt（启动前由 ensure_game_lang 写好），与这个参数无关
        let user_properties = "{}".to_string();

        let user_type = if auth.auth_type == AuthType::OAuth {
            "msa"
        } else {
            "mojang"
        };

        let mut map = HashMap::new();
        // 键必须带上 `${}` 里那个 `$`：版本 json 原文就是 `${auth_player_name}`，
        // 少了 `$` 只会替换花括号那一段，替换完留下一个裸 `$`（实测会拼出
        // `-cp $E:\...\client.jar`、`--username $`，游戏直接起不来）
        map.insert("${auth_player_name}", auth.user_name.clone());
        map.insert("${version_name}", version_name);
        map.insert("${game_directory}", game_dir.to_string_lossy().to_string());
        map.insert("${assets_root}", assets_path.to_string_lossy().to_string());
        map.insert("${assets_index_name}", assets_index);
        map.insert("${auth_uuid}", auth.uuid.clone());
        map.insert("${auth_access_token}", token);
        map.insert("${user_properties}", user_properties);
        map.insert("${user_type}", user_type.to_string());
        map.insert("${version_type}", self.get_version_type());
        map.insert("${natives_directory}", native.to_string_lossy().to_string());
        map.insert(
            "${library_directory}",
            libraries_path::get_lib_dir().to_string_lossy().to_string(),
        );
        map.insert("${classpath_separator}", sep.to_string());
        map.insert("${launcher_name}", names::MML.to_string());
        map.insert("${launcher_version}", mml_names::VERSION.clone());
        map.insert("${classpath}", classpath.to_string());
        // 这两个是微软账户的客户端 id 与 xuid，离线 / 外置账户没有对应值，置空即可 ——
        // 留着 `${clientid}` 字面量会被游戏当成真的 id
        map.insert("${clientid}", String::new());
        map.insert("${auth_xuid}", String::new());

        for index in 0..args.len() {
            for (key, value) in map.iter() {
                args[index] = args[index].replace(key, value);
            }
        }
    }
}
