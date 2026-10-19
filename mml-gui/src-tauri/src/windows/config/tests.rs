//! 配置窗口的单测

use std::fs;
use std::path::Path;

use mml_base::hash_helper;
use uuid::Uuid;

use crate::dtos::{FileItemType, NbtKindDto};

use super::files::*;
use super::nbt::*;
use super::nbt_edit::*;
use super::source::*;

use crate::dtos::{NbtEditDto, NbtOpDto};
use mml_nbt::nbt_types::{compound, int, list, string};
use mml_nbt::{NBT_END_ORDER, NbtType};

/// 造一个复合标签
fn compound_of(entries: Vec<(&str, NbtType)>) -> NbtType {
    let mut obj = compound();
    for (key, value) in entries {
        obj.data.insert(key.to_string(), value);
    }
    obj.to_nbt()
}

/// 造一个列表标签；元素类型序号取第一个元素的，空列表用 END
fn list_of(items: Vec<NbtType>) -> NbtType {
    let num = items
        .first()
        .map(|item| item.get_num())
        .unwrap_or(NBT_END_ORDER);
    let mut obj = list(num);
    for item in items {
        obj.add_item(item);
    }
    obj.to_nbt()
}

/// 造一份带复合与列表的样本
fn sample_nbt() -> NbtType {
    compound_of(vec![
        (
            "Data",
            compound_of(vec![
                ("GameType", int(1).to_nbt()),
                ("Name", string("x").to_nbt()),
            ]),
        ),
        ("Items", list_of(vec![int(3).to_nbt(), int(4).to_nbt()])),
    ])
}

/// 造一条"改值"的改动
fn set_at(path: &[&str], value: &str) -> NbtEditDto {
    NbtEditDto {
        op: NbtOpDto::Set,
        path: path.iter().map(|item| item.to_string()).collect(),
        kind: None,
        value: value.to_string(),
    }
}

/// 造一条"新增"的改动
fn add_at(path: &[&str], kind: NbtKindDto, value: &str) -> NbtEditDto {
    NbtEditDto {
        op: NbtOpDto::Add,
        path: path.iter().map(|item| item.to_string()).collect(),
        kind: Some(kind),
        value: value.to_string(),
    }
}

/// 造一条"改名"的改动
fn rename_at(path: &[&str], new_name: &str) -> NbtEditDto {
    NbtEditDto {
        op: NbtOpDto::Rename,
        path: path.iter().map(|item| item.to_string()).collect(),
        kind: None,
        value: new_name.to_string(),
    }
}

/// 造一条"删除"的改动
fn remove_at(path: &[&str]) -> NbtEditDto {
    NbtEditDto {
        op: NbtOpDto::Remove,
        path: path.iter().map(|item| item.to_string()).collect(),
        kind: None,
        value: String::new(),
    }
}

/// 按路径取一个标量标签
fn at<'a>(root: &'a mut NbtType, path: &[&str]) -> &'a mut NbtType {
    let path: Vec<String> = path.iter().map(|item| item.to_string()).collect();
    nbt_at_mut(root, &path).unwrap()
}

/// 标量按路径改：复合成员、列表元素都能走到
#[test]
fn apply_nbt_edits_changes_scalars() {
    let mut root = sample_nbt();
    apply_nbt_edits(
        &mut root,
        &[
            set_at(&["Data", "GameType"], "2"),
            set_at(&["Items", "1"], "9"),
            set_at(&["Data", "Name"], " 前后有空格也照收 "),
        ],
    )
    .unwrap();

    assert_eq!(
        at(&mut root, &["Data", "GameType"]).as_int().unwrap().data,
        2
    );
    assert_eq!(at(&mut root, &["Items", "1"]).as_int().unwrap().data, 9);

    // 字符串只 trim 掉解析用的空白，存的是原样（前后空格保留）
    assert_eq!(
        at(&mut root, &["Data", "Name"]).as_string().unwrap().data,
        " 前后有空格也照收 "
    );
}

/// 路径对不上、值解析不了都要拒掉：宁可报错，也不能写坏文件
#[test]
fn apply_nbt_edits_rejects_bad_edits() {
    let bad = [
        // 键不存在
        set_at(&["Nope"], "1"),
        // 值不是整数
        set_at(&["Data", "GameType"], "abc"),
        // 指到了复合上（改值只认标量与数组）
        set_at(&["Data"], "1"),
        // 列表下标越界
        set_at(&["Items", "9"], "1"),
        // 列表下标不是整数
        set_at(&["Items", "x"], "1"),
    ];

    for edit in bad {
        assert!(
            apply_nbt_edits(&mut sample_nbt(), &[edit]).is_err(),
            "应当拒绝"
        );
    }
}

/// 数组元素按下标改：三种元素类型都认，越界与解析不了都拒掉
#[test]
fn apply_nbt_edits_edits_array_items() {
    use mml_nbt::nbt_types::{byte_array, int_array, long_array};

    let mut root = compound_of(vec![
        ("Bytes", byte_array(vec![1, 2]).to_nbt()),
        ("Ints", int_array(vec![7]).to_nbt()),
        ("Longs", long_array(vec![9]).to_nbt()),
    ]);

    apply_nbt_edits(
        &mut root,
        &[
            set_at(&["Bytes", "1"], "255"),
            set_at(&["Ints", "0"], "-1"),
            set_at(&["Longs", "0"], "9000000000"),
        ],
    )
    .unwrap();

    match at(&mut root, &["Bytes"]) {
        NbtType::ByteArray(v) => assert_eq!(v.data, vec![1, 255]),
        other => panic!("类型变了：{}", other),
    }
    match at(&mut root, &["Ints"]) {
        NbtType::IntArray(v) => assert_eq!(v.data, vec![-1]),
        other => panic!("类型变了：{}", other),
    }
    match at(&mut root, &["Longs"]) {
        NbtType::LongArray(v) => assert_eq!(v.data, vec![9_000_000_000]),
        other => panic!("类型变了：{}", other),
    }

    // 越界、解析不了、以及想整段替换数组（元素只能一个个改）都要拒
    for edit in [
        set_at(&["Bytes", "9"], "1"),
        set_at(&["Bytes", "0"], "256"),
        set_at(&["Ints", "0"], "x"),
        set_at(&["Bytes", "x"], "1"),
        set_at(&["Bytes"], "1, 2"),
    ] {
        assert!(
            apply_nbt_edits(&mut sample_nbt(), &[edit]).is_err(),
            "应当拒绝"
        );
    }
}

/// 数组与列表一样能增删元素：下标等于长度是追加，删一个后面的下标往前挪
#[test]
fn apply_nbt_edits_adds_and_removes_array_items() {
    use mml_nbt::nbt_types::int_array;

    let mut root = compound_of(vec![("Ints", int_array(vec![1, 2]).to_nbt())]);

    apply_nbt_edits(
        &mut root,
        &[
            add_at(&["Ints", "2"], NbtKindDto::Int, "3"),
            add_at(&["Ints", "0"], NbtKindDto::Int, "0"),
            remove_at(&["Ints", "1"]),
        ],
    )
    .unwrap();

    match at(&mut root, &["Ints"]) {
        NbtType::IntArray(v) => assert_eq!(v.data, vec![0, 2, 3]),
        other => panic!("类型变了：{}", other),
    }

    // 下标越界、值解析不了、加到上限之外都要拒
    let full: Vec<i32> = (0..MAX_ARRAY_EDIT as i32).collect();
    let mut full_root = compound_of(vec![("Ints", int_array(full).to_nbt())]);
    assert!(
        apply_nbt_edits(
            &mut full_root,
            &[add_at(&["Ints", "0"], NbtKindDto::Int, "1")]
        )
        .is_err(),
        "到上限后不该再加"
    );
    for edit in [
        add_at(&["Ints", "9"], NbtKindDto::Int, "1"),
        add_at(&["Ints", "2"], NbtKindDto::Int, "x"),
        remove_at(&["Ints", "9"]),
    ] {
        let mut root = compound_of(vec![("Ints", int_array(vec![1, 2]).to_nbt())]);
        assert!(apply_nbt_edits(&mut root, &[edit]).is_err(), "应当拒绝");
    }
}

/// 增删：复合加成员、列表加元素、两处都能删，根标签删不得
#[test]
fn apply_nbt_edits_adds_and_removes() {
    let mut root = sample_nbt();
    apply_nbt_edits(
        &mut root,
        &[
            // 复合加成员
            add_at(&["Data", "New"], NbtKindDto::Int, "7"),
            // 列表追加：下标等于长度
            add_at(&["Items", "2"], NbtKindDto::Int, "5"),
            // 复合删成员、列表删元素
            remove_at(&["Data", "Name"]),
            remove_at(&["Items", "0"]),
        ],
    )
    .unwrap();

    assert_eq!(at(&mut root, &["Data", "New"]).as_int().unwrap().data, 7);
    match at(&mut root, &["Items"]) {
        NbtType::List(v) => {
            assert_eq!(v.len(), 2);
            assert_eq!(v.get_item(0).unwrap().as_int().unwrap().data, 4);
            assert_eq!(v.get_item(1).unwrap().as_int().unwrap().data, 5);
        }
        other => panic!("类型变了：{}", other),
    }
    assert!(
        nbt_at_mut(&mut root, &["Data".to_string(), "Name".to_string()]).is_err(),
        "成员应当已删掉"
    );

    // 键已存在、下标越界、类型对不上、父容器不是容器、删根标签都要拒
    for edit in [
        add_at(&["Data", "GameType"], NbtKindDto::Int, "1"),
        add_at(&["Items", "9"], NbtKindDto::Int, "1"),
        add_at(&["Items", "0"], NbtKindDto::String, "x"),
        add_at(&["Data", "GameType", "Deep"], NbtKindDto::Int, "1"),
        add_at(&["Data", "Bad"], NbtKindDto::End, "1"),
        remove_at(&["Data", "Nope"]),
        remove_at(&["Items", "9"]),
        remove_at(&[]),
    ] {
        assert!(
            apply_nbt_edits(&mut sample_nbt(), &[edit]).is_err(),
            "应当拒绝"
        );
    }
}

/// 改名：只换键名，标签本体连同子树原样搬过去
#[test]
fn apply_nbt_edits_renames_members() {
    let mut root = sample_nbt();
    apply_nbt_edits(
        &mut root,
        &[
            // 标量改名：类型与值都不动
            rename_at(&["Data", "Name"], "Title"),
            // 复合改名：整棵子树跟着走
            rename_at(&["Data"], "Info"),
            // 改成同名：不动结构，也不算错
            rename_at(&["Info", "GameType"], "GameType"),
        ],
    )
    .unwrap();

    assert_eq!(
        at(&mut root, &["Info", "Title"]).as_string().unwrap().data,
        "x"
    );
    assert_eq!(
        at(&mut root, &["Info", "GameType"]).as_int().unwrap().data,
        1
    );
    assert!(
        nbt_at_mut(&mut root, &["Data".to_string()]).is_err(),
        "旧键应当已不在"
    );

    // 新键已被占用、旧键不存在、父容器不是复合（列表元素）、改根标签都要拒
    for edit in [
        rename_at(&["Data", "GameType"], "Name"),
        rename_at(&["Data", "Nope"], "X"),
        rename_at(&["Items", "0"], "X"),
        rename_at(&[], "X"),
    ] {
        assert!(
            apply_nbt_edits(&mut sample_nbt(), &[edit]).is_err(),
            "应当拒绝"
        );
    }
}

/// 空列表由第一个元素定元素类型：加进去之后读得回来
#[test]
fn apply_nbt_edits_fills_empty_list() {
    let mut root = compound_of(vec![("Empty", {
        let obj = list(NBT_END_ORDER);
        obj.to_nbt()
    })]);

    apply_nbt_edits(
        &mut root,
        &[add_at(&["Empty", "0"], NbtKindDto::String, "hi")],
    )
    .unwrap();

    match at(&mut root, &["Empty"]) {
        NbtType::List(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v.get_item(0).unwrap().as_string().unwrap().data, "hi");
        }
        other => panic!("类型变了：{}", other),
    }
}

/// 改过标量后写回再读出来：值真的落到字节里，压缩方式沿用原文件
///
/// 走的是 [`config_set_nbt`] 的那条链路：读 → 改 → 写；`NbtFile` 的读写往返
/// 只在 mml-nbt 里各自做过，这里钉住"改过的文件还能读回来"
#[test]
fn nbt_edits_survive_round_trip() {
    use std::io::Cursor;

    use mml_nbt::nbt_file::{CompressType, NbtFile};

    let file = NbtFile::new(sample_nbt(), CompressType::GZip);
    let mut bytes = Vec::new();
    file.write(&mut bytes).unwrap();

    let mut read = NbtFile::read(&mut Cursor::new(&bytes)).unwrap();
    apply_nbt_edits(
        &mut read.nbt,
        &[
            set_at(&["Data", "GameType"], "2"),
            add_at(&["Data", "Added"], NbtKindDto::String, "加进来的"),
        ],
    )
    .unwrap();
    let mut out = Vec::new();
    read.write(&mut out).unwrap();

    // 写回的是另一份字节（压缩帧与哈希都会变），但必须读得回来
    assert_ne!(out, bytes);
    let mut back = NbtFile::read(&mut Cursor::new(&out)).unwrap();
    assert_eq!(back.compress, CompressType::GZip);
    assert_eq!(
        at(&mut back.nbt, &["Data", "GameType"])
            .as_int()
            .unwrap()
            .data,
        2
    );
    assert_eq!(
        at(&mut back.nbt, &["Data", "Added"])
            .as_string()
            .unwrap()
            .data,
        "加进来的"
    );
}

/// 来源决定基准目录：空串相对游戏目录，存档名相对该存档目录，两处都挡穿越
#[test]
fn resolve_source_switches_base() {
    let game_path = Path::new("D:/mml/inst/.minecraft");
    let saves_path = Path::new("D:/mml/inst/.minecraft/saves");

    // 实例来源：相对游戏目录
    assert_eq!(
        resolve_source_in(game_path, saves_path, "", "options.txt").unwrap(),
        game_path.join("options.txt")
    );
    // 存档来源：相对该存档目录，**不再套** `saves/<目录>/` 前缀
    assert_eq!(
        resolve_source_in(game_path, saves_path, "world", "level.dat").unwrap(),
        saves_path.join("world").join("level.dat")
    );
    assert!(resolve_source_in(game_path, saves_path, "world", "../options.txt").is_err());
    assert!(resolve_source_in(game_path, saves_path, "../world", "level.dat").is_err());
}

/// 相对路径要挡掉穿越：`..`、绝对路径、空串都不行，正反斜杠都认
#[test]
fn resolve_under_rejects_escape() {
    let base = Path::new("D:/mml/game/.minecraft");

    assert!(resolve_under(base, "options.txt").is_ok());
    assert!(resolve_under(base, "config/abc.toml").is_ok());
    // 前端可能把 Windows 的反斜杠原样传回来
    assert!(resolve_under(base, "config\\abc.toml").is_ok());

    assert!(resolve_under(base, "../gui_config.json").is_err());
    assert!(resolve_under(base, "config/../../x.txt").is_err());
    assert!(resolve_under(base, "/etc/passwd").is_err());
    assert!(resolve_under(base, "").is_err());
    assert!(resolve_under(base, "   ").is_err());
}

/// 二进制那几类要被认出来：前端据此选视图
#[test]
fn binary_extensions_are_marked() {
    for name in [
        "level.dat",
        "level.dat_old",
        "advancements.rio",
        "structure.nbt",
        "r.0.0.mca",
        "icon.png",
        "photo.JPEG",
        "datapacks/pack.zip",
        "mod.jar",
    ] {
        assert!(
            matches!(
                get_file_type(name),
                FileItemType::Nbt
                    | FileItemType::Mca
                    | FileItemType::Image
                    | FileItemType::Archive
            ),
            "{name} 应认成二进制"
        );
    }
}

/// 常见配置文件的类型要认对，认不出来的一律纯文本
#[test]
fn common_extensions_are_typed() {
    let cases = [
        ("config/a.json", FileItemType::Json),
        ("pack.mcmeta", FileItemType::Json),
        ("config/a.JSONC", FileItemType::Json),
        ("config/a.json5", FileItemType::Json5),
        ("config/a.toml", FileItemType::Toml),
        ("config/a.yml", FileItemType::Yaml),
        ("config/a.YAML", FileItemType::Yaml),
        ("config/a.properties", FileItemType::Properties),
        ("config/old.cfg", FileItemType::Properties),
        ("config/a.conf", FileItemType::Properties),
        ("config/a.ini", FileItemType::Properties),
        ("config/a.xml", FileItemType::Xml),
        ("kubejs/server.js", FileItemType::JavaScript),
        ("kubejs/server.mjs", FileItemType::JavaScript),
        ("data/x.snbt", FileItemType::Snbt),
        ("data/f.mcfunction", FileItemType::Text),
        ("README", FileItemType::Text),
        ("options.txt", FileItemType::Text),
    ];

    for (name, want) in cases {
        assert_eq!(get_file_type(name), want, "{name} 的类型不对");
    }
}

/// 文本编辑只认文本：二进制那几类必须挡掉，它们走结构视图或二进制视图
#[test]
fn text_edit_refuses_binary() {
    assert!(check_binary(Path::new("options.txt")).is_ok());
    assert!(check_binary(Path::new("config/a.toml")).is_ok());
    assert!(check_binary(Path::new("data/f.mcfunction")).is_ok());

    for name in ["level.dat", "r.0.0.mca", "icon.png", "datapacks/pack.zip"] {
        assert!(
            check_binary(Path::new(name)).is_err(),
            "{name} 不该给文本编辑"
        );
    }
}

/// 二进制分段读取：越界与超长都夹到文件范围内，不报错
#[test]
fn slice_is_clamped_to_file() {
    let dir = mml_testutil::temp_dir().join(format!("mml-config-slice-{}", Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("icon.png");
    fs::write(&file, b"0123456789").unwrap();

    let whole = read_slice(&file, 0, 4096).unwrap();
    assert_eq!((whole.offset, whole.length, whole.total), (0, 10, 10));
    assert_eq!(whole.data, hash_helper::gen_base64_bytes(b"0123456789"));

    // 中间一段
    let mid = read_slice(&file, 4, 3).unwrap();
    assert_eq!((mid.offset, mid.length), (4, 3));
    assert_eq!(mid.data, hash_helper::gen_base64_bytes(b"456"));

    // 尾巴不足一页：给剩下的
    let tail = read_slice(&file, 8, 4096).unwrap();
    assert_eq!(tail.length, 2);
    assert_eq!(tail.data, hash_helper::gen_base64_bytes(b"89"));

    // 越界：给空段，不是错误
    let over = read_slice(&file, 100, 16).unwrap();
    assert_eq!((over.offset, over.length), (10, 0));

    // 一次要得再多也只给一页
    fs::write(&file, vec![7u8; MAX_SLICE_SIZE as usize + 16]).unwrap();
    let big = read_slice(&file, 0, u32::MAX).unwrap();
    assert_eq!(big.length, MAX_SLICE_SIZE);
    assert_eq!(big.total, u64::from(MAX_SLICE_SIZE) + 16);

    let _ = fs::remove_dir_all(&dir);
}

/// NBT 转树：复合按键名排序、数组只报个数、列表记下标、标量带值
#[test]
fn nbt_tree_is_converted() {
    use mml_nbt::nbt_types::{NbtByte, NbtByteArray, NbtCompound, NbtInt, NbtList, NbtString};

    let mut inner = NbtCompound::new();
    inner
        .data
        .insert(String::from("z"), NbtInt::new(2).to_nbt());
    inner.data.insert(
        String::from("a"),
        NbtString::new(String::from("hi")).to_nbt(),
    );

    let mut list = NbtList::new(3);
    assert!(list.add_item(NbtInt::new(7).to_nbt()));
    assert!(list.add_item(NbtInt::new(8).to_nbt()));

    let mut root = NbtCompound::new();
    root.data.insert(String::from("nums"), list.to_nbt());
    root.data.insert(String::from("nested"), inner.to_nbt());
    root.data
        .insert(String::from("b"), NbtByte::new(5).to_nbt());
    root.data.insert(
        String::from("arr"),
        NbtByteArray::new(vec![1, 2, 3]).to_nbt(),
    );

    let node = nbt_node(String::new(), &root.to_nbt());
    assert_eq!(node.kind, NbtKindDto::Compound);
    assert!(node.name.is_empty());

    // 成员按键名排序：arr / b / nested / nums
    let names: Vec<&str> = node.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["arr", "b", "nested", "nums"]);

    assert_eq!(node.children[0].kind, NbtKindDto::ByteArray);
    assert_eq!(node.children[0].length, 3);
    assert!(node.children[0].value.is_empty());
    // 短数组展开成元素行，元素类型是数组的元素类型、名字是下标
    let items = &node.children[0].items;
    assert_eq!(items.len(), 3);
    assert_eq!(items[1].name, "1");
    assert_eq!(items[1].kind, NbtKindDto::Byte);
    assert_eq!(items[1].value, "2");

    assert_eq!(node.children[1].kind, NbtKindDto::Byte);
    assert_eq!(node.children[1].value, "5");

    // 内层复合也排序：a / z
    let nested = &node.children[2];
    let inner_names: Vec<&str> = nested.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(inner_names, ["a", "z"]);
    assert_eq!(nested.children[0].value, "hi");

    // 列表元素按下标命名，类型与值都在
    let nums = &node.children[3];
    assert_eq!(nums.kind, NbtKindDto::List);
    assert_eq!(nums.length, 2);
    assert_eq!(nums.items[0].name, "0");
    assert_eq!(nums.items[0].kind, NbtKindDto::Int);
    assert_eq!(nums.items[0].value, "7");
    assert_eq!(nums.items[1].value, "8");
}
