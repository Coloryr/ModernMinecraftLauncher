//! 光影包读取测试。
//!
//! 光影包样本不提交进 git，测试运行时通过 Modrinth API 解析各项目的
//! 最新版本并下载到缓存目录。网络不可用时跳过，打印提示而不 fail。

mod common;

use mml_game::game_shaderpacks;

/// 从 Modrinth 下载多个常见光影包并解析名称与说明
#[test]
fn read_shaderpacks() {
    let mut downloaded = 0;
    let mut parsed = 0;

    for slug in common::SHADERPACK_SLUGS {
        let Some(path) = common::download_latest(slug) else {
            eprintln!("[跳过] 光影包 {slug} 下载失败（网络不可用？）");
            continue;
        };
        downloaded += 1;

        match game_shaderpacks::read_shaderpacks(&path) {
            Ok(obj) => {
                parsed += 1;
                println!("{slug}: name = {}, comment = {}", obj.name, obj.comment);
            }
            Err(err) => {
                // 下载成功但解析失败：多半是样本本身不含语言文件，仅提示不中断
                eprintln!("[警告] 光影包 {slug} 解析失败：{err}");
            }
        }
    }

    if downloaded == 0 {
        eprintln!("全部光影包样本均无法下载（需要网络），测试跳过");
        return;
    }
    assert!(parsed > 0, "下载成功但未能解析任何光影包");
}
