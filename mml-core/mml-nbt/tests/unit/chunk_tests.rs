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

