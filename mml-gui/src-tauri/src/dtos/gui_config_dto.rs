//! GUI 配置前端 DTO：TS 命名，camelCase wire
//!
//! 配置文件 `gui_config.json` 落盘用 Rust 命名，见 `../gui_config.rs`，
//! 仅当跨 Tauri IPC 传输到前端时转成此结构：
//! - 字段 camelCase = 前端 `mml-vue/src/lib/guiConfig.ts` 的类型名
//! - 枚举值 = Rust 变体名，如 `System` / `Dark` / `Multi` / `Left`
//! - locale 为字符串，core `Lang` 未实现 serde

use serde::{Deserialize, Serialize};

use mml_names::Lang;

use crate::gui_config::{
    CapeConfig, CapeDisplay, ClientConfig, CollectWindowConfig, CustomCssItemObj,
    ExperimentalConfig, GuiConfig, HeadConfig, HeadType, LaunchCheckConfig, LoginLockObj,
    MainWindowConfig, SidebarSide, SkinConfig, SkinDisplay, StartupPage, Theme, ViewMode,
    WindowMode,
};

/// 皮肤显示设置，前端 wire 为 camelCase，字段即 `skin`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SkinConfigDto {
    /// Skin2DA / Skin2DB / Skin3D / Skin3DD / Skin3DRotate
    pub display: SkinDisplay,
    /// 绕 X 轴旋转角度，单位度；只有 Skin3DRotate 用
    pub pitch: f32,
    /// 绕 Y 轴旋转角度，单位度；只有 Skin3DRotate 用
    pub yaw: f32,
}

/// 披风显示设置，前端 wire 为 camelCase，字段即 `cape`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CapeConfigDto {
    /// Cape2D / Cape3D / Elytra
    pub display: CapeDisplay,
    /// 披风 3D 绕 X 轴旋转角度，单位度；只有 Cape3D 用
    pub pitch: f32,
    /// 披风 3D 绕 Y 轴旋转角度，单位度；只有 Cape3D 用
    pub yaw: f32,
}

/// 头像设置，前端 wire 为 camelCase，字段即 `head`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HeadConfigDto {
    /// Head2DA / Head3DA / Head3DB / Head3DC / Head2DB
    pub head_type: HeadType,
    /// 绕 X 轴旋转角度，单位度；只有 Head3DB 用
    pub x: f32,
    /// 绕 Y 轴旋转角度，单位度；只有 Head3DB 用
    pub y: f32,
}

/// 主窗口配置，前端 wire 为 camelCase，字段即 `mainWindow`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MainWindowConfigDto {
    /// 侧栏位置：Left / Right
    pub sidebar_side: SidebarSide,
    /// 侧栏是否收起
    pub sidebar_collapsed: bool,
    /// 实例列表显示模式：list / group / grid，默认 list
    pub view_mode: ViewMode,
    /// 启动时先显示哪一屏：Home / List，默认 Home
    pub startup_page: StartupPage,
    /// 当前选中实例 uuid，空串表示未选中
    pub selected_instance: String,
}

/// 收藏界面设置，前端 wire 为 camelCase，字段即 `collect`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CollectConfigDto {
    /// 显示整合包
    pub modpack: bool,
    /// 显示模组
    pub show_mod: bool,
    /// 显示资源包
    pub resource_pack: bool,
    /// 显示光影包
    pub shaderpack: bool,
}

/// 登录方式锁定的一个条目，前端 wire 为 camelCase
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LoginLockItemDto {
    /// 账户添加类型：offline / microsoft / littleskin / selflittleskin / authlib / nide8
    pub ty: String,
    /// 登录模型名字，添加账户时下拉里的显示名；空串表示用类型名 / 服务器地址显示
    pub name: String,
    /// 锁定的服务器，authlib 为认证服务器地址，nide8 为服务器 ID，空串表示不指定
    pub server: String,
}

impl From<LoginLockObj> for LoginLockItemDto {
    /// 磁盘上的登录方式锁定项 → 前端 wire 形态
    ///
    /// - `o`: 磁盘配置里的锁定条目
    ///
    /// # 返回值
    ///
    /// 返回字段一一对应的 DTO
    fn from(o: LoginLockObj) -> Self {
        Self {
            ty: o.ty,
            name: o.name,
            server: o.server,
        }
    }
}

impl From<LoginLockItemDto> for LoginLockObj {
    /// 前端 wire 形态 → 磁盘上的登录方式锁定项，保存配置时用
    ///
    /// - `d`: 前端提交的锁定条目
    ///
    /// # 返回值
    ///
    /// 返回字段一一对应的磁盘配置
    fn from(d: LoginLockItemDto) -> Self {
        Self {
            ty: d.ty,
            name: d.name,
            server: d.server,
        }
    }
}

/// 一条已安装的自定义 CSS，前端 wire 为 camelCase
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomCssItemDto {
    /// 条目 id，CSS 文件在 `custom_ui/css` 下叫 `<id>.css`
    pub id: String,
    /// 显示名，导入时的文件名
    pub name: String,
    /// 是否启用
    pub enabled: bool,
}

impl From<CustomCssItemObj> for CustomCssItemDto {
    /// 磁盘上的自定义 CSS 条目 → 前端 wire 形态
    ///
    /// - `o`: 磁盘配置里的条目
    ///
    /// # 返回值
    ///
    /// 返回字段一一对应的 DTO
    fn from(o: CustomCssItemObj) -> Self {
        Self {
            id: o.id,
            name: o.name,
            enabled: o.enabled,
        }
    }
}

impl From<CustomCssItemDto> for CustomCssItemObj {
    /// 前端 wire 形态 → 磁盘上的自定义 CSS 条目，保存配置时用
    ///
    /// - `d`: 前端提交的条目
    ///
    /// # 返回值
    ///
    /// 返回字段一一对应的磁盘配置
    fn from(d: CustomCssItemDto) -> Self {
        Self {
            id: d.id,
            name: d.name,
            enabled: d.enabled,
        }
    }
}

/// 客户端设置，前端 wire 为 camelCase，字段即 `client`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientConfigDto {
    /// 主窗口实例设置里显示 MOTD 卡片
    pub motd_card: bool,
    /// MOTD 卡片刷新间隔，单位秒
    pub motd_interval: u32,
    /// 登录方式锁定总开关，false 表示锁定列表不生效
    pub login_lock_on: bool,
    /// 登录方式锁定列表，空表示不锁定，可添加多个条目
    pub login_lock: Vec<LoginLockItemDto>,
    /// 游戏自动进服，启动时自动进入 `auto_join_server`
    pub auto_join: bool,
    /// 启动实例时检查账户是否已被其它实例占用
    pub account_check: bool,
    /// 实例配了自动进入存档时，启动前是否弹窗确认
    pub auto_save_notice: bool,
    /// 游戏自动进服地址，host 或 host:port，空串表示不自动进服
    pub auto_join_server: String,
    /// MOTD 显示地址，host 或 host:port，空串表示不显示真实服务器信息
    pub motd_server: String,
    /// 实例锁定：锁定的实例 uuid，空串表示未锁定；锁定时主窗口不允许切换到其它实例
    pub lock_instance: String,
    /// 锁定客户端设置：由运行目录的隐藏锁文件推导，只读
    pub lock_client: bool,
    /// 自定义主页面：启用后主窗口的启动器主页换成导入的页面
    pub custom_home: bool,
    /// 自定义 CSS 总开关
    pub custom_css_on: bool,
    /// 已安装的自定义 CSS，顺序即注入顺序
    pub custom_css: Vec<CustomCssItemDto>,
    /// 完全自定义界面总开关
    pub custom_page_on: bool,
    /// 自定义音乐：启用后主窗口播放 `music_source`
    pub music_on: bool,
    /// 自定义音乐来源：本地音频文件路径或 http(s) 直链，空串表示没配
    pub music_source: String,
    /// 窗口材质：auto / mica / acrylic / blur / none
    pub window_effect: String,
    /// 窗口底色不透明度，单位百分比
    pub window_opacity: u8,
    /// 自定义程序图标文件名，空串表示用内置图标
    pub program_icon: String,
}

impl From<ClientConfig> for ClientConfigDto {
    /// 磁盘客户端设置 → 前端 wire 形态
    ///
    /// - `c`: 磁盘配置里的客户端设置
    ///
    /// # 返回值
    ///
    /// 返回转换后的 DTO，登录方式锁定列表逐项转换
    fn from(c: ClientConfig) -> Self {
        Self {
            motd_card: c.motd_card,
            motd_interval: c.motd_interval,
            login_lock_on: c.login_lock_on,
            login_lock: c.login_lock.into_iter().map(Into::into).collect(),
            auto_join: c.auto_join,
            account_check: c.account_check,
            auto_save_notice: c.auto_save_notice,
            auto_join_server: c.auto_join_server,
            motd_server: c.motd_server,
            lock_instance: c.lock_instance,
            lock_client: crate::client_lock::locked(),
            custom_home: c.custom_home,
            custom_css_on: c.custom_css_on,
            custom_css: c.custom_css.into_iter().map(Into::into).collect(),
            custom_page_on: c.custom_page_on,
            music_on: c.music_on,
            music_source: c.music_source,
            window_effect: c.window_effect,
            window_opacity: c.window_opacity,
            program_icon: c.program_icon,
        }
    }
}

impl From<ClientConfigDto> for ClientConfig {
    /// 前端 wire 形态 → 磁盘客户端设置，保存配置时用
    ///
    /// - `c`: 前端提交的客户端设置
    ///
    /// # 返回值
    ///
    /// 返回可落盘的客户端设置
    fn from(c: ClientConfigDto) -> Self {
        Self {
            motd_card: c.motd_card,
            motd_interval: c.motd_interval,
            login_lock_on: c.login_lock_on,
            login_lock: c.login_lock.into_iter().map(Into::into).collect(),
            auto_join: c.auto_join,
            account_check: c.account_check,
            auto_save_notice: c.auto_save_notice,
            auto_join_server: c.auto_join_server,
            motd_server: c.motd_server,
            lock_instance: c.lock_instance,
            custom_home: c.custom_home,
            custom_css_on: c.custom_css_on,
            custom_css: c.custom_css.into_iter().map(Into::into).collect(),
            custom_page_on: c.custom_page_on,
            music_on: c.music_on,
            music_source: c.music_source,
            window_effect: c.window_effect,
            window_opacity: c.window_opacity,
            program_icon: c.program_icon,
        }
    }
}

/// 实验性功能开关，wire 为 camelCase
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExperimentalConfigDto {
    /// 启动器自身以管理员身份运行
    pub launcher_admin: bool,
    /// 启动实例时提升到管理员身份
    pub instance_admin: bool,
    /// 模组与资源包下载到公共目录，实例内用硬链接映射
    pub resource_link: bool,
    /// 允许通过 URI（自定义协议）直接启动实例
    pub uri_launch: bool,
    /// 在沙盒里启动实例
    pub sandbox_launch: bool,
}

/// 启动前检查开关，wire 为 camelCase
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchCheckConfigDto {
    /// 实例有模组、加载器却是原版时提醒
    pub check_loader: bool,
    /// 模组偏多、最大内存偏低时提醒
    pub check_memory: bool,
}

/// GUI 配置，前端 wire 为 camelCase，即 `window_get_gui_config` 的返回值
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GuiConfigDto {
    /// 主题：System / Light / Dark
    pub theme: Theme,
    /// 强调色：预设 id（blue / green / …）或 custom
    pub accent: String,
    /// 自定义强调色，`#rrggbb`
    pub custom_accent: String,
    /// 语言：zh_cn / en_us
    pub locale: Lang,
    /// 窗口模式：Multi / Single
    pub window_mode: WindowMode,
    /// 主窗口配置
    pub main_window: MainWindowConfigDto,
    /// 头像设置
    pub head: HeadConfigDto,
    /// 收藏界面设置
    pub collect: CollectConfigDto,
    /// 客户端设置
    pub client: ClientConfigDto,
    /// 实验性功能开关
    pub experimental: ExperimentalConfigDto,
    /// 启动前检查
    pub launch_check: LaunchCheckConfigDto,
    /// 界面字体族名，空串表示默认字体栈
    pub font: String,
    /// 皮肤显示设置：Skin2DA / Skin2DB / Skin3D / Skin3DD / Skin3DRotate 与旋转角度
    pub skin: SkinConfigDto,
    /// 披风显示设置：Cape2D / Cape3D / Elytra 与 3D 角度
    pub cape: CapeConfigDto,
    /// 背景图来源，文件路径或网址，空串表示无背景图
    pub bg_source: String,
    /// 背景图不透明度，单位 %，取值 5–100
    pub bg_opacity: u32,
    /// 背景图模糊，单位 px，取值 0–40
    pub bg_blur: u32,
    /// 背景图原始分辨率，单位 %，取值 10–100
    pub bg_native_size: u32,
}

/// 主题 / 强调色变更事件的负载
///
/// 设置窗口改完只影响它自己那个 webview，其它窗口靠这条事件跟上，见 `emit_gui_theme_change`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiThemeDto {
    /// 主题：System / Light / Dark
    pub theme: Theme,
    /// 强调色：预设 id（blue / green / …）或 custom
    pub accent: String,
    /// 自定义强调色，`#rrggbb`
    pub custom_accent: String,
}

impl From<GuiConfig> for GuiConfigDto {
    /// 磁盘配置 → 前端 wire 形态
    ///
    /// - `c`: 磁盘上的 GUI 配置
    ///
    /// # 返回值
    ///
    /// 返回各子结构一并转换后的 DTO
    fn from(c: GuiConfig) -> Self {
        Self {
            theme: c.theme,
            accent: c.accent,
            custom_accent: c.custom_accent,
            locale: c.locale,
            window_mode: c.window_mode,
            main_window: MainWindowConfigDto {
                sidebar_side: c.main_window.sidebar_side,
                sidebar_collapsed: c.main_window.sidebar_collapsed,
                view_mode: c.main_window.view_mode,
                startup_page: c.main_window.startup_page,
                selected_instance: c.main_window.selected_instance,
            },
            head: HeadConfigDto {
                head_type: c.head.head_type,
                x: c.head.x,
                y: c.head.y,
            },
            collect: CollectConfigDto {
                modpack: c.collect_window.modpack,
                show_mod: c.collect_window.show_mod,
                resource_pack: c.collect_window.resource_pack,
                shaderpack: c.collect_window.shaderpack,
            },
            client: ClientConfigDto {
                motd_card: c.client.motd_card,
                motd_interval: c.client.motd_interval,
                login_lock_on: c.client.login_lock_on,
                login_lock: c.client.login_lock.into_iter().map(Into::into).collect(),
                auto_join: c.client.auto_join,
                account_check: c.client.account_check,
                auto_save_notice: c.client.auto_save_notice,
                auto_join_server: c.client.auto_join_server,
                motd_server: c.client.motd_server,
                lock_instance: c.client.lock_instance,
                lock_client: crate::client_lock::locked(),
                custom_home: c.client.custom_home,
                custom_css_on: c.client.custom_css_on,
                custom_css: c.client.custom_css.into_iter().map(Into::into).collect(),
                custom_page_on: c.client.custom_page_on,
                music_on: c.client.music_on,
                music_source: c.client.music_source,
                window_effect: c.client.window_effect,
                window_opacity: c.client.window_opacity,
                program_icon: c.client.program_icon,
            },
            experimental: ExperimentalConfigDto {
                launcher_admin: c.experimental.launcher_admin,
                instance_admin: c.experimental.instance_admin,
                resource_link: c.experimental.resource_link,
                uri_launch: c.experimental.uri_launch,
                sandbox_launch: c.experimental.sandbox_launch,
            },
            launch_check: LaunchCheckConfigDto {
                check_loader: c.launch_check.check_loader,
                check_memory: c.launch_check.check_memory,
            },
            font: c.font,
            skin: SkinConfigDto {
                display: c.skin.display,
                pitch: c.skin.pitch,
                yaw: c.skin.yaw,
            },
            cape: CapeConfigDto {
                display: c.cape.display,
                pitch: c.cape.pitch,
                yaw: c.cape.yaw,
            },
            bg_source: c.bg_source,
            bg_opacity: c.bg_opacity,
            bg_blur: c.bg_blur,
            bg_native_size: c.bg_native_size,
        }
    }
}

impl From<GuiConfigDto> for GuiConfig {
    /// 前端 wire 形态 → 磁盘配置
    ///
    /// - `d`: 前端提交的 GUI 配置
    ///
    /// # 返回值
    ///
    /// 返回可落盘的 GUI 配置
    fn from(d: GuiConfigDto) -> Self {
        Self {
            theme: d.theme,
            accent: d.accent,
            custom_accent: d.custom_accent,
            locale: d.locale,
            window_mode: d.window_mode,
            main_window: MainWindowConfig {
                sidebar_side: d.main_window.sidebar_side,
                sidebar_collapsed: d.main_window.sidebar_collapsed,
                view_mode: d.main_window.view_mode,
                startup_page: d.main_window.startup_page,
                selected_instance: d.main_window.selected_instance,
            },
            head: HeadConfig {
                head_type: d.head.head_type,
                x: d.head.x,
                y: d.head.y,
            },
            collect_window: CollectWindowConfig {
                modpack: d.collect.modpack,
                show_mod: d.collect.show_mod,
                resource_pack: d.collect.resource_pack,
                shaderpack: d.collect.shaderpack,
            },
            client: ClientConfig {
                motd_card: d.client.motd_card,
                motd_interval: d.client.motd_interval,
                login_lock_on: d.client.login_lock_on,
                login_lock: d.client.login_lock.into_iter().map(Into::into).collect(),
                auto_join: d.client.auto_join,
                account_check: d.client.account_check,
                auto_save_notice: d.client.auto_save_notice,
                auto_join_server: d.client.auto_join_server,
                motd_server: d.client.motd_server,
                lock_instance: d.client.lock_instance,
                custom_home: d.client.custom_home,
                custom_css_on: d.client.custom_css_on,
                custom_css: d.client.custom_css.into_iter().map(Into::into).collect(),
                custom_page_on: d.client.custom_page_on,
                music_on: d.client.music_on,
                music_source: d.client.music_source,
                window_effect: d.client.window_effect,
                window_opacity: d.client.window_opacity,
                program_icon: d.client.program_icon,
            },
            experimental: ExperimentalConfig {
                launcher_admin: d.experimental.launcher_admin,
                instance_admin: d.experimental.instance_admin,
                resource_link: d.experimental.resource_link,
                uri_launch: d.experimental.uri_launch,
                sandbox_launch: d.experimental.sandbox_launch,
            },
            launch_check: LaunchCheckConfig {
                check_loader: d.launch_check.check_loader,
                check_memory: d.launch_check.check_memory,
            },
            font: d.font,
            skin: SkinConfig {
                display: d.skin.display,
                pitch: d.skin.pitch,
                yaw: d.skin.yaw,
            },
            cape: CapeConfig {
                display: d.cape.display,
                pitch: d.cape.pitch,
                yaw: d.cape.yaw,
            },
            bg_source: d.bg_source,
            bg_opacity: d.bg_opacity,
            bg_blur: d.bg_blur,
            bg_native_size: d.bg_native_size,
            // 不属于设置页，由各自的命令维护；保存设置时从旧配置整份带回来
            resource_window: Default::default(),
            config_window: Default::default(),
        }
    }
}
