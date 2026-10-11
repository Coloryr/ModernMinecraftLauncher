//! 存档区块读写
//!
//! 面向 MCA 区域文件：前 8192 字节是文件头，1024 项位置表后跟 1024 项时间戳表，
//! 数据区按 4 KiB 扇区对齐存放各区块的压缩 NBT。
use std::{
    io::{Cursor, Read, Seek, SeekFrom, Write},
    sync::Mutex,
};

use chrono::Utc;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use rayon::iter::*;

use crate::{
    io_error,
    nbt_file::{CompressType, NbtFile},
};

/// 二维坐标
///
/// 同一个结构在不同函数里承载不同口径：世界坐标、区块坐标、区域坐标。
/// 用于区块时 `y` 装的是 Z 坐标。
pub struct PointI32 {
    /// X 坐标
    pub x: i32,
    /// Y 坐标；区块数据里装的是 Z 坐标
    pub y: i32,
}

/// 单个区块的 NBT 与其坐标
pub struct ChunkNbt {
    /// 该区块的 NBT 文件，含自身的压缩格式
    pub nbt: NbtFile,
    /// 区块坐标，取自 xPos / zPos，旧存档取 Position 数组
    pub point: PointI32,
}

/// 文件头中的一项：该表项对应区块的位置、扇区数与写入时间
#[derive(Debug, PartialEq, Eq)]
pub struct ChunkInfo {
    /// 表项序号，取值 0–1023，与文件头位置表一一对应
    pub index: u32,
    /// 数据起始偏移；读出后是字节偏移，写回文件头时换算成 4 KiB 扇区号
    pub pos: u32,
    /// 占用的扇区数，0 表示该表项没有区块
    pub count: u8,
    /// 最后写入时间，Unix 时间戳，单位秒
    pub time: u32,
    /// 数据实际字节数；当前读写流程未回填，恒为 0
    pub size: u32,
}

impl Default for ChunkInfo {
    fn default() -> Self {
        Self {
            index: Default::default(),
            pos: Default::default(),
            count: Default::default(),
            time: Default::default(),
            size: Default::default(),
        }
    }
}

/// 一个区域文件的全部区块
pub struct ChunkData {
    /// 各表项的区块，长度固定 1024，`None` 表示该位置没有区块
    pub nbt: Vec<Option<ChunkNbt>>,
    /// 文件头的位置表，长度固定 1024，下标即表项序号
    pub pos: Vec<ChunkInfo>,
}

/// 读取文件头的 1024 项位置表，把扇区偏移换算成字节偏移
///
/// - `stream`: 区域文件流，从当前位置起必须能读出 8192 字节文件头
fn read_chunk_head<R: Read>(stream: &mut R) -> CoreResult<Vec<ChunkInfo>> {
    let mut temp = vec![0u8; 8192];

    stream.read_exact(&mut temp).map_err(|err| io_error(err))?;

    let mut pos = Vec::<ChunkInfo>::new();
    pos.resize_with(1024, || Default::default());
    for index in 0..1024 {
        let po: u32 = (temp[index * 4] as u32) << 16
            | (temp[(index * 4) + 1] as u32) << 8
            | temp[(index * 4) + 2] as u32;
        let time: u32 = (temp[(index * 4) + 4096] as u32) << 24
            | (temp[(index * 4) + 4097] as u32) << 16
            | (temp[(index * 4) + 4098] as u32) << 8
            | temp[(index * 4) + 4099] as u32;

        pos[index].pos = po * 4096;
        pos[index].count = temp[(index * 4) + 3];
        pos[index].time = time;
        pos[index].index = index as u32;
    }

    Ok(pos)
}

impl ChunkData {
    /// 读取整个区域文件，逐个解析有数据的区块
    ///
    /// - `stream`: 区域文件流，会跨线程共享，内部加锁串行 seek
    ///
    /// # 返回值
    ///
    /// `nbt` 与 `pos` 都是 1024 项，下标与文件头表项一致；`count` 为 0 的表项填 `None`
    pub fn read_chunk<R: Read + Seek + Send + Sync>(stream: &mut R) -> CoreResult<ChunkData> {
        let head = read_chunk_head(stream)?;
        let file_mutex = Mutex::new(stream);

        let nbts: Vec<Option<ChunkNbt>> = (0..head.len())
            .into_par_iter()
            .map(|idx| {
                let item = &head[idx];

                if item.count == 0 {
                    return Ok::<_, ErrorType>(None);
                }

                // seek 与 read 必须整体加锁，否则并行线程会互相错位
                let buffer = {
                    let mut file_guard = file_mutex.lock().unwrap();
                    file_guard
                        .seek(SeekFrom::Start(item.pos as u64))
                        .map_err(|err| io_error(err))?;

                    // 前 4 字节是长度，第 5 字节是压缩代号，长度把代号也算在内
                    let mut temp = [0u8; 5];
                    file_guard
                        .read_exact(&mut temp)
                        .map_err(|err| io_error(err))?;

                    let item_size =
                        u32::from_be_bytes([temp[0], temp[1], temp[2], temp[3]]) as usize;
                    let mut buffer = vec![0u8; item_size - 1];
                    file_guard
                        .read_exact(&mut buffer)
                        .map_err(|err| io_error(err))?;

                    buffer
                };

                let mut cursor = Cursor::new(buffer);
                let nbt_file = NbtFile::read(&mut cursor)?;
                let nbt = nbt_file.nbt.as_compound();

                match nbt {
                    None => Err(ErrorType::NbtTypeError),
                    Some(nbt) => {
                        let mut x = 0;
                        let mut z = 0;
                        if let Some(nbt) = nbt.data.get("xPos").and_then(|v| v.as_int()) {
                            x = nbt.data;
                        }
                        if let Some(nbt) = nbt.data.get("zPos").and_then(|v| v.as_int()) {
                            z = nbt.data;
                        }
                        if let Some(value) = nbt.data.get("Position").and_then(|v| v.as_int_array())
                        {
                            if value.data.len() >= 2 {
                                x = value.data[0];
                                z = value.data[1];
                            }
                        }

                        Ok(Some(ChunkNbt {
                            nbt: nbt_file,
                            point: PointI32 { x, y: z },
                        }))
                    }
                }
            })
            .collect::<Vec<Result<_, _>>>()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ChunkData {
            nbt: nbts,
            pos: head,
        })
    }

    /// 写出数据区，并把每个区块的偏移与扇区数回填到位置表
    ///
    /// - `stream`: 目标区域文件流，从当前位置起写 8192 字节文件头占位与数据区
    fn write_chunk_data<W: Write + Seek>(&mut self, stream: &mut W) -> CoreResult<()> {
        if self.nbt.len() == 0 {
            return Ok(());
        }

        let buf = vec![0u8; 8192];
        // 数据区从文件头之后开始
        let mut now = 8192;
        stream.write_all(&buf).map_err(|err| io_error(err))?;

        let time = Utc::now().timestamp() as u32;

        for item in self.nbt.iter() {
            if item.is_none() {
                continue;
            }
            let nbt = item.as_ref().unwrap();
            let nbt_data = {
                let mut stream = Cursor::new(Vec::<u8>::new());
                nbt.nbt.write(&mut stream)?;
                stream.into_inner()
            };

            let len = nbt_data.len() + 5;
            let pos = chunk_to_head_pos(&nbt.point) as usize;

            // 位置表存扇区号，扇区数按 4 KiB 向上取整
            self.pos[pos].time = time;
            self.pos[pos].pos = now / 4096;
            self.pos[pos].count = (len as f64 / 4096.0).ceil() as u8;

            let buf = u32::to_be_bytes((nbt_data.len() + 1) as u32);
            stream.write_all(&buf).map_err(|err| io_error(err))?;
            let buf = [get_chunk_compress_type(&nbt.nbt.compress)];
            stream.write_all(&buf).map_err(|err| io_error(err))?;
            stream.write_all(&nbt_data).map_err(|err| io_error(err))?;
            now += len as u32;
            let less = len % 4096;
            if less > 0 {
                let buf = vec![0u8; 4096 - less];
                stream.write_all(&buf).map_err(|err| io_error(err))?;
                now += buf.len() as u32;
            }
        }

        Ok(())
    }

    /// 把位置表与时间戳表写回文件开头
    ///
    /// - `stream`: 目标区域文件流，会先 seek 回起点
    fn write_head<W: Write + Seek>(&mut self, stream: &mut W) -> CoreResult<()> {
        stream
            .seek(SeekFrom::Start(0))
            .map_err(|err| io_error(err))?;

        for item in self.pos.iter() {
            if item.count == 0 {
                let data = [0u8; 4];
                stream.write_all(&data).map_err(|err| io_error(err))?;
            } else {
                // 位置表每项 4 字节：高 3 字节是扇区号，最低字节是扇区数
                let mut data = u32::to_be_bytes(item.pos);
                data[0] = data[1];
                data[1] = data[2];
                data[2] = data[3];
                data[3] = item.count;
                stream.write_all(&data).map_err(|err| io_error(err))?;
            }
        }
        for item in self.pos.iter() {
            if item.count == 0 {
                let data = [0u8; 4];
                stream.write_all(&data).map_err(|err| io_error(err))?;
            } else {
                let data = u32::to_be_bytes(item.time);
                stream.write_all(&data).map_err(|err| io_error(err))?;
            }
        }

        Ok(())
    }

    /// 先写数据区再回填文件头，写出的区域文件可被 `read_chunk` 读回
    ///
    /// - `stream`: 目标区域文件流，起始位置必须是文件开头
    pub fn write_chunk<W: Write + Seek>(&mut self, stream: &mut W) -> CoreResult<()> {
        self.write_chunk_data(stream)?;
        self.write_head(stream)?;

        Ok(())
    }
}

/// 世界坐标转区块坐标，按 16 取整
///
/// - `pos`: 世界坐标
///
/// # 返回值
///
/// 区块坐标；负坐标用算术右移，向负无穷取整
pub fn pos_to_chunk(pos: &PointI32) -> PointI32 {
    PointI32 {
        x: pos.x >> 4,
        y: pos.y >> 4,
    }
}

/// 区块坐标转 MCA 区域坐标，按 32 取整
///
/// - `pos`: 区块坐标
///
/// # 返回值
///
/// 区域坐标，即 `.mca` 文件名里的那一对数字
pub fn chunk_to_region(pos: &PointI32) -> PointI32 {
    PointI32 {
        x: pos.x >> 5,
        y: pos.y >> 5,
    }
}

/// 区块坐标转文件头表项下标
///
/// - `pos`: 区域内的区块坐标
///
/// # 返回值
///
/// 位置表下标，取值 0–1023；坐标超出区域时按低 5 位折回
pub fn chunk_to_head_pos(pos: &PointI32) -> i32 {
    (pos.x & 31) + (pos.y & 31) * 32
}

/// 把压缩类型映射成 MCA 区块头里的压缩代号
///
/// - `compress`: 压缩类型
fn get_chunk_compress_type(compress: &CompressType) -> u8 {
    match compress {
        CompressType::None => 3,
        CompressType::GZip => 1,
        CompressType::Zlib => 2,
        CompressType::Lz4 => 4,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Seek, SeekFrom};

    use super::*;
    use crate::nbt_types;

    /// 构造一个带 xPos / zPos 的区块 NBT
    fn make_chunk_nbt(x: i32, z: i32) -> NbtFile {
        let mut com = nbt_types::compound();
        com.data.insert("xPos".into(), nbt_types::int(x).to_nbt());
        com.data.insert("zPos".into(), nbt_types::int(z).to_nbt());
        NbtFile::new(com.to_nbt(), CompressType::Zlib)
    }

    /// pos_to_chunk：世界坐标除以 16，向下取整
    #[test]
    fn pos_to_chunk_math() {
        let point = pos_to_chunk(&PointI32 { x: 5, y: 7 });
        assert_eq!((point.x, point.y), (0, 0));

        let point = pos_to_chunk(&PointI32 { x: 16, y: 31 });
        assert_eq!((point.x, point.y), (1, 1));

        // 负坐标向下取整
        let point = pos_to_chunk(&PointI32 { x: -1, y: -16 });
        assert_eq!((point.x, point.y), (-1, -1));

        let point = pos_to_chunk(&PointI32 { x: -17, y: 15 });
        assert_eq!((point.x, point.y), (-2, 0));
    }

    /// chunk_to_region：区块坐标除以 32，向下取整
    #[test]
    fn chunk_to_region_math() {
        let point = chunk_to_region(&PointI32 { x: 0, y: 31 });
        assert_eq!((point.x, point.y), (0, 0));

        let point = chunk_to_region(&PointI32 { x: 32, y: 33 });
        assert_eq!((point.x, point.y), (1, 1));

        let point = chunk_to_region(&PointI32 { x: -1, y: -32 });
        assert_eq!((point.x, point.y), (-1, -1));
    }

    /// chunk_to_head_pos：文件头表项索引，始终落在 0–1023
    #[test]
    fn chunk_to_head_pos_math() {
        assert_eq!(chunk_to_head_pos(&PointI32 { x: 0, y: 0 }), 0);
        assert_eq!(chunk_to_head_pos(&PointI32 { x: 1, y: 0 }), 1);
        assert_eq!(chunk_to_head_pos(&PointI32 { x: 0, y: 1 }), 32);
        assert_eq!(chunk_to_head_pos(&PointI32 { x: 31, y: 31 }), 1023);
        // 负坐标按低 5 位折回
        assert_eq!(chunk_to_head_pos(&PointI32 { x: -1, y: -1 }), 1023);
        assert_eq!(chunk_to_head_pos(&PointI32 { x: 32, y: 32 }), 0);
    }

    /// 区块压缩类型与 MCA 格式压缩代号的映射
    #[test]
    fn compress_type_mapping() {
        assert_eq!(get_chunk_compress_type(&CompressType::GZip), 1);
        assert_eq!(get_chunk_compress_type(&CompressType::Zlib), 2);
        assert_eq!(get_chunk_compress_type(&CompressType::None), 3);
        assert_eq!(get_chunk_compress_type(&CompressType::Lz4), 4);
    }

    /// ChunkInfo 默认值全为 0
    #[test]
    fn chunk_info_default() {
        let info = ChunkInfo::default();
        assert_eq!(
            (info.index, info.pos, info.count, info.time, info.size),
            (0, 0, 0, 0, 0)
        );
    }

    /// 写区块到内存流后再读回，区块坐标应保持一致
    #[test]
    fn write_then_read_round_trip() {
        // 区块 (0,0) 与 (1,1)，分别落在头表索引 0 和 33
        let mut chunk_data = ChunkData {
            nbt: vec![
                Some(ChunkNbt {
                    nbt: make_chunk_nbt(0, 0),
                    point: PointI32 { x: 0, y: 0 },
                }),
                Some(ChunkNbt {
                    nbt: make_chunk_nbt(1, 1),
                    point: PointI32 { x: 1, y: 1 },
                }),
            ],
            pos: (0..1024).map(|_| ChunkInfo::default()).collect(),
        };

        let mut stream = Cursor::new(Vec::<u8>::new());
        chunk_data.write_chunk(&mut stream).unwrap();

        stream.seek(SeekFrom::Start(0)).unwrap();
        let back = ChunkData::read_chunk(&mut stream).unwrap();

        assert_eq!(back.nbt.len(), 1024);
        assert_eq!(back.pos.len(), 1024);

        // 索引 0 与 33 处应有区块数据，坐标一致
        let first = back.nbt[0].as_ref().unwrap();
        assert_eq!((first.point.x, first.point.y), (0, 0));
        let second = back.nbt[33].as_ref().unwrap();
        assert_eq!((second.point.x, second.point.y), (1, 1));

        // 其余表项为空
        assert!(back.nbt[1].is_none());
        assert!(back.nbt[1023].is_none());
    }

    /// 空区块列表写入时跳过数据区，仅写出全零的 8192 字节文件头
    #[test]
    fn write_empty_chunk_data() {
        let mut chunk_data = ChunkData {
            nbt: vec![],
            pos: (0..1024).map(|_| ChunkInfo::default()).collect(),
        };

        let mut stream = Cursor::new(Vec::<u8>::new());
        chunk_data.write_chunk(&mut stream).unwrap();

        let bytes = stream.into_inner();
        assert_eq!(bytes.len(), 8192);
        assert!(bytes.iter().all(|&b| b == 0));
    }
}
