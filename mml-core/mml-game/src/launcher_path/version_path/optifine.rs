//! OptiFine 数据
//!
//! OptiFine 版本表的读写与缓存。

use super::*;

use std::{collections::HashMap, sync::Arc};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::uuids;

use crate::loader::optifine_obj::OptifineObj;

/// 加载高清修复版本信息
///
/// 从文件读取并填充缓存；文件不存在时忽略，解析失败记录日志
pub(super) fn load_optifine() {
    let local = OPTIFINE_FILE.get().unwrap();
    if !local.exists() || !local.is_file() {
        return;
    }

    let json = serialize_tools::json_from_file::<HashMap<String, OptifineObj>>(local);

    match json {
        Err(err) => {
            mml_log::error_type(err);
        }
        Ok(data) => {
            let mut list = OPTIFINE_LOADER.write().unwrap();
            list.clear();

            list.extend(data.into_iter().map(|(k, v)| (k, Arc::new(v))));
        }
    };
}

/// 保存高清修复版本信息
///
/// 将缓存写回文件；写盘失败仅记录错误，不影响调用方
pub(super) fn save_optifine() {
    let file = OPTIFINE_FILE.get().unwrap();
    let list = OPTIFINE_LOADER.read().unwrap();
    let value = &*list;

    let ref_map: HashMap<&String, &OptifineObj> =
        value.iter().map(|(k, v)| (k, v.as_ref())).collect();

    config_save::save(uuids::OPTIFINE_UUID, &ref_map, file);
}

/// 添加高清修复信息
///
/// - `obj`: 高清修复信息
///
/// # 返回值
///
/// 返回缓存的高清修复信息
pub fn add_optifine(obj: OptifineObj) -> Arc<OptifineObj> {
    let info = Arc::new(obj);
    // 写锁必须**先释放**再调 `save_optifine`：后者内部要取 OPTIFINE_LOADER 的**读锁**，
    // 而 `std::sync::RwLock` 不可重入，同线程持写锁时再取读锁会永久阻塞 = 自死锁。
    let mut list = OPTIFINE_LOADER.write().unwrap();
    list.insert(info.version.clone(), info.clone());

    drop(list);

    save_optifine();

    info
}

/// 获取高清修复信息
///
/// - `version`: 版本号
///
/// # 返回值
///
/// 返回高清修复信息；不存在返回 `None`
pub fn get_optifine(version: &str) -> Option<Arc<OptifineObj>> {
    let list = OPTIFINE_LOADER.read().unwrap();
    Some(list.get(version)?.clone())
}
