//! 从 ColorMC 迁移：首次启动问一次，复制 / 移动 / 不迁移
//!
//! **迁移本身是原样的文件夹搬运**：把 ColorMC 工作目录的顶层条目复制或移动到 M²L 的
//! 运行目录，不做字段级转换、不做合并。两边目录结构同构 —— `minecraft/{instances,assets,
//! libraries,versions}`、`config.json`、`collect.json` 语义一致，两边实例又都在
//! `minecraft/instances/<名>/game.json`，所以实例搬过来 M²L 直接能用。
//!
//! **实例目录是唯一不原样覆盖的东西**，见 [`import`]：与已有实例重名时导入的那个改名、
//! 两份都留；uuid 冲突时只给导入的那个换新；ColorMC 的 `GroupName` 并进 `group_save.json`，
//! 同名组复用。实例的身份是 `game.json` 里的 uuid，改名 / 换 uuid 都必须写回文件。
//!
//! 搬完再做一次**兼容性检测**，见 [`check`]：哪些能直接用、哪些用起来有偏差、哪些 ColorMC
//! 有而 M²L 读不懂。实例是不是"真的能用"用 **M²L 自己的解析器**判定，即 `game.json` →
//! [`InstanceSettingObj`] —— M²L 只认"目录里有能解析的 `game.json`"的实例，所以解析失败
//! 就等于"搬过来了也看不见"。
//!
//! 实例目录里那份 `guisetting.json` 是 ColorMC 的界面设置，**M²L 不读**：M²L 自己那份叫
//! `gui_setting.json`，改名就是为了不撞它，理由见 `crate::gui_setting` 的文件头。
//! 于是那里的模组分组 / 备注 / 日志设置不会迁移过来 —— 但**文件本身留着、不会被覆盖**，
//! 弹窗里会把这件事说出来。实例的**分组名**不在这份文件里，走的是下面那条。
//!
//! 两条与用户约定好的口径，弹窗上也要写清楚：
//! - **普通条目一律覆盖**：运行目录里同名的文件 / 目录直接被 ColorMC 的版本盖掉。Windows 的
//!   `rename` 不覆盖已存在的目标，所以移动时先删掉目标条目。实例目录例外，见上；
//! - **移动 = 重命名**：按顶层条目 `rename`，同盘瞬时完成；跨盘报 `CrossesDevices` 时回退成
//!   "复制 + 删源"。**全部条目都搬完**才删空的源目录，任何一条失败都保留源目录。
//!
//! 单个文件失败**不中断**整次迁移，比如被别的进程占用、无权限，记进报告的 `failed` 里 ——
//! 真实场景下 `logs.log` / `config.json` / `count.dat` 可能正被 ColorMC 或本进程打开。
//!
//! 首次启动的"问过了"状态存在运行目录下的 `colormc_migrate.json`，见 [`mark`]：
//! 它表达的是"这个运行目录已经问过了"，删掉它就能重新触发询问。

mod check;
mod detect;
mod import;
mod migrate;

pub(crate) mod window;

#[cfg(test)]
mod tests;

pub(crate) use self::window::progress;
