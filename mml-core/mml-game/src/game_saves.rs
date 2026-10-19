//! 游戏实例存档相关
//! 包括存档里面的数据包
use std::{
    collections::HashMap,
    io::{Read, Seek},
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::{Datelike, Local, Timelike};
use mml_base::{
    archives::{ArchiveType, BaseArchive, BaseArchiveGui},
    serialize_tools::{self, MiniJsonObj},
};
use mml_config::config_save;
use mml_names::{
    i18_items::error_type::{
        CoreResult, DataNotFoundData, ErrorType, FileSystemErrorData,
    },
    names, uuids,
};
use mml_nbt::{
    nbt_file::NbtFile,
    nbt_types::{NbtByte, NbtCompound, NbtInt, NbtList, NbtLong, NbtString},
};
use mml_net::chunkbase_api;
use mml_sys::path_helper;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::launcher::instance_setting_obj::InstanceSettingObj;
use crate::error::{archive_read_err, serializer_err};

/// 存档备份信息
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SaveBackupObj {
    /// 存档目录名，相对实例的 saves 目录
    pub dir: String,
    /// 该存档的备份文件名列表，按加入顺序排列
    pub back: Vec<String>,
}

/// 游戏存档，由 level.dat 解析而来
#[derive(Default)]
pub struct SaveObj {
    /// 地图种子
    pub random_seed: i64,
    /// 上次游玩时间，单位毫秒的 Unix 时间戳
    pub last_played: i64,
    /// 游戏模式：0 生存 / 1 创造 / 2 冒险 / 3 旁观
    pub game_type: i32,
    /// 是否极限模式，非 0 即开启
    pub hard_core: u8,
    /// 世界名字，界面上的存档名
    pub level_name: String,
    /// 难度：0 和平 / 1 简单 / 2 普通 / 3 困难
    pub difficulty: u8,
    /// 生成器名字，如 `minecraft:large_biomes`
    pub generator_name: String,
    /// 存档目录的完整路径
    pub path: PathBuf,
    /// 存档图标文件路径，没有 icon.png 时为 `None`
    pub icon: Option<PathBuf>,
    /// level.dat 缺少必要字段时为 `true`，此时其余字段全是缺省值
    pub broken: bool,
    /// 解析出的原始 NBT，写回时以它为准
    pub nbt: NbtFile,
}

/// 存档内的数据包信息
pub struct SaveDataPackObj {
    /// 数据包名，形如 `file/<包名>`
    pub name: String,
    /// 数据包文件或目录的完整路径
    pub path: PathBuf,
    /// 来自 pack.mcmeta 的描述文本
    pub description: String,
    /// 数据包格式版本号，元数据缺失时为 -1
    pub pack_format: i64,
    /// 是否启用；`None` 表示 level.dat 的启用与禁用列表里都没有它
    pub enable: Option<bool>,
}

/// 存档可编辑的那几项，由资源管理窗口的「编辑存档」提交
pub struct SaveEditObj {
    /// 游戏模式：0 生存 / 1 创造 / 2 冒险 / 3 旁观
    pub game_type: i32,
    /// 难度：0 和平 / 1 简单 / 2 普通 / 3 困难
    pub difficulty: u8,
    /// 是否极限模式
    pub hard_core: bool,
    /// 世界名字，界面上的存档名
    pub level_name: String,
    /// 地图种子
    pub seed: i64,
}

/// 读取存档文件
///
/// - `stream`: 存档文件流
///
/// # 返回值
///
/// 返回存档信息；格式不符时标记 `broken` 并返回原始 NBT
fn read_save<R: Read + Seek>(stream: &mut R) -> CoreResult<SaveObj> {
    let nbt_file = NbtFile::read(stream)?;
    let mut obj = SaveObj::default();

    let Some(nbt) = nbt_file.nbt.as_compound() else {
        obj.broken = true;
        obj.nbt = nbt_file;
        return Ok(obj);
    };

    let Some(data) = nbt.get_compound("Data") else {
        obj.broken = true;
        obj.nbt = nbt_file;
        return Ok(obj);
    };

    // 基础字段提取
    obj.last_played = data.get_long("LastPlayed").unwrap_or(0);
    obj.random_seed = data.get_long("RandomSeed").unwrap_or(0);
    obj.game_type = data.get_int("GameType").unwrap_or(0);
    obj.hard_core = data.get_byte("hardcore").unwrap_or(0);
    obj.difficulty = data.get_byte("Difficulty").unwrap_or(0);
    obj.level_name = data
        .get_string("LevelName")
        .map(String::from)
        .unwrap_or_default();

    // WorldGenSettings 是新版格式，含种子和生成器名
    if let Some(world_gen) = data.get_compound("WorldGenSettings") {
        if let Some(seed) = world_gen.get_long("seed") {
            obj.random_seed = seed;
        }
        let gen_name = world_gen
            .get_compound("dimensions")
            .and_then(|d: &NbtCompound| d.get_compound("minecraft:overworld"))
            .and_then(|o: &NbtCompound| o.get_compound("generator"))
            .and_then(|g: &NbtCompound| g.get_string("settings"));
        if let Some(name) = gen_name {
            obj.generator_name = name.to_string();
        }
    }

    // generatorName 是旧版格式，优先级高于 WorldGenSettings
    if let Some(name) = data.get_string("generatorName") {
        obj.generator_name = format!("minecraft:{}", name);
    }

    obj.nbt = nbt_file;
    Ok(obj)
}

impl InstanceSettingObj {
    /// 获取实例存档列表
    ///
    /// # 返回值
    ///
    /// 返回存档列表；读取失败的存档被跳过
    pub async fn get_saves(&self) -> Vec<SaveObj> {
        let dir = self.get_saves_path();
        let dirs = path_helper::get_dirs(dir);

        tokio::task::spawn_blocking(move || {
            let list = Mutex::new(Vec::new());

            dirs.par_iter().for_each(|item| {
                let file = item.join(names::LEVEL_FILE);
                if file.exists() && file.is_file() {
                    if let Ok(mut stream) = path_helper::open_read(&file) {
                        let data = read_save(&mut stream);
                        match data {
                            Ok(mut obj) => {
                                let file = item.join(names::ICON_FILE);
                                if file.exists() {
                                    obj.icon = Some(file);
                                }

                                obj.path = item.clone();
                                list.lock().unwrap().push(obj);
                            }
                            Err(err) => {
                                mml_log::error_type(err);
                            }
                        }
                    }
                }
            });

            list.into_inner().unwrap()
        })
        .await
        .unwrap_or_default()
    }

    /// 还原备份，靠 `info.dir` 定位是哪个存档
    ///
    /// 备份 zip 在备份目录里，还原目标却是存档目录 `saves/<info.dir>`；
    /// 这里必须显式拼出目标路径，否则只会在备份目录里再展开一份，真正的存档毫无变化。
    ///
    /// - `info`: 备份信息，`dir` 是存档目录名
    /// - `file`: 备份文件名，来自 [`get_backups`](Self::get_backups) 的 `back`
    /// - `gui`: 压缩进度回调
    ///
    /// # 返回值
    ///
    /// 现有存档移入回收站或解压失败时返回对应错误
    pub fn restore_backup(
        &self,
        info: &SaveBackupObj,
        file: &str,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        let backup_file = self.get_backup_path().join(file);
        let target = self.get_saves_path().join(&info.dir);

        // 先把现有存档整目录移进回收站而不是直接删：还原是破坏性操作，用户选错了还能捞回来
        if target.exists() && target.is_dir() {
            path_helper::move_to_trash(&target)?;
        }

        BaseArchive::decompress(ArchiveType::Zip, &backup_file, &target, gui)
    }

    /// 获取备份文件列表
    ///
    /// # 返回值
    ///
    /// 返回存档名到备份信息的映射；文件不存在返回空表，读取失败返回对应错误
    pub fn get_backups(&self) -> CoreResult<HashMap<String, SaveBackupObj>> {
        let file = self.get_backup_file();
        if file.exists() && file.is_file() {
            let stream = path_helper::open_read(&file)?;
            serialize_tools::json_from_stream(&stream)
        } else {
            Ok(HashMap::new())
        }
    }

    /// 保存备份信息
    ///
    /// - `info`: 存档名到备份信息的映射
    pub fn save_backups(&self, info: &HashMap<String, SaveBackupObj>) {
        let file = self.get_backup_file();
        config_save::save(uuids::mix_uuid(uuids::BACKUP_UUID, self.uuid), info, &file);
    }

    /// 导入存档
    ///
    /// - `file`: 存档压缩包路径
    ///
    /// # 返回值
    ///
    /// 打开或解压失败时返回对应错误
    pub fn import_save<P: AsRef<Path>>(&self, file: P) -> CoreResult<()> {
        let saves_dir = self.get_saves_path();
        if !saves_dir.exists() {
            path_helper::create_dir_all(&saves_dir)?;
        }

        let archive = BaseArchive::open(file.as_ref())?;

        // 如果包内所有条目都套在同一个顶层文件夹里，直接解压到 saves 目录；
        // 否则以压缩包文件名新建一个文件夹再解压进去。
        let output_dir = if archive.single_top_dir().is_some() {
            // 有唯一顶层文件夹：直接解压，顶层文件夹就是存档目录
            saves_dir
        } else {
            // 没有套壳：用压缩包文件名去掉后缀建一个新目录
            let file_path = file.as_ref();
            let stem = file_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "save".to_string());
            // 二次去后缀，处理 .tar.gz 这类双后缀
            let stem = stem.trim_end_matches(".tar").to_string();
            saves_dir.join(&stem)
        };

        path_helper::create_dir_all(&output_dir)?;
        archive.extract_all(&output_dir, None, None, None)
    }
}

impl SaveObj {
    /// 获取数据包文件夹
    ///
    /// # 返回值
    ///
    /// 返回数据包目录路径
    pub fn get_datapack_path(&self) -> PathBuf {
        self.path.join(names::GAME_DATAPACK_DIR)
    }

    /// 获取存档信息文件
    ///
    /// # 返回值
    ///
    /// 返回 level.dat 文件路径
    pub fn get_level_file(&self) -> PathBuf {
        self.path.join(names::LEVEL_FILE)
    }

    /// 删除存档
    ///
    /// # 返回值
    ///
    /// 移入回收站失败时返回对应错误
    pub fn delete(&self) -> CoreResult<()> {
        path_helper::move_to_trash(&self.path)
    }

    /// 导出存档
    ///
    /// - `path`: 导出保存位置
    /// - `archive_type`: 压缩类型
    /// - `gui`: 压缩进度回调
    ///
    /// # 返回值
    ///
    /// 压缩失败时返回对应错误
    pub fn export<P: AsRef<Path>>(
        &self,
        path: P,
        archive_type: ArchiveType,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        BaseArchive::compress(
            archive_type,
            path.as_ref(),
            self.path.as_ref(),
            None::<&Path>,
            &None,
            gui,
        )?;
        Ok(())
    }

    /// 备份存档
    ///
    /// - `instance`: 所属实例
    /// - `gui`: 压缩进度回调
    ///
    /// # 返回值
    ///
    /// 返回备份文件名，形如 `level_name_年_月_日_时_分_秒.zip`；
    /// 压缩或保存备份信息失败时返回对应错误
    pub fn backup(&self, instance: &InstanceSettingObj, gui: BaseArchiveGui) -> CoreResult<String> {
        let path = instance.get_backup_path();
        path_helper::create_dir_all(&path)?;

        let time = Local::now();
        let name = format!(
            "{}_{}_{}_{}_{}_{}_{}.zip",
            self.level_name,
            time.year(),
            time.month(),
            time.day(),
            time.hour(),
            time.minute(),
            time.second()
        );
        let file = path.join(&name);

        BaseArchive::compress(ArchiveType::Zip, &file, &self.path, None, &None, gui)?;

        let mut info = instance.get_backups()?;
        if let Some(obj) = info.get_mut(&self.level_name) {
            obj.back.push(name.clone());
        } else {
            let path = instance.get_saves_path();
            let dir = self
                .path
                .strip_prefix(path)
                .map_err(|err| {
                    ErrorType::FileSystemError(FileSystemErrorData {
                        path: self.path.clone(),
                        error: err.to_string(),
                    })
                })?
                .to_string_lossy()
                .to_string();
            let back: Vec<String> = vec![name.clone()];
            info.insert(self.level_name.clone(), SaveBackupObj { dir, back });
        }

        instance.save_backups(&info);

        Ok(name)
    }

    /// 保存 NBT 数据到文件
    ///
    /// # 返回值
    ///
    /// 写入失败时返回对应错误
    fn save_nbt(&self) -> CoreResult<()> {
        let file = self.get_level_file();
        let mut stream = path_helper::open_write(file)?;

        self.nbt.write(&mut stream)?;

        Ok(())
    }

    /// 改存档的基本信息，写回 level.dat
    ///
    /// 只动 `Data` 下的 `GameType`、`Difficulty`、`hardcore`、`LevelName` 与种子，其余字段
    /// 原样写回；缺的字段按 level.dat 的标准类型补上，否则游戏读不到、等于白改。
    ///
    /// 种子写两份：`RandomSeed` 给旧版本，`WorldGenSettings.seed` 给 1.16 起的新版本 ——
    /// 新版本以后者为准，只写前者不生效；没有这个复合标签的老存档就只写前者。
    ///
    /// 调用前要确认实例没在运行 —— 游戏退出时会用它内存里的值覆盖这个文件。
    ///
    /// - `info`: 新的信息，见 [`SaveEditObj`]
    ///
    /// # 返回值
    ///
    /// NBT 里没有 `Data` 复合标签或写盘失败时返回对应错误
    pub fn set_info(&mut self, info: &SaveEditObj) -> CoreResult<()> {
        let Some(data) = self
            .nbt
            .nbt
            .as_compound_mut()
            .and_then(|nbt| nbt.get_compound_mut("Data"))
        else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        data.data.insert(
            String::from("GameType"),
            NbtInt::new(info.game_type).to_nbt(),
        );
        data.data.insert(
            String::from("Difficulty"),
            NbtByte::new(info.difficulty).to_nbt(),
        );
        // 键名是小写的 `hardcore`，与读取处一致
        data.data.insert(
            String::from("hardcore"),
            NbtByte::new(if info.hard_core { 1 } else { 0 }).to_nbt(),
        );
        data.data.insert(
            String::from("LevelName"),
            NbtString::new(info.level_name.clone()).to_nbt(),
        );
        data.data
            .insert(String::from("RandomSeed"), NbtLong::new(info.seed).to_nbt());
        if let Some(world_gen) = data.get_compound_mut("WorldGenSettings") {
            world_gen
                .data
                .insert(String::from("seed"), NbtLong::new(info.seed).to_nbt());
        }

        self.game_type = info.game_type;
        self.difficulty = info.difficulty;
        self.hard_core = if info.hard_core { 1 } else { 0 };
        self.level_name = info.level_name.clone();
        self.random_seed = info.seed;

        self.save_nbt()
    }

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

/// 读取数据包信息
///
/// - `path`: 数据包路径
/// - `ens`: 已启用的数据包列表，`None` 表示 level.dat 里没有该字段
/// - `dis`: 已禁用的数据包列表，`None` 表示 level.dat 里没有该字段
/// - `data`: 数据包元数据，即 pack.mcmeta
///
/// # 返回值
///
/// 元数据格式不符时返回 `None`
fn read_pack(
    path: PathBuf,
    ens: Option<&NbtList>,
    dis: Option<&NbtList>,
    data: MiniJsonObj,
) -> Option<SaveDataPackObj> {
    if let Some(pack) = data.as_object().and_then(|item| item.get_object("pack")) {
        let mut obj = SaveDataPackObj {
            name: format!(
                "file/{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            path,
            description: pack.get_string("description"),
            pack_format: pack.get_opt_i64("pack_format").unwrap_or(-1),
            enable: None,
        };

        if let Some(ens) = ens {
            for item in ens.iter() {
                if let Some(name) = item.as_string()
                    && name.data.eq_ignore_ascii_case(&obj.name)
                {
                    obj.enable = Some(true);
                    break;
                }
            }
        }

        if let Some(dis) = dis {
            for item in dis.iter() {
                if let Some(name) = item.as_string()
                    && name.data.eq_ignore_ascii_case(&obj.name)
                {
                    obj.enable = Some(false);
                    break;
                }
            }
        }

        Some(obj)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use mml_names::names;
    use mml_nbt::{
        nbt_file::{CompressType, NbtFile},
        nbt_types::{NbtByte, NbtCompound, NbtInt, NbtLong, NbtString},
    };
    use mml_sys::path_helper;
    use uuid::Uuid;

    use super::{SaveEditObj, SaveObj};

    /// 改模式 / 难度 / 极限 / 名字 / 种子都要落到 level.dat 上：写回的文件重新读出来是改后的值
    #[test]
    fn set_info_writes_back_to_level_dat() {
        let dir = mml_testutil::temp_dir().join(format!("mml-save-setinfo-{}", Uuid::new_v4()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(names::LEVEL_FILE);

        // 造一个最小 level.dat：待改的五个字段先给旧值，另一个字段用来验证"其余原样写回"
        let mut world_gen = NbtCompound::new();
        world_gen
            .data
            .insert(String::from("seed"), NbtLong::new(1).to_nbt());
        let mut data = NbtCompound::new();
        data.data
            .insert(String::from("GameType"), NbtInt::new(0).to_nbt());
        data.data
            .insert(String::from("Difficulty"), NbtByte::new(0).to_nbt());
        data.data
            .insert(String::from("hardcore"), NbtByte::new(0).to_nbt());
        data.data.insert(
            String::from("LevelName"),
            NbtString::new(String::from("旧名字")).to_nbt(),
        );
        data.data
            .insert(String::from("RandomSeed"), NbtLong::new(1).to_nbt());
        data.data
            .insert(String::from("WorldGenSettings"), world_gen.to_nbt());
        data.data
            .insert(String::from("keep"), NbtInt::new(7).to_nbt());
        let mut root = NbtCompound::new();
        root.data.insert(String::from("Data"), data.to_nbt());
        let mut stream = path_helper::open_write(&file).unwrap();
        NbtFile::new(root.to_nbt(), CompressType::GZip)
            .write(&mut stream)
            .unwrap();
        drop(stream);

        // 种子取一个超过 2^53 的值：它在 level.dat 里是 i64，前端跨 IPC 走字符串就是为了它
        let seed = -4172144997902289642_i64;
        // 按真实链路走：SaveObj 持原始 NBT，改完以它为准写回
        let mut save = SaveObj {
            path: dir.clone(),
            nbt: NbtFile::read(&mut path_helper::open_read(&file).unwrap()).unwrap(),
            ..Default::default()
        };
        save.set_info(&SaveEditObj {
            game_type: 1,
            difficulty: 3,
            hard_core: true,
            level_name: String::from("新名字"),
            seed,
        })
        .unwrap();
        assert_eq!(save.game_type, 1);
        assert_eq!(save.difficulty, 3);
        assert_eq!(save.hard_core, 1);
        assert_eq!(save.level_name, "新名字");
        assert_eq!(save.random_seed, seed);

        // 重新读盘：字段真的写进去了
        let back = NbtFile::read(&mut path_helper::open_read(&file).unwrap()).unwrap();
        let data = back
            .nbt
            .as_compound()
            .and_then(|nbt| nbt.get_compound("Data"))
            .unwrap();
        assert_eq!(data.get_int("GameType"), Some(1));
        assert_eq!(data.get_byte("Difficulty"), Some(3));
        assert_eq!(data.get_byte("hardcore"), Some(1));
        assert_eq!(data.get_string("LevelName").as_deref(), Some("新名字"));
        assert_eq!(data.get_long("RandomSeed"), Some(seed));
        // 新版格式的种子也要跟着改，否则 1.16 起读的还是旧值
        assert_eq!(
            data.get_compound("WorldGenSettings")
                .and_then(|world_gen| world_gen.get_long("seed")),
            Some(seed)
        );
        assert_eq!(data.get_int("keep"), Some(7));

        let _ = fs::remove_dir_all(&dir);
    }
}
