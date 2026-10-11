//! 真实区域文件样本的读写往返测试。

use std::{
    fs::File,
    io::{Cursor, Seek, SeekFrom},
    path::Path,
};

use mml_nbt::chunk::ChunkData;

/// 读取真实样本 tests/r.0.0.mca，写出后读回时区块数与表项数仍为 1024
#[test]
fn chunk_mca() {
    let path = Path::new("tests").join("r.0.0.mca");
    let mut file = File::open(path).unwrap();

    let mut chunk = ChunkData::read_chunk(&mut file).unwrap();

    assert_eq!(chunk.nbt.len(), 1024);
    assert_eq!(chunk.pos.len(), 1024);

    let mut stream1 = Cursor::new(Vec::<u8>::new());
    chunk.write_chunk(&mut stream1).unwrap();

    stream1.seek(SeekFrom::Start(0)).unwrap();

    let chunk1 = ChunkData::read_chunk(&mut stream1).unwrap();

    assert_eq!(chunk1.nbt.len(), 1024);
    assert_eq!(chunk1.pos.len(), 1024);

    // assert_eq!(chunk.pos, chunk1.pos);
}
