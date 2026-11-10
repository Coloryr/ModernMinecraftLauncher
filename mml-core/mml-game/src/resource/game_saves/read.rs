//! 存档 level.dat 与数据包元数据的解析

use super::*;

/// 读取存档文件
///
/// - `stream`: 存档文件流
///
/// # 返回值
///
/// 返回存档信息；格式不符时标记 `broken` 并返回原始 NBT
pub(crate) fn read_save<R: Read + Seek>(stream: &mut R) -> CoreResult<SaveObj> {
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
pub(crate) fn read_pack(
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
