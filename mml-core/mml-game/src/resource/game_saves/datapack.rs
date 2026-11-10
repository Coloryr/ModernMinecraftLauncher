//! 存档数据包的读取、启用状态变更与删除

use super::*;

impl SaveObj {
    /// 获取数据包列表
    ///
    /// # 返回值
    ///
    /// 返回带启用状态的数据包列表；目录不存在返回空列表，读取失败返回对应错误
    pub fn get_datapacks(&self) -> CoreResult<Vec<SaveDataPackObj>> {
        let path = self.get_datapack_path();
        if !path.exists() || !path.is_dir() {
            return Ok(Vec::new());
        }

        let list = Mutex::new(Vec::new());

        if let Some(nbt) = self
            .nbt
            .nbt
            .as_compound()
            .and_then(|map| map.get_compound("Data"))
            .and_then(|map1| map1.get_compound("DataPacks"))
        {
            let ens = nbt.get_list("Enabled");
            let dis = nbt.get_list("Disabled");

            let files = path_helper::get_files(&path);

            files.par_iter().for_each(|item| {
                if let Some(ext) = item.extension()
                    && ext.eq_ignore_ascii_case(names::ZIP_EXT)
                {
                    let stream = path_helper::open_read(item);
                    if let Err(err) = stream {
                        mml_log::error_type(err);
                        return;
                    }

                    let stream = stream.unwrap();
                    let zip = ZipArchive::new(stream);
                    if let Err(err) = zip {
                        mml_log::error_type(ErrorType::ArchiveOpenError(FileSystemErrorData {
                            path: path.clone(),
                            error: err.to_string(),
                        }));
                        return;
                    }

                    let mut zip = zip.unwrap();
                    let meta = zip.by_name(names::PACK_META_FILE);
                    if let Err(err) = meta {
                        mml_log::error_type(archive_read_err(err));
                        return;
                    }
                    let json = MiniJsonObj::from_stream(meta.unwrap());
                    if let Err(err) = json {
                        mml_log::error_type(serializer_err(err));
                        return;
                    }
                    let json = json.unwrap();
                    // 必须传这个包自己的路径 item，不能传父目录 path：
                    // read_pack 用 file_name 拼成 `file/<包名>`，这个名字既是列表显示名，
                    // 也是与 Enabled / Disabled 比对启用状态的键；传父目录会让所有包同名，
                    // 列表里全叫 datapacks，改一个的状态等于改全部。
                    if let Some(data) = read_pack(item.clone(), ens, dis, json) {
                        list.lock().unwrap().push(data);
                    }
                }
            });

            let files = path_helper::get_dirs(&path);

            files.par_iter().for_each(|item| {
                let meta = item.join(names::PACK_META_FILE);
                if !meta.exists() || !meta.is_file() {
                    return;
                }
                let stream = path_helper::open_read(meta);
                if let Err(err) = stream {
                    mml_log::error_type(err);
                    return;
                }
                let json = MiniJsonObj::from_stream(stream.unwrap());
                if let Err(err) = json {
                    mml_log::error_type(serializer_err(err));
                    return;
                }
                let json = json.unwrap();
                // 同上：目录形式的数据包也传它自己的路径
                if let Some(data) = read_pack(item.clone(), ens, dis, json) {
                    list.lock().unwrap().push(data);
                }
            });
        }

        Ok(list.into_inner().unwrap())
    }

    /// 修改数据包启用状态
    ///
    /// - `list`: 需要变更状态的数据包列表
    ///
    /// # 返回值
    ///
    /// NBT 结构缺失或写入失败时返回对应错误
    pub fn change_data_pack(&mut self, list: &Vec<SaveDataPackObj>) -> CoreResult<()> {
        let Some(data) = self
            .nbt
            .nbt
            .as_compound_mut()
            .and_then(|map| map.get_compound_mut("Data"))
        else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };
        let Some(data_packs) = data.get_compound_mut("DataPacks") else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        // 先只读收集增删项，避免与后面的可变借用冲突
        let mut remove_from_enabled: Vec<usize> = Vec::new();
        let mut remove_from_disabled: Vec<usize> = Vec::new();
        let mut add_to_enabled: Vec<String> = Vec::new();
        let mut add_to_disabled: Vec<String> = Vec::new();

        {
            let Some(ens) = data_packs.get_list("Enabled") else {
                return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
            };
            let Some(dis) = data_packs.get_list("Disabled") else {
                return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
            };

            for item in list.iter() {
                let nbt_enable = (0..ens.len()).find(|&index| {
                    ens.get_item(index)
                        .and_then(|data| data.as_string())
                        .map(|s| s.data.eq_ignore_ascii_case(&item.name))
                        .unwrap_or(false)
                });

                let nbt_disable = (0..dis.len()).find(|&index| {
                    dis.get_item(index)
                        .and_then(|data| data.as_string())
                        .map(|s| s.data.eq_ignore_ascii_case(&item.name))
                        .unwrap_or(false)
                });

                if nbt_enable.is_some() && nbt_disable.is_some() {
                    remove_from_disabled.push(nbt_disable.unwrap());
                } else if nbt_enable.is_some() && nbt_disable.is_none() {
                    remove_from_enabled.push(nbt_enable.unwrap());
                    add_to_disabled.push(item.name.clone());
                } else if nbt_disable.is_some() {
                    remove_from_disabled.push(nbt_disable.unwrap());
                } else {
                    add_to_enabled.push(item.name.clone());
                }
            }
        }

        // 每次只取一个列表的可变引用，否则同时借用两个列表
        {
            let Some(ens) = data_packs.get_list_mut("Enabled") else {
                return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
            };
            remove_from_enabled.sort_by(|a, b| b.cmp(a));
            for idx in remove_from_enabled {
                ens.remove(idx);
            }
            for name in add_to_enabled {
                ens.add_item(NbtString::new(name).to_nbt());
            }
        }
        {
            let Some(dis) = data_packs.get_list_mut("Disabled") else {
                return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
            };
            remove_from_disabled.sort_by(|a, b| b.cmp(a));
            for idx in remove_from_disabled {
                dis.remove(idx);
            }
            for name in add_to_disabled {
                dis.add_item(NbtString::new(name).to_nbt());
            }
        }

        self.save_nbt()?;

        Ok(())
    }

    /// 删除数据包
    ///
    /// - `list`: 待删除的数据包列表
    ///
    /// # 返回值
    ///
    /// NBT 结构缺失或写入失败时返回对应错误
    pub fn delete_datapack(&mut self, list: &Vec<SaveDataPackObj>) -> CoreResult<()> {
        let Some(data) = self
            .nbt
            .nbt
            .as_compound_mut()
            .and_then(|map| map.get_compound_mut("Data"))
        else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };
        let Some(data_packs) = data.get_compound_mut("DataPacks") else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        // 先只读收集要移除的索引，避免与后面的可变借用冲突
        let mut remove_from_enabled: Vec<usize> = Vec::new();
        let mut remove_from_disabled: Vec<usize> = Vec::new();

        {
            let Some(ens) = data_packs.get_list("Enabled") else {
                return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
            };
            let Some(dis) = data_packs.get_list("Disabled") else {
                return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
            };

            for item in list.iter() {
                if let Some(idx) = (0..ens.len()).find(|&index| {
                    ens.get_item(index)
                        .and_then(|data| data.as_string())
                        .map(|s| s.data.eq_ignore_ascii_case(&item.name))
                        .unwrap_or(false)
                }) {
                    remove_from_enabled.push(idx);
                }

                if let Some(idx) = (0..dis.len()).find(|&index| {
                    dis.get_item(index)
                        .and_then(|data| data.as_string())
                        .map(|s| s.data.eq_ignore_ascii_case(&item.name))
                        .unwrap_or(false)
                }) {
                    remove_from_disabled.push(idx);
                }
            }
        }

        // 每次只取一个列表的可变引用，按索引降序删除
        if !remove_from_enabled.is_empty() {
            let Some(ens) = data_packs.get_list_mut("Enabled") else {
                return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
            };
            remove_from_enabled.sort_by(|a, b| b.cmp(a));
            for idx in remove_from_enabled {
                ens.remove(idx);
            }
        }

        if !remove_from_disabled.is_empty() {
            let Some(dis) = data_packs.get_list_mut("Disabled") else {
                return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
            };
            remove_from_disabled.sort_by(|a, b| b.cmp(a));
            for idx in remove_from_disabled {
                dis.remove(idx);
            }
        }

        self.save_nbt()?;

        Ok(())
    }

    /// 打开种子信息网站
    ///
    /// - `version`: 游戏版本
    pub fn open_world_seed(&self, version: &str) {
        let url = chunkbase_api::gen_url(
            version,
            self.random_seed,
            self.generator_name == "minecraft:large_biomes",
        );
        mml_sys::open_helper::open_url(&url);
    }
}
