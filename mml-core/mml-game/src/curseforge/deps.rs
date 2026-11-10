//! CurseForge 模组依赖递归解析

use super::*;

/// 模组依赖列表
pub struct CurseForgeModDependenciesRes {
    /// 名字
    pub name: String,
    /// 图标地址，项目接口给的是远程地址，没有时是 None
    pub icon: Option<String>,
    /// 项目编号
    pub mod_id: u64,
    /// 是否可选
    pub opt: bool,
    /// 候选文件，按"合适程度"排好，**第一个是自动选中的那个**；调用方可以让用户改选
    pub list: Vec<CurseForgeFileDataObj>,
}

/// 获取模组依赖
///
/// - `obj`: CurseForge 文件信息
/// - `version`: 游戏版本
/// - `loader`: 加载器类型
///
/// # 返回值
///
/// 返回依赖列表，含递归解析出的子依赖，已去重
pub async fn get_mod_dependencies(
    obj: &CurseForgeFileDataObj,
    version: &str,
    loader: LoaderType,
) -> Vec<CurseForgeModDependenciesRes> {
    let ids = Mutex::new(HashSet::new());
    let handle = tokio::runtime::Handle::current();
    let dependencies = obj.dependencies.clone();
    let version = version.to_string();

    tokio::task::spawn_blocking(move || {
        get_mod_dependencies_inner(&dependencies, &version, &loader, &ids, &handle)
    })
    .await
    .unwrap()
}

/// 递归解析模组依赖
///
/// - `dependencies`: 依赖列表
/// - `version`: 游戏版本
/// - `loader`: 加载器类型
/// - `ids`: 已解析的项目 ID 集合，避免重复解析
/// - `handle`: 用于并发查询的运行时句柄
///
/// # 返回值
///
/// 返回新解析出的依赖列表
fn get_mod_dependencies_inner(
    dependencies: &Option<Vec<DependenciesObj>>,
    version: &str,
    loader: &LoaderType,
    ids: &Mutex<HashSet<u64>>,
    handle: &tokio::runtime::Handle,
) -> Vec<CurseForgeModDependenciesRes> {
    let dep = match dependencies {
        Some(dep) if !dep.is_empty() => dep,
        _ => return Vec::new(),
    };

    let list: Mutex<Vec<CurseForgeModDependenciesRes>> = Mutex::new(Vec::new());

    dep.par_iter().for_each(|item| {
        // 原子检查并插入：HashSet::insert 在元素已存在时返回 false
        let mut ids_guard = ids.lock().unwrap();
        if !ids_guard.insert(item.mod_id) {
            return;
        }

        drop(ids_guard);

        let id = item.mod_id.to_string();
        // 只保留"要用户去下"的两种关系：1 内嵌库 / 4 工具 / 5 明确冲突 / 6 包含都不算前置。
        // 这里算的是"是否可选"，写进结果时**不要再取反**
        let optional = match item.relation_type {
            2 => true,
            3 => false,
            _ => return,
        };

        let (res1, res2) = handle.block_on(async {
            let res1 = curseforge_api::get_files_page(CurseFogreArg {
                id: Some(id.clone()),
                version: Some(version.to_string()),
                loader: to_loader_id(loader),
                ..Default::default()
            })
            .await;

            if res1.is_err() {
                return (None, None);
            }

            let data = res1.unwrap();
            if data.data.is_empty() {
                return (None, None);
            }

            let res2 = curseforge_api::get_mod_info(&id).await;
            (Some(data), res2.ok())
        });

        let Some(data) = res1 else { return };
        let Some(data1) = res2 else { return };

        // 在移动 data.data 之前，先递归获取第一个文件的依赖
        let sub_deps =
            get_mod_dependencies_inner(&data.data[0].dependencies, version, loader, ids, handle);

        list.lock().unwrap().push(CurseForgeModDependenciesRes {
            name: data1.data.name.clone(),
            icon: data1.data.logo.url.clone(),
            mod_id: data1.data.id,
            opt: optional,
            list: data.data,
        });

        // 添加未被标记的子依赖，避免同时持有 ids 和 list 锁
        for item5 in sub_deps {
            let mut ids_guard = ids.lock().unwrap();
            if ids_guard.contains(&item5.mod_id) {
                continue;
            }
            ids_guard.insert(item5.mod_id);
            drop(ids_guard);
            list.lock().unwrap().push(item5);
        }
    });

    list.into_inner().unwrap()
}
