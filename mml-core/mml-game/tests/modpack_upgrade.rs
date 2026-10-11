//! 整合包升级测试，入口 `upgrade_modpack`：**差异计算** + manifest 写回
//!
//! 分工：
//! - 差异计算走 `mml_game::modpack::diff` 的**纯函数**，无 IO、无网络，无网也能全绿；
//! - manifest 写回与端到端用例在文件末尾，走 `network_available()` 门控，无网自动跳过。
//!
//! 这里固定**现有判定规则**的行为：四条路径统一成"**根据新旧清单做两向比对**"，
//! 即新有旧无→下载、旧有新无→删除、同键但版本或哈希变→又下又删。

use std::path::PathBuf;

use mml_game::curseforge::pack_obj::{CurseForgePackObj, FilesObj};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher_path::instance_path::OnlineInfoList;
use mml_game::modpack::{diff, manifest};
use mml_game::modrinth::pack_obj::ModrinthPackFileObj;
use mml_net::modrinth_api::version_obj::HasheObj;

// ===================== 构造器 =====================

/// CurseForge manifest 里的一个文件项，`required` 不参与比对
fn cf(project_id: u64, file_id: u64) -> FilesObj {
    FilesObj {
        project_id,
        file_id,
        required: true,
    }
}

/// Modrinth 索引里的一个文件项，只有 `sha1` 参与比对
fn mr(path: &str, sha1: &str) -> ModrinthPackFileObj {
    ModrinthPackFileObj {
        path: path.to_string(),
        hashes: HasheObj {
            sha1: sha1.to_string(),
            sha512: String::new(),
        },
        ..Default::default()
    }
}

/// 在线信息项，比对只看 `modid` / `fileid` / `sha1`
fn online(modid: &str, fileid: &str, sha1: &str) -> OnlineInfoObj {
    OnlineInfoObj {
        modid: modid.to_string(),
        fileid: fileid.to_string(),
        sha1: sha1.to_string(),
        ..Default::default()
    }
}

/// 在线信息表：`mod_id` 到信息的映射
fn online_map(items: &[OnlineInfoObj]) -> OnlineInfoList {
    items.iter().map(|i| (i.modid.clone(), i.clone())).collect()
}

// ===================== 结果归一化，断言与顺序无关 =====================

/// 把文件项归一成排序后的 `(project_id, file_id)` 对
fn cf_pairs(items: &[&FilesObj]) -> Vec<(u64, u64)> {
    let mut v: Vec<(u64, u64)> = items.iter().map(|f| (f.project_id, f.file_id)).collect();
    v.sort_unstable();
    v
}

/// 把 Modrinth 文件项归一成排序后的路径列表
fn mr_paths(items: &[&ModrinthPackFileObj]) -> Vec<String> {
    let mut v: Vec<String> = items.iter().map(|f| f.path.clone()).collect();
    v.sort();
    v
}

/// 把在线信息归一成排序后的 `(modid, fileid)` 对
fn online_pairs(items: &[OnlineInfoObj]) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = items
        .iter()
        .map(|i| (i.modid.clone(), i.fileid.clone()))
        .collect();
    v.sort();
    v
}

// ===================== CF：有旧 manifest，按 project_id 配对 =====================

/// 同 `project_id`、`file_id` 变 → 新版进"要下载"、旧版进"要删除"
#[test]
fn cf_file_id_changed_is_add_and_remove() {
    let new = [cf(1, 2)];
    let old = [cf(1, 1)];
    let d = diff::diff_curseforge_files(&new, &old);
    assert_eq!(cf_pairs(&d.add), vec![(1, 2)]);
    assert_eq!(cf_pairs(&d.remove), vec![(1, 1)]);
}

/// 只在新清单 → 新增
#[test]
fn cf_only_in_new_is_add() {
    let new = [cf(1, 1), cf(2, 2)];
    let old = [cf(1, 1)];
    let d = diff::diff_curseforge_files(&new, &old);
    assert_eq!(cf_pairs(&d.add), vec![(2, 2)]);
    assert!(d.remove.is_empty(), "旧清单里的项都配上了，不该有删除");
}

/// 只在旧清单 → 删除
#[test]
fn cf_only_in_old_is_remove() {
    let new = [cf(1, 1)];
    let old = [cf(1, 1), cf(3, 3)];
    let d = diff::diff_curseforge_files(&new, &old);
    assert!(d.add.is_empty(), "没有新增/变更");
    assert_eq!(cf_pairs(&d.remove), vec![(3, 3)]);
}

/// `project_id` 与 `file_id` 都不变 → 两个列表都不进，不重复下载
#[test]
fn cf_unchanged_is_in_neither() {
    let new = [cf(1, 1), cf(2, 2)];
    let old = [cf(2, 2), cf(1, 1)];
    let d = diff::diff_curseforge_files(&new, &old);
    assert!(d.add.is_empty(), "都不变时不该下载");
    assert!(d.remove.is_empty(), "都不变时不该删除");
}

/// 边界：新空 / 旧空 / 双空清单都不应 panic
#[test]
fn cf_empty_lists_do_not_panic() {
    let empty: [FilesObj; 0] = [];

    let d = diff::diff_curseforge_files(&empty, &empty);
    assert!(d.add.is_empty() && d.remove.is_empty());

    // 新空、旧有 → 全删
    let old = [cf(1, 1)];
    let d = diff::diff_curseforge_files(&empty, &old);
    assert!(d.add.is_empty());
    assert_eq!(cf_pairs(&d.remove), vec![(1, 1)]);

    // 新有、旧空 → 全新增
    let new = [cf(1, 1)];
    let d = diff::diff_curseforge_files(&new, &empty);
    assert_eq!(cf_pairs(&d.add), vec![(1, 1)]);
    assert!(d.remove.is_empty());
}

/// 同一 `project_id` 在新清单里出现两次：旧项只被**一个**新项配对
///
/// 配对时必须跳过已配对的旧项：新 `[1/1, 1/2]` 对旧 `[1/1]` 时，第二个新项 (1,2)
/// 没有可配的旧项，算新增；旧项 (1,1) 仍在新清单里，不能删。
#[test]
fn cf_duplicate_project_id_pairs_each_old_item_once() {
    let new = [cf(1, 1), cf(1, 2)];
    let old = [cf(1, 1)];
    let d = diff::diff_curseforge_files(&new, &old);
    assert_eq!(cf_pairs(&d.add), vec![(1, 2)]);
    assert!(
        d.remove.is_empty(),
        "旧项 (1,1) 仍在新清单里，不该被删（改动前会误删）"
    );
}

/// 同上，旧清单里还有另一个同 `project_id` 的项：各自配对，只删真正消失的那个
///
/// 新 `[1/1, 1/2]` 对旧 `[1/1, 1/3]`：(1,1) 与 (1,1) 相同不动，(1,2) 与 (1,3) 配对后
/// 算变更，(1,3) 进删除。若不跳过已配对项，(1,2) 会重复配对 (1,1) 而误删仍在用的文件。
#[test]
fn cf_duplicate_project_id_only_removes_the_vanished_file() {
    let new = [cf(1, 1), cf(1, 2)];
    let old = [cf(1, 1), cf(1, 3)];
    let d = diff::diff_curseforge_files(&new, &old);
    assert_eq!(cf_pairs(&d.add), vec![(1, 2)]);
    assert_eq!(
        cf_pairs(&d.remove),
        vec![(1, 3)],
        "只该删真正从新清单里消失的 (1,3)"
    );
}

// ===================== Modrinth：有旧 manifest，按包内路径配对 =====================

/// `sha1` 变 → 新文件进"要下载"、旧文件进"要删除"
#[test]
fn mr_sha1_changed_is_add_and_remove() {
    let new = [mr("mods/a.jar", "sha-new")];
    let old = [mr("mods/a.jar", "sha-old")];
    let d = diff::diff_modrinth_files(&new, &old);
    assert_eq!(mr_paths(&d.add), vec!["mods/a.jar"]);
    assert_eq!(mr_paths(&d.remove), vec!["mods/a.jar"]);
}

/// 只在新清单 → 新增；只在旧清单 → 删除
#[test]
fn mr_only_in_one_side() {
    let new = [mr("mods/a.jar", "sha-a"), mr("mods/b.jar", "sha-b")];
    let old = [mr("mods/a.jar", "sha-a"), mr("mods/c.jar", "sha-c")];
    let d = diff::diff_modrinth_files(&new, &old);
    assert_eq!(mr_paths(&d.add), vec!["mods/b.jar"]);
    assert_eq!(mr_paths(&d.remove), vec!["mods/c.jar"]);
}

/// 路径相同且 `sha1` 相同 → 两个列表都不进，才算真正不动
#[test]
fn mr_same_path_and_sha1_is_in_neither() {
    let new = [mr("mods/a.jar", "sha-a")];
    let old = [mr("mods/a.jar", "sha-a")];
    let d = diff::diff_modrinth_files(&new, &old);
    assert!(d.add.is_empty(), "路径与 sha1 都没变，不该下载");
    assert!(d.remove.is_empty(), "路径与 sha1 都没变，不该删除");
}

/// 改名：同 `sha1` 不同路径时，新路径下载、旧路径删除
///
/// 配对键必须是**包内路径**：按 `sha1` 抵消会把改名当成"同一文件"，新路径永远不会被创建。
#[test]
fn mr_renamed_path_is_downloaded_and_old_path_removed() {
    let new = [mr("mods/a.jar", "sha-a")];
    let old = [mr("mods/a-renamed.jar", "sha-a")];
    let d = diff::diff_modrinth_files(&new, &old);
    assert_eq!(mr_paths(&d.add), vec!["mods/a.jar"]);
    assert_eq!(mr_paths(&d.remove), vec!["mods/a-renamed.jar"]);
}

/// 边界：空清单不应 panic
#[test]
fn mr_empty_lists_do_not_panic() {
    let empty: [ModrinthPackFileObj; 0] = [];

    let d = diff::diff_modrinth_files(&empty, &empty);
    assert!(d.add.is_empty() && d.remove.is_empty());

    let old = [mr("mods/a.jar", "sha-a")];
    let d = diff::diff_modrinth_files(&empty, &old);
    assert_eq!(mr_paths(&d.remove), vec!["mods/a.jar"]);

    let new = [mr("mods/a.jar", "sha-a")];
    let d = diff::diff_modrinth_files(&new, &empty);
    assert_eq!(mr_paths(&d.add), vec!["mods/a.jar"]);
}

/// 同一个 `sha1` 出现在两个不同路径：两个都要下载，旧路径要删除
///
/// 按路径配对后两个新路径在旧清单里都不存在，都进 `add`；旧路径进 `remove`。
/// 按 `sha1` 配对时只会下载其中一个，另一个路径永远不会被创建。
#[test]
fn mr_same_sha1_two_paths_both_downloaded() {
    let new = [mr("mods/a.jar", "sha-x"), mr("mods/b.jar", "sha-x")];
    let old = [mr("mods/c.jar", "sha-x")];
    let d = diff::diff_modrinth_files(&new, &old);
    assert_eq!(mr_paths(&d.add), vec!["mods/a.jar", "mods/b.jar"]);
    assert_eq!(mr_paths(&d.remove), vec!["mods/c.jar"]);
}

// ===================== CF：无旧 manifest，按 mod_id 比在线信息 =====================

/// 同 `mod_id` 但 `fileid` 不同 → 变更：新增进 add、旧进 remove
#[test]
fn cf_online_changed_is_add_and_remove() {
    let new = online_map(&[online("100", "200", "sha-new")]);
    let old = online_map(&[online("100", "100", "sha-old")]);
    let d = diff::diff_curseforge_online(&new, &old);
    assert_eq!(online_pairs(&d.add), vec![("100".into(), "200".into())]);
    assert_eq!(online_pairs(&d.remove), vec![("100".into(), "100".into())]);
}

/// 同 `mod_id`、`fileid` 相同但 `sha1` 变 → 也算变更
#[test]
fn cf_online_sha1_change_is_a_change() {
    let new = online_map(&[online("100", "100", "sha-new")]);
    let old = online_map(&[online("100", "100", "sha-old")]);
    let d = diff::diff_curseforge_online(&new, &old);
    assert_eq!(online_pairs(&d.add), vec![("100".into(), "100".into())]);
    assert_eq!(online_pairs(&d.remove), vec![("100".into(), "100".into())]);
}

/// 新有旧无 → 新增；**旧有新无 → 删除**
///
/// `online_info` 只由整合包路径写入，`update_online` 只清理已消失的记录、
/// `copy_to_other` 只复制，所以删它记录的文件不会误伤用户自己装的模组。
#[test]
fn cf_online_old_only_is_removed() {
    let new = online_map(&[online("100", "1", "sha-a"), online("200", "2", "sha-b")]);
    let old = online_map(&[online("100", "1", "sha-a"), online("300", "3", "sha-c")]);
    let d = diff::diff_curseforge_online(&new, &old);
    assert_eq!(online_pairs(&d.add), vec![("200".into(), "2".into())]);
    assert_eq!(
        online_pairs(&d.remove),
        vec![("300".into(), "3".into())],
        "新清单里没有 300 → 要删除"
    );
}

/// 完全相同 → 两个列表都不进；空表不 panic；"新空旧有" → 全删
#[test]
fn cf_online_identical_and_empty() {
    let same = online_map(&[online("100", "1", "sha-a")]);
    let d = diff::diff_curseforge_online(&same, &same);
    assert!(d.add.is_empty() && d.remove.is_empty());

    let empty: OnlineInfoList = OnlineInfoList::new();
    let d = diff::diff_curseforge_online(&empty, &empty);
    assert!(d.add.is_empty() && d.remove.is_empty());

    // 新清单为空 → 已安装的全都要删
    let d = diff::diff_curseforge_online(&empty, &same);
    assert!(d.add.is_empty());
    assert_eq!(online_pairs(&d.remove), vec![("100".into(), "1".into())]);

    let d = diff::diff_curseforge_online(&same, &empty);
    assert_eq!(online_pairs(&d.add), vec![("100".into(), "1".into())]);
    assert!(d.remove.is_empty());
}

// ===================== Modrinth：无旧 manifest，按 mod_id 比在线信息 =====================

/// 同 `mod_id` 但 `fileid` 不同 → 变更
#[test]
fn mr_online_changed_is_add_and_remove() {
    let new = online_map(&[online("aaa", "v2", "sha-new")]);
    let old = online_map(&[online("aaa", "v1", "sha-old")]);
    let d = diff::diff_modrinth_online(&new, &old);
    assert_eq!(online_pairs(&d.add), vec![("aaa".into(), "v2".into())]);
    assert_eq!(online_pairs(&d.remove), vec![("aaa".into(), "v1".into())]);
}

/// 新有旧无 → 新增；**旧有新无 → 删除**，与 CF 的在线信息路径一致
///
/// 新清单里没有的已安装模组也要进 `remove`，不能只处理变更项。
#[test]
fn mr_online_old_only_is_removed() {
    let new = online_map(&[online("aaa", "v1", "sha-a"), online("bbb", "v1", "sha-b")]);
    let old = online_map(&[online("aaa", "v1", "sha-a"), online("ccc", "v1", "sha-c")]);
    let d = diff::diff_modrinth_online(&new, &old);
    assert_eq!(online_pairs(&d.add), vec![("bbb".into(), "v1".into())]);
    assert_eq!(
        online_pairs(&d.remove),
        vec![("ccc".into(), "v1".into())],
        "新清单里没有 ccc → 要删除"
    );
}

/// 完全相同 → 两个列表都不进；空表不 panic；"新空旧有" → 全删
#[test]
fn mr_online_identical_and_empty() {
    let same = online_map(&[online("aaa", "v1", "sha-a")]);
    let d = diff::diff_modrinth_online(&same, &same);
    assert!(d.add.is_empty() && d.remove.is_empty());

    let empty: OnlineInfoList = OnlineInfoList::new();
    let d = diff::diff_modrinth_online(&empty, &empty);
    assert!(d.add.is_empty() && d.remove.is_empty());

    // 新清单为空 → 已安装的全都要删
    let d = diff::diff_modrinth_online(&empty, &same);
    assert!(d.add.is_empty());
    assert_eq!(online_pairs(&d.remove), vec![("aaa".into(), "v1".into())]);

    let d = diff::diff_modrinth_online(&same, &empty);
    assert_eq!(online_pairs(&d.add), vec![("aaa".into(), "v1".into())]);
    assert!(d.remove.is_empty());
}

// ===================== manifest 写回，离线 =====================

/// 每个用例各用各的子目录，见 AGENTS.md §6：落盘是异步的，共用目录会互相踩
fn case_dir(case: &str) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml-modpack-upgrade-{case}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 升级成功后写回：**覆盖**旧清单，读回来是新内容，下次升级据此比对
#[test]
fn manifest_write_then_read_back_overwrites() {
    let path = case_dir("manifest_overwrite").join("manifest.json");

    let old = CurseForgePackObj {
        files: vec![cf(1, 1)],
        ..Default::default()
    };
    manifest::write_manifest(&path, &old).expect("写回旧清单");
    let read: CurseForgePackObj = manifest::read_manifest(&path).expect("写回后应能读回");
    assert_eq!(cf_pairs(&read.files.iter().collect::<Vec<_>>()), vec![(1, 1)]);

    // 覆盖：同一路径再写新清单
    let new = CurseForgePackObj {
        files: vec![cf(1, 2), cf(2, 2)],
        ..Default::default()
    };
    manifest::write_manifest(&path, &new).expect("覆盖写回新清单");
    let read: CurseForgePackObj = manifest::read_manifest(&path).expect("覆盖后应能读回");
    let mut pairs: Vec<(u64, u64)> = read.files.iter().map(|f| (f.project_id, f.file_id)).collect();
    pairs.sort_unstable();
    assert_eq!(pairs, vec![(1, 2), (2, 2)], "旧内容应被完全覆盖");
}

/// 旧清单损坏 / 缺失 / 不是合法 JSON → 一律 `None`，调用方落到无旧 manifest 分支，不 panic
#[test]
fn manifest_read_missing_or_corrupt_is_none() {
    let dir = case_dir("manifest_corrupt");

    // 文件不存在
    assert!(manifest::read_manifest::<CurseForgePackObj>(dir.join("nope.json")).is_none());

    // 空文件
    let empty = dir.join("empty.json");
    std::fs::write(&empty, "").unwrap();
    assert!(manifest::read_manifest::<CurseForgePackObj>(&empty).is_none());

    // 截断 / 非法 JSON
    let broken = dir.join("broken.json");
    std::fs::write(&broken, "{\"files\": [").unwrap();
    assert!(manifest::read_manifest::<CurseForgePackObj>(&broken).is_none());

    // 合法 JSON 但结构不符
    let wrong = dir.join("wrong.json");
    std::fs::write(&wrong, "[1, 2, 3]").unwrap();
    assert!(manifest::read_manifest::<CurseForgePackObj>(&wrong).is_none());
}

/// 比对过程**不碰磁盘**：跑完四条路径的差异计算后，manifest 文件一个字节都没变
///
/// 这是"取消 / 比对失败时旧 manifest 不被写坏"的离线可测部分：写回在两个 worker 里都是
/// `check_upgrade` 的**最后一步**，前面任何 `?` 提前返回都不会走到写回。
#[test]
fn diff_does_not_touch_manifest_files() {
    let path = case_dir("diff_readonly").join("manifest.json");
    let pack = CurseForgePackObj {
        files: vec![cf(1, 1)],
        ..Default::default()
    };
    manifest::write_manifest(&path, &pack).expect("先写一份旧清单");
    let before = std::fs::read(&path).expect("读旧清单字节");

    // 四条路径都跑一遍，结果无所谓，关键是别动文件
    let _ = diff::diff_curseforge_files(&[cf(1, 2)], &[cf(1, 1)]);
    let _ = diff::diff_modrinth_files(&[mr("mods/a.jar", "x")], &[mr("mods/b.jar", "y")]);
    let _ = diff::diff_curseforge_online(
        &online_map(&[online("100", "2", "sha-new")]),
        &online_map(&[online("100", "1", "sha-old")]),
    );
    let _ = diff::diff_modrinth_online(
        &online_map(&[online("aaa", "v2", "sha-new")]),
        &online_map(&[online("aaa", "v1", "sha-old")]),
    );

    let after = std::fs::read(&path).expect("再读旧清单字节");
    assert_eq!(before, after, "差异计算不应改动 manifest 文件");
}

// ===================== 端到端，在线，走门控 =====================
//
// 用 Modrinth 上的**真包** `optimized-fps`，NeoForge，17 个文件约 12MB：
// 装旧版 → 升级到新版 → 断言"磁盘上的文件集合 == 新版清单"，该下的下了、该删的删了，
// 并断言实例元数据没被重建：名称 / 图标 / 分组。无网或找不到"两个文件集合不同的版本"
// 时直接跳过并打印提示，不 fail。
//
// 版本号不硬编码：Modrinth 返回的是"最新在前"的列表，这里取最新版 + 第一个与它文件集合
// 不同的版本 —— 断言的是**不变量**：升级后磁盘 == 新清单，不依赖具体版本号的字段。

mod common;

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, Once, RwLock};

use mml_base::archives::BaseArchive;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::modpack::BaseModPackWorker;
use mml_game::modrinth::pack_obj::ModrinthPackObj;
use mml_net::modrinth_api::version_obj::ModrinthVersionObj;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// 端到端用的项目 slug，NeoForge 小包，文件少下载快
const E2E_SLUG: &str = "optimized-fps";

/// 初始化链，与 mml_core::init 相同；每进程只跑一次
fn ensure_init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir = mml_testutil::temp_dir().join(format!(
            "mml-modpack-upgrade-run-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();

        // 可选测试代理 `MML_TEST_PROXY=ip:port`：reqwest 的 Auto 模式不读 HTTP_PROXY
        // 等环境变量，必须写进配置，与 `real_pack_download.rs` 同一写法
        if let Ok(proxy) = std::env::var("MML_TEST_PROXY") {
            let (ip, port) = proxy
                .split_once(':')
                .expect("MML_TEST_PROXY 格式应为 ip:port");
            use mml_config::config_obj::ProxyState;
            {
                let mut config = mml_config::write_config();
                config.http.work_proxy = ProxyState::User;
                config.http.login_proxy = ProxyState::User;
                config.http.proxy_ip = ip.to_string();
                config.http.proxy_port = port.parse().expect("MML_TEST_PROXY 端口不合法");
            }
            eprintln!("[e2e][代理] 走显式代理 {proxy}");
        }

        mml_config::config_save::start();
        mml_game::init(mml_base::get_base_dir()).unwrap();
        mml_net::init();

        // CurseForge API key，与 GUI 运行时同源：没设就跳过 CF 用例
        if let Ok(key) = std::env::var("MML_CF_API_KEY") {
            mml_net::curseforge_api::set_key(&key);
        }

        mml_downloader::init(&dir).unwrap();
        mml_downloader::set_gui_handel(Box::new(TestDownloader));
        mml_downloader::start();
    });
}

/// 内核那几个系统是进程级单例，见 AGENTS.md §6；端到端用例串行执行
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 端到端用的下载回调，把进度打到标准错误
struct TestDownloader;

impl mml_downloader::IDownloadGui for TestDownloader {
    /// 单文件进度回调，每 25% 打一行
    fn update(&self, thread: u32, file: &Arc<mml_downloader::download_item::DownloadItem>) {
        // 端到端要真下 17+ 个模组：不打日志会看着像卡住
        let pro = file.progress() as u64;
        if pro > 0 && pro % 25 == 0 {
            eprintln!("[e2e][下载] 线程 {thread} {} {pro}%", file.base.name);
        }
    }

    /// 任务级状态回调，新建 / 结束 / 进度各打一行
    fn update_task(&self, state: mml_downloader::DownloadTaskState) {
        match state {
            mml_downloader::DownloadTaskState::AddTask(id) => {
                eprintln!("[e2e][下载] 新任务 {id}")
            }
            mml_downloader::DownloadTaskState::RemoveTask(id) => {
                eprintln!("[e2e][下载] 任务结束 {id}")
            }
            mml_downloader::DownloadTaskState::UpdateTask(obj) => {
                eprintln!("[e2e][下载] 任务 {} 进度 {:.1}%", obj.id, obj.progress)
            }
        }
    }
}

/// 网络探测，与 `modpack_install.rs` 同一口径
async fn network_available() -> bool {
    let ok = mml_net::get_work_client()
        .get_bytes("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .await
        .map(|data| !data.is_empty())
        .unwrap_or(false);
    if !ok {
        eprintln!("跳过: 网络不可用（无法下载 Mojang 版本清单）");
    }
    ok
}

/// 从 .mrpack 里读出 `modrinth.index.json`
fn read_index(mrpack: &Path) -> ModrinthPackObj {
    let file = std::fs::File::open(mrpack).expect("打开 mrpack");
    let mut zip = zip::ZipArchive::new(file).expect("读 zip");
    let mut entry = zip
        .by_name(mml_names::names::MODRINTH_FILE)
        .expect("包内应有 modrinth.index.json");
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("读清单文本");
    mml_base::serialize_tools::json_from_bytes(text.as_bytes()).expect("解析 modrinth.index.json")
}

/// 清单里的文件路径归一成排序后的列表，路径相对游戏目录
fn index_paths(index: &ModrinthPackObj) -> Vec<String> {
    let mut v: Vec<String> = index.files.iter().map(|f| f.path.clone()).collect();
    v.sort();
    v
}

/// 读出 .mrpack 里某个条目的**原始字节**，用于把包里的清单原样写进实例
fn read_zip_entry(mrpack: &Path, name: &str) -> Vec<u8> {
    let file = std::fs::File::open(mrpack).expect("打开 mrpack");
    let mut zip = zip::ZipArchive::new(file).expect("读 zip");
    let mut entry = zip.by_name(name).expect("包内应有该条目");
    let mut data = Vec::new();
    entry.read_to_end(&mut data).expect("读条目内容");
    data
}

/// 读出 .mrpack 里某个前缀下的条目：返回 `(去掉前缀的相对路径, 内容)`
///
/// 用来断言 overrides 是否解压与覆盖正确：`overrides/xxx` 落到游戏目录的 `xxx`
fn zip_entries_under(mrpack: &Path, prefix: &str) -> Vec<(String, Vec<u8>)> {
    let file = std::fs::File::open(mrpack).expect("打开 mrpack");
    let mut zip = zip::ZipArchive::new(file).expect("读 zip");
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).expect("取条目");
        let name = entry.name().expect("取条目名").to_string();
        let Some(rel) = name.strip_prefix(prefix) else {
            continue;
        };
        if rel.is_empty() || rel.ends_with('/') {
            continue;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).expect("读条目内容");
        out.push((rel.to_string(), data));
    }
    out
}

/// 清单里"路径 → sha1"，用于判断两个版本的文件集合是否真的不同
fn index_hashes(index: &ModrinthPackObj) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = index
        .files
        .iter()
        .map(|f| (f.path.clone(), f.hashes.sha1.clone()))
        .collect();
    v.sort();
    v
}

/// 挑一对"文件集合确实不同"的版本：最新版 + 第一个与它不同的旧版
///
/// 返回 `(旧版本对象, 旧包本地路径, 旧清单, 新版本对象, 新包本地路径, 新清单)`；
/// 网络不可用或找不到不同的版本时返回 `None`，调用方跳过。
async fn pick_version_pair() -> Option<(
    ModrinthVersionObj,
    std::path::PathBuf,
    ModrinthPackObj,
    ModrinthVersionObj,
    std::path::PathBuf,
    ModrinthPackObj,
)> {
    let versions = common::project_versions_async(E2E_SLUG).await?;
    // `ModrinthVersionObj` 没有 `Clone`：全程用所有权传，别 clone
    let mut usable: Vec<ModrinthVersionObj> = versions
        .into_iter()
        .filter(|v| v.files.iter().any(|f| f.primary || !f.url.is_empty()))
        .collect();
    if usable.is_empty() {
        eprintln!("跳过: {E2E_SLUG} 没有可下载的版本");
        return None;
    }

    // 列表是"最新在前"：第一个当新版
    let new_ver = usable.remove(0);
    let new_pack = common::download_version_async(&new_ver).await?;
    let new_index = read_index(&new_pack);
    let new_hashes = index_hashes(&new_index);

    // 往后找：文件集合与新版本不同的那个当旧版，集合按路径与 sha1 比
    for candidate in usable.into_iter().take(4) {
        let Some(pack) = common::download_version_async(&candidate).await else {
            continue;
        };
        let index = read_index(&pack);
        if index_hashes(&index) != new_hashes {
            return Some((candidate, pack, index, new_ver, new_pack, new_index));
        }
    }

    eprintln!("跳过: {E2E_SLUG} 的候选版本里找不到文件集合不同的两个版本");
    None
}

/// 端到端：装旧版 → 升级新版 → 磁盘文件集合 == 新版清单，且实例元数据不被重建
///
/// **这也是一个回归用例**：升级时 `extract` 若把包根清单写进实例 base 目录，就会覆盖
/// `check_upgrade` 要读的"旧清单"，差异算成空，**既不下载也不删除**。断言 ① 抓的就是这个。
///
/// **默认不跑**：要真下载整合包引用的全部模组，耗时数分钟。显式跑：
/// `cargo test -p mml-game --test modpack_upgrade -- --ignored --nocapture`
#[ignore = "在线真包测试：会下载整合包与全部模组，耗时数分钟；用 -- --ignored 显式跑"]
#[tokio::test]
async fn upgrade_modpack_end_to_end_matches_new_manifest() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if !network_available().await {
        return;
    }
    let Some((old_ver, old_pack, old_index, new_ver, new_pack, new_index)) =
        pick_version_pair().await
    else {
        return;
    };
    let expected = diff::diff_modrinth_files(&new_index.files, &old_index.files);
    eprintln!(
        "[e2e] {}：{}（{} 个文件）→ {}（{} 个文件）；差异：要下载 {}、要删除 {}",
        E2E_SLUG,
        old_ver.version_number,
        old_index.files.len(),
        new_ver.version_number,
        new_index.files.len(),
        expected.add.len(),
        expected.remove.len()
    );

    // 装旧版
    // 直接构造"已装旧版"状态，省掉旧版模组的下载：离线建实例 → 旧清单原样写进 base 目录 →
    // 按旧清单放占位文件并解压旧包 overrides，后面的删除与覆盖断言才有意义。
    eprintln!("[e2e] 构造已装旧版的状态（跳过旧版安装下载）…");
    let name = format!("mr-upgrade-{}", Uuid::new_v4().simple());
    let instance = InstanceSettingObj {
        name: name.clone(),
        version: old_index
            .dependencies
            .get("minecraft")
            .cloned()
            .unwrap_or_default(),
        is_modpack: true,
        modpack_type: mml_game::launcher::ModPackType::Modrinth,
        ..Default::default()
    };
    let game = instance
        .create_instance_in_group(None, None)
        .await
        .expect("建实例失败");
    let uuid = game.read().unwrap().uuid;
    let (game_path, base_path, name_before, icon_before) = {
        let read = game.read().unwrap();
        (
            read.get_game_path(),
            read.get_base_path(),
            read.name.clone(),
            read.icon.clone(),
        )
    };
    let group_before = mml_game::game_group::group_of(&uuid);

    // 旧清单：原样写进 base 目录
    std::fs::write(
        base_path.join(mml_names::names::MODRINTH_FILE),
        read_zip_entry(&old_pack, mml_names::names::MODRINTH_FILE),
    )
    .expect("写旧清单");
    // 旧清单里的文件：占位内容
    for f in &old_index.files {
        let p = game_path.join(&f.path);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(&p, b"old-placeholder").unwrap();
    }
    // 旧包 overrides：照旧解压进游戏目录
    for (rel, data) in zip_entries_under(&old_pack, "overrides/") {
        let p = game_path.join(&rel);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(&p, &data).unwrap();
    }
    eprintln!(
        "[e2e] 旧版状态就绪 uuid={uuid}（清单占位文件 {} 个）",
        old_index.files.len()
    );

    // ① 安装后：旧包的 overrides 已解压到游戏目录，`overrides/xxx` 落到 `xxx`
    let old_overrides = zip_entries_under(&old_pack, "overrides/");
    assert!(
        !old_overrides.is_empty(),
        "真包应带 overrides，否则这条断言没有意义"
    );
    for (rel, data) in &old_overrides {
        let path = game_path.join(rel);
        assert!(path.exists(), "旧包 overrides 的 {rel} 应落到游戏目录");
        assert_eq!(
            &std::fs::read(&path).expect("读文件"),
            data,
            "旧包 overrides 的 {rel} 内容应一致"
        );
    }
    eprintln!(
        "[e2e] ① 旧包 overrides 的 {} 个文件已落到游戏目录",
        old_overrides.len()
    );

    // 升级到新版；`ModrinthVersionObj` 没有 `Clone`，直接按所有权传给要 `&mut` 的 `upgrade_modpack`
    let mut new_ver_mut = new_ver;
    eprintln!("[e2e] 开始升级到 {}…", new_ver_mut.version_number);
    mml_game::modrinth::upgrade_modpack(
        &game,
        &mut new_ver_mut,
        None,
        None,
        CancellationToken::new(),
    )
    .await
    .expect("升级整合包失败");
    eprintln!("[e2e] 升级完成，开始断言");

    // ② 新清单里的每个文件都在磁盘上，即该下的下了
    let mut missing = Vec::new();
    for path in index_paths(&new_index) {
        if !game_path.join(&path).exists() {
            missing.push(path);
        }
    }
    assert!(
        missing.is_empty(),
        "新清单里有 {} 个文件没落到磁盘: {:?}",
        missing.len(),
        missing
    );
    eprintln!(
        "[e2e] ② 新清单 {} 个文件全部在磁盘上",
        index_paths(&new_index).len()
    );

    // ③ 旧清单里有、新清单里没有的文件都删掉了，即该删的删了
    let new_paths = index_paths(&new_index);
    let mut leftover = Vec::new();
    for path in index_paths(&old_index) {
        if !new_paths.contains(&path) && game_path.join(&path).exists() {
            leftover.push(path);
        }
    }
    assert!(
        leftover.is_empty(),
        "旧清单里已移除的 {} 个文件还留在磁盘上: {:?}",
        leftover.len(),
        leftover
    );
    eprintln!("[e2e] ③ 旧清单里已移除的文件都已删除");

    // ④ 实例元数据没被重建：升级传的是 None，图标 / 名称 / 分组保持原样
    {
        let read = game.read().unwrap();
        assert_eq!(read.name, name_before, "升级不该改实例名");
        assert_eq!(read.icon, icon_before, "升级不该改图标");
    }
    assert_eq!(
        mml_game::game_group::group_of(&uuid),
        group_before,
        "升级不该改分组归属"
    );
    eprintln!("[e2e] ④ 实例名 / 图标 / 分组保持原样");

    // ⑤ 实例里的清单已写回新版，下次升级据此比对
    let written: ModrinthPackObj =
        manifest::read_manifest(base_path.join(mml_names::names::MODRINTH_FILE))
            .expect("升级后实例里应有 modrinth.index.json");
    assert_eq!(
        index_paths(&written),
        new_paths,
        "实例里的清单应被新版覆盖"
    );
    eprintln!("[e2e] ⑤ 实例里的清单已写回新版");

    // ⑥ 升级后新包的 overrides 覆盖到游戏目录：每个文件都应存在，不能被"删除旧清单里已移除
    // 的文件"误删；同路径时内容必须是新包的。清单里也有的路径跳过，那种文件由下载步骤负责。
    let new_overrides = zip_entries_under(&new_pack, "overrides/");
    assert!(
        !new_overrides.is_empty(),
        "真包应带 overrides，否则这条断言没有意义"
    );
    let mut covered = 0;
    for (rel, data) in &new_overrides {
        if new_index.files.iter().any(|f| f.path == *rel) {
            continue;
        }
        let path = game_path.join(rel);
        assert!(path.exists(), "升级后新包 overrides 的 {rel} 应存在");
        if old_overrides.iter().any(|(o, _)| o == rel) {
            assert_eq!(
                &std::fs::read(&path).expect("读文件"),
                data,
                "升级应把 {rel} 覆盖成新包 overrides 的内容"
            );
            covered += 1;
        }
    }
    eprintln!(
        "[e2e] ⑥ 新包 overrides 的 {} 个文件都在（其中 {covered} 个覆盖了旧包同名文件）",
        new_overrides.len()
    );

    // 清理：`delete_instance` 走回收站，测试环境可能失败，内核设计是"失败则保留实例数据"，
    // 所以这里容错，并直接删掉测试目录，都在 target/temp 下。
    let _ = mml_game::delete_instance(&uuid);
    let _ = std::fs::remove_dir_all(game.read().unwrap().get_base_path());
}

/// 端到端：升级被取消时**不动**旧清单，写回是最后一步，前面的 `?` 提前返回
///
/// **默认不跑**：同样要装一次真包。显式跑：
/// `cargo test -p mml-game --test modpack_upgrade -- --ignored --nocapture`
#[ignore = "在线真包测试：会下载整合包与全部模组，耗时数分钟；用 -- --ignored 显式跑"]
#[tokio::test]
async fn upgrade_modpack_cancelled_keeps_old_manifest() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if !network_available().await {
        return;
    }
    let Some((old_ver, old_pack, _old_index, new_ver, _new_pack, _new_index)) =
        pick_version_pair().await
    else {
        return;
    };

    let uuid = mml_game::add_game::install_archive_from_file(
        &old_pack,
        None,
        None,
        None,
        None,
        None,
        None,
        mml_game::add_game::PackType::Modrinth,
        CancellationToken::new(),
    )
    .await
    .expect("安装旧版整合包失败");

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let manifest_path = game
        .read()
        .unwrap()
        .get_base_path()
        .join(mml_names::names::MODRINTH_FILE);
    let before = std::fs::read(&manifest_path).expect("读旧清单字节");

    // 预取消：worker 会在下载完新包、读完版本信息后的第一个取消检查点返回 TaskCancel
    let cancel = CancellationToken::new();
    cancel.cancel();
    // ModrinthVersionObj 没有 Clone，直接按所有权传给要 &mut 的 upgrade_modpack
    let mut new_ver_mut = new_ver;
    let res = mml_game::modrinth::upgrade_modpack(&game, &mut new_ver_mut, None, None, cancel).await;
    assert!(res.is_err(), "预取消的升级应当返回错误");

    let after = std::fs::read(&manifest_path).expect("再读旧清单字节");
    assert_eq!(
        before, after,
        "取消的升级不该改动旧清单（{}）",
        old_ver.version_number
    );

    // 清理：`delete_instance` 走回收站，测试环境可能失败，内核设计是"失败则保留实例数据"，
    // 所以这里容错，并直接删掉测试目录，都在 target/temp 下。
    let _ = mml_game::delete_instance(&uuid);
    let _ = std::fs::remove_dir_all(game.read().unwrap().get_base_path());
}


// ===================== extract 与实例清单的交互，离线回归 =====================
//
// 这一段用的是上面 e2e 段的 `ensure_init` / `TEST_LOCK`，同进程单例，但**完全离线**：
// 不需要网络、毫秒级。

/// 合成一个整合包 zip：根目录放清单，`overrides/` 下放一个覆盖文件
fn make_pack_zip(path: &Path, manifest: &str, override_rel: &str, override_data: &[u8]) {
    let file = std::fs::File::create(path).expect("建 zip");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(mml_names::names::MODRINTH_FILE, options)
        .unwrap();
    zip.write_all(manifest.as_bytes()).unwrap();
    zip.start_file(
        format!("{}/{}", mml_names::names::OVERRIDE_DIR, override_rel),
        options,
    )
    .unwrap();
    zip.write_all(override_data).unwrap();
    zip.finish().unwrap();
}

/// 升级时 `extract` **不能**把包根清单写进实例 base 目录
///
/// "差异算成空"那个 bug 的离线回归：`extract_pack_files` 会把不在 `overrides/` 下的条目
/// 写进 base 目录，包根清单正是其中之一，覆盖后 `check_upgrade` 会把新清单当旧清单读。
/// 这里同时验证反面：不排除时确实会被覆盖，那正是 bug 的成因。
#[test]
fn extract_must_not_overwrite_instance_manifest() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    // 离线造实例：`InstanceSettingObj` 直接构造，不需要网络
    let dir = format!("extract-manifest-{}", Uuid::new_v4().simple());
    let instance = InstanceSettingObj {
        uuid: Uuid::new_v4(),
        name: dir.clone(),
        dir: dir.clone(),
        ..Default::default()
    };
    let game: mml_game::GameInstance = Arc::new(RwLock::new(instance));
    let (base_path, game_path) = {
        let g = game.read().unwrap();
        (g.get_base_path(), g.get_game_path())
    };
    std::fs::create_dir_all(&base_path).unwrap();
    std::fs::create_dir_all(&game_path).unwrap();

    // 实例里的旧清单，内容与包里的故意不同
    let old_manifest = "{\"formatVersion\":1,\"versionId\":\"old-installed\"}";
    let manifest_path = base_path.join(mml_names::names::MODRINTH_FILE);
    std::fs::write(&manifest_path, old_manifest).unwrap();

    // 合成包：根目录是**新清单**，`overrides/probe.txt` 是覆盖文件
    let pack = mml_testutil::temp_dir().join(format!(
        "mml-modpack-extract-{}.mrpack",
        Uuid::new_v4().simple()
    ));
    make_pack_zip(
        &pack,
        "{\"formatVersion\":1,\"versionId\":\"new-pack\"}",
        "probe.txt",
        b"from-overrides",
    );

    let archive = BaseArchive::open(&pack).expect("打开合成包");
    let mut worker =
        BaseModPackWorker::new(archive, None, None, None, CancellationToken::new(), None);
    worker.game = Some(game.clone());

    // 与 `upgrade_modpack` 一致的解压：把包根清单排除在外
    worker
        .extract_pack_files(
            mml_names::names::OVERRIDE_DIR,
            Some(vec![mml_names::names::MODRINTH_FILE.to_string()]),
        )
        .expect("解压覆盖文件");

    assert_eq!(
        std::fs::read_to_string(&manifest_path).unwrap(),
        old_manifest,
        "extract 不该覆盖实例里的旧清单（否则差异会算成空）"
    );
    assert_eq!(
        std::fs::read(game_path.join("probe.txt")).unwrap(),
        b"from-overrides",
        "overrides 仍应落到游戏目录"
    );

    // 反面：不排除清单时它确实会被覆盖 —— 这就是那个 bug 的成因
    worker
        .extract_pack_files(mml_names::names::OVERRIDE_DIR, None)
        .expect("解压（含清单）");
    assert_ne!(
        std::fs::read_to_string(&manifest_path).unwrap(),
        old_manifest,
        "不排除清单时 extract 会覆盖它（所以升级必须排除）"
    );

    let _ = std::fs::remove_file(&pack);
}

// ===================== CurseForge 端到端，在线且需要 API key =====================

/// 读整合包 zip 里的 `manifest.json`
fn read_cf_manifest(zip_path: &Path) -> CurseForgePackObj {
    let data = read_zip_entry(zip_path, mml_names::names::MANIFEST_FILE);
    mml_base::serialize_tools::json_from_bytes(&data).expect("解析 manifest.json")
}

/// 端到端 CurseForge：装旧版 → 升级新版 → 实例文件集合 == 新版清单，元数据不变
///
/// 与 Modrinth 那条同构，CF 侧的差别：
/// - **需要 `MML_CF_API_KEY`**，CF API 无 key 直接 403，没设就打印跳过；
/// - CF 的 `manifest.json` 只记 `projectID` / `fileID`，没有文件路径，路径来自实例的
///   `online_info`，安装与升级时写入，所以断言以 `online_info` 为准；
/// - 这条走**真实安装**，因为 CF 的旧状态要按 file id 反查文件名，构造起来还得再调 API；
/// - overrides 的断言没放进来：CF 清单没有路径，判断不了某个 overrides 路径是否也归清单管。
///
/// 显式跑：
/// ```text
/// $env:MML_CF_API_KEY = "<key>"
/// cargo test -p mml-game --test modpack_upgrade -- --ignored --nocapture upgrade_curseforge
/// ```
#[ignore = "在线真包测试：需要 MML_CF_API_KEY，且会下载整合包与全部模组；用 -- --ignored 显式跑"]
#[tokio::test]
async fn upgrade_curseforge_pack_end_to_end_matches_new_manifest() {
    use mml_net::curseforge_api::{self, CurseFogreArg};

    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if curseforge_api::get_key().is_err() {
        eprintln!("跳过: 未设置 MML_CF_API_KEY 环境变量（CurseForge API 需要 key）");
        return;
    }
    if !network_available().await {
        return;
    }

    // 找一个整合包与它的文件页；CF 按时间倒序，第一个最新
    let list = curseforge_api::get_modpack_list(CurseFogreArg {
        filter: Some("Fabulously Optimized".to_string()),
        page_size: Some(5),
        ..Default::default()
    })
    .await
    .expect("搜索 CurseForge 整合包失败");
    let Some(item) = list.data.first() else {
        eprintln!("跳过: 搜索结果为空");
        return;
    };
    let mut files = curseforge_api::get_files_page(CurseFogreArg {
        id: Some(item.id.to_string()),
        page_size: Some(10),
        ..Default::default()
    })
    .await
    .expect("获取 CurseForge 文件列表失败");
    if files.data.len() < 2 {
        eprintln!("跳过: {} 的文件不足两个版本", item.name);
        return;
    }
    let (new_idx, old_idx) = (0usize, files.data.len() - 1);
    let (new_name, old_name) = (
        files.data[new_idx].display_name.clone(),
        files.data[old_idx].display_name.clone(),
    );
    eprintln!("[e2e-cf] {}：{} → {}", item.name, old_name, new_name);

    // 新版包本身，CF 的 CDN 公开、不需要 key，用来比对清单
    let new_pack = {
        let url = files.data[new_idx].download_url.clone().unwrap_or_default();
        assert!(!url.is_empty(), "新版文件没有下载地址");
        let cache = mml_testutil::temp_dir().join(format!(
            "mml-cf-upgrade-{}.zip",
            files.data[new_idx].id
        ));
        if !cache.exists() {
            let data = mml_net::get_work_client()
                .get_bytes(&url)
                .await
                .expect("下载新版整合包失败");
            std::fs::write(&cache, &data).expect("写缓存");
        }
        cache
    };
    let new_index = read_cf_manifest(&new_pack);
    let mut new_pairs: Vec<(u64, u64)> = new_index
        .files
        .iter()
        .map(|f| (f.project_id, f.file_id))
        .collect();
    new_pairs.sort_unstable();

    // 装旧版，走真实安装
    eprintln!("[e2e-cf] 开始安装旧版（要下载它引用的全部模组）…");
    let uuid = mml_game::add_game::install_curseforge(
        &mut files.data[old_idx],
        None,
        None,
        None,
        None,
        None,
        CancellationToken::new(),
    )
    .await
    .expect("安装旧版整合包失败");
    eprintln!("[e2e-cf] 旧版安装完成 uuid={uuid}");

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let (base_path, name_before, icon_before) = {
        let read = game.read().unwrap();
        (read.get_base_path(), read.name.clone(), read.icon.clone())
    };
    let group_before = mml_game::game_group::group_of(&uuid);

    // 旧清单是安装时写进 base 的 `manifest.json`，升级会覆盖它，先快照给断言 ② 用
    let old_manifest: CurseForgePackObj =
        manifest::read_manifest(base_path.join(mml_names::names::MANIFEST_FILE))
            .expect("安装后实例里应有 manifest.json");
    let old_projects: Vec<u64> = old_manifest.files.iter().map(|f| f.project_id).collect();

    // 升级到新版
    eprintln!("[e2e-cf] 开始升级到 {}…", new_name);
    mml_game::curseforge::upgrade_modpack(
        &game,
        &mut files.data[new_idx],
        None,
        None,
        CancellationToken::new(),
    )
    .await
    .expect("升级整合包失败");
    eprintln!("[e2e-cf] 升级完成，开始断言");

    // ① 新版清单里的每个 project 都在实例的 online_info 里，且文件确实在磁盘上
    let online = game.read().unwrap().read_online_info();
    let game_path = game.read().unwrap().get_game_path();
    let mut missing = Vec::new();
    for f in &new_index.files {
        match online.get(&f.project_id.to_string()) {
            Some(info) => {
                let p = game_path.join(&info.path).join(&info.file);
                if !p.exists() {
                    missing.push(format!("{}（online_info 有记录但文件不在）", f.project_id));
                }
            }
            None => missing.push(format!("{}（online_info 里没有）", f.project_id)),
        }
    }
    assert!(
        missing.is_empty(),
        "新版清单里有 {} 个没落到实例: {:?}",
        missing.len(),
        missing
    );
    eprintln!("[e2e-cf] ① 新版清单 {} 个文件都在", new_index.files.len());

    // ② 旧清单里有、新清单里没有的 project：不该再留在 online_info 里
    let new_projects: Vec<u64> = new_index.files.iter().map(|f| f.project_id).collect();
    let stale: Vec<u64> = old_projects
        .iter()
        .copied()
        .filter(|p| !new_projects.contains(p) && online.contains_key(&p.to_string()))
        .collect();
    assert!(
        stale.is_empty(),
        "旧清单里已移除的 {} 个 project 还留在 online_info: {:?}",
        stale.len(),
        stale
    );
    eprintln!("[e2e-cf] ② 旧清单里已移除的 project 都已清掉");

    // ③ 实例元数据没被重建
    {
        let read = game.read().unwrap();
        assert_eq!(read.name, name_before, "升级不该改实例名");
        assert_eq!(read.icon, icon_before, "升级不该改图标");
    }
    assert_eq!(
        mml_game::game_group::group_of(&uuid),
        group_before,
        "升级不该改分组归属"
    );
    eprintln!("[e2e-cf] ③ 实例名 / 图标 / 分组保持原样");

    // ④ 实例里的清单已写回新版，与新版包里的 manifest.json 逐条一致
    let written: CurseForgePackObj =
        manifest::read_manifest(base_path.join(mml_names::names::MANIFEST_FILE))
            .expect("升级后实例里应有 manifest.json");
    let mut written_pairs: Vec<(u64, u64)> = written
        .files
        .iter()
        .map(|f| (f.project_id, f.file_id))
        .collect();
    written_pairs.sort_unstable();
    assert_eq!(written_pairs, new_pairs, "实例里的清单应被新版覆盖");
    eprintln!("[e2e-cf] ④ 实例里的清单已写回新版");

    // 清理：`delete_instance` 走回收站，测试环境可能失败，内核设计是"失败则保留实例数据"，
    // 所以这里容错，并直接删掉测试目录，都在 target/temp 下。
    let _ = mml_game::delete_instance(&uuid);
    let _ = std::fs::remove_dir_all(game.read().unwrap().get_base_path());
}