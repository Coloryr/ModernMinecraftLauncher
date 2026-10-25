//! 桌面快捷方式：右键实例「创建快捷方式」
//!
//! 快捷方式指向启动器本体并带上 `--game <uuid>`（见 `crate::start_arg`）：
//! 双击后启动器按实例启动，已经在跑时由单实例转发给它。

use std::path::PathBuf;

use uuid::Uuid;

use crate::err_keys;

/// 在桌面为实例创建快捷方式
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 成功返回快捷方式的完整路径；实例不存在、平台不支持或创建失败返回错误串
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_create_shortcut(uuid: String) -> Result<String, String> {
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::UUID.to_string())?;
    let instance =
        mml_game::get_instance(&id).ok_or_else(|| err_keys::GAME_NOT_FOUND.to_string())?;
    let snapshot = instance.read().unwrap().clone();
    let icon = ico_of(&snapshot.get_icon_file_or_default());

    create_shortcut(&snapshot.name, &uuid, icon.as_deref())
        .map(|path| path.to_string_lossy().to_string())
}

/// 取快捷方式要用的图标：把实例图标转成 `.ico` 放在旁边
///
/// `.lnk` 的 `SetIconLocation` 只认 PE / `.ico`，PNG 指过去只会得到一个空白图标，
/// 所以这里就地把实例图标转成单条 256×256 的 ICO（PNG 内嵌，Vista 起支持）。
/// 已经是 `.ico`、或者转好的那份比原图新，就直接复用。
///
/// - `source`: 实例图标文件，通常是 `icon.png`
///
/// # 返回值
///
/// 可用的 `.ico` 路径；图标不存在或转不出来时返回 `None`，调用方退回启动器图标
fn ico_of(source: &std::path::Path) -> Option<PathBuf> {
    if !source.is_file() {
        return None;
    }
    if source
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ico"))
    {
        return Some(source.to_path_buf());
    }

    let target = source.with_extension("ico");
    let newer = || {
        let (Ok(icon), Ok(plain)) = (std::fs::metadata(&target), std::fs::metadata(source)) else {
            return false;
        };
        match (icon.modified(), plain.modified()) {
            (Ok(icon), Ok(plain)) => icon >= plain,
            _ => false,
        }
    };
    if newer() {
        return Some(target);
    }

    let bytes = std::fs::read(source).ok()?;
    let png = square_png(&bytes, 256)?;
    std::fs::write(&target, ico_of_png(&png)).ok()?;
    Some(target)
}

/// 把任意图片缩放到 `size` 见方的透明画布上，按 PNG 输出
///
/// - `bytes`: 原始图片字节，格式自动识别
/// - `size`: 边长，快捷方式图标取 256
///
/// # 返回值
///
/// PNG 字节；解不出来返回 `None`
fn square_png(bytes: &[u8], size: u32) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?;
    let scaled = image.resize(size, size, image::imageops::FilterType::Lanczos3);
    let mut canvas = image::RgbaImage::new(size, size);
    let (width, height) = (scaled.width(), scaled.height());
    image::imageops::overlay(
        &mut canvas,
        &scaled.to_rgba8(),
        ((size - width) / 2) as i64,
        ((size - height) / 2) as i64,
    );

    let mut out = Vec::new();
    canvas
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// 把一张 PNG 包成只有一条目的 ICO
///
/// 条目里直接放 PNG 数据（Vista 起 shell 认），省掉 BMP 那套掩码。
///
/// - `png`: PNG 字节，边长 256 时宽度 / 高度字段写 0
///
/// # 返回值
///
/// ICO 文件字节
fn ico_of_png(png: &[u8]) -> Vec<u8> {
    const HEADER: u32 = 6 + 16;
    let mut out = Vec::with_capacity(png.len() + HEADER as usize);
    out.extend_from_slice(&0u16.to_le_bytes()); // 保留
    out.extend_from_slice(&1u16.to_le_bytes()); // 1 = 图标
    out.extend_from_slice(&1u16.to_le_bytes()); // 条目数
    out.extend_from_slice(&[0, 0, 0, 0]); // 宽 256 / 高 256 / 调色板 / 保留
    out.extend_from_slice(&1u16.to_le_bytes()); // 颜色平面
    out.extend_from_slice(&32u16.to_le_bytes()); // 位深
    out.extend_from_slice(&(png.len() as u32).to_le_bytes());
    out.extend_from_slice(&HEADER.to_le_bytes()); // 数据偏移
    out.extend_from_slice(png);
    out
}

/// 快捷方式文件名：Windows 不允许的字符换成下划线，去掉首尾空白与结尾的点
///
/// - `name`: 实例名
///
/// # 返回值
///
/// 可以直接当文件名的名字；全是非法字符时退回 `M²L`
fn shortcut_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect();
    let cleaned = cleaned.trim_end_matches('.').trim();
    if cleaned.is_empty() {
        "M²L".to_string()
    } else {
        cleaned.to_string()
    }
}

#[cfg(windows)]
mod platform {
    use std::path::PathBuf;

    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, IPersistFile,
    };
    use windows::Win32::UI::Shell::{
        FOLDERID_Desktop, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink,
    };
    use windows::core::{HSTRING, Interface};

    /// 桌面目录：走 Known Folder，重定向到 OneDrive 的桌面也能找对
    fn desktop_dir() -> Result<PathBuf, String> {
        let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None) }
            .map_err(|err| err.message().to_string())?;
        let text = unsafe { raw.to_string() }.map_err(|err| err.to_string());
        unsafe { CoTaskMemFree(Some(raw.as_ptr() as *const _)) };
        Ok(PathBuf::from(text?))
    }

    /// 见 [`super::create_shortcut`]
    pub(super) fn create(
        name: &str,
        uuid: &str,
        icon: Option<&std::path::Path>,
    ) -> Result<PathBuf, String> {
        let dir = desktop_dir()?;
        write_link(&dir, name, uuid, icon)
    }

    /// 在指定目录写一个指向本程序的 `.lnk`
    ///
    /// - `dir`: 放快捷方式的目录
    /// - `name`: 实例名，会先过 [`super::shortcut_name`]
    /// - `uuid`: 实例 uuid，作为 `--game` 的参数
    /// - `icon`: 快捷方式图标（`.ico`）；`None` 时用启动器自己的图标
    ///
    /// # 返回值
    ///
    /// 快捷方式的完整路径；COM 调用失败返回错误串
    pub(super) fn write_link(
        dir: &std::path::Path,
        name: &str,
        uuid: &str,
        icon: Option<&std::path::Path>,
    ) -> Result<PathBuf, String> {
        let exe = std::env::current_exe().map_err(|err| err.to_string())?;
        let exe_text = exe.to_string_lossy().to_string();
        let dir_text = exe
            .parent()
            .map(|dir| dir.to_string_lossy().to_string())
            .unwrap_or_default();
        let path = dir.join(format!("{}.lnk", super::shortcut_name(name)));

        unsafe {
            // 命令跑在 worker 线程上，这里自己要初始化一次 COM；已初始化时返回 S_FALSE，忽略
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|err| err.message().to_string())?;
            let exe_wide = HSTRING::from(exe_text.as_str());
            let dir_wide = HSTRING::from(dir_text.as_str());
            let args_wide = HSTRING::from(format!("--game {uuid}"));
            // 描述只放实例名：它是数据不是文案，免得这里写死一句中文
            let desc_wide = HSTRING::from(name);
            // 有实例图标就用它，没有才退回启动器自己的 exe 图标
            let icon_wide = icon.map(|path| HSTRING::from(path.to_string_lossy().as_ref()));
            link.SetPath(&exe_wide)
                .map_err(|err| err.message().to_string())?;
            link.SetArguments(&args_wide)
                .map_err(|err| err.message().to_string())?;
            link.SetWorkingDirectory(&dir_wide)
                .map_err(|err| err.message().to_string())?;
            link.SetDescription(&desc_wide)
                .map_err(|err| err.message().to_string())?;
            link.SetIconLocation(icon_wide.as_ref().unwrap_or(&exe_wide), 0)
                .map_err(|err| err.message().to_string())?;

            let file: IPersistFile = link.cast().map_err(|err| err.message().to_string())?;
            let wide = HSTRING::from(path.to_string_lossy().as_ref());
            file.Save(&wide, true)
                .map_err(|err| err.message().to_string())?;
        }

        Ok(path)
    }
}

#[cfg(windows)]
fn create_shortcut(
    name: &str,
    uuid: &str,
    icon: Option<&std::path::Path>,
) -> Result<PathBuf, String> {
    platform::create(name, uuid, icon)
}

#[cfg(not(windows))]
fn create_shortcut(
    _name: &str,
    _uuid: &str,
    _icon: Option<&std::path::Path>,
) -> Result<PathBuf, String> {
    Err(err_keys::SHORTCUT_PLATFORM.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真的写一个 .lnk 出来：COM 这一路没有别的办法验证
    #[cfg(windows)]
    #[test]
    fn writes_a_real_link() {
        let dir = crate::test_support::temp_dir("shortcut-link");
        let path = platform::write_link(
            &dir,
            "All the Mods 10-8.3",
            "0b5c1f5e-0000-0000-0000-000000000000",
            None,
        )
        .expect("应能写出 .lnk");
        assert!(path.is_file(), "快捷方式应已落盘：{}", path.display());
        assert!(std::fs::metadata(&path).unwrap().len() > 100);
        crate::test_support::cleanup(&[&dir]);
    }

    /// 实例图标（png）就地转成 .ico，第二次直接复用
    #[test]
    fn instance_icon_becomes_ico() {
        let dir = crate::test_support::temp_dir("shortcut-ico");
        let png = dir.join("icon.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 30, 30, 255]))
            .save(&png)
            .unwrap();

        let ico = ico_of(&png).expect("应能转出 ico");
        assert_eq!(ico, dir.join("icon.ico"));
        let bytes = std::fs::read(&ico).unwrap();
        assert_eq!(&bytes[0..6], &[0, 0, 1, 0, 1, 0], "ICO 头部不对");
        assert_eq!(&bytes[22..30], b"\x89PNG\r\n\x1a\n", "条目里应是 PNG");

        // 转好的比原图新：再调一次不重写
        assert_eq!(ico_of(&png).unwrap(), ico);
        // 没有图标就没有 ico
        assert!(ico_of(&dir.join("none.png")).is_none());

        crate::test_support::cleanup(&[&dir]);
    }

    /// 非法字符换成下划线，纯空白退回默认名
    #[test]
    fn shortcut_name_is_filesystem_safe() {
        assert_eq!(shortcut_name("All the Mods 10-8.3"), "All the Mods 10-8.3");
        assert_eq!(shortcut_name("a/b:c*d?e\"f<g>h|i"), "a_b_c_d_e_f_g_h_i");
        assert_eq!(shortcut_name("   "), "M²L");
    }
}
