//! URL 协议注册：mml:// 等快捷启动协议，写注册表 / 系统配置

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
