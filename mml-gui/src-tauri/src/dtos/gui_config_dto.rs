//! GUI 配置前端 DTO：TS 命名，camelCase wire
//!
//! 配置文件 `gui_config.json` 落盘用 Rust 命名，见 `../gui_config.rs`，
//! 仅当跨 Tauri IPC 传输到前端时转成此结构：
//! - 字段 camelCase = 前端 `mml-vue/src/lib/guiConfig.ts` 的类型名
//! - 枚举值 = Rust 变体名："Dark" / "Light" / "Multi" / "Single" / "Left" / "Right"
//! - locale 为字符串，core `Lang` 未实现 serde

use serde::{Deserialize, Serialize};

use mml_names::Lang;

use crate::gui_config::{
    CapeConfig, CapeDisplay, ClientConfig, CollectConfig, GuiConfig, HeadConfig, HeadType,
    LoginLockObj, MainWindowConfig, SidebarSide, SkinConfig, SkinDisplay, StartupPage, Theme,
    ViewMode, WindowMode,
};

/// 皮肤显示设置，前端 wire 为 camelCase，字段即 `skin`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SkinConfigDto {
    /// Skin2DA / Skin2DB / Skin3D / Skin3DD / Skin3DRotate
    pub display: SkinDisplay,
    /// 3D 旋转 X
    pub pitch: f32,
    /// 3D 旋转 Y
    pub yaw: f32,
}

/// 披风显示设置，前端 wire 为 camelCase，字段即 `cape`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CapeConfigDto {
    /// Cape2D / Cape3D / Elytra
    pub display: CapeDisplay,
    /// 披风 3D 旋转 X
    pub pitch: f32,
    /// 披风 3D 旋转 Y
    pub yaw: f32,
}

/// 头像设置，前端 wire 为 camelCase，字段即 `head`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HeadConfigDto {
    /// Head2DA / Head3DA / Head3DB / Head2DB
    pub head_type: HeadType,
    /// 3D 旋转 X
    pub x: f32,
    /// 3D 旋转 Y
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
    /// 游戏自动进服地址，host 或 host:port，空串表示不自动进服
    pub auto_join_server: String,
    /// MOTD 显示地址，host 或 host:port，空串表示不显示真实服务器信息
    pub motd_server: String,
    /// 实例锁定：锁定的实例 uuid，空串表示未锁定；锁定时主窗口不允许切换到其它实例
    pub lock_instance: String,
    /// 自定义主页面：启用后主窗口的启动器主页换成导入的页面
    pub custom_home: bool,
    /// 自定义音乐：启用后主窗口播放 `music_source`
    pub music_on: bool,
    /// 自定义音乐来源：本地音频文件路径或 http(s) 直链，空串表示没配
    pub music_source: String,
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
            auto_join_server: c.auto_join_server,
            motd_server: c.motd_server,
            lock_instance: c.lock_instance,
            custom_home: c.custom_home,
            music_on: c.music_on,
            music_source: c.music_source,
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
            auto_join_server: c.auto_join_server,
            motd_server: c.motd_server,
            lock_instance: c.lock_instance,
            custom_home: c.custom_home,
            music_on: c.music_on,
            music_source: c.music_source,
        }
    }
}

/// GUI 配置，前端 wire 为 camelCase，即 `window_get_gui_config` 的返回值
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GuiConfigDto {
    /// 主题：Dark / Light
    pub theme: Theme,
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
                modpack: c.collect.modpack,
                show_mod: c.collect.show_mod,
                resource_pack: c.collect.resource_pack,
                shaderpack: c.collect.shaderpack,
            },
            client: ClientConfigDto {
                motd_card: c.client.motd_card,
                motd_interval: c.client.motd_interval,
                login_lock_on: c.client.login_lock_on,
                login_lock: c.client.login_lock.into_iter().map(Into::into).collect(),
                auto_join: c.client.auto_join,
                auto_join_server: c.client.auto_join_server,
                motd_server: c.client.motd_server,
                lock_instance: c.client.lock_instance,
                custom_home: c.client.custom_home,
                music_on: c.client.music_on,
                music_source: c.client.music_source,
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
            collect: CollectConfig {
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
                auto_join_server: d.client.auto_join_server,
                motd_server: d.client.motd_server,
                lock_instance: d.client.lock_instance,
                custom_home: d.client.custom_home,
                music_on: d.client.music_on,
                music_source: d.client.music_source,
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
        }
    }
}
