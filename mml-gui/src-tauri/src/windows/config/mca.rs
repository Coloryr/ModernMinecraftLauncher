//! 区域文件的区块表与区块内查找
//!
//! 按区块粒度解压、按名字或类型在区块里找目标，给的是可回传的路径。

use std::io::Cursor;
use std::path::Path;

use mml_nbt::chunk::ChunkData;
use mml_nbt::nbt_file::NbtFile;
use mml_nbt::NbtType;
use mml_sys::path_helper;

use crate::dtos::{ChunkFindDto, FileItemType, McaChunkDto};
use crate::err_keys;

use crate::windows::{instance_snapshot, nbt_session};

use super::source::{get_file_type, resolve_source, resolve_under};


/// 读取一个区域文件（`.mca`）的区块表
///
/// 只读 8 KiB 文件头，**不解析任何一个区块**：一个区域文件最多 1024 个区块，
/// 全解析一遍只为列个表不值当。要看某个区块的字节走 [`config_get_bytes_slice`]。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `file`: 来源内的相对路径
///
/// # 返回值
///
/// 有数据的区块，按表项下标升序；空表项不返回。路径非法返回 `err.fileName`，
/// 扩展名不是 `.mca` 返回 `err.notMcaFile`，文件头读不满返回内核错误串
#[gui_macros::ipc_group("config")]
#[gui_macros::command]
pub fn config_get_mca(
    game: String,
    source: String,
    file: String,
) -> Result<Vec<McaChunkDto>, String> {
    let game = instance_snapshot(&game)?;
    let path = resolve_source(&game, &source, &file)?;
    if get_file_type(&path) != FileItemType::Mca {
        return Err(String::from(err_keys::NOT_MCA_FILE));
    }

    let mut stream = path_helper::open_read(&path).map_err(|err| err.to_string())?;
    // 文件头固定 8 KiB：更短的文件连位置表都不完整，按"空区域文件"报
    let len = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    if len < 8192 {
        return Err(String::from(err_keys::MCA_FILE_EMPTY));
    }
    let head = ChunkData::read_head(&mut stream).map_err(|err| err.to_string())?;

    Ok(head
        .iter()
        .filter(|item| item.count > 0)
        .map(|item| McaChunkDto {
            index: item.index,
            // 表项下标 = `(x & 31) + (z & 31) * 32`，见 mml_nbt::chunk::chunk_to_head_pos
            x: (item.index % 32) as i32,
            z: (item.index / 32) as i32,
            pos: item.pos,
            sectors: item.count,
            time: item.time,
        })
        .collect())
}

/// 按方块坐标在存档里找实体或方块实体；不给坐标就整份存档扫一遍
///
/// 文件名与表项下标都由坐标算出来，不需要用户自己挑区域文件：
/// 方块 `(x, z)` → 区块 `(x >> 4, z >> 4)` → 区域 `(cx >> 5, cz >> 5)` 与表项 `(cx & 31) + (cz & 31) * 32`。
/// 实体在 `entities/`、方块实体在 `region/`，两边的区域文件各自一套。
///
/// - `game`: 实例 uuid
/// - `source`: 来源，空串为实例本身，否则是存档目录名
/// - `kind`: 找哪一类，取值 `Entities` / `block_entities`
/// - `x`: 方块坐标 X；与 `z` 都不给表示整份存档扫
/// - `z`: 方块坐标 Z
/// - `name`: id 片段；空串表示只定位区块
///
/// # 返回值
///
/// 命中的区域文件、表项下标与节点路径；没命中时 `file` 与 `path` 都为空。
/// 路径非法返回 `err.fileName`，这一类取错返回 `err.mcaChunkEmpty`
#[gui_macros::ipc_group("config")]
#[gui_macros::command]
pub fn config_find_chunk(
    game: String,
    source: String,
    kind: String,
    x: Option<i32>,
    z: Option<i32>,
    name: String,
) -> Result<ChunkFindDto, String> {
    let game = instance_snapshot(&game)?;
    if kind != "Entities" && kind != "block_entities" {
        return Err(String::from(err_keys::MCA_CHUNK_EMPTY));
    }

    // 实体与方块实体分开放：1.17 起实体在 entities/，方块实体还在 region/
    let dir = if kind == "Entities" {
        "entities"
    } else {
        "region"
    };
    let base = if source.is_empty() {
        game.get_game_path()
    } else {
        resolve_under(&game.get_saves_path(), &source)?
    };

    let (files, index) = match (x, z) {
        (Some(x), Some(z)) => {
            let chunk_x = x >> 4;
            let chunk_z = z >> 4;
            let region_x = chunk_x >> 5;
            let region_z = chunk_z >> 5;
            let name = format!("{dir}/r.{region_x}.{region_z}.mca");
            let index = ((chunk_x & 31) + (chunk_z & 31) * 32) as u32;
            (vec![name], Some(index))
        }
        // 没给坐标：这一类下面的区域文件全扫
        _ => {
            let mut list: Vec<String> = Vec::new();
            for file in path_helper::get_all_files(&base.join(dir)) {
                if get_file_type(&file) != FileItemType::Mca {
                    continue;
                }
                let Ok(rel) = file.strip_prefix(&base) else {
                    continue;
                };
                list.push(rel.to_string_lossy().replace('\\', "/"));
            }
            list.sort();
            (list, None)
        }
    };

    for rel in files {
        let path = resolve_under(&base, &rel)?;
        if !path.is_file() {
            continue;
        }
        if let Some(hit) = find_in_file(&path, &kind, &name, index)? {
            return Ok(ChunkFindDto {
                file: rel,
                index: hit.index,
                path: hit.path.unwrap_or_default(),
            });
        }
    }

    Ok(ChunkFindDto {
        file: String::new(),
        index: 0,
        path: Vec::new(),
    })
}

/// 区域文件里的命中结果
pub(super) struct ChunkHit {
    /// 命中的表项下标
    index: u32,
    /// 节点路径；`None` 表示只定位到区块、没挑条目
    path: Option<Vec<String>>,
}

/// 在一个区域文件里找：给了表项下标就只看那一块，否则扫全部有数据的区块
///
/// 扫描直接读每块的字节、不建会话 —— 扫一遍可能在会话表里堆几百份用不上的句柄。
///
/// - `path`: 区域文件绝对路径
/// - `kind`: `Entities` 或 `block_entities`
/// - `name`: id 片段；空串表示只定位区块
/// - `only`: 只看这一表项；`None` 表示扫全部
///
/// # 返回值
///
/// 见 [`ChunkHit`]；没命中返回 `None`
pub(super) fn find_in_file(
    path: &Path,
    kind: &str,
    name: &str,
    only: Option<u32>,
) -> Result<Option<ChunkHit>, String> {
    let data = std::fs::read(path).map_err(|err| err.to_string())?;
    if data.len() < 8192 {
        return Err(String::from(err_keys::MCA_FILE_EMPTY));
    }
    let mut cursor = Cursor::new(data.as_slice());
    let head = ChunkData::read_head(&mut cursor).map_err(|err| err.to_string())?;

    for item in head.iter().filter(|item| item.count > 0) {
        if let Some(only) = only {
            if item.index != only {
                continue;
            }
        }
        let Ok(payload) = nbt_session::chunk_bytes(&data, item.index) else {
            continue;
        };
        let mut cursor = Cursor::new(payload);
        let Ok(nbt) = NbtFile::read(&mut cursor) else {
            continue;
        };
        let hit = find_in_chunk(&nbt.nbt, kind, name);
        // 只定位区块时不挑条目，命中即返回
        if hit.is_some() || name.is_empty() {
            return Ok(Some(ChunkHit {
                index: item.index,
                path: hit,
            }));
        }
    }

    Ok(None)
}

/// 在区块的某一类列表里按 id 找第一个包含 `name` 的条目
///
/// - `nbt`: 区块根标签
/// - `kind`: `Entities` 或 `block_entities`
/// - `name`: id 片段；空串取第一个条目
///
/// # 返回值
///
/// 命中节点的路径，如 `["block_entities", "7"]`；没有这一列或没命中返回 `None`
pub(super) fn find_in_chunk(nbt: &NbtType, kind: &str, name: &str) -> Option<Vec<String>> {
    let list = nbt.as_compound()?.data.get(kind)?.as_list()?;
    for (index, item) in list.iter().enumerate() {
        let Some(compound) = item.as_compound() else {
            continue;
        };
        let hit = match compound.data.get("id").and_then(|value| value.as_string()) {
            Some(id) => name.is_empty() || id.data.contains(name),
            // 没有 id 的条目只在"只要区块"时算命中
            None => name.is_empty(),
        };
        if hit {
            return Some(vec![kind.to_string(), index.to_string()]);
        }
    }
    None
}
