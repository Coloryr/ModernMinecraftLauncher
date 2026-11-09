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
