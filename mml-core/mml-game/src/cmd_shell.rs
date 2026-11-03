//! 启动前 / 启动后命令的解析与解释器分派
//!
//! 命令文本按行拆：首行是程序与它的参数，之后每行算一个参数，带空格的路径不必加引号。
//! 首词指向脚本时按扩展名挑解释器 —— `.py` 用 Python、`.sh` 用 `sh`、`.ps1` 用 pwsh、
//! `.bat` / `.cmd` 用 `cmd` —— 同一条命令在 Windows 与 Linux / macOS 上都能写。
//!
//! 占位符由调用方先替换。命令里的路径分隔符按平台统一：Windows 上 `/` 换成 `\`（词首的
//! 开关不动），其它平台 `\` 换成 `/`，所以同一份配置两边都能写。

use std::path::{Path, PathBuf};

use mml_names::i18_items::error_type::{
    CoreResult, ErrorType, PathNotExistsData, ScriptRunnerMissingData, ScriptUnsupportedData,
};

/// 解析好的命令：实际要起的程序与它的参数
pub(crate) struct ShellCommand {
    /// 要启动的程序，脚本会被换成它的解释器
    pub(crate) program: PathBuf,
    /// 程序参数；脚本本身是解释器的第一个参数
    pub(crate) args: Vec<String>,
}

/// 解析命令文本，找出要启动的程序与参数
///
/// - `cmd`: 命令文本，占位符由调用方先替换
/// - `base`: 实例根目录，只写名字的程序也会在这里找
/// - `game`: 游戏目录
///
/// # 返回值
///
/// 见 [`ShellCommand`]；程序或它需要的解释器找不到时返回对应错误
pub(crate) fn resolve(cmd: &str, base: &Path, game: &Path) -> CoreResult<ShellCommand> {
    let cmd = native_separators(cmd);
    let mut lines = cmd.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next().unwrap_or_default();
    let mut tokens = split_command_line(first);
    if tokens.is_empty() {
        return Err(ErrorType::PathNotExists(PathNotExistsData {
            path: PathBuf::from(first),
        }));
    }

    let name = tokens.remove(0);

    // 首行剩下的词是参数，之后每行也是一个参数：带空格的路径不必加引号
    let mut args: Vec<String> = tokens;
    args.extend(lines.map(|line| unquote(line).to_string()));

    // `python.exe` 这类只写名字的：先按 PATH / 实例目录找，找不到再用启动器自带的
    let program = match find_program(&name, base, game) {
        Some(program) => program,
        None if is_python_name(&name) => find_python().ok_or_else(|| {
            ErrorType::PathNotExists(PathNotExistsData {
                path: PathBuf::from(&name),
            })
        })?,
        None => {
            return Err(ErrorType::PathNotExists(PathNotExistsData {
                path: PathBuf::from(&name),
            }));
        }
    };

    match script_runner(&program)? {
        Some((runner, prefix)) => {
            let mut all = prefix;
            all.push(program.display().to_string());
            all.extend(args);
            Ok(ShellCommand {
                program: runner,
                args: all,
            })
        }
        None => Ok(ShellCommand { program, args }),
    }
}

/// 把命令里的路径分隔符换成当前平台的写法
///
/// Windows 上只换词中间的 `/`，词首的 `/c`、`/?` 是开关，换了就跑不起来。
///
/// - `cmd`: 命令文本
///
/// # 返回值
///
/// 换好分隔符的命令文本
fn native_separators(cmd: &str) -> String {
    #[cfg(windows)]
    {
        let mut out = String::with_capacity(cmd.len());
        let mut word_start = true;

        for c in cmd.chars() {
            match c {
                c if c.is_whitespace() || c == '"' => {
                    word_start = true;
                    out.push(c);
                }
                '/' if !word_start => out.push('\\'),
                c => {
                    word_start = false;
                    out.push(c);
                }
            }
        }

        out
    }
    #[cfg(not(windows))]
    {
        cmd.replace('\\', "/")
    }
}

/// 脚本要用的解释器
///
/// - `path`: 已经找到的程序路径
///
/// # 返回值
///
/// 不是脚本返回 `Ok(None)`；是脚本返回解释器与它自己的参数；解释器缺失或在当前平台
/// 跑不了时返回对应错误
fn script_runner(path: &Path) -> CoreResult<Option<(PathBuf, Vec<String>)>> {
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    let missing = |runner: &str| {
        ErrorType::ScriptRunnerMissing(ScriptRunnerMissingData {
            path: path.to_path_buf(),
            runner: runner.to_string(),
        })
    };
    let unsupported = || {
        ErrorType::ScriptUnsupported(ScriptUnsupportedData {
            path: path.to_path_buf(),
        })
    };

    match ext.as_str() {
        "py" => match find_python() {
            Some(python) => Ok(Some((python, Vec::new()))),
            None => Err(missing("python3")),
        },
        "sh" => {
            if cfg!(windows) {
                return Err(unsupported());
            }
            let sh = if Path::new("/bin/sh").is_file() {
                PathBuf::from("/bin/sh")
            } else {
                find_on_path("sh").ok_or_else(|| missing("sh"))?
            };
            Ok(Some((sh, Vec::new())))
        }
        "ps1" => {
            if let Some(pwsh) = find_on_path("pwsh") {
                return Ok(Some((pwsh, vec!["-File".to_string()])));
            }
            // 只有 Windows PowerShell 时补上放行：默认执行策略会拦住脚本
            if let Some(ps) = find_on_path("powershell") {
                let prefix = ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]
                    .iter()
                    .map(|item| item.to_string())
                    .collect();
                return Ok(Some((ps, prefix)));
            }
            Err(missing("pwsh"))
        }
        "bat" | "cmd" => {
            if !cfg!(windows) {
                return Err(unsupported());
            }
            let cmd = find_on_path("cmd").ok_or_else(|| missing("cmd"))?;
            // `cmd /c call <脚本>`：走 call 才不会因为脚本路径带空格而把第一段当成命令
            let prefix = ["/c", "call"]
                .iter()
                .map(|item| item.to_string())
                .collect();
            Ok(Some((cmd, prefix)))
        }
        _ => Ok(None),
    }
}

/// 找 Python：先按 `PATH` 找系统的，再用启动器自带的 `tools/python`
///
/// # 返回值
///
/// 找到返回解释器路径；都没有返回 `None`
fn find_python() -> Option<PathBuf> {
    let mut stub = None;

    for name in ["python3", "python"] {
        let Some(found) = find_on_path(name) else {
            continue;
        };

        // 商店版只是"应用执行别名"，直接跑会去开商店，留作最后的退路
        if is_store_stub(&found) {
            stub = stub.or(Some(found));
            continue;
        }

        return Some(found);
    }

    mml_tools::python::find().or(stub)
}

/// 命令要不要 Python
///
/// - `cmd`: 命令文本
/// - `base`: 实例根目录
/// - `game`: 游戏目录
///
/// # 返回值
///
/// 脚本是 `.py`，或者程序名就是 `python` / `python3` / `py` 时为真
fn needs_python(cmd: &str, base: &Path, game: &Path) -> bool {
    let cmd = native_separators(cmd);
    let Some(first) = cmd.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return false;
    };
    let Some(name) = split_command_line(first).into_iter().next() else {
        return false;
    };

    if is_python_name(&name) {
        return true;
    }

    let program = find_program(&name, base, game).unwrap_or_else(|| PathBuf::from(&name));
    program
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
}

/// 程序名是不是 Python，`python` / `python3` / `py` 都算
///
/// - `name`: 命令里的程序名，可以带路径和扩展名
///
/// # 返回值
///
/// 是 Python 时为真
fn is_python_name(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    matches!(stem.as_str(), "python" | "python3" | "py")
}

/// 命令要用 Python 而本机没有时，先把运行环境下到启动器的 `tools/python`
///
/// - `cmd`: 命令文本，占位符由调用方先替换
/// - `base`: 实例根目录
/// - `game`: 游戏目录
///
/// # 返回值
///
/// 下载或解压失败时返回对应错误；用不上 Python 或已经有 Python 时直接返回
pub(crate) async fn ensure_runner(cmd: &str, base: &Path, game: &Path) -> CoreResult<()> {
    if find_python().is_some() || !needs_python(cmd, base, game) {
        return Ok(());
    }

    // 只有 Windows 有现成的嵌入式包；其它平台用系统自带的 python3
    #[cfg(windows)]
    {
        mml_tools::python::download().await?;
    }

    Ok(())
}

/// Windows 商店版 Python 的"应用执行别名"只是个空壳
fn is_store_stub(path: &Path) -> bool {
    cfg!(windows) && path.to_string_lossy().to_ascii_lowercase().contains(r"\windowsapps\")
}

/// 按 Windows 命令行习惯把一行拆成程序名与参数
///
/// - `line`: 命令行文本，双引号内的空白不拆开
///
/// # 返回值
///
/// 拆出的词；空行返回空表，引号本身不进结果
fn split_command_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    let mut quoted = false;

    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }

    if started {
        out.push(cur);
    }

    out
}

/// 去掉参数两端成对的英文双引号
fn unquote(item: &str) -> &str {
    let item = item.trim();
    match item.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        Some(inner) => inner,
        None => item,
    }
}

/// 裸程序名要补的扩展名，取值来自 `PATHEXT`
fn exec_exts() -> Vec<String> {
    #[cfg(windows)]
    {
        let raw = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        let mut out: Vec<String> = Vec::new();
        for ext in raw.split(';').map(str::trim).filter(|e| !e.is_empty()) {
            out.push(ext.to_string());
            out.push(ext.to_ascii_lowercase());
        }
        out.dedup();
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// 在候选目录里找程序文件，名称没有扩展名时依次补 `PATHEXT` 里的扩展名
///
/// - `name`: 程序名或相对路径
/// - `dirs`: 候选目录
///
/// # 返回值
///
/// 命中返回该文件的路径
fn find_in_dirs<I>(name: &str, dirs: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let bare = Path::new(name).extension().is_none();

    for dir in dirs {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }

        if bare {
            for ext in exec_exts() {
                let cand = dir.join(format!("{name}{ext}"));
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }

    None
}

/// 在 `PATH` 里找一个程序
///
/// - `name`: 程序名，不带路径
///
/// # 返回值
///
/// 找到返回它的完整路径
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    find_in_dirs(name, std::env::split_paths(&path))
}

/// 找启动前 / 启动后命令要跑的程序
///
/// 依次按给定路径、实例根目录、游戏目录找；只写名字时再按 `PATH` 找，
/// 与在命令行里直接敲这个名字的行为一致。
///
/// - `name`: 程序名，可以带路径，也可以只有名字
/// - `base`: 实例根目录
/// - `game`: 游戏目录
///
/// # 返回值
///
/// 找到返回程序路径，找不到返回 `None`
fn find_program(name: &str, base: &Path, game: &Path) -> Option<PathBuf> {
    let direct = PathBuf::from(name);
    if direct.is_file() {
        return Some(direct);
    }

    if let Some(found) = find_in_dirs(name, [base.to_path_buf(), game.to_path_buf()]) {
        return Some(found);
    }

    // 带路径的分隔符说明用户指定了位置，查 PATH 只会找一个同名的别的程序
    if name.contains(['\\', '/', ':']) {
        return None;
    }

    find_on_path(name)
}

#[cfg(test)]
#[path = "../tests/unit/cmd_shell_tests.rs"]
mod tests;
