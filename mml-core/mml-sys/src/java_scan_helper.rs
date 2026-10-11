//! Java 运行时扫描：注册表 / 包管理器 / 常见安装目录

use std::{collections::HashSet, path::PathBuf};

/// 搜索本机 Java 运行时：where 命令 + 注册表 + 常见安装目录
///
/// - `java_paths`: 搜索结果集合，就地插入发现的 javaw.exe 路径
#[cfg(target_os = "windows")]
pub fn find_java_inner(java_paths: &mut HashSet<PathBuf>) {
    use mml_names::i18_items::error_type::CoreResult;
    use mml_names::i18_items::error_type::DataNotFoundData;
    use mml_names::i18_items::error_type::ErrorType;
    use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

    use crate::process_helper;

    /// 读取 Oracle JDK/JRE 注册表项下的 JavaHome 值
    fn get_oracle_java_from_registry(key_path: &str) -> CoreResult<Vec<PathBuf>> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let subkey = hklm.open_subkey(key_path).map_err(|_| {
            ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_path.to_string()))
        })?;
        let mut paths = Vec::new();

        for name in subkey.enum_keys() {
            let key_name = name.map_err(|_| {
                ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_path.to_string()))
            })?;
            let key = subkey
                .open_subkey(&key_name)
                .map_err(|_| ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_name)))?;
            if let Ok(java_home) = key.get_value::<String, _>("JavaHome") {
                paths.push(PathBuf::from(java_home));
            }
        }

        Ok(paths)
    }

    /// 读取 Eclipse Adoptium 注册表项下 `...\hotspot\MSI` 的 Path 值
    fn get_adoptium_java_from_registry(key_path: &str) -> CoreResult<Vec<PathBuf>> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let subkey = hklm.open_subkey(key_path).map_err(|_| {
            ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_path.to_string()))
        })?;
        let mut paths = Vec::new();

        for name in subkey.enum_keys() {
            let key_name = name.map_err(|_| {
                ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_path.to_string()))
            })? + r"\hotspot\MSI";
            let key = subkey
                .open_subkey(&key_name)
                .map_err(|_| ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_name)))?;
            if let Ok(java_home) = key.get_value::<String, _>("Path") {
                paths.push(PathBuf::from(java_home));
            }
        }

        Ok(paths)
    }

    /// 读取 Azul Zulu 注册表项下的 InstallationPath 值
    fn get_zulu_java_from_registry(key_path: &str) -> CoreResult<Vec<PathBuf>> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let subkey = hklm.open_subkey(key_path).map_err(|_| {
            ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_path.to_string()))
        })?;
        let mut paths = Vec::new();

        for name in subkey.enum_keys() {
            let key_name = name.map_err(|_| {
                ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_path.to_string()))
            })?;
            let key = subkey
                .open_subkey(&key_name)
                .map_err(|_| ErrorType::DataNotFound(DataNotFoundData::RegistryKey(key_name)))?;
            if let Ok(java_home) = key.get_value::<String, _>("InstallationPath") {
                paths.push(PathBuf::from(java_home));
            }
        }

        Ok(paths)
    }

    if let Ok(paths) = process_helper::run_command_arg("where", &["javaw.exe"]) {
        for path in paths {
            java_paths.insert(PathBuf::from(path));
        }
    }

    if let Ok(install_paths) =
        get_oracle_java_from_registry(r"SOFTWARE\JavaSoft\Java Runtime Environment\")
    {
        for install_path in install_paths {
            let java_exe = install_path.join("bin").join("javaw.exe");
            if java_exe.exists() {
                java_paths.insert(java_exe);
            }
        }
    }
    if let Ok(install_paths) = get_oracle_java_from_registry(r"SOFTWARE\JavaSoft\JDK\") {
        for install_path in install_paths {
            let java_exe = install_path.join("bin").join("javaw.exe");
            if java_exe.exists() {
                java_paths.insert(java_exe);
            }
        }
    }
    if let Ok(install_paths) = get_adoptium_java_from_registry(r"SOFTWARE\Eclipse Adoptium\JDK\") {
        for install_path in install_paths {
            let java_exe = install_path.join("bin").join("javaw.exe");
            if java_exe.exists() {
                java_paths.insert(java_exe);
            }
        }
    }
    if let Ok(install_paths) = get_zulu_java_from_registry(r"SOFTWARE\Azul Systems\Zulu\") {
        for install_path in install_paths {
            let java_exe = install_path.join("bin").join("javaw.exe");
            if java_exe.exists() {
                java_paths.insert(java_exe);
            }
        }
    }

    let common_paths = [
        r"C:\Program Files\Java",
        r"C:\Program Files (x86)\Java",
        r"C:\Program Files\Eclipse Adoptium",
        r"C:\Program Files\Azul\Zulu",
    ];

    for base_path in common_paths {
        if let Ok(entries) = std::fs::read_dir(base_path) {
            for entry in entries.flatten() {
                let java_exe = entry.path().join("bin").join("javaw.exe");
                if java_exe.exists() {
                    java_paths.insert(java_exe);
                }
            }
        }
    }
}

/// 搜索本机 Java 运行时：which 命令 + 发行版包管理器 + 常见安装目录
///
/// - `java_paths`: 搜索结果集合，就地插入发现的 java 路径
#[cfg(target_os = "linux")]
pub(crate) fn find_java_inner(java_paths: &mut HashSet<PathBuf>) {
    use std::path::Path;

    use crate::process_helper;

    /// 把符号链接解析为真实路径，非链接时原样返回
    fn resolve_symlink(path: &str) -> Result<PathBuf, std::io::Error> {
        let path = Path::new(path);
        if path.is_symlink() {
            std::fs::read_link(path)
        } else {
            Ok(path.to_path_buf())
        }
    }

    /// 在 Arch Linux 上通过 pacman 查找 Java
    fn find_java_on_arch(java_paths: &mut HashSet<PathBuf>) {
        // 查询已安装的 JDK/JRE 包
        if let Ok(packages) = process_helper::run_command_arg("pacman", &["-Qs", "jre|jdk"]) {
            for package_line in packages {
                let parts: Vec<&str> = package_line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                let package_name = parts[0];

                // 查询包文件列表
                if let Ok(files) = process_helper::run_command_arg("pacman", &["-Ql", package_name])
                {
                    for file in files {
                        if file.ends_with("/bin/java") {
                            let parts: Vec<&str> = file.split_whitespace().collect();
                            if parts.len() >= 2 {
                                if let Ok(resolved) = resolve_symlink(parts[1]) {
                                    java_paths.insert(resolved);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// 扫描常见 Java 安装目录
    fn scan_common_java_dirs(java_paths: &mut HashSet<PathBuf>) {
        let common_dirs = ["/usr/lib/jvm", "/usr/java", "/opt/java", "/opt/jdk"];

        for dir in common_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let java_exe = entry.path().join("bin").join("java");
                    if java_exe.exists() {
                        java_paths.insert(java_exe);
                    }
                }
            }
        }
    }

    if let Ok(paths) = process_helper::run_command_arg("which", &["java"]) {
        for path in paths {
            if let Ok(resolved) = resolve_symlink(&path) {
                java_paths.insert(resolved);
            }
        }
    }

    let system_info = crate::get_system_info();

    match system_info.distribution.as_str() {
        "ubuntu" | "debian" => {
            if let Ok(paths) =
                process_helper::run_command_arg("update-alternatives", &["--list", "java"])
            {
                for path in paths {
                    if let Ok(resolved) = resolve_symlink(&path) {
                        java_paths.insert(resolved);
                    }
                }
            }
        }
        "arch" | "manjaro" => {
            find_java_on_arch(java_paths);
        }
        _ => {
            scan_common_java_dirs(java_paths);
        }
    }

    scan_common_java_dirs(java_paths);
}

/// 搜索本机 Java 运行时：java_home 命令 + 系统 JavaVirtualMachines 目录
///
/// - `java_paths`: 搜索结果集合，就地插入发现的 java 路径
#[cfg(target_os = "macos")]
pub(crate) fn find_java_inner(java_paths: &mut HashSet<PathBuf>) {
    use std::path::Path;
    use std::process::Command;

    // 使用 /usr/libexec/java_home
    if let Ok(output) = Command::new("/usr/libexec/java_home").arg("-V").output() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            if line.contains("Java SE") || line.contains("JDK") {
                if let Some(path_start) = line.find('/') {
                    let path = &line[path_start..];
                    if let Some(path_end) = path.find(".jdk") {
                        let java_home = &path[..path_end + 4];
                        let java_exe = Path::new(java_home).join("bin").join("java");
                        if java_exe.exists() {
                            java_paths.insert(java_exe);
                        }
                    }
                }
            }
        }
    }

    // 扫描 /Library/Java/JavaVirtualMachines
    if let Ok(entries) = std::fs::read_dir("/Library/Java/JavaVirtualMachines") {
        for entry in entries.flatten() {
            let java_exe = entry
                .path()
                .join("Contents")
                .join("Home")
                .join("bin")
                .join("java");
            if java_exe.exists() {
                java_paths.insert(java_exe);
            }
        }
    }
}
