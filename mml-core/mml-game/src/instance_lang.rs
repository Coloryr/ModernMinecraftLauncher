//! 实例游戏内语言
//!
//! 从版本资源索引里列 `minecraft/lang/*.json` 与旧版 `*.lang`，读出 `language.name`；
//! 资源对象与本地语言文件缓存两个来源都试，读到的名字按语言代码缓存。

use std::{
    collections::HashMap,
    path::Path,
    sync::{LazyLock, RwLock},
};

use mml_names::names;
use mml_sys::path_helper;
use uuid::Uuid;

use crate::get_instance;
use crate::launcher_path::{assets_path, version_path};

/// 游戏内语言，语言代码加显示名
pub struct InstanceLangObj {
    /// 语言代码：资源索引里 `minecraft/lang/<code>.json` 的 `<code>`，如 `zh_cn`
    pub code: String,
    /// 显示名：该语言文件里的 `language.name`，如“简体中文”；读不到时为语言代码
    pub name: String,
}

/// 获取实例的游戏内语言列表，从资源索引里查 `minecraft/lang/*.json`
///
/// 1.7.10 及更早的语言文件是 `minecraft/lang/*.lang`（代码大写，如 `zh_CN`），
/// 1.13 起才改成 `*.json`（代码小写）；两种都收，代码原样保留 —— 写进 `options.txt`
/// 的 `lang:` 必须和游戏认的那个完全一致。
///
/// 每项的显示名就写在该语言文件里，即 `language.name`，如 zh_CN 对应“简体中文”，
/// 与游戏内语言菜单同一份数据；资源没下全或文件里没有这个键时回落为语言代码本身。
///
/// 资源索引未下载或版本数据缺失时返回空列表，由前端回退默认语言，即中文与英文。
///
/// - `uuid`: 实例 UUID
///
/// # 返回值
///
/// 返回按语言代码排序的语言列表，每项含代码与显示名
pub fn get_instance_langs(uuid: &Uuid) -> Vec<InstanceLangObj> {
    let Some(instance) = get_instance(uuid) else {
        return Vec::new();
    };
    let version = instance.read().unwrap().version.clone();
    let mut langs: Vec<InstanceLangObj> = lang_entries(&version)
        .into_iter()
        .map(|entry| {
            let name = read_lang_name(&entry.hash, &entry.code, entry.legacy)
                .unwrap_or_else(|| entry.code.clone());
            InstanceLangObj {
                code: entry.code,
                name,
            }
        })
        .collect();
    langs.sort_by(|a, b| a.code.cmp(&b.code));
    langs
}

/// 资源索引里的一条语言
struct LangEntry {
    /// 游戏认的语言代码，旧版大写（`zh_CN`），新版小写（`zh_cn`）
    code: String,
    /// 语言文件的资源哈希
    hash: String,
    /// 是否旧版 `.lang` 文本（1.7.10 及更早）
    legacy: bool,
}

/// 从版本的资源索引里列出全部语言：`minecraft/lang/*.json` 与 `*.lang` 都要收
///
/// - `version`: 游戏版本号
///
/// # 返回值
///
/// 语言条目列表；索引未下载或版本数据缺失时为空
fn lang_entries(version: &str) -> Vec<LangEntry> {
    let Ok(obj) = version_path::get_version(version) else {
        return Vec::new();
    };
    let Some(index) = &obj.asset_index else {
        return Vec::new();
    };
    let Ok(assets) = assets_path::get_index(index) else {
        return Vec::new();
    };
    assets
        .objects
        .iter()
        .filter_map(|(key, item)| {
            let name = key.strip_prefix(names::LANG_KEY1)?;
            let (code, legacy) = if let Some(code) = name.strip_suffix(".json") {
                (code, false)
            } else if let Some(code) = name.strip_suffix(".lang") {
                (code, true)
            } else {
                return None;
            };
            Some(LangEntry {
                code: code.to_string(),
                hash: item.hash.clone(),
                legacy,
            })
        })
        .collect()
}

/// 在版本支持的语言里找与给定语言（忽略大小写）匹配的代码
///
/// 同一种语言在旧版是 `zh_CN`、新版是 `zh_cn`，写进 `options.txt` 必须是该版本认的那个。
///
/// - `version`: 游戏版本号
/// - `want`: 期望语言，如 `zh_CN` / `zh_cn`
///
/// # 返回值
///
/// 游戏实际使用的代码；版本数据或索引取不到时返回 `None`
pub fn game_lang_code(version: &str, want: &str) -> Option<String> {
    lang_entries(version)
        .into_iter()
        .map(|entry| entry.code)
        .find(|code| code.eq_ignore_ascii_case(want))
}

/// 已解析出的语言显示名缓存，键为语言代码
///
/// 打开一次实例设置要把上百个语言文件的 `language.name` 读出来，单个 500KB 上下，
/// 而这份名单在所有实例之间是共用的，读到的名字缓存起来，之后再开就直接命中。
/// **只缓存真读到名字的项**：回落成代码的不进缓存，等资源下全了还能再补上。
static LANG_NAMES: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 从资源对象里读出语言文件的显示名，即 `language.name`
///
/// - `hash`: 语言文件在资源索引里的 sha1
/// - `code`: 语言代码，缓存命中时直接返回
/// - `legacy`: 是否旧版 `.lang` 文本（1.7.10 及更早）
///
/// # 返回值
///
/// 返回显示名；两处都读不到、解析失败或没有该键时返回 `None`
fn read_lang_name(hash: &str, code: &str, legacy: bool) -> Option<String> {
    if let Some(name) = LANG_NAMES.read().unwrap().get(code) {
        return Some(name.clone());
    }

    // 两个来源按可靠性排序：一是游戏资源对象 `assets/…/objects/<sha1>`，与游戏内语言菜单
    // 同一来源，资源下全时最权威；二是本地语言文件缓存 `langs/<code>.json`，
    // mml-tex-draw 从客户端 jar 提取或按资源索引下载，方块与物品名单也用它。
    // **玩家还没启动过游戏时资源对象是空的**，就得靠后者，否则下拉里只会看到一堆 zh_cn 这样的代码。
    let name = read_object_lang_name(hash, legacy).or_else(|| read_cached_lang_name(code))?;
    LANG_NAMES
        .write()
        .unwrap()
        .insert(code.to_string(), name.clone());
    Some(name)
}

/// 从游戏资源对象里读语言名
///
/// 对象按 sha1 前两位分目录存放。**读不到就静默返回 None**，不走
/// [`assets_path::read_assets_text`]，那个失败时会记一条错误日志，而这里一次要读上百个
/// 语言文件，资源没下全就会把日志刷满。
fn read_object_lang_name(hash: &str, legacy: bool) -> Option<String> {
    if hash.len() < 2 {
        return None;
    }
    let file = assets_path::get_obj_dir().join(&hash[..2]).join(hash);
    lang_name_from_file(&file, legacy)
}

/// 从 mml-tex-draw 的语言文件缓存 `langs/<code>.json` 里读语言名
///
/// 旧版代码是大写（`zh_CN`），缓存按小写文件名存，找不到时再试小写一次。
fn read_cached_lang_name(code: &str) -> Option<String> {
    let dir = mml_base::get_base_dir().join(names::LANG_DIR);
    let mut names = vec![format!("{code}.json")];
    let lower = code.to_lowercase();
    if lower != code {
        names.push(format!("{lower}.json"));
    }
    names
        .iter()
        .find_map(|name| lang_name_from_file(&dir.join(name), false))
}

/// 读一个语言文件里的 `language.name`
///
/// 新版是 JSON（`{"language":{"name":"简体中文"}}`），旧版是 `key=value` 文本
/// （`language.name=简体中文`）。
///
/// - `file`: 语言文件路径
/// - `legacy`: 是否旧版 `.lang` 文本
///
/// # 返回值
///
/// 返回显示名；文件不存在、解析失败或没有该键时返回 `None`
fn lang_name_from_file(file: &Path, legacy: bool) -> Option<String> {
    if !file.is_file() {
        return None;
    }
    let text = path_helper::read_text(file).ok()?;
    if legacy {
        return text
            .lines()
            .find_map(|line| line.strip_prefix("language.name=").map(str::trim))
            .filter(|value| !value.is_empty())
            .map(String::from);
    }
    // 按 Value 解析而不是直接 HashMap<String, String>：语言文件里万一有个非字符串的值，
    // 整份解析就废了，而这里只关心 language.name 一项，别被别处带崩
    let map = mml_base::serialize_tools::json_from_str::<HashMap<String, serde_json::Value>>(&text)
        .ok()?;
    map.get(names::LANG_NAME_KEY)
        .and_then(|item| item.as_str())
        .map(String::from)
}

#[cfg(test)]
mod lang_tests {
    use super::*;

    /// 旧版 `.lang` 是 key=value 文本，语言名在 `language.name` 这一行；
    /// 同一份文件按新版 JSON 解析必须失败，两种格式不能混着猜
    #[test]
    fn legacy_lang_file_is_key_value_text() {
        let dir = mml_testutil::temp_dir().join(format!("mml-lang-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("zh_CN.lang");
        std::fs::write(&file, "language.name=简体中文\nlanguage.region=中国\n").unwrap();

        assert_eq!(
            lang_name_from_file(&file, true).as_deref(),
            Some("简体中文")
        );
        assert_eq!(lang_name_from_file(&file, false), None);
    }
}
