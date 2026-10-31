//! 内置配置文件的保留 UUID
//!
//! 每个被启动器管理的独立配置/数据文件分配一个固定 UUID，
//! 供后台保存队列去重与识别：相同 UUID 的新任务会替换旧任务。

use std::sync::LazyLock;

use uuid::{Uuid, uuid};

/// 全局配置 config.json 的保存任务
pub const CONFIG_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
/// 账户信息 auth.json 的保存任务
pub const AUTH_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");
/// OptiFine 信息 optifine.json 的保存任务
pub const OPTIFINE_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");
/// LiteLoader 信息 liteloader.json 的保存任务
pub const LITELOADER_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000004");
/// 备份信息 backup 的保存任务
pub const BACKUP_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000005");
/// 在线文件信息保存任务
pub const ONLINE_FILE_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000006");
/// 整合包服务端信息保存任务
pub const SERVERPACK_FILE_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000008");
/// 窗口几何 window_save.json 的保存任务
pub const WINDOW_FILE_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000009");
/// GUI 配置 gui_config.json 的保存任务
pub const GUI_CONFIG_FILE_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000a");
/// 账户选择 auth_select.json 的保存任务
pub const AUTH_SELECT_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000b");
/// 收藏夹 collect.json 的保存任务
pub const COLLECT_UUID: Uuid = uuid!("00000000-0000-0000-0000-00000000000c");
/// 分组表 group_save.json 的保存任务
///
/// 用 `...0011`：`...0001`~`...0010` 已被 GUI 各窗口的 uuid 占用，
/// 见 mml-gui/src-tauri/src/windows/mod.rs 的 WINDOWS_INFO
pub const GROUP_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000011");
/// 方块窗口用户数据 block_groups.json 的保存任务
pub const BLOCK_GROUPS_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000012");
/// 云同步与手柄 integrations.json 的保存任务。
pub const INTEGRATIONS_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000013");

/// 实例映射记录 link_info.json 的保存任务命名空间
///
/// 每个实例用 `mix_uuid(实例 uuid, LINK_UUID)` 派生自己的保存任务。
pub const LINK_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000014");

/// 公共模组索引 mod_download.json 的保存任务
pub const MOD_DOWNLOAD_INFO_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000015");
/// 公共资源包索引 resourcepack_download.json 的保存任务
pub const RESOURCE_PACK_DOWNLOAD_INFO_UUID: Uuid = uuid!("00000000-0000-0000-0000-000000000016");

/// 全部内置保留 UUID，供 [`check_uuid`] 遍历
static UUIDS: LazyLock<Vec<Uuid>> = LazyLock::new(|| {
    vec![
        CONFIG_UUID,
        AUTH_UUID,
        OPTIFINE_UUID,
        LITELOADER_UUID,
        BACKUP_UUID,
        ONLINE_FILE_UUID,
        SERVERPACK_FILE_UUID,
        WINDOW_FILE_UUID,
        GUI_CONFIG_FILE_UUID,
        AUTH_SELECT_UUID,
        COLLECT_UUID,
        GROUP_UUID,
        BLOCK_GROUPS_UUID,
        INTEGRATIONS_UUID,
        LINK_UUID,
        MOD_DOWNLOAD_INFO_UUID,
        RESOURCE_PACK_DOWNLOAD_INFO_UUID,
    ]
});

/// 检查是否是内置配置文件的保留 UUID
///
/// - `uuid`: 需要检查的 uuid
///
/// # 返回值
///
/// 命中任一内置保留 UUID 时为 `true`
pub fn check_uuid(uuid: Uuid) -> bool {
    for item in UUIDS.iter() {
        if uuid.eq(item) {
            return true;
        }
    }

    false
}

/// 由两个 UUID 派生出混合 UUID
///
/// - `uuid1`: 第一个 UUID，作为 UUIDv5 的命名空间
/// - `uuid2`: 第二个 UUID，作为 UUIDv5 的名称数据
///
/// # 返回值
///
/// 派生出的 UUIDv5，同参数结果确定，参数顺序会影响结果
pub fn mix_uuid(uuid1: Uuid, uuid2: Uuid) -> Uuid {
    Uuid::new_v5(&uuid1, uuid2.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 所有内置配置 UUID 都应被 check_uuid 识别
    #[test]
    fn check_builtin_uuids() {
        for uuid in UUIDS.iter() {
            assert!(check_uuid(*uuid), "内置 UUID {uuid} 应被识别");
        }
    }

    /// 随机 UUID 不应被识别为配置 UUID
    #[test]
    fn check_random_uuid() {
        assert!(!check_uuid(Uuid::new_v4()));
        assert!(!check_uuid(Uuid::nil()));
    }

    /// 内置 UUID 互不重复
    #[test]
    fn builtin_uuids_unique() {
        let mut list = UUIDS.clone();
        list.sort();
        let before = list.len();
        list.dedup();
        assert_eq!(list.len(), before, "内置 UUID 不应重复");
        assert_eq!(before, UUIDS.len());
    }

    /// mix_uuid 基于 UUIDv5，同参数结果确定，不同参数结果不同
    #[test]
    fn mix_uuid_deterministic() {
        let a = mix_uuid(CONFIG_UUID, AUTH_UUID);
        let b = mix_uuid(CONFIG_UUID, AUTH_UUID);
        assert_eq!(a, b, "同参数混合结果应一致");
        assert_ne!(a, mix_uuid(AUTH_UUID, CONFIG_UUID), "参数顺序影响结果");
        assert_ne!(a, CONFIG_UUID);
        assert_ne!(a, AUTH_UUID);
        assert_eq!(a.get_version_num(), 5, "混合结果应是 UUIDv5");
    }
}
