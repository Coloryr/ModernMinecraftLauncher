//! 结构文件读取测试。
//!
//! 样本不提交进 git，改为在测试内用 mml-nbt 程序化生成各格式的最小
//! 有效样本，再交给 `game_schematics::read_schematic_file` 读取并断言字段。

use std::path::{Path, PathBuf};

use mml_nbt::NbtType;
use mml_nbt::nbt_file::{CompressType, NbtFile};
use mml_nbt::nbt_types::{
    NbtByteArray, NbtCompound, NbtInt, NbtList, NbtLongArray, NbtShort, NbtString,
};

use mml_game::game_schematics::{self, SchematicType};

/// 构造 NBT Short 值
fn short(v: i16) -> NbtType {
    NbtShort::new(v).to_nbt()
}

/// 构造 NBT Int 值
fn int(v: i32) -> NbtType {
    NbtInt::new(v).to_nbt()
}

/// 构造 NBT String 值
fn string(v: &str) -> NbtType {
    NbtString::new(v.to_string()).to_nbt()
}

/// 写一个根 Compound 到临时文件并返回路径。
fn write_sample(root: NbtCompound, name: &str) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!("mml-schematic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let file = std::fs::File::create(&path).unwrap();
    let nbt = NbtFile::new(root.to_nbt(), CompressType::None);
    nbt.write(&mut std::io::BufWriter::new(file)).unwrap();
    path
}

/// 生成最小的原版 `.schematic`：只需 Height / Length / Width 三个 Short。
fn minecraft_sample() -> PathBuf {
    let mut root = NbtCompound::new();
    root.data.insert("Width".into(), short(3));
    root.data.insert("Height".into(), short(4));
    root.data.insert("Length".into(), short(5));
    write_sample(root, "test.schematic")
}

/// 生成最小的 WorldEdit `.schem`：尺寸 + 调色板 + 方块字节数据。
fn worldedit_sample() -> PathBuf {
    let mut root = NbtCompound::new();
    root.data.insert("Width".into(), short(3));
    root.data.insert("Height".into(), short(4));
    root.data.insert("Length".into(), short(5));
    root.data.insert(
        "BlockData".into(),
        NbtByteArray::new(vec![0, 0, 1]).to_nbt(),
    );

    let mut palette = NbtCompound::new();
    palette.data.insert("minecraft:air".into(), int(0));
    palette.data.insert("minecraft:stone".into(), int(1));
    root.data.insert("Palette".into(), palette.to_nbt());

    write_sample(root, "test.schem")
}

/// 生成最小的机械动力 `.nbt`：size + palette + blocks。
fn create_sample() -> PathBuf {
    let mut root = NbtCompound::new();

    let mut size = NbtList::new(3); // TAG_Int
    size.add_item(int(1)); // width
    size.add_item(int(2)); // height
    size.add_item(int(3)); // length
    root.data.insert("size".into(), size.to_nbt());

    let mut palette = NbtList::new(10); // TAG_Compound
    let mut air = NbtCompound::new();
    air.data.insert("Name".into(), string("minecraft:air"));
    palette.add_item(air.to_nbt());
    let mut stone = NbtCompound::new();
    stone.data.insert("Name".into(), string("minecraft:stone"));
    palette.add_item(stone.to_nbt());
    root.data.insert("palette".into(), palette.to_nbt());

    let mut blocks = NbtList::new(10);
    for state in [0, 0, 1] {
        let mut block = NbtCompound::new();
        block.data.insert("state".into(), int(state));
        blocks.add_item(block.to_nbt());
    }
    root.data.insert("blocks".into(), blocks.to_nbt());

    write_sample(root, "test.nbt")
}

/// 生成最小的投影 `.litematic`：Metadata + Regions，内含 BlockStatePalette 与 BlockStates。
fn litematic_sample() -> PathBuf {
    let mut root = NbtCompound::new();

    let mut meta = NbtCompound::new();
    meta.data.insert("Name".into(), string("test-litematic"));
    meta.data.insert("Author".into(), string("mml-test"));
    meta.data
        .insert("Description".into(), string("generated in-test"));
    let mut size = NbtCompound::new();
    size.data.insert("x".into(), int(3)); // 长
    size.data.insert("y".into(), int(2)); // 高
    size.data.insert("z".into(), int(1)); // 宽
    meta.data.insert("EnclosingSize".into(), size.to_nbt());
    meta.data.insert("TotalBlocks".into(), int(2));
    root.data.insert("Metadata".into(), meta.to_nbt());

    let mut main = NbtCompound::new();
    let mut palette = NbtList::new(10);
    let mut air = NbtCompound::new();
    air.data.insert("Name".into(), string("minecraft:air"));
    palette.add_item(air.to_nbt());
    let mut stone = NbtCompound::new();
    stone.data.insert("Name".into(), string("minecraft:stone"));
    palette.add_item(stone.to_nbt());
    main.data
        .insert("BlockStatePalette".into(), palette.to_nbt());
    // 2 位打包：第 0 位方块 = 1 即 stone，第 1 位 = 0 即 air，共 2 个方块
    main.data
        .insert("BlockStates".into(), NbtLongArray::new(vec![1]).to_nbt());

    let mut regions = NbtCompound::new();
    regions.data.insert("Main".into(), main.to_nbt());
    root.data.insert("Regions".into(), regions.to_nbt());

    write_sample(root, "test.litematic")
}

/// 读取生成的 `.schematic` 文件
fn read_generated(path: &Path, schematic_type: SchematicType) -> game_schematics::SchematicObj {
    let mut file = std::fs::File::open(path).unwrap();
    game_schematics::read_schematic_file(&mut file, schematic_type).unwrap()
}

/// 校验原版 `.schematic` 只按三个 Short 读出尺寸
#[test]
fn read_schematic() {
    let sche = read_generated(&minecraft_sample(), SchematicType::Minecraft);
    assert_eq!(sche.width, 3);
    assert_eq!(sche.height, 4);
    assert_eq!(sche.length, 5);
}

/// 校验 WorldEdit `.schem` 的尺寸与方块统计
#[test]
fn read_schem() {
    let sche = read_generated(&worldedit_sample(), SchematicType::WorldEdit);
    assert_eq!(sche.width, 3);
    assert_eq!(sche.height, 4);
    assert_eq!(sche.length, 5);
    assert_eq!(sche.block_count, 3);
    assert_eq!(sche.block_types, 2);
    assert_eq!(sche.blocks.get("minecraft:air"), Some(&2));
    assert_eq!(sche.blocks.get("minecraft:stone"), Some(&1));
}

/// 校验机械动力 `.nbt` 的尺寸与方块统计
#[test]
fn read_nbt() {
    let sche = read_generated(&create_sample(), SchematicType::Create);
    assert_eq!(sche.width, 1);
    assert_eq!(sche.height, 2);
    assert_eq!(sche.length, 3);
    assert_eq!(sche.block_count, 3);
    assert_eq!(sche.block_types, 2);
    assert_eq!(sche.blocks.get("minecraft:air"), Some(&2));
    assert_eq!(sche.blocks.get("minecraft:stone"), Some(&1));
}

/// 校验投影 `.litematic` 的元数据、尺寸与位打包方块统计
#[test]
fn read_litematic() {
    let sche = read_generated(&litematic_sample(), SchematicType::Litematic);
    assert_eq!(sche.name, "test-litematic");
    assert_eq!(sche.author, "mml-test");
    assert_eq!(sche.description, "generated in-test");
    assert_eq!(sche.width, 1);
    assert_eq!(sche.height, 2);
    assert_eq!(sche.length, 3);
    assert_eq!(sche.block_count, 2);
    assert_eq!(sche.blocks.get("minecraft:air"), Some(&1));
    assert_eq!(sche.blocks.get("minecraft:stone"), Some(&1));
}
