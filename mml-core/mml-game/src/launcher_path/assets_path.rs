//! 游戏资源 assets 目录

use std::{
    fs::File,
    io::Cursor,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use mml_auth::LoginObj;
use mml_base::{
    hash_helper::{self, HashType},
    serialize_tools,
};
use mml_names::{i18_items::error_type::CoreResult, names};
use mml_sys::path_helper;

use crate::mojang::{
    assets_obj::AssetsObj,
    game_arg_obj::{GameArgObj, GameAssetIndexObj},
};

/// assets 根目录，初始化时固定
static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 资源对象目录，按 SHA1 分桶存放
static OBJECTS_DIR: OnceLock<PathBuf> = OnceLock::new();
/// 资源索引目录，每个版本一份 json
static INDEX_DIR: OnceLock<PathBuf> = OnceLock::new();
/// 皮肤与披风目录，按 SHA1 分桶存放
static SKIN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化 assets 目录结构
///
/// - `dir`: 启动器运行目录，其下的 assets 目录作为根
///
/// # 返回值
///
/// 成功返回 `Ok(())`；创建目录失败返回对应错误
pub(crate) fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    let dir = BASE_DIR.get_or_init(|| dir.as_ref().join(names::GAME_ASSETS_DIR));
    if !dir.exists() {
        path_helper::create_dir_all(dir)?;
    }

    // 两个目录名不能接反：对象按 SHA1 分桶放 `objects`，每个版本一份索引放 `indexes`。
    // 接反时索引会被写进 objects、资源对象又去 indexes 里找，两边的文件都错位
    let obj = OBJECTS_DIR.get_or_init(|| dir.join(names::GAME_OBJECT_DIR));
    if !obj.is_dir() {
        path_helper::create_dir_all(obj)?;
    }

    let index = INDEX_DIR.get_or_init(|| dir.join(names::GAME_INDEX_DIR));
    if !index.is_dir() {
        path_helper::create_dir_all(index)?;
    }

    let skin = SKIN_DIR.get_or_init(|| dir.join(names::GAME_SKIN_DIR));
    if !skin.is_dir() {
        path_helper::create_dir_all(skin)?;
    }

    Ok(())
}

/// 取 assets 根目录
///
/// # 返回值
///
/// assets 根目录
pub fn get_assets_dir() -> PathBuf {
    BASE_DIR.get().unwrap().clone()
}

/// 取资源对象目录
///
/// # 返回值
///
/// 资源对象目录
pub fn get_obj_dir() -> PathBuf {
    OBJECTS_DIR.get().unwrap().clone()
}

/// 把资源索引写进索引目录
///
/// - `obj`: 版本详情，取其中的 `assetIndex` 段
/// - `data`: 资源索引文件的原始字节流
pub fn add_index(obj: &GameArgObj, data: &mut Cursor<Vec<u8>>) {
    let index = obj.asset_index.as_ref().unwrap();
    let file = INDEX_DIR.get().unwrap().join(format!("{}.json", index.id));
    path_helper::write_stream(&file, data).unwrap();
}

/// 按版本读资源索引
///
/// - `obj`: 资源索引段，取其中的 `id` 找文件
///
/// # 返回值
///
/// 解析出的资源索引；读取或解析失败返回对应错误
pub fn get_index(obj: &GameAssetIndexObj) -> CoreResult<AssetsObj> {
    let file = INDEX_DIR.get().unwrap().join(format!("{}.json", obj.id));
    let obj = serialize_tools::json_from_file::<AssetsObj>(&file)?;
    Ok(obj)
}

/// 把一张图片存成账户皮肤
///
/// - `obj`: 目标账户，文件名用它的 uuid
/// - `file`: 需要导入的图片文件
pub fn save_skin(obj: LoginObj, file: PathBuf) {
    let path = SKIN_DIR
        .get()
        .unwrap()
        .join(format!("{}_skin.png", obj.uuid));
    path_helper::copy_file(&file, &path).unwrap();
}

/// 按内容 SHA1 保存皮肤 / 披风贴图，布局与游戏 assets objects 相同
///
/// - `data`: 贴图文件内容
///
/// 皮肤与披风共用一套存储：`SKIN_DIR/<sha1前2字符>/<sha1>`，
/// 同一份贴图不论来自哪个账户或 URL 都只存一份。
///
/// # 返回值
///
/// 内容 SHA1，十六进制小写；写入失败返回对应错误
pub fn save_skin_object(data: &[u8]) -> CoreResult<String> {
    let hash = hash_helper::gen_hash(HashType::Sha1, data);
    let file = get_skin_object(&hash);
    if !file.exists() {
        path_helper::write_bytes(&file, data)?;
    }
    Ok(hash)
}

/// 按内容 SHA1 取皮肤 / 披风贴图位置
///
/// - `sha1`: 贴图内容 SHA1
///
/// # 返回值
///
/// 贴图文件位置；不保证文件存在，由调用方判断
pub fn get_skin_object(sha1: &str) -> PathBuf {
    let dir: String = sha1.chars().take(2).collect();
    Path::new(&SKIN_DIR.get().unwrap()).join(dir).join(sha1)
}

/// 按 SHA1 读取资源文件的文本内容
///
/// - `hash`: 资源文件 SHA1，十六进制小写
///
/// # 返回值
///
/// 文件文本内容；读取失败记录日志并返回 `None`
pub fn read_assets_text(hash: String) -> Option<String> {
    let dir: String = hash.chars().take(2).collect();
    let local = Path::new(&OBJECTS_DIR.get().unwrap())
        .join(dir)
        .with_file_name(hash);

    let file = path_helper::read_text(&local);
    match file {
        Err(err) => {
            mml_log::error_type(err);

            None
        }
        Ok(file) => Some(file),
    }
}

/// 按 SHA1 打开资源文件的读取流
///
/// - `hash`: 资源文件 SHA1，十六进制小写
///
/// # 返回值
///
/// 文件读取流；读取失败记录日志并返回 `None`
pub fn read_assets_stream(hash: String) -> Option<File> {
    let dir: String = hash.chars().take(2).collect();
    let local = Path::new(&OBJECTS_DIR.get().unwrap())
        .join(dir)
        .with_file_name(hash);

    let file = path_helper::open_read(&local);
    match file {
        Err(err) => {
            mml_log::error_type(err);

            None
        }
        Ok(file) => Some(file),
    }
}
