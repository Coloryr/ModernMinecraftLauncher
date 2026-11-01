//! Cleanroom 加载器下载项与原生库
//!
//! Cleanroom 是 1.12.2 的 Forge 分支，安装器/清单格式与 Forge 同源；差异集中在
//! maven 坐标（repo.cleanroommc.com）和 `version.json` 自带的完整原生库清单。
//! 安装器解析、运行库合并仍走 `forge` 的公共流程，这里只提供下载项与原生库。

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_names::names;
use mml_net::{maven_utils, urls};
use mml_sys::{Os, get_system_info};

use crate::launcher_path::libraries_path;
use crate::loader::cleanroom_obj::{CleanroomNativesObj, CleanroomRuleObj};

/// 创建 Cleanroom 下载项目
///
/// 路径形如 `com/cleanroommc/cleanroom/<版本>/cleanroom-<版本>-<jar>.jar`，
/// 安装器、universal 都放在这个坐标下。
///
/// - `version`: Cleanroom 版本号
/// - `jar`: jar 类型，取 `installer`、`universal` 之一
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
async fn build_cleanroom_item(version: &str, jar: &str, hash: bool) -> FileItemObj {
    let name = format!("cleanroom-{version}-{jar}");
    let base = format!("{}com/cleanroommc/cleanroom/{version}/", urls::CLEANROOM);
    let url = format!("{base}{name}.jar");
    let hash = if hash {
        maven_utils::try_get_hash(&url).await
    } else {
        FileHash::None
    };

    FileItemObj {
        name: format!("com.cleanroommc:cleanroom:{version}:{jar}"),
        file: libraries_path::get_lib_dir()
            .join("com")
            .join("cleanroommc")
            .join("cleanroom")
            .join(version)
            .join(format!("{name}.jar")),
        url,
        hash,
        later: Default::default(),
    }
}

/// 创建 Cleanroom 安装器下载项目
///
/// - `version`: Cleanroom 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_cleanroom_installer(version: &str, hash: bool) -> FileItemObj {
    build_cleanroom_item(version, names::FILE_INSTALLER, hash).await
}

/// 创建 Cleanroom universal 包下载项目
///
/// - `version`: Cleanroom 版本号
/// - `hash`: 是否联网获取文件哈希
///
/// # 返回值
///
/// 返回下载项
pub async fn build_cleanroom_universal(version: &str, hash: bool) -> FileItemObj {
    build_cleanroom_item(version, names::FILE_UNIVERSAL, hash).await
}

/// 判断原生库的规则是否放行当前平台
///
/// 与 Minecraft 的规则语义一致：没有规则时放行；有规则时由最后一条匹配的规则决定，
/// 一条都没匹配上则不放行。
///
/// - `rules`: 规则列表
/// - `platform`: 当前系统名
/// - `arch`: 当前架构名
///
/// # 返回值
///
/// 允许当前平台使用该原生库时为 `true`
fn rules_allow(rules: &[CleanroomRuleObj], platform: &str, arch: &str) -> bool {
    if rules.is_empty() {
        return true;
    }

    let mut allow = false;
    for rule in rules {
        let os = &rule.os;
        let name_ok = os.name.is_empty() || os.name == platform;
        let arch_ok = os.arch.as_deref().is_none_or(|value| value == arch);
        if name_ok && arch_ok {
            allow = rule.action == "allow";
        }
    }
    allow
}

/// 生成 Cleanroom 的分平台原生库下载项
///
/// 这些 jar 不进 classpath，下载后由 `LaterRun::UnpackNative` 解到实例的 natives 目录。
///
/// - `data`: 安装器里的 version.json 原文
/// - `mc`: 游戏版本号，决定 natives 落到哪个子目录
///
/// # 返回值
///
/// 当前平台对应的原生库下载项；清单解析失败或没有原生库时为空
pub fn build_cleanroom_natives(data: &str, mc: &str) -> Vec<FileItemObj> {
    let Ok(obj) = serde_json::from_str::<CleanroomNativesObj>(data) else {
        return Vec::new();
    };

    let sys = get_system_info();
    let platform = match sys.os {
        Os::Windows => "windows",
        Os::Linux => "linux",
        Os::MacOS => "osx",
        _ => return Vec::new(),
    };
    let arch = match sys.system_arch {
        mml_sys::ArchEnum::X86 => "x86",
        mml_sys::ArchEnum::X86_64 => "x64",
        mml_sys::ArchEnum::Arm => "arm32",
        mml_sys::ArchEnum::AArch64 => "arm64",
        mml_sys::ArchEnum::Unknown => "",
    };
    let native_dir = libraries_path::get_native_dir(Some(mc));

    let mut list = Vec::new();
    for item in obj.libraries {
        // 同一个原生库按架构列了多条，只挑当前系统和架构都匹配的那一条
        if !rules_allow(&item.rules, platform, arch) {
            continue;
        }
        let Some(classifier) = item.natives.get(platform) else {
            continue;
        };
        let Some(artifact) = item.downloads.classifiers.get(classifier) else {
            continue;
        };
        let path = if artifact.path.is_empty() {
            maven_utils::version_name_to_path(&format!("{}:{}", item.name, classifier))
        } else {
            artifact.path.clone()
        };

        list.push(FileItemObj {
            name: format!("{}:{}", item.name, classifier),
            file: libraries_path::get_lib_dir().join(path),
            url: artifact.url.clone(),
            hash: if artifact.sha1.is_empty() {
                FileHash::None
            } else {
                FileHash::Sha1(artifact.sha1.clone())
            },
            later: LaterRun::UnpackNative(native_dir.clone()),
        });
    }

    list
}

#[cfg(test)]
#[path = "../../tests/unit/loader_cleanroom_tests.rs"]
mod tests;
