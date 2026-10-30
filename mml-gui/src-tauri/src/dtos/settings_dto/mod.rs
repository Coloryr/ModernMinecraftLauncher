//! 设置窗口 DTO，TS 命名，camelCase wire
//!
//! 网络设置对应 core `HttpObj`，启动设置对应 `RunArgObj` + Java 列表；
//! core 枚举的 serde_repr 数字形态在 wire 上转成变体名字符串，
//! 与 gui_config 的枚举 wire 约定一致，值即 Rust 变体名。

mod conv;
mod launch;
mod local_files;
mod misc;
mod network;

pub use self::launch::RunArgSettingDto;
pub use self::local_files::{LocalFileEntryDto, LocalFileInstanceDto};
pub use self::misc::{BgInfoDto, LaunchSettingDto, SettingsDefaultsDto, WindowSettingDto};
pub use self::network::{DnsSettingDto, GameCheckSettingDto, NetworkSettingDto};
