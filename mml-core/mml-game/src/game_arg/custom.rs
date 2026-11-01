use super::*;
use super::args::*;
use super::libs::*;

impl InstanceSettingObj {
    /// 创建自定义json启动内容，走MMC整合包格式
    ///
    /// - `arg`: 启动配置
    /// - `obj`: 待填充的启动内容
    /// - `game_args`: 启动器自定义游戏参数
    ///
    /// # 返回值
    ///
    /// 返回是否成功，资源文件缺失返回对应错误。
    pub(super) async fn make_custom_arg(
        &self,
        arg: &GameLaunchArg,
        obj: &mut GameLaunchObj,
        game_args: Vec<String>,
    ) -> CoreResult<()> {
        // 运行库三层叠加，后面的覆盖前面的：原版 → 加载器 → patch。
        // get_libs 的去重是"后插入者胜"，所以顺序必须就是这个顺序，
        // patch 的库留在下面 custom 循环里最后加。
        // 原版这份先按实例版本号补齐：patch 通常不带客户端下载地址，
        // 不补会缺 net.minecraft.client.main.Main。失败不拦启动，patch 也可能自带
        let vanilla = self.check_version_update().await.ok();
        if let Some(game) = &vanilla
            && obj.game_jar.file.as_os_str().is_empty()
        {
            obj.game_jar = game.build_game_item();
        }

        let mut log4j: Option<&LoggingObj> = None;
        // 按 order 升序叠加：patch 是分层覆写，主类 / 运行库 / tweak / jvm 参数全靠这个顺序定胜负。
        // read_custom_json 返回的是 HashMap，不排的话顺序随哈希走 —— 出现过主类被判成
        // net.minecraft.json（order -2）里那个原版 net.minecraft.client.main.Main 的情况
        let mut custom: Vec<_> = self.read_custom_json().into_iter().collect();
        custom.sort_by(|a, b| a.1.order.cmp(&b.1.order).then_with(|| a.0.cmp(&b.0)));

        // MMC 的 conflicts 表达"这份组件顶掉了谁"，被顶掉的那份整层都不该再叠加：
        // 典型的 org.lwjgl3 顶掉 org.lwjgl —— 两边的 org.lwjgl.openal 同时进 classpath 会
        // 触发 "Sealing violation in already loaded package org.lwjgl.openal"
        drop_conflicting_patches(&mut custom);

        // 补丁里有 net.minecraft 组件时，它就是原版那一层的完整替换，Mojang 清单的运行库
        // 不能再补一遍；否则被顶掉的 LWJGL2 之类会顺着"补空缺"溜回 classpath
        let override_minecraft = has_minecraft_component(&custom);

        for (_, item) in custom.iter() {
            // 安全log4j
            if item.base.logging.is_some() {
                log4j = item.base.logging.as_ref();
            }

            // 下载项
            if !item.base.downloads.client.url.is_empty() {
                if let Some(mc_version) = &item.minecraft_version {
                    let file = libraries_path::get_game_file(mc_version);
                    let source = mml_config::read_config().http.source;
                    obj.game_jar = FileItemObj {
                        name: format!("{mc_version}.jar"),
                        file,
                        url: if source == mml_config::config_obj::SourceLocal::Offical {
                            item.base.downloads.client.url.clone()
                        } else {
                            mml_net::url_helper::get_minecraft_client(
                                &item.base.downloads.client.url,
                                mc_version,
                            )
                        },
                        hash: FileHash::Sha1(item.base.downloads.client.sha1.clone()),
                        later: Default::default(),
                    };
                } else {
                    let file = libraries_path::get_game_file_with_custom(&item.base.id);
                    let mut url = item.base.downloads.client.url.clone();
                    mml_net::url_helper::change_source(&mut url);
                    obj.game_jar = FileItemObj {
                        name: format!("{}.jar", item.base.id),
                        file,
                        url,
                        hash: FileHash::Sha1(item.base.downloads.client.sha1.clone()),
                        later: Default::default(),
                    };
                }
            }

            // 资源文件
            if let Some(asset_index) = &item.base.asset_index {
                let assets = assets_path::get_index(asset_index);
                if assets.is_err() {
                    let data = mml_net::mojang_api::get_assets(&asset_index.url).await?;
                    let assets_obj: AssetsObj = serialize_tools::json_from_bytes(&data)?;
                    if assets_obj.objects.is_empty() {
                        return Err(ErrorType::DataNotFound(DataNotFoundData::Version(
                            asset_index.id.clone(),
                        )));
                    }
                    assets_path::add_index(&item.base, &mut Cursor::new(data));
                }
                obj.assets = asset_index.clone();
            }

            // 运行库
            if item.base.libraries.is_some() {
                let native_dir = obj.native_dir.clone();
                // 同上：MMC 的本地库就在实例目录的 libraries 下，Labymod 的 LabyMod-4.jar 是这种
                let libs = item
                    .base
                    .build_game_libraries(&native_dir, crate::get_instance(&self.uuid))
                    .await;
                for lib_item in &libs {
                    do_lib_item(lib_item, &mut obj.game_libs)?;
                }
            }

            // Java版本
            if let Some(java_ver) = &item.base.java_version {
                obj.java_versions.insert(java_ver.major_version);
            }

            // 主类
            if !item.base.main_class.is_empty() {
                obj.main_class = item.base.main_class.clone();
            }

            // 游戏参数 (V1)
            if item.base.minecraft_arguments.is_some() {
                obj.game_args.extend(make_v1_game_arg(&item.base));
            }

            // 启动参数 (V2)
            if item.base.arguments.is_some() {
                obj.game_args.extend(make_v2_game_arg(&item.base));
                obj.jvm_args.extend(make_v2_jvm_arg(&item.base));
            }

            // MMC特性：mainJar —— 这次使用的客户端 jar，替换语义（自定义 jar 就靠它顶掉原版）
            if !item.main_jar.name.is_empty() {
                let allow = mojang::check_allow(&item.main_jar.rules);
                if allow {
                    let file = if item.main_jar.downloads.artifact.path.is_empty() {
                        mml_net::maven_utils::version_name_to_path(&item.main_jar.name)
                    } else {
                        item.main_jar.downloads.artifact.path.clone()
                    };
                    obj.game_jar = FileItemObj {
                        name: item.main_jar.name.clone(),
                        file: libraries_path::get_lib_dir().join(&file),
                        url: mml_net::url_helper::replace_minecraft_libraries(
                            &item.main_jar.downloads.artifact.url,
                        ),
                        hash: if item.main_jar.downloads.artifact.sha1.is_empty() {
                            Default::default()
                        } else {
                            FileHash::Sha1(item.main_jar.downloads.artifact.sha1.clone())
                        },
                        later: Default::default(),
                    };
                }
            }

            // MMC特性：+mainJar —— 加号是追加语义，把它当一个库加进 classpath，不动客户端 jar
            if let Some(add) = &item.add_main_jar
                && mojang::check_allow(&add.rules)
            {
                let file = if add.downloads.artifact.path.is_empty() {
                    mml_net::maven_utils::version_name_to_path(&add.name)
                } else {
                    add.downloads.artifact.path.clone()
                };
                do_lib_item(
                    &FileItemObj {
                        name: add.name.clone(),
                        file: libraries_path::get_lib_dir().join(&file),
                        url: mml_net::url_helper::replace_minecraft_libraries(
                            &add.downloads.artifact.url,
                        ),
                        hash: if add.downloads.artifact.sha1.is_empty() {
                            Default::default()
                        } else {
                            FileHash::Sha1(add.downloads.artifact.sha1.clone())
                        },
                        later: Default::default(),
                    },
                    &mut obj.game_libs,
                )?;
            }

            // MMC特性：CompatibleJavaMajors
            if let Some(java_majors) = &item.compatible_java_majors {
                for ver in java_majors {
                    obj.java_versions.insert(*ver);
                }
            }

            // MMC特性：AddJvmArgs
            if let Some(add_jvm) = &item.add_jvm_args {
                if obj.jvm_args.is_empty() {
                    obj.jvm_args
                        .extend(V1_JVM_ARG.iter().map(|s| s.to_string()));
                }
                obj.jvm_args.extend(add_jvm.iter().cloned());
            }

            // MMC特性：AddTweakers
            if let Some(tweakers) = &item.add_tweakers {
                obj.game_args.push("--tweakClass".to_string());
                obj.game_args.extend(tweakers.iter().cloned());
            }
        }

        // 加载器层放在 patch 之后、并且只补 patch 完全没有的构件：
        // MMC 实例的加载器本来就由 patches 描述（GTNH 的 Forge 就来自 net.minecraftforge.json），
        // 先塞一遍加载器库会得到重复的 forge，还会把 patch 声明为 "Early Classpath" 的
        // lwjgl3ify-forgePatches.jar 挤到 Forge 后面 —— RFB 的早期类覆盖就失效了
        let loader_libs = match self.loader {
            LoaderType::Forge | LoaderType::NeoForge | LoaderType::Cleanroom => {
                self.get_forge_libs().await.ok().map(|res| res.loaders)
            }
            LoaderType::Fabric => self.get_fabric_libs().await.ok(),
            LoaderType::Quilt => self.get_quilt_libs().await.ok(),
            LoaderType::OptiFine => self.get_optifine_libs().await.ok(),
            _ => None,
        };
        if let Some(libs) = &loader_libs {
            for item in libs {
                let key = libraries_path::LibVersionObj::new(&item.name);
                let exists = obj.game_libs.iter().any(|it| {
                    libraries_path::LibVersionObj::new(&it.name).eq_without_version(&key)
                });
                if !exists {
                    do_lib_item(item, &mut obj.game_libs)?;
                }
            }
        }

        // 原版运行库放最后：patch 与加载器都没给的构件才用原版的。
        // 这里只补空缺、不靠 get_libs 的"后插入者胜"去覆盖 —— 实测那样会漏，
        // 同一个构件多层都带时留下的可能是原版（gson 2.2.4 盖掉了 patch 的 2.10.1）
        if let Some(game) = &vanilla
            && !override_minecraft
        {
            let native_dir = obj.native_dir.clone();
            let libs = game
                .build_game_libraries(&native_dir, crate::get_instance(&self.uuid))
                .await;
            for item in &libs {
                let key = libraries_path::LibVersionObj::new(&item.name);
                let exists = obj.game_libs.iter().any(|it| {
                    libraries_path::LibVersionObj::new(&it.name).eq_without_version(&key)
                });
                if !exists {
                    do_lib_item(item, &mut obj.game_libs)?;
                }
            }
        }

        // 如果没有任何JVM参数，使用V1默认
        if obj.jvm_args.is_empty() {
            obj.jvm_args
                .extend(V1_JVM_ARG.iter().map(|s| s.to_string()));
        }

        // 创建启动器自定义JVM参数
        let java_version = obj.java_versions.iter().next().copied().unwrap_or(8);
        let jvm_args = self
            .make_jvm_arg(&arg.auth, java_version as u8, arg.mixin.as_ref())
            .await?;

        obj.jvm_args.extend(jvm_args);

        // MMC patch 可能没有 assetIndex（net.minecraft 组件没有展开成 patch）。
        // 不补的话 --assetIndex 为空，游戏找不到资源索引，表现为黑屏 + 所有声音缺失
        if obj.assets.id.is_empty()
            && let Some(game) = &vanilla
            && let Some(asset_index) = &game.asset_index
        {
            if assets_path::get_index(asset_index).is_err() {
                let data = mojang_api::get_assets(&asset_index.url).await?;
                let assets_obj: AssetsObj = serialize_tools::json_from_bytes(&data)?;
                if assets_obj.objects.is_empty() {
                    return Err(ErrorType::DataNotFound(DataNotFoundData::Version(
                        asset_index.id.clone(),
                    )));
                }
                assets_path::add_index(game, &mut Cursor::new(data));
            }
            obj.assets = asset_index.clone();
        }

        // MMC 的 Cleanroom patch 只提供 tweaker，基础参数（username / accessToken /
        // gameDir / assets 等）由 net.minecraft 组件提供；导入的 MMC 实例没有把组件
        // 展开成 patch 时，这里用原版版本补一份，否则 joptsimple 会报缺少 accessToken
        let has_auth = obj
            .game_args
            .iter()
            .any(|arg| arg == "--username" || arg == "--accessToken");
        if !has_auth && let Some(game) = &vanilla {
            if game.is_game_version_v2() {
                obj.game_args.extend(make_v2_game_arg(game));
            } else {
                obj.game_args.extend(make_v1_game_arg(game));
            }
        }

        obj.game_args.extend(game_args);

        // log4j安全处理
        if let Some(logging) = log4j {
            let log4j_item = mojang::build_log4j_item(logging);
            let arg_str = logging
                .client
                .argument
                .replace("${path}", &log4j_item.file.to_string_lossy());
            obj.log4j_xml = Some(log4j_item);
            obj.jvm_args.push(arg_str);
        }

        Ok(())
    }
}
