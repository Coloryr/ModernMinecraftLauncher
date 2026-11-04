use super::*;
use super::args::*;
use super::libs::*;

impl InstanceSettingObj {
    /// 创建常规启动内容
    ///
    /// - `arg`: 启动配置
    /// - `obj`: 待填充的启动内容
    /// - `game_args`: 启动器自定义游戏参数
    ///
    /// # 返回值
    ///
    /// 返回是否成功，游戏版本不存在或运行库路径不存在返回对应错误。
    pub(super) async fn make_normal_arg(
        &self,
        arg: &GameLaunchArg,
        obj: &mut GameLaunchObj,
        game_args: Vec<String>,
    ) -> CoreResult<()> {
        let game = self.check_version_update().await?;
        let v2 = game.is_game_version_v2();

        // 老版本清单没有 javaVersion 字段（如 1.6.4），什么都不插，
        // 由组件选择处统一兜底到 Java 8
        if let Some(java_ver) = &game.java_version {
            obj.java_versions.insert(java_ver.major_version);
        }

        // 处理运行库
        {
            let native_dir = obj.native_dir.clone();

            // 构建游戏核心jar
            obj.game_jar = game.build_game_item();

            // 处理原版运行库
            let skip_game_libs = self.loader == LoaderType::Custom
                && self.custom_loader.as_ref().is_some_and(|c| c.remove_lib);

            if !skip_game_libs {
                // 传实例：本地库（MMC 的 MMC-hint: local 与实例目录回退）要靠它找到
                let libs = game
                    .build_game_libraries(&native_dir, crate::get_instance(&self.uuid))
                    .await;
                for item in &libs {
                    // Cleanroom 的 MMC 补丁用空 jar 顶掉原版 LWJGL 2，这里直接跳过；
                    // 其余原版库要保留：Cleanroom 清单并不覆盖全部，缺了 ICU / patchy
                    // 之类会连渲染初始化都走不完
                    if self.loader == LoaderType::Cleanroom
                        && (item.name.starts_with("org.lwjgl.lwjgl:")
                            || !matches!(item.later, LaterRun::None))
                    {
                        continue;
                    }
                    do_lib_item(item, &mut obj.game_libs)?;
                }
            }

            // 根据加载器处理
            let mut loader_libs: Option<Vec<FileItemObj>> = None;
            let mut install_libs: Option<Vec<FileItemObj>> = None;

            match self.loader {
                LoaderType::Forge | LoaderType::NeoForge | LoaderType::Cleanroom => {
                    let res = self.get_forge_libs().await?;
                    loader_libs = Some(res.loaders);
                    install_libs = Some(res.installs);

                    // Cleanroom 的清单声明要 Java 25：覆盖原版 1.12.2 的 Java 8，
                    // 否则自动选择会按最低版本挑到 8
                    if self.loader == LoaderType::Cleanroom
                        && let Some(java) = self.get_forge().and_then(|loader| {
                            loader.java_version.as_ref().map(|j| j.major_version)
                        })
                    {
                        obj.java_versions.clear();
                        obj.java_versions.insert(java);
                    }

                    if v2 {
                        launcher_path::ready_forge_wrapper()?;
                        if let Some(ref mut loader) = loader_libs {
                            loader.push(launcher_path::get_forge_wrapper());
                        }
                    }
                }
                LoaderType::Fabric => {
                    loader_libs = Some(self.get_fabric_libs().await?);
                }
                LoaderType::Quilt => {
                    loader_libs = Some(self.get_quilt_libs().await?);
                }
                LoaderType::OptiFine => {
                    loader_libs = Some(self.get_optifine_libs().await?);
                    launcher_path::ready_optifine_wrapper()?;
                    if let Some(ref mut loader) = loader_libs {
                        loader.push(launcher_path::get_optifine_wrapper());
                    }
                }
                LoaderType::Custom => {
                    let loader_file = self.get_loader_file();
                    if !loader_file.exists() {
                        return Err(ErrorType::PathNotExists(PathNotExistsData {
                            path: loader_file.clone(),
                        }));
                    }
                    let res = self.decode_loader_jar_with_path(&loader_file).await?;
                    loader_libs = Some(res.libs);
                }
                LoaderType::Normal => {}
                LoaderType::LiteLoader => {
                    loader_libs = Some(self.get_liteloader_libs().await?);
                }
            }

            if let Some(loader) = loader_libs {
                for item in &loader {
                    if !item.file.as_os_str().is_empty() {
                        obj.loader_libs.push(item.clone());
                    }
                }
            }
            if let Some(install) = install_libs {
                for item in &install {
                    if !item.file.as_os_str().is_empty() {
                        obj.installer_libs.push(item.clone());
                    }
                }
            }
        }

        // JVM参数
        let remove_jvm = self
            .jvm_arg
            .as_ref()
            .is_some_and(|j| j.remove_jvm_arg.unwrap_or(false));

        if !remove_jvm {
            if v2 {
                obj.jvm_args.extend(make_v2_jvm_arg(&game));
            } else {
                obj.jvm_args
                    .extend(V1_JVM_ARG.iter().map(|s| s.to_string()));
            }
            obj.jvm_args.extend(make_loader_jvm_arg(v2, &self).await);
        }

        // 创建启动器自定义JVM参数
        let java_version = obj.java_versions.iter().next().copied().unwrap_or(8);
        let jvm_args = self
            .make_jvm_arg(
                &arg.auth,
                java_version as u8,
                arg.mixin.as_ref(),
                &obj.jvm_args,
            )
            .await?;

        obj.jvm_args.extend(jvm_args);

        // 游戏参数
        let remove_game = self
            .jvm_arg
            .as_ref()
            .is_some_and(|j| j.remove_game_arg.unwrap_or(false));

        if !remove_game {
            if v2 {
                obj.game_args.extend(make_v2_game_arg(&game));
                obj.game_args.extend(make_loader_v2_game_arg(&self));
            } else if self.loader != LoaderType::Normal {
                obj.game_args.extend(make_loader_v1_game_arg(&self, &game));
            } else {
                obj.game_args.extend(make_v1_game_arg(&game));
            }
        }

        obj.game_args.extend(game_args);

        // 资源文件
        if let Some(asset_index) = &game.asset_index {
            let assets = assets_path::get_index(asset_index);
            if assets.is_err() {
                let data = mojang_api::get_assets(&asset_index.url).await?;
                let assets_obj: AssetsObj = serialize_tools::json_from_bytes(&data)?;
                if assets_obj.objects.is_empty() {
                    return Err(ErrorType::DataNotFound(DataNotFoundData::Version(
                        asset_index.id.clone(),
                    )));
                }
                assets_path::add_index(&game, &mut Cursor::new(data));
            }
            obj.assets = asset_index.clone();
        }

        obj.main_class = self.make_mainclass()?;

        Ok(())
    }
}
