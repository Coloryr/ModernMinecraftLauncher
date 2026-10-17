//! NBT 文件的读写模块
//!
//! 本模块实现了 Minecraft NBT 文件格式的完整读写支持。
//! NBT 文件可以以未压缩格式存储，也可以使用 GZip、Zlib 或 Lz4 压缩。
//! 文件格式遵循 Minecraft 规范：文件头为根标签类型序号，Compound 根标签还带
//! 标签名称，后跟实际的 NBT 数据。
//!
//! # 支持的文件格式
//!
//! - `.dat` / `.nbt` — 未压缩的 NBT 文件
//! - GZip 压缩，Minecraft 默认格式，魔数 `1F 8B`
//! - Zlib 压缩，魔数 `78`
//! - Lz4 压缩，魔数 `4C 5A 34` 或 `04 22 4D`
//!
//! # 支持的功能
//!
//! - 自动检测压缩格式，靠文件头魔数判断
//! - 读取 Compound 根标签名称
//! - 读取和写入所有 NBT 标签类型
//! - 支持的 NBT 标签类型：End, Byte, Short, Int, Long, Float, Double,
//!   ByteArray, String, List, Compound, IntArray, LongArray

use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};

use flate2::{
    read::{GzDecoder, ZlibDecoder},
    write::{GzEncoder, ZlibEncoder},
};
use mml_names::i18_items::error_type::{CoreResult, ErrorType};

use crate::nbt_types::NbtCompound;
use crate::{NBT_BYTE_ORDER, NBT_END_ORDER, NbtType, io_error, nbt_types};

/// NBT 文件的压缩类型
///
/// 表示 NBT 文件在存储时所使用的压缩格式。
/// 支持无压缩、GZip、Zlib 和 Lz4 四种模式。
#[derive(Clone, Debug, PartialEq)]
pub enum CompressType {
    /// 不压缩，原始数据直接读写
    None,
    /// GZip 压缩，魔数 `1F 8B`，Minecraft Java 版默认
    GZip,
    /// Zlib 压缩，魔数 `78`，部分旧版 Minecraft 数据使用
    Zlib,
    /// Lz4 压缩，魔数 `4C 5A 34` 或 `04 22 4D`，Minecraft 基岩版常用
    Lz4,
}

/// 以字符串形式展示压缩类型名称
impl fmt::Display for CompressType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompressType::None => write!(f, "none"),
            CompressType::GZip => write!(f, "gzip"),
            CompressType::Zlib => write!(f, "zlib"),
            CompressType::Lz4 => write!(f, "lz4"),
        }
    }
}

/// NBT 文件
///
/// 一个完整的 NBT 文件由根标签和压缩格式组成：读入时压缩格式由魔数自动判定，
/// 写出时按 `compress` 选择编码器。
///
/// # 示例
///
/// ```ignore
/// use mml_nbt::nbt_file::{NbtFile, CompressType};
/// use mml_nbt::NbtType;
///
/// // 从文件读取
/// let mut file = std::fs::File::open("level.dat").unwrap();
/// let nbt_file = NbtFile::read(&mut file).unwrap();
///
/// // 写入文件
/// let nbt_file = NbtFile::new(NbtType::compound(), CompressType::GZip);
/// let mut file = std::fs::File::create("output.dat").unwrap();
/// nbt_file.write(&mut file).unwrap();
/// ```
pub struct NbtFile {
    /// 根 NBT 标签，通常是 Compound；退化文件里也可能是单个 Byte 或 End
    pub nbt: NbtType,
    /// 压缩格式，写出时用它选编码器，读入时按检测结果回填
    pub compress: CompressType,
}

/// NBT 文件的默认值
///
/// 默认创建一个空的 End 标签、无压缩的 NBT 文件。
impl Default for NbtFile {
    fn default() -> Self {
        Self {
            nbt: NbtType::end(),
            compress: CompressType::None,
        }
    }
}

impl NbtFile {
    /// 创建一个新的 NBT 文件实例
    ///
    /// - `nbt`: 根 NBT 标签
    /// - `compress`: 压缩格式，决定写出时用哪种编码器
    ///
    /// # 返回值
    ///
    /// 配置好的 `NbtFile`，两个字段都不做校验
    pub fn new(nbt: NbtType, compress: CompressType) -> Self {
        Self { nbt, compress }
    }

    /// 从流中读取 NBT 文件
    ///
    /// 按文件头魔数判定压缩格式，解压后解析 NBT 数据。除标准文件外还兼容两种退化形态：
    /// - 仅 1 字节 `0x00`，视为单个 `End` 标签
    /// - 仅 2 字节 `类型 ID + 值`，视为单个 `Byte` 标签
    ///
    /// 其余按标准 NBT 文件处理：至少 3 字节文件头加 NBT 数据，可带压缩。
    ///
    /// - `stream`: 实现了 `Read + Seek` 的文件流，读取过程中会 seek 回起点
    ///
    /// # 返回值
    ///
    /// 解析出的 `NbtFile`，压缩格式按检测结果回填
    ///
    /// # 错误
    ///
    /// - IO 错误 — 读取流失败
    /// - `NbtReadError` — 数据格式无效或 NBT 标签类型序号不合法
    pub fn read<R: Read + Seek>(stream: &mut R) -> CoreResult<Self> {
        // 读取前 3 个字节用于检测压缩格式
        let mut temp = [0u8; 3];
        let size = stream.read(&mut temp).map_err(|err| io_error(err))?;
        // 检测用的试探读要还原，后续解码仍从文件头开始
        stream
            .seek(SeekFrom::Start(0))
            .map_err(|err| io_error(err))?;

        // 仅 1 字节的 TAG_End 文件
        if size == 1 && temp[0] == NBT_END_ORDER {
            return Ok(NbtFile {
                nbt: NbtType::end(),
                compress: CompressType::None,
            });
        }
        // 仅 2 字节的单个 TAG_Byte 文件
        else if size == 2 && temp[0] == NBT_BYTE_ORDER {
            return Ok(NbtFile {
                nbt: nbt_types::byte(temp[1]).to_nbt(),
                compress: CompressType::None,
            });
        }

        // 文件头至少需要 3 字节
        if size != 3 {
            return Err(ErrorType::NbtReadError);
        }

        let mut compress_type = CompressType::None;

        // 通过魔数检测压缩格式并创建对应的解压流
        let mut stream: Box<dyn Read> = if temp[0] == 0x1F && temp[1] == 0x8B {
            // GZip 魔数：1F 8B
            compress_type = CompressType::GZip;
            Box::new(GzDecoder::new(stream))
        } else if temp[0] == 0x78 && (temp[1] == 0x01 || temp[1] == 0x9C || temp[1] == 0xDA) {
            // Zlib 魔数：78，后一字节是压缩级别
            compress_type = CompressType::Zlib;
            Box::new(ZlibDecoder::new(stream))
        } else if (temp[0] == 0x4C && temp[1] == 0x5A && temp[2] == 0x34)
            || (temp[0] == 0x04 && temp[1] == 0x22 && temp[2] == 0x4D)
        {
            // Lz4 魔数：4C 5A 34 即 "LZ4"，或 04 22 4D 的 legacy 帧头
            compress_type = CompressType::Lz4;
            Box::new(lz4_flex::frame::FrameDecoder::new(stream))
        } else {
            // 未检测到已知压缩格式，作为无压缩数据处理
            Box::new(stream)
        };

        let mut temp = [0u8; 1];
        stream.read_exact(&mut temp).map_err(|err| io_error(err))?;
        let nbt = NbtType::get_nbt(temp[0]);
        if nbt.is_none() {
            return Err(ErrorType::NbtReadError);
        }

        let mut nbt_inner = nbt.unwrap();
        let mut root_name = String::new();
        // 只有 Compound 根标签才带名称
        if matches!(nbt_inner, NbtType::Compound(_)) {
            let mut temp = [0u8; 2];
            stream.read_exact(&mut temp).map_err(|err| io_error(err))?;
            let len = u16::from_be_bytes(temp);
            if len > 0 {
                let mut temp = vec![0; len as usize];
                stream.read_exact(&mut temp).map_err(|err| io_error(err))?;
                root_name = crate::nbt_types::decode_string(temp);
            }
        }
        nbt_inner.read(&mut stream)?;

        // 名称非空说明原文件是命名根标签，按原名包回 Compound
        if root_name.is_empty() {
            Ok(NbtFile {
                nbt: nbt_inner,
                compress: compress_type,
            })
        } else {
            let mut com = NbtCompound::new();
            com.data.insert(root_name, nbt_inner);
            Ok(NbtFile {
                nbt: com.to_nbt(),
                compress: compress_type,
            })
        }
    }

    /// 将 NBT 文件写入流中
    ///
    /// 按 `compress` 选编码器，序列化 NBT 数据后写出；Compound 根标签在数据前
    /// 补 2 字节空名称，符合 Minecraft NBT 文件格式。
    ///
    /// - `stream`: 实现了 `Write` 的目标流
    ///
    /// # 返回值
    ///
    /// 数据与压缩帧尾都写出后返回 `Ok(())`，否则返回 IO 错误
    pub fn write<W: Write>(&self, stream: &mut W) -> CoreResult<()> {
        // 根据压缩类型创建对应的编码器包装流
        let mut stream: Box<dyn Write> = match self.compress {
            CompressType::None => Box::new(stream),
            CompressType::GZip => Box::new(GzEncoder::new(stream, Default::default())),
            CompressType::Zlib => Box::new(ZlibEncoder::new(stream, Default::default())),
            CompressType::Lz4 => Box::new(lz4_flex::frame::FrameEncoder::new(stream)),
        };

        let temp = [self.nbt.get_num()];
        stream.write_all(&temp).map_err(|err| io_error(err))?;

        // Compound 根标签在数据前补 2 字节空名称
        if matches!(self.nbt, NbtType::Compound(_)) {
            let mut temp = [0u8; 2];
            stream.write_all(&mut temp).map_err(|err| io_error(err))?;
        }

        self.nbt.write(&mut stream)?;

        // 压缩编码器必须 flush 才会写出帧尾
        stream.flush().map_err(|err| io_error(err))?;

        Ok(())
    }
}

/// 以调试友好的格式展示 NBT 文件信息
///
/// 输出格式：`<NBT 内容> (compress: <压缩类型>)`
impl fmt::Display for NbtFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (compress: {})", self.nbt, self.compress)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Cursor, Seek, SeekFrom},
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::nbt_types;

    /// 构造一个简单的根 Compound：{ "byte": 1b, "int": 2 }
    fn sample_compound() -> NbtType {
        let mut com = NbtCompound::new();
        com.data.insert("byte".into(), nbt_types::byte(1).to_nbt());
        com.data.insert("int".into(), nbt_types::int(2).to_nbt());
        com.to_nbt()
    }

    /// 压缩类型在内存流中写入后读取，内容与压缩类型均应保持一致
    #[test]
    fn round_trip_all_compressions() {
        for compress in [
            CompressType::None,
            CompressType::GZip,
            CompressType::Zlib,
            CompressType::Lz4,
        ] {
            let file = NbtFile::new(sample_compound(), compress.clone());

            let mut stream = Cursor::new(Vec::<u8>::new());
            file.write(&mut stream).unwrap();

            stream.seek(SeekFrom::Start(0)).unwrap();
            let back = NbtFile::read(&mut stream).unwrap();

            assert_eq!(back.compress, compress);
            assert!(file.nbt.eq(&back.nbt));
        }
    }

    /// 单字节 0x00 文件应识别为 End 标签
    #[test]
    fn read_end_only_file() {
        let mut stream = Cursor::new(vec![0u8]);
        let file = NbtFile::read(&mut stream).unwrap();
        assert!(matches!(file.nbt, NbtType::End(_)));
        assert_eq!(file.compress, CompressType::None);
    }

    /// 两字节 `[TAG_Byte, 值]` 文件应识别为单个 Byte 标签
    #[test]
    fn read_single_byte_file() {
        let mut stream = Cursor::new(vec![NBT_BYTE_ORDER, 42]);
        let file = NbtFile::read(&mut stream).unwrap();
        assert_eq!(file.nbt.as_byte().unwrap().data, 42);
        assert_eq!(file.compress, CompressType::None);
    }

    /// 过短的非法文件应返回错误
    #[test]
    fn read_too_short_returns_error() {
        // 2 字节但首字节为 TAG_End，不符合任何已知格式
        let mut stream = Cursor::new(vec![0u8, 5]);
        assert!(NbtFile::read(&mut stream).is_err());
    }

    /// 压缩格式的 Display 名称
    #[test]
    fn compress_display() {
        assert_eq!(CompressType::None.to_string(), "none");
        assert_eq!(CompressType::GZip.to_string(), "gzip");
        assert_eq!(CompressType::Zlib.to_string(), "zlib");
        assert_eq!(CompressType::Lz4.to_string(), "lz4");
    }

    /// NbtFile 的 Display 包含压缩类型信息
    #[test]
    fn file_display() {
        let file = NbtFile::new(NbtType::end(), CompressType::GZip);
        let text = file.to_string();
        assert!(text.contains("END"));
        assert!(text.contains("gzip"));
    }

    /// NbtFile 的默认值为 End 标签 + 无压缩
    #[test]
    fn default_values() {
        let file = NbtFile::default();
        assert!(matches!(file.nbt, NbtType::End(_)));
        assert_eq!(file.compress, CompressType::None);
    }

    /// 在临时目录下以真实文件做读写往返，验证跨平台的文件 IO 行为
    #[test]
    fn file_round_trip_on_disk() {
        // 唯一临时子目录，避免并发冲突
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            mml_testutil::temp_dir().join(format!("mml-nbt-test-{}-{}", std::process::id(), nanos));
        fs::create_dir_all(&dir).unwrap();

        let path: PathBuf = dir.join("test.dat");
        let result = (|| -> CoreResult<()> {
            let file = NbtFile::new(sample_compound(), CompressType::GZip);
            {
                let mut out = fs::File::create(&path).map_err(|err| io_error(err))?;
                file.write(&mut out)?;
            }

            let mut input = fs::File::open(&path).map_err(|err| io_error(err))?;
            let back = NbtFile::read(&mut input)?;
            assert_eq!(back.compress, CompressType::GZip);
            assert!(file.nbt.eq(&back.nbt));
            Ok(())
        })();

        // 测试结束清理临时目录
        let _ = fs::remove_dir_all(&dir);
        result.unwrap();
    }
}
