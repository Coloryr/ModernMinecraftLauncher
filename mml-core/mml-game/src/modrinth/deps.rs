//! Modrinth 模组依赖递归解析

use super::*;

/// 模组依赖列表
pub struct ModrinthModDependenciesRes {
    /// 名字
    pub name: String,
    /// 图标地址，项目接口给的是远程地址，没有时是 None
    pub icon: Option<String>,
    /// 项目编号
    pub mod_id: String,
    /// 是否可选
    pub opt: bool,
    /// 候选文件，按"合适程度"排好，**第一个是自动选中的那个**；调用方可以让用户改选
    pub list: Vec<ModrinthVersionObj>,
}

/// 获取模组依赖
///
/// - `obj`: Modrinth 文件信息
/// - `version`: 游戏版本
/// - `loader`: 加载器类型
///
/// # 返回值
///
/// 返回依赖列表，含递归解析出的子依赖，已去重
pub async fn get_mod_dependencies(
    obj: &ModrinthVersionObj,
    version: &str,
    loader: LoaderType,
) -> Vec<ModrinthModDependenciesRes> {
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
    dependencies: &Vec<DependencieObj>,
    version: &str,
    loader: &LoaderType,
    ids: &Mutex<HashSet<String>>,
    handle: &tokio::runtime::Handle,
) -> Vec<ModrinthModDependenciesRes> {
    let list: Mutex<Vec<ModrinthModDependenciesRes>> = Mutex::new(Vec::new());

    dependencies.par_iter().for_each(|item| {
        // 无 project_id 的依赖是打包内置文件，如 Mod Menu Helper.zip，无法通过项目 API 解析
        let Some(project_id) = &item.project_id else {
            return;
        };

        // 原子检查并插入：HashSet::insert 在元素已存在时返回 false
        let mut ids_guard = ids.lock().unwrap();
        if !ids_guard.insert(project_id.clone()) {
            return;
        }

        drop(ids_guard);

        let id = project_id.clone();
        // 只保留"要用户去下"的两种类型。embedded 是打包在 jar 里的、incompatible 是明确
        // 冲突的，都不该出现在前置列表里，也不必为它们发请求。
        // 注意这里算的是"是否可选"，写进结果时**不要再取反**
        let optional = if item.dependency_type.eq_ignore_ascii_case("required") {
            false
        } else if item.dependency_type.eq_ignore_ascii_case("optional") {
            true
        } else {
            return;
        };

        let (res1, res2) = handle.block_on(async {
            let res1 = modrinth_api::get_project(&id).await;

            match res1 {
                Ok(data) => match &item.version_id {
                    // 依赖指名了版本：候选就只有这一个
                    Some(version) => (
                        Ok(data),
                        modrinth_api::get_version(project_id, version)
                            .await
                            .map(|item| vec![item]),
                    ),
                    None => {
                        let loader = to_loader_id(loader);
                        // 整份候选都带上：前端要让用户在这些版本里挑，不只用第一个
                        let data1 = modrinth_api::get_file_versions(
                            project_id,
                            Some(version),
                            loader.as_deref(),
                        )
                        .await;

                        (Ok(data), data1)
                    }
                },
                Err(err) => (Err(err), Err(ErrorType::InvalidOperation)),
            }
        });

        let Ok(data) = res1 else { return };
        let Ok(data1) = res2 else { return };
        // 该项目在本版本 / 加载器下一个文件都没有：当作这条依赖解析失败，整条跳过
        let Some(first) = data1.first() else { return };

        let sub_deps =
            get_mod_dependencies_inner(&first.dependencies, version, loader, ids, handle);

        // 图标：项目接口给的是空串，按"没有"处理
        let icon = (!data.icon_url.is_empty()).then(|| data.icon_url.clone());

        list.lock().unwrap().push(ModrinthModDependenciesRes {
            name: data.title,
            icon,
            mod_id: data.id.clone(),
            opt: optional,
            list: data1,
        });

        // 添加未被标记的子依赖，避免同时持有 ids 和 list 锁
        for item5 in sub_deps {
            let mut ids_guard = ids.lock().unwrap();
            if ids_guard.contains(&item5.mod_id) {
                continue;
            }
            ids_guard.insert(item5.mod_id.clone());
            drop(ids_guard);
            list.lock().unwrap().push(item5);
        }
    });

    list.into_inner().unwrap()
}
