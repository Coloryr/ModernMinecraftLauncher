//! 实例相关目录 / 文件路径
//!
//! 子模块:
//!
//! | 模块 | 职责 |
//! | --- | --- |
//! | `assets_path` | 资源文件目录 |
//! | `instance_path` | 实例目录 |
//! | `libraries_path` | 运行库目录 |
//! | `version_path` | 版本目录 |

use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use mml_base::file_item::FileItemObj;
use mml_names::{i18_items::error_type::CoreResult, names};
use mml_sys::path_helper;

pub mod assets_path;
pub mod instance_path;
pub mod libraries_path;
pub mod version_path;

/// 内置的 ColorASM jar 数据
const COLORASM_FILE: &[u8] = include_bytes!("../../assets/ColorASM-1.1-all.jar");

/// ColorASM jar 的存放位置
static COLORASM: LazyLock<PathBuf> = LazyLock::new(|| {
    let local = libraries_path::get_lib_dir()
        .join("com")
        .join("coloryr")
        .join("colorasm")
        .join("1.1")
        .join("ColorASM-1.1-all.jar");

    local
});

/// 内置的 ForgeWrapper jar 数据
const WRAPPER_FILE: &[u8] = include_bytes!("../../assets/ForgeWrapper-prism-2025-12-07.jar");

/// ForgeWrapper jar 的存放位置
static FORGE_WRAPPER: LazyLock<PathBuf> = LazyLock::new(|| {
    let local = libraries_path::get_lib_dir()
        .join("io")
        .join("github")
        .join("zekerzhayard")
        .join("prism-2025-12-07")
        .join("ForgeWrapper-prism-2025-12-07.jar");

    local
});

/// 内置的 OptiFine Wrapper jar 数据
const OPTIFINE_FILE: &[u8] = include_bytes!("../../assets/OptifineWrapper-1.1.jar");

/// OptiFine Wrapper jar 的存放位置
static OPTIFINE_WRAPPER: LazyLock<PathBuf> = LazyLock::new(|| {
    let local = libraries_path::get_lib_dir()
        .join("com")
        .join("coloryr")
        .join("optifinewrapper")
        .join("1.1")
        .join("optifinewrapper-1.1.jar");

    local
});

/// 初始化文件夹
///
/// - `dir`: 工作的目录
///
/// # 返回值
///
/// 成功返回 `Ok(())`；创建目录失败返回对应错误
pub(crate) fn init<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    let dir = dir.as_ref().join(names::MINECRAFT_DIR);
    if !dir.exists() {
        path_helper::create_dir_all(&dir)?;
    }

    assets_path::init(&dir)?;
    version_path::init(&dir)?;
    instance_path::init(&dir)?;
    libraries_path::init(&dir)?;

    Ok(())
}

/// 准备ForgeWrapper jar
///
/// # 返回值
///
/// 成功返回 `Ok(())`；写出文件失败返回对应错误
pub fn ready_forge_wrapper() -> CoreResult<()> {
    let local = FORGE_WRAPPER.clone();

    if !local.exists() {
        path_helper::write_bytes(&local, WRAPPER_FILE)?;
    }

    Ok(())
}

/// 准备 ColorASM jar
///
/// # 返回值
///
/// 成功返回 `Ok(())`；写出文件失败返回对应错误
pub fn ready_colorasm() -> CoreResult<()> {
    let local = COLORASM.clone();

    if !local.exists() {
        path_helper::write_bytes(&local, COLORASM_FILE)?;
    }

    Ok(())
}

/// 准备 OptiFine Wrapper jar
///
/// # 返回值
///
/// 成功返回 `Ok(())`；写出文件失败返回对应错误
pub fn ready_optifine_wrapper() -> CoreResult<()> {
    let local = OPTIFINE_WRAPPER.clone();

    if !local.exists() {
        path_helper::write_bytes(&local, OPTIFINE_FILE)?;
    }

    Ok(())
}

/// 获取 ColorASM 运行库下载项
///
/// # 返回值
///
/// 返回对应的下载项
pub fn get_colorasm() -> FileItemObj {
    FileItemObj {
        name: String::from("com.coloryr.colormc:colormcasm:1.1:all"),
        file: COLORASM.clone(),
        url: Default::default(),
        hash: Default::default(),
        later: Default::default(),
    }
}

/// 获取 ForgeWrapper 运行库下载项
///
/// # 返回值
///
/// 返回对应的下载项
pub fn get_forge_wrapper() -> FileItemObj {
    FileItemObj {
        name: String::from("io.github.zekerzhayard:ForgeWrapper:prism-2025-12-07"),
        file: FORGE_WRAPPER.clone(),
        url: Default::default(),
        hash: Default::default(),
        later: Default::default(),
    }
}

/// 获取 OptiFine Wrapper 运行库下载项
///
/// # 返回值
///
/// 返回对应的下载项
pub fn get_optifine_wrapper() -> FileItemObj {
    FileItemObj {
        name: String::from("com.coloryr:optifinewrapper:1.1"),
        file: OPTIFINE_WRAPPER.clone(),
        url: Default::default(),
        hash: Default::default(),
        later: Default::default(),
    }
}
