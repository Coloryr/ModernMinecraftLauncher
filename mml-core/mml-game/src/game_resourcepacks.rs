//! 游戏实例资源包相关
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};

use mml_base::{
    file_item::FileHash,
    hash_helper::{self, HashType},
    serialize_tools::{self, MiniJsonMap, MiniJsonObj},
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorData, ErrorType, FileSystemErrorData},
    names,
};
use mml_sys::path_helper;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use zip::ZipArchive;

use crate::launcher::instance_setting_obj::InstanceSettingObj;

/// 解析 `pack.mcmeta` 的 JSON，取出版本区间与简介
///
/// **全程按形状取值，不做结构性反序列化**：原先那套 `PackMeta` / `PackInfo` /
/// `SupportedFormats` 结构体已经删掉了。原因是那三个字段的写法太多，简介有字符串 /
/// 数组 / 对象三种，版本字段有数字 / 区间数组 / 区间对象三种，只要有一种没枚举到，
/// 整个结构体就解不出来，`process_resourcepack` 走失败分支、界面上整包显示"读取失败"。
/// 这个坑真实发生过两次：数据包的组件数组、Slightly Improved Font 的区间数组。
///
/// 现在任何一段写法不认识，最多让**那一段**回落默认值，不影响整包。
///
/// - `meta`: `pack.mcmeta` 的 JSON
/// - `lang`: 资源包自带的语言表，见 [`read_lang_map`]，用于翻简介里的 `translate` 键
///
/// # 返回值
///
/// 返回只填了简介与三个版本字段的资源包信息；`pack` 段不是对象时返回 `None`，
/// 调用方按"读取失败"处理
fn parse_pack_meta(meta: MiniJsonObj, lang: &HashMap<String, String>) -> Option<ResourcepackObj> {
    let pack = meta.as_object()?.get_object("pack")?;

    // 版本区间：显式给的 `min_format` / `max_format` 优先，两者都没写时才用
    // `supported_formats`，这是 1.20.5+ 的新写法，别让它盖掉显式给的旧字段
    let format_range = pack
        .get("supported_formats")
        .map(supported_formats_range)
        .unwrap_or_default();
    let min_format = pack.get("min_format").and_then(number_or_min).unwrap_or(0);
    let max_format = pack.get("max_format").and_then(number_or_max).unwrap_or(0);

    Some(ResourcepackObj {
        description: pack
            .get("description")
            .map(|value| flatten_text(value, lang))
            .unwrap_or_default(),
        pack_format: pack.get("pack_format").and_then(number_or_max).unwrap_or(0),
        min_format: if min_format != 0 {
            min_format
        } else {
            format_range.min
        },
        max_format: if max_format != 0 {
            max_format
        } else {
            format_range.max
        },
        ..Default::default()
    })
}

/// `pack_format` / `min_format` / `max_format` 的取值：数字直接用，**数组取最小值**
///
/// 这三个字段都可能被写成区间数组，真实样本如 Mod Menu Helper 的
/// `"min_format":[97,1]`，口径与 `serialize_tools::deserialize_number_or_min` 一致。
///
/// - `value`: 字段的 JSON 值
///
/// # 返回值
///
/// 返回取到的数字；既不是数字也不是数字数组时返回 `None`
fn number_or_min(value: &MiniJsonObj) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }

    value
        .as_list()?
        .iter()
        .filter_map(|item| item.as_i64())
        .min()
}

/// 同上，**数组取最大值**，口径同 `serialize_tools::deserialize_number_or_max`
///
/// - `value`: 字段的 JSON 值
///
/// # 返回值
///
/// 返回取到的数字；既不是数字也不是数字数组时返回 `None`
fn number_or_max(value: &MiniJsonObj) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }

    value
        .as_list()?
        .iter()
        .filter_map(|item| item.as_i64())
        .max()
}

/// `supported_formats` 的**三种**写法 → 资源包格式版本区间
///
/// 三种都在真实资源包里见过，前两种当初都是"整包读取失败"报上来的：
/// - 对象：`{"min_inclusive": 5, "max_inclusive": 75}`，两个键都可选，只写 min
///   表示"这一版及以后"
/// - 数组：`[8, 9999]`，就是 `[min, max]`
/// - 单个数字：`20`，表示"从这一版起"
///
/// 缺的那一侧按"未知"给 `0`。
#[derive(Default)]
struct FormatRange {
    /// 区间下界，即最早的资源包格式版本；未知时为 `0`
    min: i64,
    /// 区间上界，即最晚的资源包格式版本；未知时为 `0`
    max: i64,
}

/// 解析 `supported_formats`，见 [`FormatRange`]
///
/// - `value`: `supported_formats` 的 JSON 值
///
/// # 返回值
///
/// 见 [`FormatRange`]；缺的一侧为 `0`
fn supported_formats_range(value: &MiniJsonObj) -> FormatRange {
    // 对象写法：两个键都可选
    if let Some(map) = value.as_object() {
        return FormatRange {
            min: map
                .get("min_inclusive")
                .and_then(number_or_min)
                .unwrap_or(0),
            max: map
                .get("max_inclusive")
                .and_then(number_or_max)
                .unwrap_or(0),
        };
    }

    // 数组 `[min, max]`；只给一个元素时等价于"从这一版起"，空数组当"没给"
    if let Some(list) = value.as_list() {
        let numbers: Vec<i64> = list.iter().filter_map(|item| item.as_i64()).collect();
        return FormatRange {
            min: numbers.first().copied().unwrap_or(0),
            max: numbers.get(1).copied().unwrap_or(0),
        };
    }

    // 单个数字写法：`"supported_formats": 20`
    FormatRange {
        min: value.as_i64().unwrap_or(0),
        max: 0,
    }
}

/// 把文本组件的 JSON 值抠成一句能显示的文字，简介要的就是这个
///
/// 按形状分派，[`MiniJsonObj`] 的取值方法对不上的形状一律返回 `None`，所以数字 /
/// 布尔 / `null` 自然落到最后一条：
/// - **字符串** —— 本身就是文字；
/// - **数组** —— 每个元素各自展平后按顺序拼接，多个组件合起来就是一段话；
/// - **对象** —— 文本组件，见 [`tran_text`]；
/// - **其余** —— 给空串。
///
/// 最后一条是有意的：这些形状**不能**退回去 `to_string()` 成 JSON，那样简介里会
/// 冒出 `null` / `true` 这种字面量，纯属噪音，`serde_json::Value` 的 `to_string()`
/// 就是坑在这。
///
/// - `value`: 文本组件的 JSON 值
/// - `lang`: 资源包自带的语言表，见 [`read_lang_map`]，只用于翻 `translate` 的 key
///
/// # 返回值
///
/// 返回展平后的纯文字；不认识的形状返回空串
fn flatten_text(value: &MiniJsonObj, lang: &HashMap<String, String>) -> String {
    if let Some(text) = value.as_string() {
        return text;
    }

    if let Some(list) = value.as_list() {
        return list.iter().map(|item| flatten_text(item, lang)).collect();
    }

    if let Some(map) = value.as_object() {
        return tran_text(&map, lang);
    }

    String::new()
}

/// 文本组件对象 → 一句纯文本
///
/// 取文字的优先级：
/// 1. `text` —— 直接写死的文字；
/// 2. `translate` —— **先查资源包自带的语言表**，查不到用同级 `fallback`，
///    连 `fallback` 都没有才把 key 原样显示出来；
/// 3. 单写的 `fallback`。
///
/// 最后把 `extra` 里的子组件**追加**在主文字之后，可以嵌多层。
///
/// | 写法 | 语言表命中 | 结果 |
/// | --- | --- | --- |
/// | `{"text": "一句话"}` | — | `一句话` |
/// | `{"translate": "k", "fallback": "兜底"}` | 命中 → `译文` | `译文` |
/// | `{"translate": "k", "fallback": "兜底"}` | 未命中 | `兜底` |
/// | `{"translate": "k"}` | 未命中 | `k` |
/// | `{"text": "A", "extra": [{"text": "B"}]}` | — | `AB` |
///
/// 这个顺序与游戏里的口径一致：key 命中就用译文，否则 fallback，再否则显示 key 本身。
/// 裸 key 不好看，但比留空强，至少能看出这是个没翻出来的条目；而**绝不能**把整个
/// 组件 `to_string()` 成 JSON 塞进简介。
///
/// `keybind` / `score` / `selector` / `nbt` 这类只有客户端渲染得出来的组件
/// **不给文字**，留空，不过它们自己的 `extra` 照样会接着处理。
///
/// - `map`: 文本组件对象的键值对
/// - `lang`: 资源包自带的语言表，见 [`read_lang_map`]
///
/// # 返回值
///
/// 返回展平后的纯文字；只有客户端组件时为空串
fn tran_text(map: &MiniJsonMap, lang: &HashMap<String, String>) -> String {
    let mut out = if let Some(text) = map.get("text") {
        flatten_text(text, lang)
    } else if let Some(key) = map.get("translate").and_then(|item| item.as_string()) {
        // 优先级：包自带语言表 → 同级 `fallback` → key 本身
        match lang.get(&key) {
            Some(text) => text.clone(),
            None => match map.get("fallback") {
                Some(fallback) => flatten_text(fallback, lang),
                None => key,
            },
        }
    } else if let Some(fallback) = map.get("fallback") {
        flatten_text(fallback, lang)
    } else {
        String::new()
    };

    // `extra` 是**追加**在主文字之后的子组件，可以嵌多层
    if let Some(extra) = map.get("extra") {
        out.push_str(&flatten_text(extra, lang));
    }

    out
}

/// 这个条目是不是资源包自带的某个语言表：`assets/<命名空间>/lang/<语言代码>.json`
///
/// 命名空间是包自己起的，如 `fo`、`sodium`、`minecraft`，**与 key 长什么样无关**：
/// 翻译键归哪个命名空间，由它写在哪个文件里决定。
///
/// - `name`: zip 里的条目名
/// - `code`: 语言代码，如 `zh_cn`
///
/// # 返回值
///
/// 命中返回 `true`
fn is_pack_lang_file(name: &str, code: &str) -> bool {
    let parts: Vec<&str> = name.split('/').collect();

    // 正好四段：多一层少一层都不是语言表根目录下的文件
    parts.len() == 4
        && parts[0] == "assets"
        && !parts[1].is_empty()
        && parts[2] == "lang"
        && parts[3].strip_suffix(".json") == Some(code)
}

/// 读资源包自带的语言表，即 `assets/<命名空间>/lang/<语言代码>.json` 里的键值对
///
/// 简介里的 `translate` 键就靠它翻，语言表是**包自己带的**，所以这条完全离线，
/// 不需要客户端 jar、也不需要资源索引。
///
/// 多命中几个命名空间时逐个合并，同一个 key 不该重名，真重了以后读到的为准。
/// 语言代码用界面语言 `zh_cn` / `en_us`，与游戏的语言代码同名；**该语言没有就用
/// `en_us` 兜底**，包的作者一般都会写英文，比拿不到译文去显示裸 key 强。
///
/// - `zip`: 已打开的资源包
/// - `code`: 语言代码
///
/// # 返回值
///
/// 返回翻译键到译文的映射；这个语言连同 `en_us` 兜底都没有时返回空表
fn read_lang_map<Z: Read + std::io::Seek>(
    zip: &mut ZipArchive<Z>,
    code: &str,
) -> HashMap<String, String> {
    let mut map = HashMap::new();

    // 先按界面语言找，找不到再按英文找
    for code in [code, "en_us"] {
        // `file_names()` 借着 `zip` 不放，先把命中的条目名收下来，再逐个读，读要可变借用
        let names: Vec<String> = zip
            .file_names()
            .map(|item| item.unwrap_or_default())
            .filter(|name| is_pack_lang_file(&name.to_string(), code))
            .map(|name| name.to_string())
            .collect();

        for name in names {
            let Ok(mut file) = zip.by_name(&name) else {
                continue;
            };
            let mut text = String::new();
            if file.read_to_string(&mut text).is_err() {
                continue;
            }
            // 译文以外的值，如数字、嵌套对象，直接丢掉：解成 `Value` 才不会因为一个怪值
            // 让整份语言表作废
            let Ok(part) =
                serialize_tools::json_from_str::<HashMap<String, serde_json::Value>>(&text)
            else {
                continue;
            };
            map.extend(
                part.into_iter()
                    .filter_map(|(key, value)| Some((key, value.as_str()?.to_string()))),
            );
        }

        if !map.is_empty() {
            break;
        }
    }

    map
}

/// 资源包信息
pub struct ResourcepackObj {
    /// 来自 pack.mcmeta 的简介，已展平成纯文字
    pub description: String,
    /// `pack_format` 声明的版本号，缺失为 0
    pub pack_format: i64,
    /// 支持的起始版本号，缺失为 0
    pub min_format: i64,
    /// 支持的结束版本号，缺失为 0
    pub max_format: i64,
    /// 文件校验，同时记录 sha1 与 sha256
    pub hash: FileHash,
    /// 资源包文件或目录的完整路径
    pub path: PathBuf,
    /// pack.png 的原始字节，没有图标时为 `None`
    pub icon: Option<Vec<u8>>,
    /// 是否读取失败，失败时界面按"读取失败"展示
    pub fail: bool,
    /// 是否已在 options.txt 的 `resourcePacks` 里启用
    pub enable: bool,
}

impl Default for ResourcepackObj {
    /// 文本与版本号为缺省值，`icon` 为 `None`，`fail` 与 `enable` 为 `false`
    fn default() -> Self {
        Self {
            description: Default::default(),
            pack_format: Default::default(),
            min_format: Default::default(),
            max_format: Default::default(),
            hash: Default::default(),
            path: Default::default(),
            icon: Default::default(),
            fail: Default::default(),
            enable: Default::default(),
        }
    }
}

/// 解析材质包
///
/// - `path`: 材质包文件路径，zip 格式
/// - `lang`: 界面语言代码，如 `zh_cn`、`en_us`；简介里的 `translate` 键按它查
///   资源包自带的语言表，见 [`read_lang_map`]
///
/// # 返回值
///
/// 返回材质包信息；打开或读取失败返回对应错误
pub fn process_resourcepack<P: AsRef<Path>>(path: P, lang: &str) -> CoreResult<ResourcepackObj> {
    let file = path_helper::open_read(&path)?;
    let mut zip = ZipArchive::new(file).map_err(|err| {
        ErrorType::ArchiveOpenError(FileSystemErrorData {
            path: path.as_ref().to_path_buf(),
            error: err.to_string(),
        })
    })?;

    // 语言表要在解 pack.mcmeta **之前**读：简介里的 `translate` 键拿它才翻得出来，
    // 而 `zip.by_name` 的可变借用要一直持续到结构体解完，中间插不进去
    let lang_map = read_lang_map(&mut zip, lang);

    // 解析 pack.mcmeta
    let mut pack = {
        let meta = zip.by_name(names::PACK_META_FILE).map_err(|err| {
            ErrorType::ArchiveReadError(ErrorData {
                error: err.to_string(),
            })
        })?;

        // 按形状取值，走 `MiniJsonMap`，不做结构性反序列化：写法认不出来也只丢那一段
        match MiniJsonObj::from_stream(meta)
            .ok()
            .and_then(|json| parse_pack_meta(json, &lang_map))
        {
            Some(obj) => obj,
            None => ResourcepackObj {
                fail: true,
                ..Default::default()
            },
        }
    };

    // 读取图标
    if let Ok(mut icon) = zip.by_name(names::PACK_ICON_FILE) {
        let size = icon.size() as usize;
        let mut vec = Vec::with_capacity(size);
        icon.read_to_end(&mut vec).map_err(|err| {
            ErrorType::ArchiveReadError(ErrorData {
                error: err.to_string(),
            })
        })?;
        pack.icon = Some(vec);
    }

    Ok(pack)
}

impl ResourcepackObj {
    /// 删除
    ///
    /// # 返回值
    ///
    /// 移入回收站失败时返回对应错误
    pub fn remove(&self) -> CoreResult<()> {
        path_helper::move_to_trash(&self.path)
    }
}

impl InstanceSettingObj {
    /// 获取资源包列表
    ///
    /// - `lang`: 界面语言代码，如 `zh_cn`、`en_us`，用于翻简介里的 `translate` 键
    ///
    /// # 返回值
    ///
    /// 返回资源包列表；读取失败的文件会记录日志并标记 `fail`
    pub async fn get_resourcepacks(&self, lang: &str) -> Vec<ResourcepackObj> {
        let dir = self.get_resourcepacks_path();
        if !dir.exists() || !dir.is_dir() {
            Default::default()
        } else {
            let files = path_helper::get_files(&dir);
            // `spawn_blocking` 要 `'static`，借用进不去，收成 `String`
            let lang = lang.to_string();
            let config = if let Ok(data) = self.get_options() {
                let def = String::from("[]");
                let data = data.get("resourcePacks").unwrap_or(&def);
                serde_json::from_str::<Vec<String>>(data).unwrap_or_default()
            } else {
                Vec::new()
            };

            tokio::task::spawn_blocking(move || {
                let list = Mutex::new(Vec::new());

                files.par_iter().for_each(|item| {
                    let sha1 = hash_helper::gen_hash_from_file(HashType::Sha1, item);
                    let sha256 = hash_helper::gen_hash_from_file(HashType::Sha256, item);

                    if sha1.is_err() || sha256.is_err() {
                        return;
                    }

                    let hash = FileHash::Sha1Sha256(sha1.unwrap(), sha256.unwrap());

                    // 是否启用：options.txt 的 `resourcePacks` 里记的是 `file/<文件名>`
                    let enabled = config.contains(&format!(
                        "file/{}",
                        item.file_name().unwrap_or_default().to_string_lossy()
                    ));

                    // 如果是压缩包
                    if let Some(ext) = item.extension()
                        && ext.eq_ignore_ascii_case(names::ZIP_EXT)
                    {
                        match process_resourcepack(item, &lang) {
                            Ok(mut obj) => {
                                obj.hash = hash;
                                obj.path = item.clone();
                                // 启用状态由 options.txt 决定，与包解没解出来无关：
                                // 读得出来的包也要填，否则界面上一律显示"未启用"
                                obj.enable = enabled;

                                list.lock().unwrap().push(obj);
                                return;
                            }
                            Err(err) => {
                                mml_log::error_type(err);
                            }
                        }
                    }

                    list.lock().unwrap().push(ResourcepackObj {
                        description: Default::default(),
                        pack_format: Default::default(),
                        min_format: Default::default(),
                        max_format: Default::default(),
                        hash,
                        path: item.clone(),
                        icon: Default::default(),
                        fail: true,
                        enable: enabled,
                    });
                });

                list.into_inner().unwrap()
            })
            .await
            .unwrap_or_default()
        }
    }

    /// 启用材质包
    ///
    /// - `pack`: 待启用的资源包
    ///
    /// # 返回值
    ///
    /// 写 options.txt 失败时返回对应错误
    pub fn enable_resourcepacks(&self, pack: &ResourcepackObj) -> CoreResult<()> {
        let mut opt = self.get_options().unwrap_or_default();

        let mut config = if let Some(data) = opt.get("resourcePacks") {
            serde_json::from_str::<Vec<String>>(data).unwrap_or_default()
        } else {
            Vec::new()
        };

        config.push(format!(
            "file/{}",
            pack.path.file_name().unwrap_or_default().to_string_lossy()
        ));

        opt.insert(
            String::from("resourcePacks"),
            serialize_tools::json_to_string(&config).unwrap_or(String::from("[]")),
        );

        self.save_options(&opt, None)?;

        Ok(())
    }

    /// 禁用材质包
    ///
    /// - `pack`: 待禁用的资源包
    ///
    /// # 返回值
    ///
    /// 写 options.txt 失败时返回对应错误
    pub fn disable_resourcepacks(&self, pack: &ResourcepackObj) -> CoreResult<()> {
        let mut opt = self.get_options().unwrap_or_default();

        let mut config = if let Some(data) = opt.get("resourcePacks") {
            serde_json::from_str::<Vec<String>>(data).unwrap_or_default()
        } else {
            Vec::new()
        };

        config.retain(|data| {
            data != &format!(
                "file/{}",
                pack.path.file_name().unwrap_or_default().to_string_lossy()
            )
        });

        opt.insert(
            String::from("resourcePacks"),
            serialize_tools::json_to_string(&config).unwrap_or(String::from("[]")),
        );

        self.save_options(&opt, None)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ResourcepackObj, flatten_text, is_pack_lang_file, parse_pack_meta};
    use crate::test_support::{TextEntry, text_entry};
    use mml_base::serialize_tools::MiniJsonObj;
    use std::collections::HashMap;

    /// 从 JSON 文本造一个 [`MiniJsonObj`]；`from_value` 是私有的，只能走字符串
    fn mini(text: &str) -> MiniJsonObj {
        MiniJsonObj::from_str(text).unwrap()
    }

    /// 空语言表：只看 `fallback` / key 这条兜底链
    fn no_lang() -> HashMap<String, String> {
        HashMap::new()
    }

    /// 造一张语言表
    fn lang(list: &[TextEntry]) -> HashMap<String, String> {
        list.iter()
            .map(|e| (e.name.to_string(), e.text.to_string()))
            .collect()
    }

    /// 字符串 / 数组 / 标量：按形状取值，**不许**漏出 `"null"` 这种字面量
    #[test]
    fn flatten_plain_shapes() {
        assert_eq!(flatten_text(&mini(r#""一句话""#), &no_lang()), "一句话");
        assert_eq!(flatten_text(&mini(r#"["A","B"]"#), &no_lang()), "AB");
        assert_eq!(flatten_text(&mini("null"), &no_lang()), "");
        assert_eq!(flatten_text(&mini("1"), &no_lang()), "");
        assert_eq!(flatten_text(&mini("true"), &no_lang()), "");
    }

    /// 对象：`text` 优先，数组按顺序拼接
    #[test]
    fn flatten_text_field() {
        assert_eq!(
            flatten_text(&mini(r#"{"text":"一句话"}"#), &no_lang()),
            "一句话"
        );
        assert_eq!(
            flatten_text(&mini(r#"[{"text":"A"},{"text":"B"}]"#), &no_lang()),
            "AB"
        );
    }

    /// `translate`：语言表命中就用译文，**压过** `fallback`；没命中才退回兜底链
    #[test]
    fn flatten_translate_resolves_from_lang() {
        let table = lang(&[text_entry("some.key", "译文")]);

        // 命中：译文优先于 `fallback`
        assert_eq!(
            flatten_text(
                &mini(r#"{"translate":"some.key","fallback":"兜底文字"}"#),
                &table
            ),
            "译文"
        );
        // 表里没这个 key：用 `fallback`
        assert_eq!(
            flatten_text(
                &mini(r#"{"translate":"other.key","fallback":"兜底文字"}"#),
                &table
            ),
            "兜底文字"
        );
        // 连 `fallback` 都没有：把 key 原样给出来
        assert_eq!(
            flatten_text(&mini(r#"{"translate":"other.key"}"#), &table),
            "other.key"
        );
    }

    /// 只写了 `fallback`，没有 `text` / `translate`，也要认
    #[test]
    fn flatten_bare_fallback() {
        assert_eq!(
            flatten_text(&mini(r#"{"fallback":"兜底文字"}"#), &no_lang()),
            "兜底文字"
        );
    }

    /// `extra` 追加在主文字之后，可以嵌多层；里面的 `translate` 查同一张表
    #[test]
    fn flatten_extra_is_appended() {
        assert_eq!(
            flatten_text(&mini(r#"{"text":"A","extra":[{"text":"B"}]}"#), &no_lang()),
            "AB"
        );
        assert_eq!(
            flatten_text(
                &mini(r#"{"text":"A","extra":[{"text":"B","extra":[{"text":"C"}]}]}"#),
                &no_lang()
            ),
            "ABC"
        );
        assert_eq!(
            flatten_text(
                &mini(r#"{"text":"A","extra":[{"translate":"k"}]}"#),
                &lang(&[text_entry("k", "译文")])
            ),
            "A译文"
        );
    }

    /// 只有客户端渲染得出来的组件：不给文字，但它们的 `extra` 照样处理
    #[test]
    fn flatten_client_only_components_keep_extra() {
        assert_eq!(
            flatten_text(&mini(r#"{"keybind":"key.jump"}"#), &no_lang()),
            ""
        );
        assert_eq!(
            flatten_text(
                &mini(r#"{"score":{"name":"a","objective":"b"}}"#),
                &no_lang()
            ),
            ""
        );
        assert_eq!(
            flatten_text(
                &mini(r#"{"keybind":"key.jump","extra":[{"text":"X"}]}"#),
                &no_lang()
            ),
            "X"
        );
    }

    /// 走一遍 `pack.mcmeta` 的解析：简介与版本区间的各种写法都要收得下
    #[test]
    fn parse_pack_meta_shapes() {
        /// 从 JSON 文本造一个资源包信息，只填 pack.mcmeta 相关字段
        ///
        /// - `meta`: `pack.mcmeta` 的 JSON 文本
        ///
        /// # 返回值
        ///
        /// 返回解析出的资源包信息，其余字段取缺省值
        fn parse(meta: &str) -> ResourcepackObj {
            parse_pack_meta(mini(meta), &no_lang()).expect("pack 段是对象就应解析成功")
        }

        // 简介：字符串 / 数组 / 对象
        assert_eq!(
            parse(r#"{"pack":{"description":"一句话"}}"#).description,
            "一句话"
        );
        // 数组形式是当初"整包读取失败"的元凶：必须解析得出来
        assert_eq!(
            parse(r#"{"pack":{"description":[{"text":"一句话"},{"text":"！"}]}}"#).description,
            "一句话！"
        );
        assert_eq!(
            parse(r#"{"pack":{"description":{"translate":"k","fallback":"兜底文字"}}}"#)
                .description,
            "兜底文字"
        );
        // 简介缺失 / 给 null / 给数字：兜空串，**不算读取失败**
        assert_eq!(parse(r#"{"pack":{}}"#).description, "");
        assert_eq!(parse(r#"{"pack":{"description":null}}"#).description, "");
        assert_eq!(parse(r#"{"pack":{"description":123}}"#).description, "");

        // `supported_formats` 的三种写法：对象 / 数组 / 单个数字
        let obj = parse(r#"{"pack":{"supported_formats":{"min_inclusive":5,"max_inclusive":75}}}"#);
        assert_eq!((obj.min_format, obj.max_format), (5, 75));
        let obj = parse(r#"{"pack":{"supported_formats":[8,9999]}}"#);
        assert_eq!((obj.min_format, obj.max_format), (8, 9999));
        let obj = parse(r#"{"pack":{"supported_formats":20}}"#);
        assert_eq!((obj.min_format, obj.max_format), (20, 0));

        // 显式给的 `min_format` / `max_format` 优先于 `supported_formats`
        let obj = parse(r#"{"pack":{"min_format":11,"max_format":22,"supported_formats":[5,75]}}"#);
        assert_eq!((obj.min_format, obj.max_format), (11, 22));

        // 版本字段写成区间数组，Mod Menu Helper 的真实写法：min 取小、max 取大
        let obj = parse(r#"{"pack":{"pack_format":18,"min_format":[97,1],"max_format":[97,1]}}"#);
        assert_eq!(obj.pack_format, 18);
        assert_eq!((obj.min_format, obj.max_format), (1, 97));

        // `pack` 段根本不存在 / 不是对象 —— 这才算读不出来
        assert!(parse_pack_meta(mini(r#"{"pack":"x"}"#), &no_lang()).is_none());
        assert!(parse_pack_meta(mini("{}"), &no_lang()).is_none());
    }

    /// 语言表条目名的判定：正好四段、命名空间非空、只认 `.json`
    #[test]
    fn pack_lang_file_shape() {
        assert!(is_pack_lang_file("assets/fo/lang/zh_cn.json", "zh_cn"));
        assert!(is_pack_lang_file(
            "assets/minecraft/lang/en_us.json",
            "en_us"
        ));

        // 语言代码不同、层数不对、后缀不对 —— 都不算
        assert!(!is_pack_lang_file("assets/fo/lang/zh_cn.json", "en_us"));
        assert!(!is_pack_lang_file("assets/fo/lang/zh_cn.json.bak", "zh_cn"));
        assert!(!is_pack_lang_file("assets/fo/lang/sub/zh_cn.json", "zh_cn"));
        assert!(!is_pack_lang_file("assets/lang/zh_cn.json", "zh_cn"));
        assert!(!is_pack_lang_file("fo/lang/zh_cn.json", "zh_cn"));
        assert!(!is_pack_lang_file("assets/fo/zh_cn.json", "zh_cn"));
        // zip 里的目录条目也不是语言表，真实包里就有 `assets/fo/lang/` 这么一条
        assert!(!is_pack_lang_file("assets/fo/lang/", "zh_cn"));
    }
}
