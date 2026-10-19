//! M²L 的 IPC 源生成器
//!
//! 扫 Rust 源码，收集 IPC 命令、事件与跨 IPC 类型，产出：
//!
//! - `mml-ipc/src/types/`：按来源 .rs 拆开的 DTO，一个文件一个模块，外加 barrel
//! - `mml-ipc/src/bindings.ts`：解耦传输层的命令包装，再导出全部类型
//! - `mml-ipc/src/listens.ts`：事件名常量、`EventPayloads` 与订阅函数
//! - `$OUT_DIR/invokes_gen.rs`：`tauri_commands!` 注册宏 + `listens` 常量
//!
//! [`generate`] 是纯函数：只读源码、只返回文本，不碰环境变量也不写文件。
//! 写文件与 cargo 指令由调用方 `src-tauri/build.rs` 负责。
//!
//! 命令标记是 `#[gui_macros::command]`，事件标记是 `#[gui_macros::emit]`；
//! 两者都是裸文本匹配，注释里写出字面量同样会被当成标记。

mod emit_rs;
mod emit_ts;
pub mod scan;
pub mod ts;

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    path::{Path, PathBuf},
};

use scan::{ScannedEvent, commands_in, events_in, fns_after, module_path, scan_types, walk_rs};
use ts::{TsParam, result_ok, ts_params, ts_type};

/// 抓命令的标记，本仓约定，不做成配置
const CMD_MARK: &str = "#[gui_macros::command]";

/// TS 类型的来源：只有这些位置的类型会进 `types/`
///
/// 刻意收窄范围：`collect_utils.rs` 那种磁盘类型不该出现在 IPC 类型里。
#[derive(Debug, Clone)]
pub struct TypeSources {
    /// 目录前缀，相对 `src_dir` 且带结尾 `/`；其下所有类型都收
    pub dirs: Vec<String>,
    /// 单文件，相对 `src_dir`；只收其中的枚举
    pub enum_files: Vec<String>,
}

impl Default for TypeSources {
    /// 默认来源：`dtos/` 与 `models/` 全部类型，外加 `gui_config.rs` 里的枚举
    ///
    /// # 返回值
    ///
    /// 默认配置
    fn default() -> Self {
        Self {
            dirs: vec!["dtos/".into(), "models/".into()],
            enum_files: vec!["gui_config.rs".into()],
        }
    }
}

/// 生成配置
#[derive(Debug, Clone, Default)]
pub struct GenConfig {
    /// 要扫描的 Rust 源码根，通常是 `<crate>/src`；必须存在，否则 [`generate`] 报错
    pub src_dir: PathBuf,
    /// TS 类型的来源，默认 [`TypeSources::default`]
    pub type_sources: TypeSources,
    /// 外部 crate 类型 → TS 类型覆盖，如 `mml-names` 的 `Lang`。
    /// 不在表里的类型名按「本 crate 生成」处理，直接用原名
    pub external_types: BTreeMap<String, String>,
}

/// 生成结果，调用方自己决定往哪写
#[derive(Debug, Clone, PartialEq)]
pub struct Generated {
    /// `mml-ipc/src/bindings.ts`：解耦传输层的命令包装 + 再导出类型
    pub bindings_ts: String,
    /// `mml-ipc/src/listens.ts`：事件名常量 + `EventPayloads` + 订阅函数
    pub listens_ts: String,
    /// `mml-ipc/src/` 下的 DTO 文件，每个来源 .rs 一份，外加 barrel `types/index.ts`
    pub type_files: Vec<GeneratedFile>,
    /// `$OUT_DIR/invokes_gen.rs`：`tauri_commands!` 注册宏 + `listens` 常量
    pub invokes_gen_rs: String,
}

/// 一个生成文件：相对 `mml-ipc/src` 的路径与内容
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedFile {
    /// 相对 `mml-ipc/src` 的路径，如 `types/dtos/instance_dto.ts`
    pub rel_path: String,
    /// 整个文件内容
    pub content: String,
}

/// 生成失败的原因
#[derive(Debug)]
pub enum GenError {
    /// `src_dir` 不存在，通常是忘了设置 `GenConfig.src_dir`
    SrcDirMissing(PathBuf),
}

impl fmt::Display for GenError {
    /// 把错误渲染成中文提示
    ///
    /// - `f`: 格式化输出目标
    ///
    /// # 返回值
    ///
    /// `fmt::Result`；只有写失败才返回 `Err`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SrcDirMissing(p) => write!(f, "源码目录不存在: {}", p.display()),
        }
    }
}

impl std::error::Error for GenError {}

/// 一条命令的名字与它在 crate 内的完整路径，供生成注册表用
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CommandRef {
    /// 命令名，即 Rust 函数名
    pub(crate) name: String,
    /// crate 内的完整路径，如 `windows::resource::mods::resource_mod_notes`
    pub(crate) path: String,
}

/// 扫描到的类型声明，生成 `bindings.ts` 的中间表示
#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    /// 类型名，即 `pub struct X` / `pub enum X` 里的 X
    pub name: String,
    /// 声明前的属性原文，含 `#[derive(...)]` / `#[serde(...)]` 与 `///` 文档行
    pub attrs: String,
    /// 花括号内的正文原文，即字段 / 变体声明
    pub body: String,
    /// 是枚举时为 `true`，结构体为 `false`
    pub is_enum: bool,
}

/// 一条命令的签名，参数已转成 TS
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sig {
    /// 来源模块路径，如 `windows::resource::mods`；决定默认分组
    pub(crate) module: String,
    /// 命令名，即 Rust 函数名，如 `window_get_window_sizes`
    pub(crate) name: String,
    /// 显式分组标记的值；`None` 表示按来源模块最后一段分组
    pub(crate) group: Option<String>,
    /// 转成 TS 的函数参数表
    pub(crate) params: Vec<TsParam>,
    /// 返回类型原文；没有写返回类型时是空串
    pub(crate) ret: String,
}

impl Sig {
    /// 生效的组键：显式分组优先，否则取来源模块最后一段，即 .rs 文件名或目录名
    ///
    /// 拆文件时靠它把子模块的命令并回原来的组：`windows::resource::mods` 加上
    /// 给命令加上分组标记之后组键仍是 `resource`，前端调用点不用改。
    ///
    /// # 返回值
    ///
    /// 组键；没有显式分组时是模块路径的最后一段
    pub(crate) fn group_key(&self) -> &str {
        match &self.group {
            Some(g) => g.as_str(),
            None => self.module.rsplit("::").next().unwrap_or(&self.module),
        }
    }
}

/// 扫源码生成契约文本。纯函数：不写文件、不读环境变量
///
/// - `cfg`: 生成配置，含源码根、类型来源与外部 crate 类型映射
///
/// # 返回值
///
/// 见 [`Generated`]；`src_dir` 不存在时返回 [`GenError::SrcDirMissing`]
pub fn generate(cfg: &GenConfig) -> Result<Generated, GenError> {
    let src = cfg.src_dir.as_path();
    if !src.is_dir() {
        return Err(GenError::SrcDirMissing(cfg.src_dir.clone()));
    }

    let mut files = Vec::new();
    walk_rs(src, &mut files);
    files.sort();

    // 命令与事件：排序去重保证输出稳定
    let mut commands: Vec<CommandRef> = Vec::new();
    let mut events: Vec<ScannedEvent> = Vec::new();
    for file in &files {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        let module = module_path(src, file);
        for name in fns_after(&text, CMD_MARK) {
            let path = if module.is_empty() {
                name.clone()
            } else {
                format!("{module}::{name}")
            };
            commands.push(CommandRef { name, path });
        }
        events.extend(events_in(&text));
    }
    commands.sort();
    commands.dedup();
    events.sort_by_key(ScannedEvent::event_name);
    events.dedup_by(|a, b| a.event_name() == b.event_name());

    // 类型声明：来自 dtos/ 与 models/ 的全部类型，外加 gui_config.rs 里的枚举
    // Theme / WindowMode / SidebarSide / ViewMode / HeadType 被 GuiConfigDto 引用；
    // 按来源 .rs 分组，一个文件一份产物
    let mut type_groups: BTreeMap<String, Vec<Decl>> = BTreeMap::new();
    for file in &files {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        let rel = rel_path(src, file);
        let wanted = cfg
            .type_sources
            .dirs
            .iter()
            .any(|d| rel.starts_with(d.as_str()));
        let enums_only = cfg.type_sources.enum_files.iter().any(|f| f == &rel);
        if !wanted && !enums_only {
            continue;
        }
        let mut decls = Vec::new();
        for d in scan_types(&text) {
            if enums_only && !d.is_enum {
                continue;
            }
            // 没有 serde derive 的不是 IPC 类型
            if !d.attrs.contains("Serialize") && !d.attrs.contains("Deserialize") {
                continue;
            }
            decls.push(d);
        }
        if !decls.is_empty() {
            type_groups.entry(type_module(&rel)).or_default().extend(decls);
        }
    }
    let declared: BTreeSet<String> = type_groups
        .values()
        .flatten()
        .map(|d| d.name.clone())
        .collect();

    // 命令签名：带来源模块，供按 .rs 分组用
    let mut sigs: Vec<Sig> = Vec::new();
    for file in &files {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        let module = module_path(src, file);
        for cmd in commands_in(&text) {
            sigs.push(Sig {
                module: module.clone(),
                name: cmd.name,
                group: cmd.group,
                params: ts_params(&cmd.params, &cfg.external_types),
                ret: ts_type(&result_ok(&cmd.ret), &cfg.external_types),
            });
        }
    }
    sigs.sort_by(|a, b| (&a.module, &a.name).cmp(&(&b.module, &b.name)));

    Ok(Generated {
        bindings_ts: emit_ts::bindings_ts(&sigs, &declared),
        listens_ts: emit_ts::listens_ts(&events, &cfg.external_types, &declared),
        type_files: emit_ts::type_files(&type_groups, &cfg.external_types),
        invokes_gen_rs: emit_rs::invokes_gen(&events, &commands),
    })
}

/// 来源 .rs 路径 -> 相对 types/ 的输出模块名
///
/// - `rel`: 相对 `src_dir` 的路径，如 `dtos/instance_dto.rs`
///
/// # 返回值
///
/// 如 `dtos/instance_dto.ts`；`gui_config.rs` -> `gui_config.ts`
fn type_module(rel: &str) -> String {
    format!("{}.ts", rel.strip_suffix(".rs").unwrap_or(rel))
}

/// 只在内容变化时写，返回是否真的写了
///
/// 落盘固定用 CRLF：生成文本统一 `\n`，写文件前再转行尾；
/// 比较也按 CRLF 做，磁盘上还是 LF 的旧文件会被改写一次。
/// 内容相同就跳过：否则每次 cargo 构建都会改动 mtime，触发前端重跑。
///
/// - `path`: 目标文件，父目录不存在会自动建
/// - `content`: 要写入的文本，`\n` 行尾
///
/// # 返回值
///
/// `true` = 确实写了；`false` = 内容相同、跳过
pub fn write_if_changed(path: &Path, content: &str) -> io::Result<bool> {
    let content = to_crlf(content);
    if fs::read_to_string(path).ok().as_deref() == Some(content.as_ref()) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content.as_ref())?;
    Ok(true)
}

/// 把裸 `\n` 转成 CRLF，已经是 CRLF 时原样借用
fn to_crlf(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let has_bare_lf = bytes
        .iter()
        .enumerate()
        .any(|(i, &b)| b == b'\n' && (i == 0 || bytes[i - 1] != b'\r'));
    if !has_bare_lf {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    let mut prev = '\0';
    for ch in text.chars() {
        if ch == '\n' && prev != '\r' {
            out.push('\r');
        }
        out.push(ch);
        prev = ch;
    }
    Cow::Owned(out)
}

/// 相对 `src` 的路径，统一用 `/` 分隔，Windows 上同样如此
///
/// - `src`: 源码根目录
/// - `file`: 要换算的文件路径
///
/// # 返回值
///
/// 相对路径；`file` 不在 `src` 下时返回空串
fn rel_path(src: &Path, file: &Path) -> String {
    file.strip_prefix(src)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// src_dir 不存在要报错，而不是静默生成空产物
    #[test]
    fn test_generate_rejects_missing_src_dir() {
        let cfg = GenConfig {
            src_dir: PathBuf::from("H:/Temp/__definitely_not_here__"),
            ..Default::default()
        };
        assert!(matches!(generate(&cfg), Err(GenError::SrcDirMissing(_))));
    }

    /// 内容没变不写，返回 false；变了才写
    #[test]
    fn test_write_if_changed_skips_identical() {
        let dir = mml_testutil::temp_dir().join("gui_ipc_gen_wic");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("a/b.ts");
        assert!(write_if_changed(&path, "x").unwrap(), "首次应写入");
        assert!(!write_if_changed(&path, "x").unwrap(), "内容相同不该写");
        assert!(write_if_changed(&path, "y").unwrap(), "内容变了应写入");
        assert_eq!(fs::read_to_string(&path).unwrap(), "y");
        let _ = fs::remove_dir_all(&dir);
    }

    /// 生成文件固定写 CRLF：LF 旧文件改写一次，之后同样内容不重写
    #[test]
    fn test_write_if_changed_writes_crlf() {
        let dir = mml_testutil::temp_dir().join("gui_ipc_gen_crlf");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("a/b.ts");

        // 磁盘上是 LF：要改写成 CRLF
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "a\nb\n").unwrap();
        assert!(
            write_if_changed(&path, "a\nb\n").unwrap(),
            "LF 旧文件应改写"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "a\r\nb\r\n");

        // 已经是 CRLF：同样内容不再写
        assert!(
            !write_if_changed(&path, "a\nb\n").unwrap(),
            "内容相同不该写"
        );

        // 输入本身已经是 CRLF 时也不能叠成 \r\r\n
        assert!(!write_if_changed(&path, "a\r\nb\r\n").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "a\r\nb\r\n");

        let _ = fs::remove_dir_all(&dir);
    }

    /// 已经是 CRLF 的文本原样借用，不重复转换
    #[test]
    fn test_to_crlf_keeps_existing_crlf() {
        let text = "a\r\nb\r\n";
        assert!(matches!(to_crlf(text), Cow::Borrowed(_)));
        assert_eq!(to_crlf("a\nb\n").as_ref(), "a\r\nb\r\n");
    }
}
