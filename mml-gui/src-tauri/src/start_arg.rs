//! 启动参数：命令行与 URI 形式的"启动请求"
//!
//! 对照 ColorMC 的 `ColorMCGui.StartArg` 与 `GuiNames`，格式保持一致，协议换成我们自己的 `mml:`：
//! - `--game <uuid>`：启动实例，快捷方式用
//! - `--install <路径>` / `--import <路径>`：用该文件 / 文件夹打开「添加实例」
//! - `mml://run/<uuid>`：启动实例
//! - `mml://install/<路径>`：同上，用路径打开「添加实例」
//!
//! 提权与重启还会在参数前面塞 `--run-dir <目录>` 与 `--restart`：前者由 [`run_dir`]
//! 在解析启动请求之前取走，后者只是「跳过单开检查」的标记。
//!
//! 快捷启动钩子（ColorMC 的 `--register`）与 `modrinth:` 协议还没实现：前者我们没有那套钩子，
//! 后者要资源窗口支持按项目 id 打开。
//!
//! 解析结果**先存下来**，等前端初始化完成再取走执行（对应 ColorMC 里按 `IsInit` 分的两个分支）：
//! 启动瞬间内核实例表还没加载，这时候直接动手会找不到实例。

use std::sync::{LazyLock, Mutex};

use crate::dtos::StartArgDto;

/// 命令行：启动实例
const CMD_GAME: &str = "--game";
/// 命令行：用文件打开添加实例
const CMD_INSTALL: &str = "--install";
/// 命令行：用文件打开添加实例，MMC 风格的别名
const CMD_IMPORT: &str = "--import";
/// 自定义协议前缀
const URL_PREFIX: &str = "mml:";
/// 协议段：启动实例
const URL_RUN: &str = "//run/";
/// 协议段：用文件打开添加实例
const URL_INSTALL: &str = "//install/";

/// 待办：前端初始化完成后取走一次
static PENDING: LazyLock<Mutex<Option<StartArgDto>>> = LazyLock::new(|| Mutex::new(None));

/// 解析启动参数并记下待办
///
/// 认不出 / 参数不全时什么都不做。
///
/// - `args`: 进程参数，第 0 项是可执行文件本身，从第 1 项开始认
pub fn collect<I: IntoIterator<Item = String>>(args: I) {
    let args: Vec<String> = args.into_iter().collect();
    if let Some(parsed) = parse(args.get(1..).unwrap_or_default()) {
        *PENDING.lock().unwrap() = Some(parsed);
    }
}

/// 解析一条启动请求
///
/// 先剥掉提权与重启用的前置参数，再认命令行、`mml:` 协议与裸文件路径三种形式。
///
/// - `args`: 不含可执行文件的参数，前面可以带 `--run-dir <目录>` 或 `--restart`
///
/// # 返回值
///
/// 解析出的启动请求；认不出时返回 `None`
fn parse(args: &[String]) -> Option<StartArgDto> {
    let first = args.first()?;

    if first == "--run-dir" {
        return parse(args.get(2..)?);
    }
    if first == mml_names::names::COMMAND_RESTART {
        return parse(&args[1..]);
    }

    if first == CMD_GAME {
        args.get(1).map(|uuid| StartArgDto::game(uuid))
    } else if first == CMD_INSTALL || first == CMD_IMPORT {
        args.get(1).map(|path| StartArgDto::install(path))
    } else if let Some(rest) = first.strip_prefix(URL_PREFIX) {
        if let Some(uuid) = rest.strip_prefix(URL_RUN) {
            Some(StartArgDto::game(uuid))
        } else {
            rest.strip_prefix(URL_INSTALL)
                .map(|path| StartArgDto::install(path))
        }
    } else if std::path::Path::new(first).is_file() {
        Some(StartArgDto::install(first))
    } else {
        None
    }
}

/// 读取提权和重启时传递的数据目录
///
/// - `args`: 不含可执行文件的参数列表
///
/// # 返回值
///
/// 首个显式数据目录；没有指定时返回 `None`
pub fn run_dir(args: impl IntoIterator<Item = String>) -> Option<std::path::PathBuf> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--run-dir" {
            return args.next().map(std::path::PathBuf::from);
        }
        if arg == CMD_INSTALL || arg == CMD_IMPORT || arg == CMD_GAME {
            args.next();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn dropped_zip_and_install_flag_use_same_request() {
        let dir = crate::test_support::temp_dir("dropped-server-pack");
        let zip = dir.join("server pack.zip");
        std::fs::write(&zip, b"zip").unwrap();
        let path = zip.to_string_lossy().to_string();
        let dropped = super::parse(&[path.clone()]).unwrap();
        let flagged = super::parse(&["--install".into(), path.clone()]).unwrap();
        assert_eq!(dropped.action, "install");
        assert_eq!(dropped.value, flagged.value);
        assert_eq!(dropped.value, path);
        let restarted = super::parse(&[
            "--restart".into(),
            "--run-dir".into(),
            dir.to_string_lossy().into_owned(),
            path.clone(),
        ])
        .unwrap();
        assert_eq!(restarted.value, path);
        assert_eq!(
            super::run_dir(["--run-dir".into(), dir.to_string_lossy().into_owned()]),
            Some(dir.clone())
        );
        crate::test_support::cleanup(&[&dir]);
    }
}

/// 取走待办，取过一次就没了
///
/// # 返回值
///
/// 有启动参数时返回它，否则 `None`
pub fn take() -> Option<StartArgDto> {
    PENDING.lock().unwrap().take()
}
