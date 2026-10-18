//! 运行库目录与运行库文件路径

use std::{
    collections::HashMap,
    fs,
    hash::Hasher,
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock, RwLock},
};

use mml_auth::AuthType;
use mml_base::{
    file_item::{
        FileHash::{Sha1, Sha256},
        FileItemObj,
    },
    hash_helper::{self, HashType},
};
use mml_names::{i18_items::error_type::CoreResult, names};
use mml_net::{
    authlib_api::{self, AuthlibInjectorObj},
    nide8_api::{self, Nide8Obj},
    urls,
};
use mml_sys::path_helper;

use crate::{
    game_arg::GameLaunchObj, launcher::instance_setting_obj::InstanceSettingObj, loader::LoaderType,
};

/// 基础路径
static LIB_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 资源文件路径
static NATIVE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// AuthlibInjector 下载项缓存
static AUTHLIB_FILE: LazyLock<RwLock<Option<FileItemObj>>> = LazyLock::new(|| RwLock::new(None));
/// Nide8 下载项缓存
static NIDE8_FILE: LazyLock<RwLock<Option<FileItemObj>>> = LazyLock::new(|| RwLock::new(None));

/// 获取运行库根目录
///
/// # 返回值
///
/// 返回运行库根目录；未初始化会 panic
pub fn get_lib_dir() -> PathBuf {
    LIB_DIR.get().unwrap().clone()
}

/// 获取外部登陆 jar
///
/// # 返回值
///
/// 返回 Authlib Injector jar 位置；未初始化返回 `None`
pub fn get_authlib_file() -> Option<PathBuf> {
    let guard = AUTHLIB_FILE.read().unwrap();
    let data = guard.as_ref()?;
    Some(data.file.clone())
}

/// 获取统一通行证 jar
///
/// # 返回值
///
/// 返回 Nide8 jar 位置；未初始化返回 `None`
pub fn get_nide8_file() -> Option<PathBuf> {
    let guard = NIDE8_FILE.read().unwrap();
    let data = guard.as_ref()?;
    Some(data.file.clone())
}

/// 运行库信息，即一条 Maven 坐标拆出来的四段
#[derive(Clone)]
pub struct LibVersionObj {
    /// 包名，对应 Maven 的 groupId
    pub pack: String,
    /// 名字，对应 Maven 的 artifactId
    pub name: String,
    /// 版本号
    pub version: String,
    /// 额外信息，对应 Maven 的 classifier；没有时为空串
    pub extr: String,
}

impl PartialEq for LibVersionObj {
    /// 只比包名、名字与额外信息，**不比版本号**
    fn eq(&self, other: &Self) -> bool {
        self.eq_without_version(other)
    }
}

impl Eq for LibVersionObj {}

/// 忽略版本号后的运行库键：包名、名字与额外信息
///
/// 与 [`LibVersionObj`] 的 `PartialEq` / `Hash` 同一口径——只认这三项，不比版本号。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LibKey {
    /// 包名，对应 Maven 的 groupId
    pub pack: String,
    /// 名字，对应 Maven 的 artifactId
    pub name: String,
    /// 额外信息，对应 Maven 的 classifier；没有时为空串
    pub extr: String,
}

impl std::hash::Hash for LibVersionObj {
    /// 与 `eq` 口径一致：只哈希包名、名字与额外信息，保证同键同哈希
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pack.hash(state);
        self.name.hash(state);
        self.extr.hash(state);
    }
}

impl LibVersionObj {
    /// 从 Maven 坐标解析运行库信息
    ///
    /// - `name`: Maven 坐标，形如 `包:名:版本:额外`
    ///
    /// # 返回值
    ///
    /// 返回解析结果；格式不完整时仅填 `name` 字段
    pub fn new(name: &str) -> Self {
        let arg: Vec<&str> = name.split(':').collect();

        if arg.len() < 3 {
            Self {
                pack: String::new(),
                name: String::from(name),
                version: String::new(),
                extr: String::new(),
            }
        } else if arg.len() > 3 {
            Self {
                pack: String::from(arg[0]),
                name: String::from(arg[1]),
                version: String::from(arg[2]),
                extr: String::from(arg[3]),
            }
        } else {
            Self {
                pack: String::from(arg[0]),
                name: String::from(arg[1]),
                version: String::from(arg[2]),
                extr: String::new(),
            }
        }
    }

    /// 判断运行库是否除了版本都一样
    ///
    /// - `obj`: 对比对象
    ///
    /// # 返回值
    ///
    /// 返回包名、名字与额外信息是否一致
    pub fn eq_without_version(&self, obj: &LibVersionObj) -> bool {
        self.pack.eq(&obj.pack) && self.name.eq(&obj.name) && self.extr.eq(&obj.extr)
    }

    /// 获取去版本的键，即包名、名字与额外信息
    ///
    /// # 返回值
    ///
    /// 见 [`LibKey`]
    pub fn key_without_version(&self) -> LibKey {
        LibKey {
            pack: self.pack.clone(),
            name: self.name.clone(),
            extr: self.extr.clone(),
        }
    }
}

/// 初始化运行库路径
///
/// - `dir`: 运行路径
///
/// # 返回值
///
/// 创建目录失败时返回对应错误
pub(crate) fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    let dir = LIB_DIR.get_or_init(|| dir.as_ref().join(names::LIBRARIES_DIR));

    let sys = mml_sys::get_system_info();

    if !dir.exists() {
        path_helper::create_dir_all(dir)?;
    }

    let dir = NATIVE_DIR.get_or_init(|| {
        dir.join(names::NATIVE_DIR)
            .join(sys.os.to_string().to_lowercase())
            .join(sys.system_arch.to_string().to_lowercase())
    });
    if !dir.exists() {
        path_helper::create_dir_all(dir)?;
    }

    Ok(())
}

/// 获取 Native 文件夹
///
/// - `version`: 游戏版本；给 `Some` 时创建并返回对应子目录
///
/// # 返回值
///
/// 返回 Native 目录
pub fn get_native_dir(version: Option<&str>) -> PathBuf {
    match version {
        Some(version) => {
            let dir = NATIVE_DIR.get().unwrap().join(version);
            if !dir.is_dir() {
                fs::create_dir(&dir).unwrap();
            }

            dir
        }
        None => NATIVE_DIR.get().unwrap().clone(),
    }
}

/// 获取游戏核心路径
///
/// - `version`: 游戏版本
///
/// # 返回值
///
/// 返回游戏核心 jar 路径
pub fn get_game_file(version: &str) -> PathBuf {
    LIB_DIR
        .get()
        .unwrap()
        .join("net")
        .join("minecraft")
        .join("client")
        .join(version)
        .join(format!("client-{version}.jar"))
}

/// 按自定义版本号取游戏核心路径
///
/// - `custom`: 自定义版本号
///
/// # 返回值
///
/// 返回游戏核心 jar 路径
pub fn get_game_file_with_custom(custom: &str) -> PathBuf {
    LIB_DIR
        .get()
        .unwrap()
        .join("net")
        .join("minecraft")
        .join("client")
        .join(format!("{custom}.jar"))
}

/// 获取 OptiFine 路径
///
/// - `mc`: 游戏版本
/// - `version`: OptiFine 版本
///
/// # 返回值
///
/// 返回 OptiFine 安装包路径
pub fn get_optifine_file(mc: &str, version: &str) -> PathBuf {
    LIB_DIR
        .get()
        .unwrap()
        .join("optifine")
        .join("installer")
        .join(format!("OptiFine-{mc}-{version}.jar"))
}

impl InstanceSettingObj {
    /// 获取本实例的 OptiFine 安装包路径
    ///
    /// # 返回值
    ///
    /// 返回 OptiFine 安装包路径；`loader_version` 缺省时 panic
    pub fn get_optifine_file(&self) -> PathBuf {
        get_optifine_file(&self.version, self.loader_version.as_ref().unwrap())
    }

    /// 获取本实例的游戏核心路径
    ///
    /// # 返回值
    ///
    /// 返回游戏核心 jar 路径
    pub fn get_game_file(&self) -> PathBuf {
        get_game_file(&self.version)
    }

    /// 获取所有运行库
    ///
    /// - `arg`: 启动参数
    ///
    /// # 返回值
    ///
    /// 返回去重后的运行库路径列表，冲突时优先保留加载器库
    pub fn get_libs(&self, arg: &GameLaunchObj) -> Vec<PathBuf> {
        let mut game_list = Vec::new();
        for item in &arg.game_libs {
            // 带后续动作的项（下载完解压 natives）不进 classpath：ColorMC 也按 Later 过滤，
            // 这类 jar 里没有类，留在 classpath 里只是噪音
            if !matches!(item.later, mml_base::file_item::LaterRun::None) {
                continue;
            }
            let key = LibVersionObj::new(&item.name);
            if let Some(pos) = game_list
                .iter()
                .position(|(k, _): &(LibVersionObj, PathBuf)| k.eq_without_version(&key))
            {
                game_list.remove(pos);
            }
            game_list.push((key, item.file.clone()));
        }

        if let Some(data) = &self.custom_loader
            && data.custom_json
        {
            return game_list
                .into_iter()
                .map(|(_, path)| path)
                .chain(std::iter::once(arg.game_jar.file.clone()))
                .collect();
        }

        let mut loader_list = Vec::new();
        for item in &arg.loader_libs {
            // 同上：解压项不进 classpath
            if !matches!(item.later, mml_base::file_item::LaterRun::None) {
                continue;
            }
            let key = LibVersionObj::new(&item.name);
            if let Some(pos) = loader_list
                .iter()
                .position(|(k, _): &(LibVersionObj, PathBuf)| k.eq_without_version(&key))
            {
                loader_list.remove(pos);
            }
            loader_list.push((key, item.file.clone()));
        }

        // 自定义加载器要判断是否后置原版库
        let result = if self.loader == LoaderType::Custom
            && let Some(data) = &self.custom_loader
            && data.offset_lib
        {
            let mut temp = HashMap::with_capacity(loader_list.len() + game_list.len());

            for (key, value) in loader_list {
                add_or_update_lib_kv(&mut temp, key, value);
            }

            // 是否删除原版库
            if !data.remove_lib {
                for (key, value) in game_list {
                    if !temp.contains_key(&key) {
                        temp.insert(key, value);
                    }
                }
            }
            temp
        } else {
            let mut temp = HashMap::with_capacity(game_list.len() + loader_list.len());

            // 原版库默认都要带，加载器库随后覆盖同键冲突项；只有自定义加载器明确要求
            // 删掉原版库时才不带。少了这个默认分支，Forge / Fabric / 原版实例的类路径里
            // 就只剩加载器库与游戏 jar，guava、netty 这些原版依赖全不见
            if !self.custom_loader.as_ref().is_some_and(|data| data.remove_lib) {
                for (key, value) in game_list {
                    add_or_update_lib_kv(&mut temp, key, value);
                }
            }

            for (key, value) in loader_list {
                add_or_update_lib_kv(&mut temp, key, value);
            }
            temp
        };

        let mut output: Vec<PathBuf> = result.into_values().collect();

        if self.loader != LoaderType::NeoForge {
            output.push(arg.game_jar.file.clone());
        }

        output
    }
}

/// 删除冲突的库
///
/// - `map`: 运行库表
/// - `key`: 新的运行库键
/// - `value`: 新的运行库路径
fn add_or_update_lib_kv(
    map: &mut HashMap<LibVersionObj, PathBuf>,
    key: LibVersionObj,
    value: PathBuf,
) {
    map.retain(|k, _| !k.eq_without_version(&key));
    map.insert(key, value);
}

/// 创建 Authlib Injector 下载实例
///
/// - `obj`: Authlib Injector 信息
///
/// # 返回值
///
/// 返回下载项
pub fn build_authlib_injector_item(obj: &AuthlibInjectorObj) -> FileItemObj {
    FileItemObj {
        name: format!("moe.yushi:authlibinjector:{}", obj.version),
        file: LIB_DIR
            .get()
            .unwrap()
            .join("moe")
            .join("yushi")
            .join("authlibinjector")
            .join(&obj.version)
            .join(format!("authlib-injector-{}.jar", obj.version)),
        url: obj.download_url.clone(),
        hash: Sha256(obj.checksums.sha256.clone()),
        later: Default::default(),
    }
}

/// 读取 authlibinjector 是否存在
///
/// # 返回值
///
/// 返回缺失或校验不符的下载项；已存在返回 `None`
async fn read_authlib_injector() -> CoreResult<Option<FileItemObj>> {
    let obj = authlib_api::get_obj().await?;
    let item = build_authlib_injector_item(&obj);

    if item.file.exists() {
        let sha256 = hash_helper::gen_hash_from_file_async(HashType::Sha256, &item.file).await?;
        if obj.checksums.sha256 != sha256 {
            Ok(Some(item))
        } else {
            *AUTHLIB_FILE.write().unwrap() = Some(item.clone());

            Ok(None)
        }
    } else {
        Ok(Some(item))
    }
}

/// 初始化 Authlib Injector，不存在则返回下载项
///
/// # 返回值
///
/// 返回需要下载的下载项；已就绪返回 `None`
pub async fn ready_authlib_injector() -> CoreResult<Option<FileItemObj>> {
    let guard = AUTHLIB_FILE.read().unwrap();
    match guard.as_ref() {
        Some(obj) => {
            let path = &obj.file;
            if !path.exists() {
                Ok(Some(obj.clone()))
            } else {
                let sha256 = hash_helper::gen_hash_from_file_async(HashType::Sha256, &path).await?;
                if !obj.hash.eq(&sha256) {
                    Ok(Some(obj.clone()))
                } else {
                    Ok(None)
                }
            }
        }
        None => read_authlib_injector().await,
    }
}

/// 创建 Nide8 Injector 下载实例
///
/// - `obj`: 下载信息
///
/// # 返回值
///
/// 返回下载项
pub fn build_nide8_item(obj: &Nide8Obj) -> FileItemObj {
    FileItemObj {
        name: format!("com.nide8.login2:nide8auth:{}", obj.jar_version),
        file: LIB_DIR
            .get()
            .unwrap()
            .join("com")
            .join("nide8")
            .join("login2")
            .join("nide8auth")
            .join(&obj.jar_version)
            .join(format!("nide8auth-{}.jar", obj.jar_version)),
        url: String::from(urls::NIDE8_JAR_URL),
        hash: Sha1(obj.jar_hash.clone()),
        later: Default::default(),
    }
}

/// 读取 nide8 是否存在
///
/// # 返回值
///
/// 返回缺失或校验不符的下载项；已存在返回 `None`
async fn read_nide8() -> CoreResult<Option<FileItemObj>> {
    let obj = nide8_api::get_obj().await?;
    let item = build_nide8_item(&obj);

    if item.file.exists() {
        let sha1 = hash_helper::gen_hash_from_file_async(HashType::Sha1, &item.file).await?;
        if obj.jar_hash != sha1 {
            Ok(Some(item))
        } else {
            *NIDE8_FILE.write().unwrap() = Some(item.clone());

            Ok(None)
        }
    } else {
        Ok(Some(item))
    }
}

/// 初始化 Nide8 Injector，不存在则返回下载项
///
/// # 返回值
///
/// 返回需要下载的下载项；已就绪返回 `None`
pub async fn ready_nide8() -> CoreResult<Option<FileItemObj>> {
    let guard = NIDE8_FILE.read().unwrap();
    match guard.as_ref() {
        Some(obj) => {
            let path = &obj.file;
            if !path.exists() {
                Ok(Some(obj.clone()))
            } else {
                let sha1 = hash_helper::gen_hash_from_file_async(HashType::Sha1, &path).await?;
                if !obj.hash.eq(&sha1) {
                    Ok(Some(obj.clone()))
                } else {
                    Ok(None)
                }
            }
        }
        None => read_nide8().await,
    }
}

/// 检查外置登陆 jar 是否存在
///
/// - `auth`: 账户类型
///
/// # 返回值
///
/// 返回需要下载的下载项；已就绪返回 `None`
pub async fn check_authlib(auth: &AuthType) -> CoreResult<Option<FileItemObj>> {
    match auth {
        AuthType::Nide8 => ready_nide8().await,
        AuthType::AuthlibInjector | AuthType::LittleSkin | AuthType::SelfLittleSkin => {
            read_authlib_injector().await
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个只有名字与落点的运行库项
    fn lib(name: &str, file: &str) -> FileItemObj {
        FileItemObj {
            name: name.to_string(),
            file: PathBuf::from(file),
            ..Default::default()
        }
    }

    /// 原版库必须进类路径，同键冲突时加载器库胜出
    ///
    /// 原版实例与带加载器的实例都没有 `custom_loader`；过去空加载器那段会把
    /// `game_libs` 整个跳过，类路径里只剩加载器库与游戏 jar，guava、netty 这些
    /// 原版依赖全不见，真起游戏立刻 NoClassDefFoundError。探针类测试测不出来：
    /// 探针不加载游戏类。
    #[test]
    fn merges_game_libs_and_loader_libs() {
        let instance = InstanceSettingObj {
            loader: LoaderType::Forge,
            ..Default::default()
        };

        let arg = GameLaunchObj {
            game_libs: vec![
                lib("com.google.guava:guava:21.0", "game/guava-21.0.jar"),
                lib("net.minecraft:launchwrapper:1.12", "game/launchwrapper.jar"),
            ],
            loader_libs: vec![
                lib(
                    "net.minecraftforge:forge:1.12.2-14.23.5.2864",
                    "loader/forge.jar",
                ),
                lib("com.google.guava:guava:32.0", "loader/guava-32.0.jar"),
            ],
            game_jar: lib("net.minecraft:client:1.12.2", "client.jar"),
            ..Default::default()
        };

        let libs = instance.get_libs(&arg);
        let paths: Vec<String> = libs
            .iter()
            .map(|item| item.to_string_lossy().to_string())
            .collect();

        assert!(
            paths.iter().any(|item| item.ends_with("loader/forge.jar")),
            "加载器库要在：{paths:?}"
        );
        assert!(
            paths
                .iter()
                .any(|item| item.ends_with("game/launchwrapper.jar")),
            "原版独有的库不能被丢掉：{paths:?}"
        );
        assert!(
            paths
                .iter()
                .any(|item| item.ends_with("loader/guava-32.0.jar")),
            "同键冲突时应保留加载器那版：{paths:?}"
        );
        assert!(
            !paths
                .iter()
                .any(|item| item.ends_with("game/guava-21.0.jar")),
            "被覆盖的原版库不该有第二份：{paths:?}"
        );
        assert!(
            paths.iter().any(|item| item.ends_with("client.jar")),
            "游戏 jar 要在：{paths:?}"
        );
    }
}
