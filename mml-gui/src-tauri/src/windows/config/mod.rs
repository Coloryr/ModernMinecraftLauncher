//! 游戏实例配置文件
//!
//! **一个实例一个窗口**：窗口身份由实例 uuid 派生、打开时就锁定，窗口内不再选择实例，
//! 见 `windows/registry` 的窗口族。NBT 会话也按实例归属，关掉某个实例的窗口不会动
//! 别的实例还开着的编辑器，见 [`crate::windows::nbt_session`]。
//!
//! 「修改实例配置」用的文件清单与四种读法：文本全文、NBT 树、区域文件区块表、
//! 任意文件的二进制分段。文件范围是 `.minecraft` 下的 options.txt 与
//! `config` / `kubejs` / `local`，另加某个存档目录（含数据包）。
//!
//! **路径一律按"相对所选来源"的口径传**：来源为空串是实例本身，即相对游戏目录；
//! 否则是存档目录名，相对该存档目录。列表给出来的路径原样回传即可，
//! 解析见 [`source::resolve_source`]。
//!
//! 文本的保存走**行级差异**而不是整份回传：前端算好"改了哪几行"（见
//! `mml-vue/src/lib/textDiff.ts`），后端按 [`crate::dtos::ConfigTextDto::sha1`] 核对磁盘上还是
//! 打开时那一份，再套用差异，见 [`files::config_save_file`]。
//!
//! NBT 走的是另一条路：打开一个 NBT 文件就是**真的把它打开**（句柄与解析结果留在会话里，
//! 见 [`crate::windows::nbt_session`]），之后的保存是"改值 / 新增 / 删除"三种操作，直接套在会话那份树上、
//! 由同一个句柄写回，不再重新打开与重新读盘。

pub(crate) mod files;
pub(crate) mod mca;
pub(crate) mod nbt;
mod nbt_edit;
mod source;
pub(crate) mod state;

#[cfg(test)]
mod tests;

pub use self::files::{config_delete_file, config_get_bytes_slice, config_get_file, config_get_files, config_get_image, config_open_file_dir, config_save_file};
pub use self::mca::{config_find_chunk, config_get_mca};
pub use self::nbt::{config_close_nbt, config_open_nbt, config_set_nbt};
pub use self::state::{config_set_tree_width, config_tree_width};

pub(super) use self::source::resolve_under;
