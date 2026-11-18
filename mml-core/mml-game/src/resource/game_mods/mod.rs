//! 游戏实例模组
//!
//! 模组类型与元数据结构在 `types`，各加载器的元数据解析在 `meta`，
//! jar 读取与扫描在 `jar`，mixin 目标在 `mixin`，ASM 变压器在 `asm`，
//! 启用 / 停用在 `toggle`，依赖与重复检测在 `depend`。

mod asm;
mod depend;
mod jar;
mod meta;
mod mixin;
mod toggle;
mod types;

pub use self::depend::{
    DependantProblem, DependantProblemKind, DependantTest, ModDuplicate, ModDuplicateFile,
    ProvidedVersion, loader_modids, mods_dep_test, mods_duplicate_test, provided_version,
};
pub use self::asm::{ASM_SOURCE_COREMODS, ASM_SOURCE_PLUGIN, ASM_SOURCE_SERVICE};
pub use self::jar::{gen_mod_uuid, read_mod_info};
pub use self::toggle::{add_disable_suffix, remove_disable_suffix};
pub use self::types::{
    AsmTarget, DependantInfo, DependantType, LoadSideType, MixinTarget, ModItemObj, ModObj,
};

#[cfg(test)]
#[path = "../../../tests/unit/game_mods_patch_tests.rs"]
mod patch_tests;

#[cfg(test)]
#[path = "../../../tests/unit/game_mods_tests.rs"]
mod tests;
