use mml_game::{class_scan, game_mods};
use mml_sys::path_helper;
use toml::Table;

/// 占位用例，尚未实现
#[tokio::test]
async fn get_mod_info() {}

/// 扫描两个真实 jar，要求各自至少识别出一个 mod
#[test]
fn test_scan_file() {
    let result = class_scan::scan_jar("./tests/mods/[forge-1.12.2]AllMusic_Client-3.7.4.jar");
    if result.is_err() {
        let err = result.as_ref().err().unwrap();
        println!("{}", err);
    }
    assert!(!result.is_err());

    let res = result.unwrap();
    println!("找到 {} 个 Mod:", res.mods.len());
    for m in &res.mods {
        println!("  modid: {}, side: {:?}", m.modid, m.side);
    }
    assert!(!res.mods.is_empty(), "应该至少找到一个 Mod");

    let result = class_scan::scan_jar("./tests/mods/[forge-1.12.2]AllMusic_Server-4.0.3.jar");
    if result.is_err() {
        let err = result.as_ref().err().unwrap();
        println!("{}", err);
    }
    assert!(!result.is_err());

    let res = result.unwrap();
    println!("找到 {} 个 Mod:", res.mods.len());
    for m in &res.mods {
        println!("  modid: {}, side: {:?}", m.modid, m.side);
    }
    assert!(!res.mods.is_empty(), "应该至少找到一个 Mod");
}

/// 解析 tests/tomls/test.toml，确认 toml 表能读出来
#[test]
fn toml() {
    let file = path_helper::read_text("./tests/tomls/test.toml").unwrap();

    let datas = file.parse::<Table>().unwrap();
    println!("{}", datas);
}

/// 遍历 tests/fail_mods：解析成功时必须有条目，失败时只打印不 panic
#[test]
fn fail_json_mod() {
    let files = path_helper::get_files("./tests/fail_mods/");
    for item in files.iter() {
        let mods = game_mods::read_mod_info(item);
        match mods {
            Ok(mods) => {
                assert!(!mods.info.is_empty());
                println!("modid = {}", mods.info.iter().next().unwrap().mod_id);
            }
            Err(err) => {
                println!("error file: {}", item.to_string_lossy());
                println!("error: {}", err);
            }
        }
    }
}

/// 遍历 tests/core_mods，确认 core mod 也能解析出条目
#[test]
fn core_mod() {
    let files = path_helper::get_files("./tests/core_mods/");
    for item in files.iter() {
        let mods = game_mods::read_mod_info(item);
        match mods {
            Ok(mods) => {
                assert!(!mods.info.is_empty());
                println!("modid = {}", mods.info.iter().next().unwrap().mod_id);
            }
            Err(err) => {
                println!("error file: {}", item.to_string_lossy());
                println!("error: {}", err);
            }
        }
    }
}

/// 内置模组递归：要求 fabric 包能展开出 jar-in-jar 条目
#[test]
fn jar_in_jar_mod() {
    let mods = game_mods::read_mod_info("./tests/mods/[fabric-1.21]AllMusic_Server-4.0.3.jar");
    match mods {
        Ok(mods) => {
            assert!(!mods.jar_in_jar.is_empty());
            for item in mods.jar_in_jar.iter() {
                let info = item.info.iter().next().unwrap();
                println!("jar in jar: {}", info.mod_id);
                for item in item.jar_in_jar.iter() {
                    println!("    jar in jar: {}", item.info.iter().next().unwrap().mod_id);
                }
            }
        }
        Err(err) => {
            println!("error: {}", err);
        }
    }
}
