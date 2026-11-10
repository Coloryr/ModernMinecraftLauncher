//! NBT 区块字节的读写
//!
//! 区块索引换算与就地写回，会话保存时按区块粒度替换。

use std::fs::File;
use std::io::{Cursor, Seek, SeekFrom, Write};

use mml_nbt::chunk::ChunkData;
use mml_nbt::nbt_file::NbtFile;

use crate::err_keys;

/// 从区域文件字节里取出一个区块的压缩负载
///
/// - `data`: 整份区域文件的字节
/// - `index`: 表项下标，0–1023
///
/// # 返回值
///
/// 该区块的负载字节，不含前面的长度与压缩代号；表项为空或越界返回 `err.mcaChunkEmpty`
pub(crate) fn chunk_bytes(data: &[u8], index: u32) -> Result<Vec<u8>, String> {
    let entry = (index as usize)
        .checked_mul(4)
        .ok_or_else(|| String::from(err_keys::MCA_CHUNK_EMPTY))?;
    let head = data
        .get(entry..entry + 4)
        .ok_or_else(|| String::from(err_keys::MCA_CHUNK_EMPTY))?;
    if head[3] == 0 {
        return Err(String::from(err_keys::MCA_CHUNK_EMPTY));
    }

    let sector = ((head[0] as usize) << 16) | ((head[1] as usize) << 8) | head[2] as usize;
    let pos = sector * 4096;
    let block = data
        .get(pos..pos + 5)
        .ok_or_else(|| String::from(err_keys::MCA_CHUNK_EMPTY))?;
    let size = u32::from_be_bytes([block[0], block[1], block[2], block[3]]) as usize;
    if size <= 1 {
        return Err(String::from(err_keys::MCA_CHUNK_EMPTY));
    }

    data.get(pos + 5..pos + 5 + (size - 1))
        .map(|slice| slice.to_vec())
        .ok_or_else(|| String::from(err_keys::MCA_CHUNK_EMPTY))
}

/// 把一个区块写回区域文件
///
/// 整份读出来、换上这一块、整份写回：扇区分配与文件头由 crate 负责，
/// 代价是保存时要解析其它区块，正确性比那点时间重要。
///
/// - `file`: 区域文件的读写句柄
/// - `index`: 表项下标
/// - `payload`: 区块内容的压缩字节，即 `NbtFile::write` 的输出
pub(super) fn write_chunk_back(file: &mut File, index: u32, payload: &[u8]) -> Result<(), String> {
    let mut cursor = Cursor::new(payload);
    let nbt = NbtFile::read(&mut cursor).map_err(|err| err.to_string())?;

    file.seek(SeekFrom::Start(0))
        .map_err(|err| err.to_string())?;
    let mut data = ChunkData::read_chunk(file).map_err(|err| err.to_string())?;
    let item = data
        .nbt
        .get_mut(index as usize)
        .and_then(|item| item.as_mut())
        .ok_or_else(|| String::from(err_keys::MCA_CHUNK_EMPTY))?;
    // 只换内容：坐标取自表项本身，改 xPos 不动表项位置
    item.nbt = nbt;

    file.seek(SeekFrom::Start(0))
        .map_err(|err| err.to_string())?;
    file.set_len(0).map_err(|err| err.to_string())?;
    data.write_chunk(file).map_err(|err| err.to_string())?;
    file.flush().map_err(|err| err.to_string())?;

    Ok(())
}
