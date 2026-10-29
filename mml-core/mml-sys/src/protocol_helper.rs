//! URL 协议注册：mml:// 等快捷启动协议，写注册表 / 系统配置

use std::path::PathBuf;

/// 读取某个协议当前注册的处理程序
///
/// 先查 HKCU 再查 HKLM：MML 接管协议时写的是 HKCU，安装版的 ColorMC 一般写在 HKLM，
/// 两边都在时仍能找回 ColorMC 的安装位置。
///
/// - `id`: 协议名，如 `colormc`
///
/// # 返回值
///
/// 处理程序的可执行文件路径；没注册或命令行里解析不出路径时返回 `None`
#[cfg(target_os = "windows")]
pub fn registered_exe(id: &str) -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };

    // 两个注册表视图都读：MML 有 x86 构建，32 位进程默认只看到 WOW6432Node 那一份
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let Ok(key) = RegKey::predef(root).open_subkey_with_flags(
                format!(r"Software\Classes\{id}\shell\open\command"),
                KEY_READ | view,
            ) else {
                continue;
            };
            let Ok(command) = key.get_value::<String, _>("") else {
                continue;
            };
            if let Some(exe) = exe_of_command(&command) {
                return Some(exe);
            }
        }
    }
    None
}

/// 非 Windows 平台没有协议注册表
///
/// - `id`: 协议名，此处未使用
///
/// # 返回值
///
/// 恒为 `None`
#[cfg(not(target_os = "windows"))]
pub fn registered_exe(_id: &str) -> Option<PathBuf> {
    None
}

/// 从协议的 `shell\open\command` 命令行里取出可执行文件路径
///
/// 安装器一般写成 `"C:\...\ColorMC.Launcher.exe" "%1"`，也有不带引号的写法。
///
/// - `command`: 注册表里的默认值
///
/// # 返回值
///
/// 可执行文件路径；空命令行返回 `None`
#[cfg(any(target_os = "windows", test))]
fn exe_of_command(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let exe = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => command.split_whitespace().next()?,
    };
    if exe.is_empty() {
        return None;
    }
    Some(PathBuf::from(exe))
}

/// 注册快捷启动协议
///
/// - `over`: true 时额外注册 `modrinth://` 与 `colormc://` 以接管既有协议；`mml://` 总是注册
pub fn register_protocol_handler(over: bool) {
    if over {
        register_protocol_handler_inner("modrinth");
        register_protocol_handler_inner("colormc");
    }

    register_protocol_handler_inner("mml");
}

/// 取消快捷启动协议，只移除 `mml://`
pub fn delete_protocol_handler() {
    delete_protocol_handler_inner("mml");
}

/// 在注册表注册协议：HKEY_CLASSES_ROOT 下建 URL Protocol 项，指向当前 exe
///
/// - `id`: 协议名，如 `mml`
#[cfg(target_os = "windows")]
fn register_protocol_handler_inner(id: &str) {
    use winreg::{RegKey, enums::HKEY_CLASSES_ROOT};

    let hklm = RegKey::predef(HKEY_CLASSES_ROOT);
    hklm.open_subkey(id)
        .and_then(|data| {
            data.set_value("", &"URL:M²L Protocol").unwrap();
            data.set_value("URL Protocol", &"").unwrap();

            let file = std::env::current_exe().unwrap();

            let (key, _) = data.create_subkey("DefaultIcon").unwrap();
            key.set_value("", &format!("\"{}\",1", file.to_string_lossy()))
                .unwrap();

            let (key, _) = data.create_subkey("shell\\open\\command").unwrap();
            key.set_value("", &format!("\"{}\" \"%1\"", file.to_string_lossy()))
                .unwrap();

            Ok(())
        })
        .unwrap();
}

/// 取消协议注册的 Windows 实现
///
/// - `id`: 协议名
#[cfg(target_os = "windows")]
fn delete_protocol_handler_inner(id: &str) {
    use winreg::{RegKey, enums::HKEY_CLASSES_ROOT};

    /// 递归删除子键：注册的协议键带 DefaultIcon / shell\open\command 子键，
    /// 直接删会因子键非空失败；键不存在视为已删除
    fn delete_tree(key: &RegKey, name: &str) {
        if let Ok(sub) = key.open_subkey(name) {
            for child in sub.enum_keys().flatten() {
                delete_tree(&sub, &child);
            }
        }
        let _ = key.delete_subkey(name);
    }

    let hklm = RegKey::predef(HKEY_CLASSES_ROOT);
    delete_tree(&hklm, id);
}

/// 非 Windows 平台没有注册表，注册协议为空实现
///
/// - `id`: 协议名，此处未使用
#[cfg(not(target_os = "windows"))]
fn register_protocol_handler_inner(_id: &str) {}

/// 非 Windows 平台没有注册表，取消注册为空实现
///
/// - `id`: 协议名，此处未使用
#[cfg(not(target_os = "windows"))]
fn delete_protocol_handler_inner(_id: &str) {}

#[cfg(test)]
mod tests {
    use super::exe_of_command;
    use std::path::PathBuf;

    /// 带引号的命令行：取第一对引号里的路径，忽略后面的参数
    #[test]
    fn parses_quoted_command() {
        assert_eq!(
            exe_of_command(r#""C:\Program Files\ColorMC\ColorMC.Launcher.exe" "%1""#),
            Some(PathBuf::from(
                r"C:\Program Files\ColorMC\ColorMC.Launcher.exe"
            ))
        );
    }

    /// 不带引号：取第一个空白前的部分
    #[test]
    fn parses_unquoted_command() {
        assert_eq!(
            exe_of_command(r"D:\ColorMC\ColorMC.Launcher.exe %1"),
            Some(PathBuf::from(r"D:\ColorMC\ColorMC.Launcher.exe"))
        );
    }

    /// 空命令行不该造出一个空路径
    #[test]
    fn empty_command_has_no_exe() {
        assert_eq!(exe_of_command("  "), None);
    }
}
