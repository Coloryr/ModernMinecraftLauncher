//! 窗口注册表：uuid / 标签 / 最小尺寸的**唯一来源**
//!
//! 从 `windows/mod.rs` 拆出来的：这里是一张纯数据表 + 几个反查函数，
//! 原先与"建窗 / 存几何 / 模型生命周期"混在一个 1200 行的文件里，
//! 改个窗口尺寸要翻半个文件。
//!
//! 尺寸口径：注册表里全是**客户区 + 逻辑像素**，与 Windows 外框之间的换算见
//! `super::geometry`。各窗口的默认尺寸就是这张表 —— 前端不再维护第二份。

mod family;
mod spec;
mod uuids;

#[cfg(test)]
mod tests;

pub(crate) use self::uuids::{
    ADD_MODPACK_WINDOW_UUID, ADD_WINDOW_UUID, BLOCK_WINDOW_UUID, EXPORT_WINDOW_UUID,
    MAIN_WINDOW_UUID, SERVER_PACK_WINDOW_UUID, uuid_for_kind, uuid_of_label,
};

pub(crate) use self::family::{
    FamilyWindowId, families, family_of_kind, family_of_label, family_window_id,
};

pub(crate) use self::spec::{MinSize, WINDOWS_INFO, min_size_of_label};
