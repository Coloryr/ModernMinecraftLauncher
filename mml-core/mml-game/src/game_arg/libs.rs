use super::*;

/// 去掉 Windows 的 `\\?\` 逐字路径前缀
///
/// - `path`: 规范化后的路径
///
/// # 返回值
///
/// 返回去掉前缀的路径；非 Windows 或本来就没有前缀时原样返回
pub(super) fn strip_verbatim(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    if let Some(rest) = path.to_string_lossy().strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

/// 处理单个运行库项
///
/// - `item`: 运行库项
/// - `game_libs`: 游戏运行库列表，有效项追加到其中
///
/// # 返回值
///
/// 返回是否成功，本地库解包失败返回对应错误。
pub(super) fn do_lib_item(item: &FileItemObj, game_libs: &mut Vec<FileItemObj>) -> CoreResult<()> {
    if !item.file.as_os_str().is_empty() {
        game_libs.push(item.clone());
    }
    // 已经在本地的解包项顺手解一次；还没下载的不能在这里读 —— 首次安装时它必然不在，
    // 直接 open_read 会让整个检查以 FileSystemError 中止（老版本 1.7.10 / 1.12.2 都带这种
    // natives 分类库）。缺失的交给下载任务的 later：下载完再按同一份 native 目录解包
    if let LaterRun::UnpackNative(native) = &item.later
        && item.file.exists()
    {
        let reader = path_helper::open_read(&item.file)?;
        mml_downloader::later_tasks::unpack_native(native, reader)?;
    }
    Ok(())
}

/// MMC 里"原版那一层"的组件 UID
pub(super) const MINECRAFT_COMPONENT: &str = "net.minecraft";

/// 补丁里有没有原版组件
///
/// 有就说明原版那一层是整份替换，Mojang 清单的运行库不能再补一遍
///
/// - `custom`: 已按 order 排好的补丁
///
/// # 返回值
///
/// 带 `net.minecraft` 组件返回 `true`
pub(super) fn has_minecraft_component(custom: &[(String, CustomGameArgObj)]) -> bool {
    custom
        .iter()
        .any(|(_, item)| item.uid == MINECRAFT_COMPONENT)
}

/// 去掉被其他补丁声明 conflicts 的组件
///
/// MMC 用 `conflicts` 表达组件不能共存：后者的整份配置都不该参与叠加，
/// 例如 `org.lwjgl3` 声明冲突 `org.lwjgl`，LWJGL2 就整层不生效。
///
/// - `custom`: 已按 order 排好的补丁，冲突的组件就地移除
pub(super) fn drop_conflicting_patches(custom: &mut Vec<(String, CustomGameArgObj)>) {
    let conflicted: HashSet<String> = custom
        .iter()
        .filter_map(|(_, item)| item.conflicts.as_deref())
        .flatten()
        .map(|conflict| conflict.uid.clone())
        .collect();
    if conflicted.is_empty() {
        return;
    }
    custom.retain(|(_, item)| !conflicted.contains(&item.uid));
}
