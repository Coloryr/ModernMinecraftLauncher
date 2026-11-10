//! 游戏实例结构文件相关
//! 包括模组的结构文件读取
use std::{
    collections::HashMap,
    io::{Read, Seek},
    path::PathBuf,
    sync::Mutex,
};

use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType},
    names,
};
use mml_nbt::{NbtType, nbt_file::NbtFile, nbt_types::NbtCompound};
use mml_sys::path_helper;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::launcher::instance_setting_obj::InstanceSettingObj;

/// 结构文件类型，由文件扩展名判定
#[derive(Debug)]
pub enum SchematicType {
    /// 原版
    Minecraft,
    /// 投影模组
    Litematic,
    /// 创世神模组
    WorldEdit,
    /// 机械动力蓝图
    Create,
}

impl Default for SchematicType {
    fn default() -> Self {
        SchematicType::Minecraft
    }
}

/// 结构文件
#[derive(Debug)]
pub struct SchematicObj {
    /// 结构名，取自文件元数据
    pub name: String,
    /// 结构文件路径
    pub path: PathBuf,
    /// 高度，单位方块
    pub height: i32,
    /// 长度，单位方块
    pub length: i32,
    /// 宽度，单位方块
    pub width: i32,
    /// 作者，取自文件元数据
    pub author: String,
    /// 描述，取自文件元数据
    pub description: String,
    /// 文件类型
    pub schematic_type: SchematicType,
    /// 方块总数
    pub block_count: u64,
    /// 方块种类数
    pub block_types: u32,
    /// 各方块数量，键为方块 ID，值为出现次数
    pub blocks: HashMap<String, u64>,
    /// 是否读取失败，失败时其余字段为默认值
    pub fail: bool,
}

impl Default for SchematicObj {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: PathBuf::new(),
            height: 0,
            length: 0,
            width: 0,
            author: String::new(),
            description: String::new(),
            schematic_type: SchematicType::Minecraft,
            block_count: 0,
            block_types: 0,
            blocks: HashMap::new(),
            fail: false,
        }
    }
}

impl SchematicObj {
    /// 删除
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；删除失败返回对应错误
    pub fn delete(&self) -> CoreResult<()> {
        path_helper::move_to_trash(&self.path)
    }
}

impl SchematicType {
    /// 按扩展名判断结构文件类型
    ///
    /// [`read_schematic_file`] 要求先给出类型，所以它必须公开：调用方要能自己从文件名认。
    ///
    /// - `ext`: 文件扩展名
    ///
    /// # 返回值
    ///
    /// 返回对应的结构文件类型；无法识别返回原版
    pub fn from_ext(ext: &str) -> Self {
        if ext.eq_ignore_ascii_case(names::LITEMATIC_EXT) {
            SchematicType::Litematic
        } else if ext.eq_ignore_ascii_case(names::SCHEMATIC_EXT) {
            SchematicType::Minecraft
        } else if ext.eq_ignore_ascii_case(names::SCHEM_EXT) {
            SchematicType::WorldEdit
        } else if ext.eq_ignore_ascii_case(names::NBT_EXT) {
            SchematicType::Create
        } else {
            SchematicType::Minecraft
        }
    }
}

/// 从 NBT 读取 Short 类型的尺寸信息，对应 Height / Length / Width 三个标签
fn read_dimensions_short(nbt: &NbtCompound, obj: &mut SchematicObj) {
    if let Some(h) = nbt.get_short("Height") {
        obj.height = h as i32;
    }
    if let Some(l) = nbt.get_short("Length") {
        obj.length = l as i32;
    }
    if let Some(w) = nbt.get_short("Width") {
        obj.width = w as i32;
    }
}

/// 根据 palette 大小计算打包所需的位数，全程整数运算
///
/// - `palette_size`: 方块调色板大小
///
/// # 返回值
///
/// 返回每个方块占用的位数
fn bits_per_entry(palette_size: usize) -> usize {
    if palette_size <= 1 {
        1
    } else {
        (usize::BITS - (palette_size - 1).leading_zeros()) as usize
    }
}

/// 读取投影模组 Litematic 结构文件
///
/// - `nbt`: 结构文件 NBT 数据
///
/// # 返回值
///
/// 返回解析出的结构信息；格式不符返回 `DataNotFound`
fn read_litematic(nbt: NbtFile) -> CoreResult<SchematicObj> {
    if let Some(nbt) = nbt.nbt.as_compound()
        && let Some(meta) = nbt.get_compound("Metadata")
    {
        let mut obj = SchematicObj {
            schematic_type: SchematicType::Litematic,
            name: meta.get_string("Name").unwrap_or_default(),
            author: meta.get_string("Author").unwrap_or_default(),
            description: meta.get_string("Description").unwrap_or_default(),
            ..Default::default()
        };

        if let Some(data) = meta.get_compound("EnclosingSize") {
            if let Some(y) = data.get_int("y") {
                obj.height = y;
            }
            if let Some(x) = data.get_int("x") {
                obj.length = x;
            }
            if let Some(z) = data.get_int("z") {
                obj.width = z;
            }
        }

        if let Some(total) = meta.get_int("TotalBlocks") {
            obj.block_count = total as u64;
        }

        if let Some(regions) = nbt.get_compound("Regions") {
            for (_, value) in regions.data.iter() {
                if let Some(region) = value.as_compound()
                    && let Some(palette) = region.get_list("BlockStatePalette")
                    && let Some(array) = region.get_long_array("BlockStates")
                {
                    let palette_size = palette.len();
                    if palette_size == 0 || array.data.is_empty() {
                        continue;
                    }

                    let bits = bits_per_entry(palette_size);
                    let block_states = array.data.as_slice();
                    let entries_per_long = 64 / bits;
                    let mask = if bits >= 64 {
                        u64::MAX
                    } else {
                        (1u64 << bits) - 1
                    };

                    let mut counts = vec![0u64; palette_size];
                    let total = obj.block_count as usize;
                    let mut processed = 0usize;

                    'outer: for &value in block_states {
                        let v = value as u64;
                        for i in 0..entries_per_long {
                            if processed >= total {
                                break 'outer;
                            }
                            let block_id = ((v >> (i * bits)) & mask) as usize;
                            if block_id < palette_size {
                                counts[block_id] += 1;
                            }
                            processed += 1;
                        }
                    }

                    for (index, count) in counts.into_iter().enumerate() {
                        if count == 0 {
                            continue;
                        }
                        if let Some(NbtType::Compound(data)) = palette.get_item(index)
                            && let Some(key) = data.get_string("Name")
                        {
                            *obj.blocks.entry(key).or_default() += count;
                        }
                    }
                }
            }
        }

        Ok(obj)
    } else {
        Err(ErrorType::DataNotFound(DataNotFoundData::Info))
    }
}

/// 读取原版结构文件
///
/// - `nbt`: 结构文件 NBT 数据
///
/// # 返回值
///
/// 返回解析出的结构信息；格式不符返回 `DataNotFound`
fn read_schematic(nbt: NbtFile) -> CoreResult<SchematicObj> {
    if let Some(nbt) = nbt.nbt.as_compound() {
        let mut obj = SchematicObj {
            schematic_type: SchematicType::Minecraft,
            ..Default::default()
        };
        read_dimensions_short(nbt, &mut obj);
        Ok(obj)
    } else {
        Err(ErrorType::DataNotFound(DataNotFoundData::Info))
    }
}

/// 去除方块名中的属性部分，如 "minecraft:stone[axis=y]" → "minecraft:stone"
///
/// - `input`: 方块名
///
/// # 返回值
///
/// 返回去属性后的方块名
fn base_name(input: &str) -> &str {
    input.find('[').map_or(input, |pos| &input[..pos])
}

/// 读取创世神模组 WorldEdit 结构文件
///
/// - `nbt`: 结构文件 NBT 数据
///
/// # 返回值
///
/// 返回解析出的结构信息；格式不符返回 `DataNotFound`
fn read_schem(nbt: NbtFile) -> CoreResult<SchematicObj> {
    if let Some(nbt) = nbt.nbt.as_compound() {
        let mut obj = SchematicObj {
            schematic_type: SchematicType::WorldEdit,
            ..Default::default()
        };
        read_dimensions_short(nbt, &mut obj);

        if let Some(block) = nbt.get_byte_array("BlockData")
            && let Some(palette) = nbt.get_compound("Palette")
        {
            let mut palettes = HashMap::<usize, String>::new();
            for (key, value) in palette.data.iter() {
                let index = value.as_int().map_or(0, |v| v.data);
                palettes.insert(index as usize, base_name(key).to_string());
            }

            let mut blocks = vec![0u64; palettes.len()];
            for &item in block.data.iter() {
                blocks[item as usize] += 1;
            }

            let mut total = 0u64;
            for (index, count) in blocks.into_iter().enumerate() {
                if count == 0 {
                    continue;
                }
                let key = palettes.remove(&index).unwrap_or_default();
                if key.is_empty() {
                    continue;
                }
                let entry = obj.blocks.entry(key).or_default();
                *entry += count;
                total += count;
            }

            obj.block_count = total;
            obj.block_types = obj.blocks.len() as u32;
        }

        Ok(obj)
    } else {
        Err(ErrorType::DataNotFound(DataNotFoundData::Info))
    }
}

/// 读取机械动力蓝图 Create 结构文件
///
/// - `nbt`: 结构文件 NBT 数据
///
/// # 返回值
///
/// 返回解析出的结构信息；格式不符返回 `DataNotFound`
fn read_nbt(nbt: NbtFile) -> CoreResult<SchematicObj> {
    if let Some(data) = nbt.nbt.as_compound() {
        let mut obj = SchematicObj {
            schematic_type: SchematicType::Create,
            ..Default::default()
        };

        if let Some(list) = data.get_list("size")
            && list.len() == 3
        {
            obj.width = list
                .get_item(0)
                .and_then(|v| v.as_int())
                .map_or(0, |v| v.data);
            obj.height = list
                .get_item(1)
                .and_then(|v| v.as_int())
                .map_or(0, |v| v.data);
            obj.length = list
                .get_item(2)
                .and_then(|v| v.as_int())
                .map_or(0, |v| v.data);
        }

        if let Some(palette) = data.get_list("palette")
            && let Some(block) = data.get_list("blocks")
        {
            let mut blocks = vec![0u64; palette.len()];
            for item in block.iter() {
                if let Some(compound) = item.as_compound()
                    && let Some(state) = compound.data.get("state")
                    && let Some(state) = state.as_int()
                {
                    blocks[state.data as usize] += 1;
                }
            }

            let mut total = 0u64;
            for (index, count) in blocks.into_iter().enumerate() {
                if count == 0 {
                    continue;
                }
                if let Some(item) = palette.get_item(index)
                    && let Some(compound) = item.as_compound()
                    && let Some(key) = compound.get_string("Name")
                {
                    *obj.blocks.entry(key).or_default() += count;
                    total += count;
                }
            }

            obj.block_count = total;
            obj.block_types = obj.blocks.len() as u32;
        }

        Ok(obj)
    } else {
        Err(ErrorType::DataNotFound(DataNotFoundData::Info))
    }
}

/// 读取结构文件
///
/// - `stream`: 结构文件数据流
/// - `schematic_type`: 结构文件类型
///
/// # 返回值
///
/// 返回解析出的结构信息；读取或解析失败返回对应错误
pub fn read_schematic_file<R: Read + Seek>(
    stream: &mut R,
    schematic_type: SchematicType,
) -> CoreResult<SchematicObj> {
    let nbt = NbtFile::read(stream)?;
    match schematic_type {
        SchematicType::Minecraft => read_schematic(nbt),
        SchematicType::Litematic => read_litematic(nbt),
        SchematicType::WorldEdit => read_schem(nbt),
        SchematicType::Create => read_nbt(nbt),
    }
}

impl InstanceSettingObj {
    /// 获取结构文件列表
    ///
    /// # 返回值
    ///
    /// 返回结构文件列表，读取失败的文件标记 `fail`
    pub async fn get_schematics(&self) -> Vec<SchematicObj> {
        let dir = self.get_schematics_path();
        let files = path_helper::get_all_files(&dir);

        tokio::task::spawn_blocking(move || {
            let list = Mutex::new(Vec::new());

            files.par_iter().for_each(|item| {
                if let Some(ext) = item.extension() {
                    let schematic_type = SchematicType::from_ext(&ext.to_string_lossy());
                    let mut obj = if let Ok(mut stream) = path_helper::open_read(item)
                        && let Ok(obj) = read_schematic_file(&mut stream, schematic_type)
                    {
                        obj
                    } else {
                        SchematicObj {
                            fail: true,
                            ..Default::default()
                        }
                    };
                    obj.path = item.clone();
                    list.lock().unwrap().push(obj);
                }
            });

            list.into_inner().unwrap()
        })
        .await
        .unwrap_or_default()
    }

    /// 导入结构文件
    ///
    /// - `files`: 待导入的文件列表，重名自动加序号后缀
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；复制失败返回对应错误
    pub fn import_schematic(&self, files: Vec<PathBuf>) -> CoreResult<()> {
        let path = self.get_schematics_path();
        path_helper::create_dir_all(&path)?;

        // 收集目标目录中已有的文件名，用于冲突检测
        let existing = path_helper::get_all_files(&path);
        let mut names: Vec<String> = existing
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        for item in files.iter() {
            if !item.is_file() {
                continue;
            }

            let file_name = match item.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };

            // 如果文件名冲突，在后面加 (1)、(2) ...
            let final_name = if names.contains(&file_name) {
                let stem = item
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let ext = item
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();

                let mut counter = 1u32;
                loop {
                    let candidate = format!("{stem}({counter}){ext}");
                    if !names.contains(&candidate) {
                        break candidate;
                    }
                    counter += 1;
                }
            } else {
                file_name
            };

            names.push(final_name.clone());
            let dest = path.join(&final_name);
            path_helper::copy_file(item, &dest)?;
        }

        Ok(())
    }
}
