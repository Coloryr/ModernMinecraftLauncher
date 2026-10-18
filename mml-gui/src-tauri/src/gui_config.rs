//! GUI 状态，配置文件 gui_config.json
//!
//! 配置文件使用 Rust 命名：snake_case 字段 / PascalCase 枚举 / locale 变体名。
//! 仅当经 Tauri IPC 传输到前端时转成 `crate::dtos::GuiConfigDto`，TS 命名 camelCase。
//! 读写统一走 [`init`] 初始化后的内存缓存，保存经 `config_save` 异步落盘。

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::{Lang, names, uuids};
use serde::{Deserialize, Serialize};

/// 主题；serde 按变体名序列化，与前端同名
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Theme {
    /// 跟随系统深浅色
    #[default]
    System,
    /// 浅色
    Light,
    /// 深色
    Dark,
}

/// 窗口模式：多窗口 / 单窗口
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum WindowMode {
    /// 多窗口，默认
    #[default]
    Multi,
    /// 单窗口
    Single,
}

/// 侧栏位置
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SidebarSide {
    /// 左侧，默认
    #[default]
    Left,
    /// 右侧
    Right,
}

/// 主窗口实例列表显示模式；wire 名小写，与前端一致
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// 列表，默认
    #[default]
    List,
    /// 分组
    Group,
    /// 平铺
    Grid,
}

/// 启动器启动时主窗口先显示哪一屏；serde 按变体名序列化，与前端同名
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum StartupPage {
    /// 启动器主页，默认
    #[default]
    Home,
    /// 项目列表
    List,
}

/// 皮肤显示模式；serde 按变体名序列化，与前端同名
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SkinDisplay {
    /// 2D 平面图 TypeA；兼容旧值的 "Skin2D" 别名
    #[serde(alias = "Skin2D")]
    Skin2DA,
    /// 2D 大图，TypeB，默认
    #[default]
    Skin2DB,
    /// 3D 等距模型，面朝上，仰视
    Skin3D,
    /// 3D 等距模型，面朝下，俯视
    Skin3DD,
    /// 3D 等距模型，按配置的 pitch / yaw 旋转
    Skin3DRotate,
}

/// 皮肤显示设置，账户列表与设置页预览共用
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SkinConfig {
    /// 显示模式：Skin2DA / Skin2DB / Skin3D / Skin3DD / Skin3DRotate
    pub display: SkinDisplay,
    /// 3D 旋转模式绕 X 轴旋转角度，单位度；只有 Skin3DRotate 用
    pub pitch: f32,
    /// 3D 旋转模式绕 Y 轴旋转角度，单位度；只有 Skin3DRotate 用
    pub yaw: f32,
}

impl Default for SkinConfig {
    /// 默认 2D 大图；旋转角度取 X 15 度 / Y 45 度，与内置的两个固定 3D 档观感接近
    fn default() -> Self {
        Self {
            display: SkinDisplay::default(),
            pitch: 15.0,
            yaw: 45.0,
        }
    }
}

/// 头像类型；serde 按变体名序列化，与前端同名
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum HeadType {
    /// 2D 平面头像
    Head2DA,
    /// 3D 头像，面朝下，俯视
    Head3DA,
    /// 3D 头像，按 X / Y 旋转
    Head3DB,
    /// 3D 头像，面朝上，仰视
    Head3DC,
    /// 2D 大头像，默认
    #[default]
    Head2DB,
}

/// 披风显示模式；serde 按变体名序列化，与前端同名
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CapeDisplay {
    /// 披风 2D 正反面，默认；卡片出正面，悬浮大图并排正面 + 背面
    #[default]
    Cape2D,
    /// 披风 3D 等距图
    Cape3D,
    /// 鞘翅 3D 等距图
    Elytra,
}

/// 披风渲染设置，账户列表与设置页预览共用
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CapeConfig {
    /// 显示模式：Cape2D / Cape3D / Elytra
    pub display: CapeDisplay,
    /// 披风 3D 绕 X 轴旋转角度，单位度；只有 Cape3D 用
    pub pitch: f32,
    /// 披风 3D 绕 Y 轴旋转角度，单位度；只有 Cape3D 用
    pub yaw: f32,
}

impl Default for CapeConfig {
    /// 默认 2D 正反面；3D 角度取 X 0 度 / Y 30 度，侧一点更立体
    fn default() -> Self {
        Self {
            display: CapeDisplay::default(),
            pitch: 0.0,
            yaw: 30.0,
        }
    }
}

/// 头像渲染设置，账户列表与设置页预览共用
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HeadConfig {
    /// 头像类型：Head2DA / Head2DB / Head3DA / Head3DB / Head3DC
    pub head_type: HeadType,
    /// 3D 头像绕 X 轴旋转角度，单位度；只有 Head3DB 用
    pub x: f32,
    /// 3D 头像绕 Y 轴旋转角度，单位度；只有 Head3DB 用
    pub y: f32,
}

impl Default for HeadConfig {
    /// 默认 2D 大头像，旋转角 X 15 度 / Y 65 度
    fn default() -> Self {
        Self {
            head_type: HeadType::default(),
            x: 15.0,
            y: 65.0,
        }
    }
}

/// 主窗口配置；字段名即 wire 名，与前端同名
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MainWindowConfig {
    /// 侧栏位置：Left / Right
    pub sidebar_side: SidebarSide,
    /// 侧栏是否收起
    pub sidebar_collapsed: bool,
    /// 实例列表显示模式：list / group / grid，默认 list
    pub view_mode: ViewMode,
    /// 启动时先显示哪一屏：Home / List，默认 Home
    pub startup_page: StartupPage,
    /// 当前选中实例 uuid；空串表示未选中
    pub selected_instance: String,
}

impl Default for MainWindowConfig {
    /// 默认：侧栏在左且收起、列表模式、启动进启动器主页、未选中实例
    fn default() -> Self {
        Self {
            sidebar_side: Default::default(),
            // 首次启动或没有配置时默认收起侧栏
            sidebar_collapsed: true,
            // 默认列表模式，用户改过后用用户的值
            view_mode: ViewMode::default(),
            // 默认先显示启动器主页
            startup_page: StartupPage::default(),
            // 默认未选中
            selected_instance: String::new(),
        }
    }
}

/// 收藏界面设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectWindowConfig {
    /// 显示整合包
    pub modpack: bool,
    /// 显示模组
    pub show_mod: bool,
    /// 显示资源包
    pub resource_pack: bool,
    /// 显示光影包
    pub shaderpack: bool,
}

impl Default for CollectWindowConfig {
    /// 默认：四类资源都显示
    fn default() -> Self {
        Self {
            modpack: true,
            show_mod: true,
            resource_pack: true,
            shaderpack: true,
        }
    }
}

/// 登录方式锁定的一个条目
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LoginLockObj {
    /// 账户添加类型：offline / microsoft / littleskin / selflittleskin / authlib / nide8
    pub ty: String,
    /// 登录模型名字，即添加账户时下拉里的显示名；空串表示用类型名或服务器地址显示
    pub name: String,
    /// 锁定的服务器：authlib 填认证服务器地址，nide8 填服务器 ID，空串表示不指定
    pub server: String,
}

/// 客户端设置：主窗口 MOTD 卡片与登录方式限制
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ClientConfig {
    /// 主窗口实例设置里显示 MOTD 卡片
    pub motd_card: bool,
    /// MOTD 卡片刷新间隔，单位秒
    pub motd_interval: u32,
    /// 登录方式锁定总开关；false 时锁定列表不生效，任何类型都可添加
    pub login_lock_on: bool,
    /// 登录方式锁定列表；空表表示不锁定，可添加多个条目
    pub login_lock: Vec<LoginLockObj>,
    /// 游戏自动进服，启动时自动进入 `auto_join_server`
    pub auto_join: bool,
    /// 游戏自动进服地址，host 或 host:port；空串表示不自动进服
    pub auto_join_server: String,
    /// MOTD 显示地址，host 或 host:port；空串表示不显示真实服务器信息
    pub motd_server: String,
    /// 实例锁定：锁定的实例 uuid，空串表示未锁定；锁定时主窗口不允许切换到其它实例
    pub lock_instance: String,
    /// 自定义主页面：启用后主窗口的启动器主页换成 base_dir/custom_home 里的 index.html
    pub custom_home: bool,
    /// 自定义音乐：启用后主窗口播放 `music_source`，标题栏给播放 / 暂停 / 停止入口
    pub music_on: bool,
    /// 自定义音乐来源：本地音频文件路径或 http(s) 直链；空串表示没配
    pub music_source: String,
}

impl Default for ClientConfig {
    /// 默认：不开 MOTD 卡片、不锁登录方式、不自动进服、不启用自定义主页面
    fn default() -> Self {
        Self {
            motd_card: false,
            motd_interval: 15,
            login_lock_on: false,
            login_lock: Vec::new(),
            auto_join: false,
            auto_join_server: String::new(),
            motd_server: String::new(),
            lock_instance: String::new(),
            custom_home: false,
            music_on: false,
            music_source: String::new(),
        }
    }
}

/// 实验性功能开关
///
/// **行为都还没接线**：这里只把用户的选择存下来，设置页客户端设置里的「实验性功能」组负责展示，
/// 真正生效要等各自的实现（提权 / 自定义协议 / 沙盒）落地。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ExperimentalConfig {
    /// 启动器自身以管理员身份运行
    pub launcher_admin: bool,
    /// 启动实例时提升到管理员身份
    pub instance_admin: bool,
    /// 允许通过 URI（自定义协议）直接启动实例
    pub uri_launch: bool,
    /// 在沙盒里启动实例
    pub sandbox_launch: bool,
}

/// 界面设置，跨窗口共用的那份界面状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiConfig {
    /// 主题：System / Light / Dark
    pub theme: Theme,
    /// 强调色：预设 id（blue / green / …）或 custom
    pub accent: String,
    /// 自定义强调色，`#rrggbb`；`accent` 为 custom 时生效
    pub custom_accent: String,
    /// 语言，取值是内核的 Lang：zh_cn / en_us
    pub locale: Lang,
    /// 窗口模式：Multi / Single
    pub window_mode: WindowMode,
    /// 主窗口配置
    pub main_window: MainWindowConfig,
    /// 头像设置
    pub head: HeadConfig,
    /// 收藏界面设置
    pub collect_window: CollectWindowConfig,
    /// 客户端设置
    pub client: ClientConfig,
    /// 实验性功能开关（界面先行，行为未接线）
    pub experimental: ExperimentalConfig,
    /// 界面字体族名；空串表示用默认字体栈
    pub font: String,
    /// 皮肤显示设置：Skin2DA / Skin2DB / Skin3D / Skin3DD / Skin3DRotate 与旋转角度
    pub skin: SkinConfig,
    /// 披风显示设置：Cape2D / Cape3D / Elytra 与 3D 角度
    pub cape: CapeConfig,
    /// 背景图来源：文件路径或网址，空串表示无背景图
    pub bg_source: String,
    /// 背景图不透明度，单位百分比，取值 5 到 100
    pub bg_opacity: u32,
    /// 背景图模糊半径，单位像素，取值 0 到 40
    pub bg_blur: u32,
    /// 背景图原始分辨率，单位百分比，取值 10 到 100；后端按此把源图缩放后落盘
    pub bg_native_size: u32,
    /// 添加资源 / 下载整合包两个窗口的浏览偏好
    pub resource_window: ResourceWindowConfig,
    /// 配置文件窗口的布局偏好
    pub config_window: ConfigWindowConfig,
}

impl Default for GuiConfig {
    /// 默认：跟随系统主题、简体中文、多窗口、无背景图
    fn default() -> Self {
        Self {
            theme: Default::default(),
            accent: "blue".to_string(),
            custom_accent: "#4f8cff".to_string(),
            locale: Lang::zh_cn,
            window_mode: Default::default(),
            main_window: Default::default(),
            head: Default::default(),
            collect_window: Default::default(),
            client: Default::default(),
            experimental: Default::default(),
            font: String::new(),
            skin: Default::default(),
            cape: Default::default(),
            bg_source: String::new(),
            bg_opacity: 100,
            bg_blur: 0,
            bg_native_size: 100,
            resource_window: Default::default(),
            config_window: Default::default(),
        }
    }
}

/// 添加资源 / 下载整合包两个窗口的浏览偏好
///
/// 这两项是"上次选了哪个下载源"，属于用户偏好而不是设置页上的项：
/// 由各自的窗口命令维护（`add_resource_*_last_source` / `add_modpack_*_last_source`），
/// 设置页保存时从旧配置整份带过来，见 `window_save_gui_config`。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceWindowConfig {
    /// 添加资源窗口上次选的下载源；空串表示没选过，界面落回列表第一个
    pub add_resource_source: String,
    /// 下载整合包窗口上次选的下载源
    pub add_modpack_source: String,
}

/// 配置文件窗口的布局偏好
///
/// 属于用户偏好而不是设置页上的项：由窗口命令 `config_tree_width` / `config_set_tree_width`
/// 维护，设置页保存时从旧配置整份带过来，见 `window_save_gui_config`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigWindowConfig {
    /// 左侧文件树的宽度，单位像素；拖中间那条分隔线改的就是它
    pub tree_width: f64,
}

impl Default for ConfigWindowConfig {
    /// 默认左栏 300 像素，与主窗口侧栏同宽
    fn default() -> Self {
        Self { tree_width: 300.0 }
    }
}

/// 配置文件路径，init 时设置
static FILE: OnceLock<PathBuf> = OnceLock::new();
/// 内存配置缓存，init 时设置
static CONFIG: OnceLock<Arc<RwLock<GuiConfig>>> = OnceLock::new();
/// 启动时没有配置文件，说明用户还没做过任何显式设置，默认主题应跟随系统
static FRESH_CONFIG: AtomicBool = AtomicBool::new(false);

/// 读取配置，启动时调用
///
/// - `path`: 运行路径，配置文件放它下面
pub fn init<P: AsRef<Path>>(path: P) {
    let file = FILE.get_or_init(|| path.as_ref().join(names::GUI_CONFIG_FILE));

    let config = if file.exists()
        && file.is_file()
        && let Ok(data) = serialize_tools::json_from_file::<GuiConfig>(file)
    {
        data
    } else {
        FRESH_CONFIG.store(true, Ordering::Release);
        GuiConfig::default()
    };
    let _ = CONFIG.set(Arc::new(RwLock::new(config)));
}

/// 启动时是否没有配置文件，前端据此让默认主题跟随系统
///
/// # 返回值
///
/// 无配置文件返回 `true`
pub fn is_fresh() -> bool {
    FRESH_CONFIG.load(Ordering::Acquire)
}

/// 当前配置
///
/// # 返回值
///
/// 返回配置快照；未初始化返回默认值
pub fn get() -> GuiConfig {
    CONFIG
        .get()
        .map(|c| c.read().unwrap().clone())
        .unwrap_or_default()
}

/// 配置目录，背景图等派生文件放这里
///
/// # 返回值
///
/// 配置所在目录；未初始化返回 `None`
pub fn dir() -> Option<PathBuf> {
    FILE.get().and_then(|p| p.parent()).map(Path::to_path_buf)
}

/// 让内核侧文案，即日志，跟随界面语言
///
/// 内核语言另有 `lang.txt` 一份，不跟随时会一直停在启动时的语言上。
///
/// - `locale`: 界面语言
pub fn sync_core_lang(locale: Lang) {
    // 已经一致就别再写一次 lang.txt
    if mml_names::get_lang_type() == locale {
        return;
    }

    if let Err(err) = mml_names::set_lang(locale) {
        mml_log::error(format!("lang.txt 写入失败: {err:?}"));
    }
}

/// 更新配置并保存
///
/// - `config`: 新配置
pub fn set(config: GuiConfig) {
    sync_core_lang(config.locale);

    if let Some(c) = CONFIG.get() {
        *c.write().unwrap() = config;
    }
    // 用户做过显式设置：配置已落盘，不再视为首次启动
    FRESH_CONFIG.store(false, Ordering::Release);
    save();
}

/// 保存配置，异步写入 gui_config.json；未初始化时忽略
pub fn save() {
    let Some(file) = FILE.get() else {
        return;
    };
    let data = CONFIG.get().unwrap().read().unwrap();
    config_save::save(uuids::GUI_CONFIG_FILE_UUID, &*data, file);
}
