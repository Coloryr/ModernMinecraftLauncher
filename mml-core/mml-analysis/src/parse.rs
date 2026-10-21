//! 崩溃报告与游戏日志的解析

use std::sync::LazyLock;

use regex::Regex;

use crate::model::{CrashReport, Environment, ExceptionFrame, LoaderKind, LogKind, ModEntry};

/// 异常行：可选的 `Caused by:` / `Suppressed:` 前缀加全限定类名
static RE_EXCEPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:Caused by: |Suppressed: )?((?:[A-Za-z_$][A-Za-z0-9_$]*\.)+[A-Za-z_$][A-Za-z0-9_$]*)(?::\s?(.*))?$",
    )
    .unwrap()
});

/// 崩溃报告里的内存行，取 `up to ... (N MiB)`；1.7.10 的报告写的是 MB
static RE_MEMORY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)up to \d+ bytes \((\d+) M(?:i)?B\)").unwrap());

/// 游戏日志行首的时间戳
static RE_LOG_TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[\d{2}:\d{2}:\d{2}\]").unwrap());

/// 解析一份崩溃报告或日志
///
/// - `text`: 日志全文
///
/// # 返回值
///
/// 见 [`CrashReport`]
pub(crate) fn parse(text: &str) -> CrashReport {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text.lines().collect();

    let kind = detect_kind(text, &lines);
    let description = find_value(&lines, "Description:");
    let time = find_value(&lines, "Time:");
    let (exception, causes, detail) = parse_exceptions(&lines);
    let environment = parse_environment(text, &lines);

    CrashReport {
        kind,
        description,
        time,
        exception,
        causes,
        environment,
        lower: text.to_lowercase(),
        raw: text.to_string(),
        detail,
    }
}

/// 判断输入属于哪一种日志
fn detect_kind(text: &str, lines: &[&str]) -> LogKind {
    if text.contains("---- Minecraft Crash Report ----") {
        return LogKind::CrashReport;
    }

    let lower = text.to_lowercase();
    if lower.contains("a fatal error has been detected by the java runtime environment")
        || lower.contains("exception_access_violation")
        || lower.contains("sigsegv")
    {
        return LogKind::JvmFatal;
    }

    if lines
        .iter()
        .any(|line| RE_LOG_TIME.is_match(line) || line.contains("[main/INFO]"))
    {
        return LogKind::GameLog;
    }

    LogKind::Unknown
}

/// 取 `Key: value` 行的值，只认第一次出现
fn find_value(lines: &[&str], key: &str) -> Option<String> {
    lines.iter().find_map(|line| {
        line.trim()
            .strip_prefix(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

/// 解析异常链，同时收集异常与栈帧原文
///
/// # 返回值
///
/// 最外层异常、`Caused by:` 链与规则匹配用的正文
fn parse_exceptions(lines: &[&str]) -> (Option<ExceptionFrame>, Vec<ExceptionFrame>, String) {
    let mut exception = None;
    let mut causes = Vec::new();
    let mut detail = Vec::new();

    for line in lines {
        let content = strip_log_prefix(line);

        if let Some(captures) = RE_EXCEPTION.captures(content) {
            let class = captures.get(1).map(|m| m.as_str()).unwrap_or_default();
            let message = captures.get(2).map(|m| m.as_str()).unwrap_or_default();
            let frame = ExceptionFrame {
                class: class.to_string(),
                message: message.trim().to_string(),
            };

            if content.starts_with("Caused by:") || content.starts_with("Suppressed:") {
                causes.push(frame);
            } else if exception.is_none() {
                exception = Some(frame);
            }

            detail.push(content.trim().to_string());
            continue;
        }

        let trimmed = content.trim_start();
        if trimmed.starts_with("at ")
            || trimmed.starts_with("... ")
            || content.starts_with("Suppressed:")
        {
            detail.push(trimmed.to_string());
        } else if line.starts_with('#') {
            // hs_err 的 `#  EXCEPTION_ACCESS_VIOLATION ...` 与 Problematic frame 都在这里
            detail.push(line.trim().to_string());
        }
    }

    (exception, causes, detail.join("\n"))
}

/// 去掉游戏日志行首的 `[时间] [线程/级别]:` 前缀
fn strip_log_prefix(line: &str) -> &str {
    if !line.starts_with('[') {
        return line;
    }

    if let Some(pos) = line.rfind("]: ") {
        return &line[pos + 3..];
    }
    if let Some(pos) = line.rfind("]:") {
        return &line[pos + 2..];
    }

    line
}

/// 解析运行环境与模组列表
fn parse_environment(text: &str, lines: &[&str]) -> Environment {
    let mut env = Environment::default();

    for line in lines {
        let trimmed = line.trim();

        if let Some(value) = trimmed.strip_prefix("Minecraft Version: ") {
            env.minecraft_version = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("Java Version: ") {
            let (version, vendor) = value
                .split_once(',')
                .map(|(version, vendor)| (version.trim(), vendor.trim()))
                .unwrap_or((value.trim(), ""));
            env.java_version = Some(version.to_string());
            if !vendor.is_empty() {
                env.java_vendor = Some(vendor.to_string());
            }
        } else if let Some(value) = trimmed.strip_prefix("Operating System: ") {
            env.os = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("Memory: ")
            && let Some(captures) = RE_MEMORY.captures(value)
            && let Some(mib) = captures.get(1)
        {
            env.memory_max_mb = mib.as_str().parse().ok();
        }
    }

    env.mods = parse_mods(lines);
    // NeoForge 的崩溃报告没有 Mod List 表，只在正文里逐条列"模组加载问题"
    for entry in parse_mod_issues(lines) {
        if !env
            .mods
            .iter()
            .any(|item| item.id.eq_ignore_ascii_case(&entry.id))
        {
            env.mods.push(entry);
        }
    }
    env.loader = detect_loader(text, &env.mods);
    env.loader_version = env.loader.and_then(|loader| {
        let id = match loader {
            LoaderKind::Forge => "forge",
            LoaderKind::NeoForge => "neoforge",
            LoaderKind::Fabric => "fabricloader",
            LoaderKind::Quilt => "quilt_loader",
        };
        env.mod_of(id).map(|entry| entry.version.clone())
    });

    if env.minecraft_version.is_none() {
        env.minecraft_version = env.mod_of("minecraft").map(|entry| entry.version.clone());
    }

    env
}

/// 解析 NeoForge 的模组加载问题段落
///
/// 报告形如 `-- Mod loading issue for: ferritecore --`，段内的 `Mod version:` 给版本。
/// NeoForge 的崩溃报告不带 Mod List 表，不解析这段就一个模组都拿不到。
fn parse_mod_issues(lines: &[&str]) -> Vec<ModEntry> {
    let mut mods: Vec<ModEntry> = Vec::new();
    let mut current: Option<usize> = None;

    for line in lines {
        let trimmed = line.trim();

        if let Some(id) = trimmed
            .strip_prefix("-- Mod loading issue for: ")
            .and_then(|rest| rest.strip_suffix(" --"))
        {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            if let Some(index) = mods.iter().position(|item| item.id.eq_ignore_ascii_case(id)) {
                current = Some(index);
            } else {
                mods.push(ModEntry {
                    id: id.to_string(),
                    name: id.to_string(),
                    version: String::new(),
                });
                current = Some(mods.len() - 1);
            }
        } else if let Some(version) = trimmed.strip_prefix("Mod version: ")
            && let Some(index) = current
        {
            mods[index].version = version.trim().to_string();
        }
    }

    mods
}

/// 解析 `Fabric Mods:` / `Mod List:` / `Loaded Mods:` 段落
fn parse_mods(lines: &[&str]) -> Vec<ModEntry> {
    let mut mods: Vec<ModEntry> = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();

        let is_fabric = trimmed.starts_with("Fabric Mods:");
        let is_table = trimmed.starts_with("Mod List:") || trimmed.starts_with("Loaded Mods:");
        if !is_fabric && !is_table {
            index += 1;
            continue;
        }

        let header_indent = indent_of(line);
        index += 1;

        while index < lines.len() {
            let entry_line = lines[index];
            let entry_trimmed = entry_line.trim();

            if entry_trimmed.is_empty() {
                index += 1;
                continue;
            }
            if indent_of(entry_line) <= header_indent || entry_trimmed.starts_with("-- ") {
                break;
            }

            if let Some(entry) = parse_mod_line(entry_trimmed)
                && !mods
                    .iter()
                    .any(|item| item.id.eq_ignore_ascii_case(&entry.id))
            {
                mods.push(entry);
            }

            index += 1;
        }
    }

    mods
}

/// 解析模组列表的一行
///
/// 支持 Forge 的 `文件 |名字 |id |版本 |侧 |清单` 表，以及 Fabric 的 `id: 名字 版本`。
fn parse_mod_line(line: &str) -> Option<ModEntry> {
    if line.contains('|') {
        let fields: Vec<&str> = line.split('|').map(str::trim).collect();
        if fields.len() < 4 || fields[2].is_empty() {
            return None;
        }
        return Some(ModEntry {
            id: fields[2].to_string(),
            name: if fields[1].is_empty() {
                fields[2].to_string()
            } else {
                fields[1].to_string()
            },
            version: fields[3].to_string(),
        });
    }

    let (id, rest) = line.split_once(": ")?;
    let id = id.trim();
    if id.is_empty() || id.contains(' ') || id.contains('/') || id.contains('\\') {
        return None;
    }

    let rest = rest.trim();
    if rest.is_empty() {
        return Some(ModEntry {
            id: id.to_string(),
            name: id.to_string(),
            version: String::new(),
        });
    }

    let mut parts = rest.rsplitn(2, ' ');
    let last = parts.next().unwrap_or_default();
    let has_version = last.chars().any(|c| c.is_ascii_digit());
    let (name, version) = if has_version {
        (parts.next().unwrap_or_default().trim(), last)
    } else {
        (rest, "")
    };

    Some(ModEntry {
        id: id.to_string(),
        name: if name.is_empty() {
            id.to_string()
        } else {
            name.to_string()
        },
        version: version.to_string(),
    })
}

/// 行首空白宽度，制表符与空格都按一个字符算
fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

/// 从模组列表与正文判断加载器
fn detect_loader(text: &str, mods: &[ModEntry]) -> Option<LoaderKind> {
    let lower = text.to_lowercase();
    let has_mod = |id: &str| mods.iter().any(|m| m.id.eq_ignore_ascii_case(id));

    if has_mod("neoforge") || lower.contains("net.neoforged") {
        Some(LoaderKind::NeoForge)
    } else if has_mod("fabricloader")
        || lower.contains("fabricloader")
        || lower.contains("fabric mods:")
    {
        Some(LoaderKind::Fabric)
    } else if has_mod("quilt_loader") || lower.contains("quilt_loader") {
        Some(LoaderKind::Quilt)
    } else if has_mod("forge") || lower.contains("net.minecraftforge") {
        Some(LoaderKind::Forge)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FABRIC: &str = r#"---- Minecraft Crash Report ----
// Don't be sad, have a hug! <3

Time: 2026-10-20 21:15:32
Description: Mod loading

net.fabricmc.loader.impl.FormattedException: Mod resolution encountered an error. Please try again with a different set of mods.
	at net.fabricmc.loader.impl.FabricLoaderImpl.load(FabricLoaderImpl.java:220)
Caused by: net.fabricmc.loader.impl.discovery.ModResolutionException: Some of your mods are incompatible with the game or each other!
	... 3 more

-- System Details --
Details:
	Minecraft Version: 1.20.1
	Java Version: 17.0.8, Microsoft
	Memory: 123456789 bytes (117 MiB) / 536870912 bytes (512 MiB) up to 2147483648 bytes (2048 MiB)
	Fabric Mods: 
		fabricloader: Fabric Loader 0.15.11
		minecraft: Minecraft 1.20.1
		sodium: Sodium 0.5.8
"#;

    #[test]
    fn parses_crash_report_header_and_exception() {
        let report = parse(FABRIC);

        assert_eq!(report.kind, LogKind::CrashReport);
        assert_eq!(report.description.as_deref(), Some("Mod loading"));
        assert_eq!(report.time.as_deref(), Some("2026-10-20 21:15:32"));
        assert_eq!(
            report.exception.as_ref().map(|e| e.class.as_str()),
            Some("net.fabricmc.loader.impl.FormattedException")
        );
        assert_eq!(report.causes.len(), 1);
    }

    #[test]
    fn parses_environment_and_mods() {
        let env = parse(FABRIC).environment;

        assert_eq!(env.minecraft_version.as_deref(), Some("1.20.1"));
        assert_eq!(env.java_version.as_deref(), Some("17.0.8"));
        assert_eq!(env.java_vendor.as_deref(), Some("Microsoft"));
        assert_eq!(env.memory_max_mb, Some(2048));
        assert_eq!(env.loader, Some(LoaderKind::Fabric));
        assert_eq!(env.loader_version.as_deref(), Some("0.15.11"));
        assert_eq!(env.mods.len(), 3);
        assert_eq!(
            env.mod_of("sodium").map(|m| m.version.as_str()),
            Some("0.5.8")
        );
    }

    #[test]
    fn parses_forge_mod_table() {
        let text = r#"---- Minecraft Crash Report ----
Description: Mod loading

-- System Details --
Details:
	Minecraft Version: 1.20.1
	Mod List: 
		sodium-1.20.1.jar |Sodium |sodium |0.5.8 |COMMON_SET|Manifest: a:b
		forge-1.20.1-47.2.0.jar |Forge |forge |47.2.0 |COMMON_SET|Manifest: c:d
"#;
        let env = parse(text).environment;

        assert_eq!(env.loader, Some(LoaderKind::Forge));
        assert_eq!(env.loader_version.as_deref(), Some("47.2.0"));
        assert_eq!(
            env.mod_of("sodium").map(|m| m.name.as_str()),
            Some("Sodium")
        );
    }

    #[test]
    fn parses_exception_from_game_log() {
        let text = r#"[21:00:00] [main/INFO]: Loading Minecraft 1.20.1
[21:00:05] [main/ERROR]: java.lang.NoSuchMethodError: 'void net.minecraft.class_310.method_1551()'
"#;
        let report = parse(text);

        assert_eq!(report.kind, LogKind::GameLog);
        assert_eq!(
            report.exception.as_ref().map(|e| e.class.as_str()),
            Some("java.lang.NoSuchMethodError")
        );
    }

    #[test]
    fn ignores_plain_text() {
        let report = parse("just a line\nand another");

        assert_eq!(report.kind, LogKind::Unknown);
        assert!(report.exception.is_none());
        assert!(report.environment.mods.is_empty());
    }
}
