//! 手柄映射：默认映射、配置目录与规则编译
//!
//! 映射配置放在运行目录的 gamepad 下，一份映射一个 json，字段用大驼峰；
//! Selected 为 true 的那份生效，没有就取文件名排序第一份。动作写字符串：
//! 键盘键名（W / Space / F5）、MouseLeft / MouseRight / MouseMiddle / Mouse4 到 Mouse8、
//! ScrollUp / ScrollDown。目录里没有可用配置时用内置默认。
//!
//! 摇杆不写进按钮 / 轴表：光标模式与游戏内转动模式各用一个摇杆，
//! 由游戏上报的鼠标抓取状态切换，见 crate::relay。

use std::{
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock, RwLock},
    time::SystemTime,
};

use mml_names::{i18_items::info_type::InfoType, names};
use serde::{Deserialize, Serialize};

use crate::reader::{axis_index, button_index};

/// 配置目录路径，加载时设置
static DIR: OnceLock<PathBuf> = OnceLock::new();

/// 当前生效的映射
static MAP: LazyLock<RwLock<CompiledMap>> = LazyLock::new(|| RwLock::new(CompiledMap::built_in()));

/// 上次扫描到的文件列表与修改时间
static SIGNATURE: LazyLock<RwLock<String>> = LazyLock::new(|| RwLock::new(String::new()));

/// 手柄按钮，统一到 Xbox 布局
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Button {
    /// 下方面键，Xbox 的 A
    South,
    /// 右方面键，Xbox 的 B
    East,
    /// 上方面键，Xbox 的 Y
    North,
    /// 左方面键，Xbox 的 X
    West,
    /// C 键，部分手柄有
    C,
    /// Z 键，部分手柄有
    Z,
    /// 左扳机
    LeftTrigger,
    /// 左扳机第二段
    LeftTrigger2,
    /// 右扳机
    RightTrigger,
    /// 右扳机第二段
    RightTrigger2,
    /// Select
    Select,
    /// Start
    Start,
    /// 中间功能键
    Mode,
    /// 左摇杆按下
    LeftThumb,
    /// 右摇杆按下
    RightThumb,
    /// 十字键上
    DPadUp,
    /// 十字键下
    DPadDown,
    /// 十字键左
    DPadLeft,
    /// 十字键右
    DPadRight,
}

/// 手柄轴
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    /// 左摇杆水平
    LeftStickX,
    /// 左摇杆垂直
    LeftStickY,
    /// 左扳机轴
    LeftZ,
    /// 右摇杆水平
    RightStickX,
    /// 右摇杆垂直
    RightStickY,
    /// 右扳机轴
    RightZ,
    /// 十字键水平
    DPadX,
    /// 十字键垂直
    DPadY,
}

/// 一份映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct GamepadConfig {
    /// 是否当前生效的配置；多份为 true 时取文件名排序第一份
    pub selected: bool,
    /// 按钮绑定
    pub buttons: Vec<ButtonRule>,
    /// 轴区间绑定
    pub axes: Vec<AxisRule>,
    /// 摇杆当鼠标用
    pub mouse: MouseRule,
}

/// 一个按钮的绑定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ButtonRule {
    /// 手柄按钮
    pub button: Button,
    /// 修饰键：按住它时这条绑定才生效；不填表示不需要
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<Button>,
    /// 动作
    pub action: String,
}

/// 一个轴区间的绑定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AxisRule {
    /// 手柄轴
    pub axis: Axis,
    /// 区间起点，-1.0 到 1.0
    pub from: f32,
    /// 区间终点，-1.0 到 1.0
    pub to: f32,
    /// 动作
    pub action: String,
}

/// 摇杆当鼠标用的参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MouseRule {
    /// 光标模式用的摇杆
    pub cursor_stick: Stick,
    /// 转动视角模式用的摇杆
    pub turn_stick: Stick,
    /// 光标模式速度，单位像素每秒
    pub cursor_speed: f32,
    /// 转动视角速度，单位像素每秒
    pub turn_speed: f32,
    /// 摇杆死区，绝对值小于该值按 0 处理
    pub deadzone: f32,
}

/// 摇杆
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stick {
    /// 左摇杆
    Left,
    /// 右摇杆
    Right,
}

/// 配置目录里的一份映射
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    /// 配置名，即文件名去掉 .json
    pub name: String,
    /// 是否当前生效
    pub active: bool,
    /// 读取或校验失败的原因；正常为 None
    pub error: Option<String>,
}

/// 配置操作的错误
#[derive(Debug, Clone)]
pub enum GamepadError {
    /// 配置名不合法
    InvalidName,
    /// 同名配置已存在
    Exists,
    /// 找不到配置
    Missing,
    /// 至少要保留一份配置
    Last,
    /// 映射内容不合法
    InvalidConfig,
    /// 读写文件失败，带原始说明
    Io(String),
}

/// 编译后的映射表，转发线程按状态数组下标直接查
#[derive(Debug, Clone)]
pub struct CompiledMap {
    /// 按钮规则
    pub buttons: Vec<ButtonRuleCompiled>,
    /// 轴规则
    pub axes: Vec<AxisRuleCompiled>,
    /// 摇杆当鼠标用
    pub mouse: MouseRuleCompiled,
}

/// 编译后的按钮规则
#[derive(Debug, Clone)]
pub struct ButtonRuleCompiled {
    /// 按钮在状态数组中的下标
    pub index: usize,
    /// 修饰键在状态数组中的下标；None 表示不需要
    pub modifier: Option<usize>,
    /// 动作
    pub action: Action,
}

/// 编译后的轴规则
#[derive(Debug, Clone)]
pub struct AxisRuleCompiled {
    /// 轴在状态数组中的下标
    pub index: usize,
    /// 区间起点
    pub from: f32,
    /// 区间终点
    pub to: f32,
    /// 动作
    pub action: Action,
    /// 轴所属的摇杆；十字键这类不归属任何摇杆时为 None
    pub stick: Option<Stick>,
}

/// 摇杆当鼠标用的参数
#[derive(Debug, Clone, Copy)]
pub struct MouseRuleCompiled {
    /// 光标模式
    pub cursor: StickRule,
    /// 转动视角模式
    pub turn: StickRule,
    /// 死区
    pub deadzone: f32,
}

/// 一个摇杆的两个轴与速度
#[derive(Debug, Clone, Copy)]
pub struct StickRule {
    /// 摇杆对应的轴
    pub axes: StickAxes,
    /// 速度，单位像素每秒
    pub speed: f32,
    /// 是哪个摇杆
    pub stick: Stick,
}

/// 一个摇杆两个轴在状态数组中的下标
#[derive(Debug, Clone, Copy)]
pub struct StickAxes {
    /// 水平轴
    pub x: usize,
    /// 垂直轴
    pub y: usize,
}

/// 手柄动作，编译后直接对应一条 IPC 消息
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// 键盘按键，GLFW 键码
    Key(i32),
    /// 鼠标按键，GLFW 编号
    Mouse(i32),
    /// 滚轮，格数正数向上
    Scroll(i32),
}

impl Default for GamepadConfig {
    fn default() -> Self {
        Self {
            // 缺 Selected 的配置不参与生效，默认那份在写文件时显式打开
            selected: false,
            buttons: vec![
                button_rule(Button::South, "Space"),
                button_rule(Button::East, "E"),
                button_rule(Button::North, "Q"),
                button_rule(Button::West, "F"),
                button_rule(Button::LeftTrigger, "MouseLeft"),
                button_rule(Button::RightTrigger, "MouseRight"),
                button_rule(Button::LeftTrigger2, "LeftShift"),
                button_rule(Button::RightTrigger2, "LeftControl"),
                button_rule(Button::Select, "Tab"),
                button_rule(Button::Start, "Escape"),
                button_rule(Button::LeftThumb, "F5"),
                button_rule(Button::DPadUp, "ScrollUp"),
                button_rule(Button::DPadDown, "ScrollDown"),
            ],
            axes: vec![
                // gilrs 的 Y 轴向上为正
                axis_rule(Axis::LeftStickY, 0.35, 1.0, "W"),
                axis_rule(Axis::LeftStickY, -1.0, -0.35, "S"),
                axis_rule(Axis::LeftStickX, -1.0, -0.35, "A"),
                axis_rule(Axis::LeftStickX, 0.35, 1.0, "D"),
            ],
            mouse: MouseRule {
                cursor_stick: Stick::Left,
                turn_stick: Stick::Right,
                cursor_speed: 700.0,
                turn_speed: 900.0,
                deadzone: 0.15,
            },
        }
    }
}

impl GamepadConfig {
    /// 校验并编译成转发线程用的规则表
    pub fn compile(&self) -> Result<CompiledMap, String> {
        let mut buttons = Vec::with_capacity(self.buttons.len());
        for rule in &self.buttons {
            let index = button_index(rule.button)
                .ok_or_else(|| format!("未知手柄按钮：{:?}", rule.button))?;
            let modifier = match rule.modifier {
                Some(modifier) => {
                    let modifier = button_index(modifier)
                        .ok_or_else(|| format!("未知修饰键：{:?}", modifier))?;
                    if modifier == index {
                        return Err(String::from("修饰键不能和按钮相同"));
                    }
                    Some(modifier)
                }
                None => None,
            };
            buttons.push(ButtonRuleCompiled {
                index,
                modifier,
                action: parse_action(&rule.action)?,
            });
        }

        let mut axes = Vec::with_capacity(self.axes.len());
        for rule in &self.axes {
            let index =
                axis_index(rule.axis).ok_or_else(|| format!("未知手柄轴：{:?}", rule.axis))?;
            if !(-1.0..=1.0).contains(&rule.from) || !(-1.0..=1.0).contains(&rule.to) {
                return Err(format!(
                    "轴区间超出 -1.0 到 1.0：{} 到 {}",
                    rule.from, rule.to
                ));
            }
            if rule.from == rule.to {
                return Err(format!("轴区间起点与终点相同：{}", rule.from));
            }
            axes.push(AxisRuleCompiled {
                index,
                from: rule.from,
                to: rule.to,
                action: parse_action(&rule.action)?,
                stick: stick_of_axis(rule.axis),
            });
        }

        let mouse = &self.mouse;
        if mouse.cursor_speed < 0.0 || mouse.turn_speed < 0.0 {
            return Err(String::from("摇杆速度不能为负"));
        }
        if !(0.0..1.0).contains(&mouse.deadzone) {
            return Err(String::from("死区应在 0.0 到 1.0 之间"));
        }

        Ok(CompiledMap {
            buttons,
            axes,
            mouse: MouseRuleCompiled {
                cursor: stick_rule(mouse.cursor_stick, mouse.cursor_speed),
                turn: stick_rule(mouse.turn_stick, mouse.turn_speed),
                deadzone: mouse.deadzone,
            },
        })
    }
}

impl CompiledMap {
    /// 内置默认映射
    fn built_in() -> Self {
        GamepadConfig::default()
            .compile()
            .expect("内置手柄映射应能编译")
    }
}

/// 一份可用的配置
struct Profile {
    /// 文件路径
    path: PathBuf,
    /// 是否标记为生效
    selected: bool,
    /// 编译后的映射
    map: CompiledMap,
}

/// 一份配置加载失败的说明
struct ProfileError {
    /// 文件路径
    path: PathBuf,
    /// 失败原因
    error: String,
}

/// 扫描配置目录的结果
struct Scan {
    /// 可用的配置，按文件名排序
    profiles: Vec<Profile>,
    /// 加载失败的条目
    errors: Vec<ProfileError>,
}

/// 加载配置目录，首次运行时建目录并写一份默认配置
///
/// - `base`: 运行目录，配置目录是其中的 gamepad
pub fn init(base: &Path) {
    let dir = base.join(names::GAMEPAD_DIR);
    let _ = DIR.set(dir.clone());

    if let Err(error) = std::fs::create_dir_all(&dir) {
        log_load_failed(&dir, &error.to_string());
        return;
    }

    if signature(&dir).is_empty() {
        match write_default(&dir) {
            Ok(file) => mml_log::info_type(InfoType::GamepadMapDefaultWritten {
                path: file.display().to_string(),
            }),
            Err(error) => log_load_failed(&dir, &error),
        }
    }

    refresh(&dir);
}

/// 配置目录有变化时重新加载
pub fn reload_if_changed() {
    let Some(dir) = DIR.get() else {
        return;
    };
    if *SIGNATURE.read().unwrap() == signature(dir) {
        return;
    }
    refresh(dir);
}

/// 当前生效的映射
pub fn current() -> CompiledMap {
    MAP.read().unwrap().clone()
}

/// 轴值是否落在区间内，起点大于终点时自动交换
pub fn in_range(value: f32, from: f32, to: f32) -> bool {
    let (low, high) = if from <= to { (from, to) } else { (to, from) };
    value >= low && value <= high
}

/// 重新扫描配置目录并应用生效的那份
fn reload(dir: &Path) {
    let scan = scan(dir);
    for item in &scan.errors {
        log_load_failed(&item.path, &item.error);
    }

    let Some(profile) = pick(&scan.profiles) else {
        *MAP.write().unwrap() = CompiledMap::built_in();
        mml_log::warn_type(InfoType::GamepadMapUnavailable {
            path: dir.display().to_string(),
        });
        return;
    };

    *MAP.write().unwrap() = profile.map.clone();
    mml_log::info_type(InfoType::GamepadMapLoaded {
        path: profile.path.display().to_string(),
    });
}

/// 选出生效的配置
///
/// 优先标记 Selected 的，否则取文件名排序第一份
fn pick(profiles: &[Profile]) -> Option<&Profile> {
    profiles
        .iter()
        .find(|item| item.selected)
        .or_else(|| profiles.first())
}

/// 扫描配置目录
fn scan(dir: &Path) -> Scan {
    let mut scan = Scan {
        profiles: Vec::new(),
        errors: Vec::new(),
    };

    for path in config_files(dir) {
        match load_file(&path) {
            Ok(config) => match config.compile() {
                Ok(map) => scan.profiles.push(Profile {
                    path,
                    selected: config.selected,
                    map,
                }),
                Err(error) => scan.errors.push(ProfileError { path, error }),
            },
            Err(error) => scan.errors.push(ProfileError { path, error }),
        }
    }

    scan
}

/// 配置目录里的全部 json 文件，按路径排序
fn config_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    files.sort();
    files
}

/// 配置目录的签名：文件名与修改时间
fn signature(dir: &Path) -> String {
    config_files(dir)
        .iter()
        .map(|path| format!("{}:{:?}", file_stem(path), file_mtime(path)))
        .collect::<Vec<_>>()
        .join("|")
}

/// 重新加载并记下当前目录签名
fn refresh(dir: &Path) {
    reload(dir);
    *SIGNATURE.write().unwrap() = signature(dir);
}

/// 写出一份默认配置，字段用大驼峰并带缩进，方便用户直接改
///
/// # 返回值
///
/// 写出的文件路径
fn write_default(dir: &Path) -> Result<PathBuf, String> {
    let file = dir.join(names::GAMEPAD_DEFAULT_MAP_FILE);
    let config = GamepadConfig {
        selected: true,
        ..GamepadConfig::default()
    };
    write_config(&file, &config)?;
    Ok(file)
}

/// 读取一份配置
fn load_file(path: &Path) -> Result<GamepadConfig, String> {
    let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

/// 写一份配置到文件，字段用大驼峰并带缩进，方便用户直接改
fn write_config(path: &Path, config: &GamepadConfig) -> Result<(), String> {
    let text = serde_json::to_string_pretty(config).map_err(|err| err.to_string())?;
    std::fs::write(path, text).map_err(|err| err.to_string())
}

/// 配置目录，未初始化时报错
fn dir() -> Result<&'static Path, GamepadError> {
    DIR.get().map(PathBuf::as_path).ok_or(GamepadError::Missing)
}

/// 配置文件名去掉扩展名
fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// 配置名对应的文件路径，同时校验名字
fn path_of(name: &str) -> Result<PathBuf, GamepadError> {
    if !valid_name(name) {
        return Err(GamepadError::InvalidName);
    }
    Ok(dir()?.join(format!("{name}.json")))
}

/// 配置名是否合法：非空、不含路径分隔符与 Windows 非法字符
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 32
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name
            .chars()
            .any(|ch| ch.is_control() || "/\\:*?\"<>|".contains(ch))
}

/// 列出配置目录里的全部映射
///
/// # 返回值
///
/// 按名字排序；读取或校验失败的条目也列出来，原因放在 error 里
pub fn profiles() -> Vec<ProfileInfo> {
    let Some(dir) = DIR.get() else {
        return Vec::new();
    };
    let scan = scan(dir);
    let active = pick(&scan.profiles).map(|item| item.path.clone());

    let mut items: Vec<ProfileInfo> = scan
        .profiles
        .iter()
        .map(|item| ProfileInfo {
            name: file_stem(&item.path),
            active: active.as_deref() == Some(item.path.as_path()),
            error: None,
        })
        .collect();
    for item in &scan.errors {
        items.push(ProfileInfo {
            name: file_stem(&item.path),
            active: false,
            error: Some(item.error.clone()),
        });
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items
}

/// 读取一份映射
///
/// - `name`: 文件名去掉 .json
///
/// # 返回值
///
/// 配置内容；文件不存在时返回 Missing，内容不合法时返回 InvalidConfig
pub fn config(name: &str) -> Result<GamepadConfig, GamepadError> {
    let path = path_of(name)?;
    if !path.is_file() {
        return Err(GamepadError::Missing);
    }
    match load_file(&path) {
        Ok(config) => Ok(config),
        Err(error) => {
            log_load_failed(&path, &error);
            Err(GamepadError::InvalidConfig)
        }
    }
}

/// 保存一份映射，内容不合法时不落盘
///
/// - `name`: 文件名去掉 .json
/// - `config`: 界面编辑后的整份映射
///
/// # 返回值
///
/// 保存并重新加载成功返回 Ok；文件不存在返回 Missing，内容不合法返回 InvalidConfig
pub fn save(name: &str, config: &GamepadConfig) -> Result<(), GamepadError> {
    let dir = dir()?;
    let path = path_of(name)?;
    if !path.is_file() {
        return Err(GamepadError::Missing);
    }
    if let Err(error) = config.compile() {
        log_load_failed(&path, &error);
        return Err(GamepadError::InvalidConfig);
    }
    write_config(&path, config).map_err(GamepadError::Io)?;
    refresh(dir);
    Ok(())
}

/// 新建一份映射，复制当前生效的那份
///
/// - `name`: 新配置的显示名，同时作为文件名
///
/// # 返回值
///
/// 建好并重新加载返回 Ok；重名返回 Exists，名字不合法返回 InvalidName
pub fn create(name: &str) -> Result<(), GamepadError> {
    let dir = dir()?;
    let path = path_of(name)?;
    if path.exists() {
        return Err(GamepadError::Exists);
    }

    let mut config = {
        let scan = scan(dir);
        match pick(&scan.profiles) {
            Some(item) => load_file(&item.path).map_err(GamepadError::Io)?,
            None => GamepadConfig::default(),
        }
    };
    config.selected = false;
    write_config(&path, &config).map_err(GamepadError::Io)?;
    refresh(dir);
    Ok(())
}

/// 删除一份映射，至少保留一份
///
/// - `name`: 要删除的配置
///
/// # 返回值
///
/// 删除并重新加载返回 Ok；只剩一份返回 Last，文件不存在返回 Missing
pub fn delete(name: &str) -> Result<(), GamepadError> {
    let dir = dir()?;
    let path = path_of(name)?;
    if !path.is_file() {
        return Err(GamepadError::Missing);
    }
    if config_files(dir).len() <= 1 {
        return Err(GamepadError::Last);
    }
    std::fs::remove_file(&path).map_err(|err| GamepadError::Io(err.to_string()))?;
    refresh(dir);
    Ok(())
}

/// 把一份映射设为当前生效，其余配置取消选中
///
/// - `name`: 要生效的配置
///
/// # 返回值
///
/// 改完并重新加载返回 Ok；文件不存在返回 Missing
pub fn set_active(name: &str) -> Result<(), GamepadError> {
    let dir = dir()?;
    let path = path_of(name)?;
    if !path.is_file() {
        return Err(GamepadError::Missing);
    }

    for item in config_files(dir) {
        let Ok(mut config) = load_file(&item) else {
            continue;
        };
        let want = item == path;
        if config.selected != want {
            config.selected = want;
            write_config(&item, &config).map_err(GamepadError::Io)?;
        }
    }
    refresh(dir);
    Ok(())
}

/// 配置界面可选的键鼠动作名
///
/// # 返回值
///
/// 按键盘 / 功能键 / 鼠标 / 滚轮排好的动作名，都能被 [`parse_action`] 解析
pub fn action_names() -> Vec<String> {
    let mut names = Vec::new();
    for ch in 'A'..='Z' {
        names.push(ch.to_string());
    }
    for ch in '0'..='9' {
        names.push(ch.to_string());
    }
    for name in [
        "Space",
        "Enter",
        "Tab",
        "Backspace",
        "Escape",
        "Insert",
        "Delete",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Up",
        "Down",
        "Left",
        "Right",
        "LeftShift",
        "LeftControl",
        "LeftAlt",
    ] {
        names.push(name.to_string());
    }
    for number in 1..=12 {
        names.push(format!("F{number}"));
    }
    for name in ["MouseLeft", "MouseRight", "MouseMiddle", "Mouse4", "Mouse5"] {
        names.push(name.to_string());
    }
    for name in ["ScrollUp", "ScrollDown"] {
        names.push(name.to_string());
    }
    names
}

/// 配置文件修改时间
fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
}

/// 记一条映射不可用的日志
fn log_load_failed(file: &Path, error: &str) {
    mml_log::warn_type(InfoType::GamepadMapLoadFailed {
        path: file.display().to_string(),
        error: error.to_string(),
    });
}

/// 建一条按钮绑定
fn button_rule(button: Button, action: &str) -> ButtonRule {
    ButtonRule {
        button,
        modifier: None,
        action: action.to_string(),
    }
}

/// 建一条轴绑定
fn axis_rule(axis: Axis, from: f32, to: f32, action: &str) -> AxisRule {
    AxisRule {
        axis,
        from,
        to,
        action: action.to_string(),
    }
}

/// 摇杆对应的轴与速度
fn stick_rule(stick: Stick, speed: f32) -> StickRule {
    StickRule {
        axes: stick_axes(stick),
        speed,
        stick,
    }
}

/// 摇杆对应的两个轴在状态数组中的下标
fn stick_axes(stick: Stick) -> StickAxes {
    let (x, y) = match stick {
        Stick::Left => (Axis::LeftStickX, Axis::LeftStickY),
        Stick::Right => (Axis::RightStickX, Axis::RightStickY),
    };
    StickAxes {
        x: axis_index(x).expect("手柄轴表应包含左右摇杆"),
        y: axis_index(y).expect("手柄轴表应包含左右摇杆"),
    }
}

/// 轴所属的摇杆；十字键不归属任何摇杆
fn stick_of_axis(axis: Axis) -> Option<Stick> {
    match axis {
        Axis::LeftStickX | Axis::LeftStickY | Axis::LeftZ => Some(Stick::Left),
        Axis::RightStickX | Axis::RightStickY | Axis::RightZ => Some(Stick::Right),
        Axis::DPadX | Axis::DPadY => None,
    }
}

/// 解析配置里的动作字符串
fn parse_action(text: &str) -> Result<Action, String> {
    if let Some(name) = text.strip_prefix("Scroll") {
        return match name {
            "Up" => Ok(Action::Scroll(1)),
            "Down" => Ok(Action::Scroll(-1)),
            _ => Err(format!("未知滚轮动作：{text}")),
        };
    }
    if let Some(name) = text.strip_prefix("Mouse") {
        let button = match name {
            "Left" => 0,
            "Right" => 1,
            "Middle" => 2,
            "4" => 3,
            "5" => 4,
            "6" => 5,
            "7" => 6,
            "8" => 7,
            _ => return Err(format!("未知鼠标按键：{text}")),
        };
        return Ok(Action::Mouse(button));
    }
    key_code(text)
        .map(Action::Key)
        .ok_or_else(|| format!("未知按键：{text}"))
}

/// 键名转 GLFW 键码，大小写不敏感
fn key_code(name: &str) -> Option<i32> {
    if name.len() == 1 {
        let ch = name.chars().next()?;
        if ch.is_ascii_alphabetic() {
            return Some(i32::from(ch.to_ascii_uppercase() as u8));
        }
        if ch.is_ascii_digit() {
            return Some(i32::from(ch as u8));
        }
    }

    let upper = name.to_ascii_uppercase();

    if let Some(number) = upper
        .strip_prefix('F')
        .and_then(|rest| rest.parse::<u32>().ok())
    {
        if (1..=25).contains(&number) {
            return Some(289 + number as i32);
        }
    }

    if let Some(rest) = upper.strip_prefix("KP") {
        if let Ok(number) = rest.parse::<u32>() {
            if number <= 9 {
                return Some(320 + number as i32);
            }
        }
        return match rest {
            "DECIMAL" => Some(330),
            "DIVIDE" => Some(331),
            "MULTIPLY" => Some(332),
            "SUBTRACT" => Some(333),
            "ADD" => Some(334),
            "ENTER" => Some(335),
            "EQUAL" => Some(336),
            _ => None,
        };
    }

    let code = match upper.as_str() {
        "SPACE" => 32,
        "APOSTROPHE" => 39,
        "COMMA" => 44,
        "MINUS" => 45,
        "PERIOD" => 46,
        "SLASH" => 47,
        "SEMICOLON" => 59,
        "EQUAL" => 61,
        "LEFTBRACKET" => 91,
        "BACKSLASH" => 92,
        "RIGHTBRACKET" => 93,
        "GRAVE" | "BACKTICK" => 96,
        "ESCAPE" | "ESC" => 256,
        "ENTER" | "RETURN" => 257,
        "TAB" => 258,
        "BACKSPACE" => 259,
        "INSERT" => 260,
        "DELETE" => 261,
        "RIGHT" => 262,
        "LEFT" => 263,
        "DOWN" => 264,
        "UP" => 265,
        "PAGEUP" => 266,
        "PAGEDOWN" => 267,
        "HOME" => 268,
        "END" => 269,
        "CAPSLOCK" => 280,
        "SCROLLLOCK" => 281,
        "NUMLOCK" => 282,
        "PRINTSCREEN" => 283,
        "PAUSE" => 284,
        "LEFTSHIFT" | "SHIFT" => 340,
        "LEFTCONTROL" | "LEFTCTRL" | "CONTROL" | "CTRL" => 341,
        "LEFTALT" | "ALT" => 342,
        "LEFTSUPER" | "SUPER" | "WIN" => 343,
        "RIGHTSHIFT" => 344,
        "RIGHTCONTROL" | "RIGHTCTRL" => 345,
        "RIGHTALT" => 346,
        "RIGHTSUPER" => 347,
        "MENU" => 348,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 界面上列出的动作名都能被解析
    #[test]
    fn action_names_parse() {
        let names = action_names();
        assert!(names.len() > 40);
        for name in names {
            assert!(parse_action(&name).is_ok(), "{name}");
        }
    }

    /// 内置默认映射可编译，关键绑定落在预期位置
    #[test]
    fn default_map_compiles() {
        let map = GamepadConfig::default().compile().unwrap();
        assert!(!map.buttons.is_empty());
        assert_eq!(map.mouse.deadzone, 0.15);

        let south = button_index(Button::South).unwrap();
        assert!(
            map.buttons
                .iter()
                .any(|rule| rule.index == south && rule.action == Action::Key(32))
        );
    }

    /// 键名、鼠标键、滚轮都能解析
    #[test]
    fn parse_actions() {
        assert_eq!(parse_action("W"), Ok(Action::Key(87)));
        assert_eq!(parse_action("space"), Ok(Action::Key(32)));
        assert_eq!(parse_action("F5"), Ok(Action::Key(294)));
        assert_eq!(parse_action("LeftShift"), Ok(Action::Key(340)));
        assert_eq!(parse_action("MouseLeft"), Ok(Action::Mouse(0)));
        assert_eq!(parse_action("MouseRight"), Ok(Action::Mouse(1)));
        assert_eq!(parse_action("ScrollUp"), Ok(Action::Scroll(1)));
        assert_eq!(parse_action("ScrollDown"), Ok(Action::Scroll(-1)));
        assert!(parse_action("Nope").is_err());
        assert!(parse_action("Mouse9").is_err());
    }

    /// 区间判定不要求起点小于终点
    #[test]
    fn range_hit() {
        assert!(in_range(-0.5, -1.0, -0.35));
        assert!(!in_range(-0.2, -1.0, -0.35));
        assert!(in_range(-0.5, -0.35, -1.0));
        assert!(!in_range(0.0, 0.35, 1.0));
    }

    /// 默认配置写出的 JSON 用大驼峰，读回来还能编译
    #[test]
    fn config_roundtrip() {
        let text = serde_json::to_string_pretty(&GamepadConfig::default()).unwrap();
        assert!(text.contains("\"Selected\""));
        assert!(text.contains("\"Buttons\""));
        assert!(text.contains("\"CursorStick\""));

        let config: GamepadConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(config.buttons.len(), GamepadConfig::default().buttons.len());
        assert!(config.compile().is_ok());
    }

    /// 修饰键不能和按钮相同
    #[test]
    fn modifier_cannot_match_button() {
        let mut config = GamepadConfig::default();
        config.buttons[0].modifier = Some(Button::South);
        assert!(config.compile().is_err());
    }

    /// 修饰键为空时不写进 JSON，设置后能读回来
    #[test]
    fn modifier_roundtrip() {
        let mut config = GamepadConfig::default();
        config.buttons[0].modifier = Some(Button::LeftTrigger);
        let text = serde_json::to_string_pretty(&config).unwrap();
        assert!(text.contains("\"Modifier\": \"leftTrigger\""));
        let back: GamepadConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(back.buttons[0].modifier, Some(Button::LeftTrigger));

        let plain = GamepadConfig::default();
        let text = serde_json::to_string_pretty(&plain).unwrap();
        assert!(!text.contains("\"Modifier\""));
    }

    /// 动作写错时整份配置编译失败
    #[test]
    fn invalid_action_is_rejected() {
        let mut config = GamepadConfig::default();
        config.buttons.push(ButtonRule {
            button: Button::South,
            modifier: None,
            action: String::from("Nope"),
        });
        assert!(config.compile().is_err());
    }

    /// 写一份配置到文件
    fn write_config(path: &Path, config: &GamepadConfig) {
        let text = serde_json::to_string_pretty(config).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// 配置名拦住路径分隔符与隐藏文件名
    #[test]
    fn names_are_validated() {
        assert!(valid_name("default"));
        assert!(valid_name("我的配置"));
        assert!(!valid_name(""));
        assert!(!valid_name("../x"));
        assert!(!valid_name("a/b"));
        assert!(!valid_name(".hidden"));
    }

    /// 写出的默认配置带 Selected，能直接生效
    #[test]
    fn write_default_selects_profile() {
        let dir = mml_testutil::temp_dir().join(format!("gamepad_default_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let file = write_default(&dir).unwrap();
        let config = load_file(&file).unwrap();
        assert!(config.selected);
        assert!(config.compile().is_ok());
    }

    /// 目录里标记 Selected 的那份生效，坏文件只记错误
    #[test]
    fn scan_picks_selected_profile() {
        let dir = mml_testutil::temp_dir().join(format!("gamepad_scan_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut first = GamepadConfig::default();
        first.selected = false;
        write_config(&dir.join("a.json"), &first);
        let mut second = GamepadConfig::default();
        second.selected = true;
        write_config(&dir.join("b.json"), &second);
        std::fs::write(dir.join("broken.json"), "{").unwrap();

        let scan = scan(&dir);
        assert_eq!(scan.profiles.len(), 2);
        assert_eq!(scan.errors.len(), 1);
        let picked = pick(&scan.profiles).unwrap();
        assert!(picked.path.ends_with("b.json"));
    }

    /// 都没标记 Selected 时取文件名排序第一份
    #[test]
    fn scan_falls_back_to_first_file() {
        let dir = mml_testutil::temp_dir().join(format!("gamepad_order_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut config = GamepadConfig::default();
        config.selected = false;
        write_config(&dir.join("b.json"), &config);
        write_config(&dir.join("a.json"), &config);

        let scan = scan(&dir);
        let picked = pick(&scan.profiles).unwrap();
        assert!(picked.path.ends_with("a.json"));
    }

    /// 目录签名随文件修改时间变化
    #[test]
    fn signature_changes_with_file() {
        let dir = mml_testutil::temp_dir().join(format!("gamepad_sig_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(signature(&dir).is_empty());

        write_config(&dir.join("a.json"), &GamepadConfig::default());
        let first = signature(&dir);
        assert!(!first.is_empty());
        assert_eq!(first, signature(&dir));
    }
}
