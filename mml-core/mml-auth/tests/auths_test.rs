//! mml-auth 账户存储模块 auths 的集成测试
//!
//! # 注意事项
//!
//! - `auths` 模块使用全局内存存储 + 磁盘文件 `auth.json` / `auth_select.json`，
//!   磁盘路径来自 `mml_base::inner_path`，即 `LOCALAPPDATA` 或 `HOME`。
//!   为避免污染真实用户数据，本测试在访问任何路径前把 `LOCALAPPDATA` / `HOME`
//!   重定向到仓库 `target/temp` 下的唯一子目录 `mml_testutil::temp_dir()`，测试结束后清理。
//! - 全局存储只能初始化一次且 `config_save` / `mml_log` 的后台线程只能 `start`
//!   一次，因此本文件只用单个顺序执行的 `#[test]` 覆盖全部流程。
//! - 全程使用假凭据，不访问网络。

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use chrono::{FixedOffset, TimeZone};
use mml_auth::{AuthType, LoginObj, UserKeyObj, auths};
use mml_base::{inner_path, serialize_tools};

/// 假凭据，与真实账户无关
const FAKE_UUID_A: &str = "00000000-0000-0000-0000-00000000aaaa";
const FAKE_UUID_B: &str = "00000000-0000-0000-0000-00000000bbbb";
const FAKE_UUID_C: &str = "00000000-0000-0000-0000-00000000cccc";

/// 测试根目录，只初始化一次，保证 env 重定向只发生一次
static TEST_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 重定向内部数据目录到临时目录并初始化日志 / 配置保存线程
fn setup() -> &'static PathBuf {
    TEST_DIR.get_or_init(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml_auth_test_{}", std::process::id()));
        // 清理上次运行残留
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // inner_path 依赖 LOCALAPPDATA 或 HOME，必须在任何路径访问之前重定向；
        // edition 2024 中 set_var 为 unsafe
        let dir_str = dir.to_str().unwrap().to_string();
        unsafe { std::env::set_var("LOCALAPPDATA", &dir_str) };
        unsafe { std::env::set_var("HOME", &dir_str) };

        // 日志与配置保存均为只能启动一次的后台线程
        mml_log::start(&dir).unwrap();
        mml_config::config_save::start();

        dir
    })
}

/// 构造一个带指定最后登录时间的假账户
fn fake_login(user: &str, uuid: &str, auth_type: AuthType, token: &str) -> LoginObj {
    let mut obj = LoginObj::new(
        user.to_string(),
        uuid.to_string(),
        token.to_string(),
        "fake-client-token".to_string(),
    );
    obj.auth_type = auth_type;
    // 固定时间便于验证 get_all 的排序
    obj.last_login = FixedOffset::east_opt(8 * 3600)
        .unwrap()
        .with_ymd_and_hms(2026, 1, 1, 12, 0, 0)
        .unwrap();
    obj
}

/// 在临时目录写一个 JSON 文件并返回路径
fn write_json(dir: &Path, name: &str, json: &str) -> PathBuf {
    let file = dir.join(name);
    std::fs::write(&file, json).unwrap();
    file
}

/// 覆盖账户存储的完整生命周期：加载 -> 查询 -> 排序 -> 导入 -> 保存/删除 ->
/// 当前账户切换 -> 清空 -> 磁盘持久化
#[test]
fn test_auths_lifecycle() {
    let dir = setup();
    let inner = inner_path::get_inner_path();
    assert!(
        dir.starts_with(mml_testutil::temp_dir()),
        "内部目录应位于临时目录下"
    );

    // ---- 1) 预写 auth.json 后 init，验证从磁盘加载 ----
    let accounts = vec![
        fake_login("Steve", FAKE_UUID_A, AuthType::OAuth, "token-a"),
        fake_login("Alex", FAKE_UUID_B, AuthType::LittleSkin, "token-b"),
    ];
    serialize_tools::json_to_file(&accounts, inner.join("auth.json")).unwrap();

    auths::init();

    let key_a = UserKeyObj {
        uuid: FAKE_UUID_A.to_string(),
        auth_type: AuthType::OAuth,
    };
    let key_b = UserKeyObj {
        uuid: FAKE_UUID_B.to_string(),
        auth_type: AuthType::LittleSkin,
    };

    // 按 UUID + 认证类型查询
    let got = auths::get(FAKE_UUID_A, AuthType::OAuth).expect("应能取到账户 A");
    assert_eq!(got.user_name, "Steve");
    assert_eq!(got.access_token, "token-a");
    assert_eq!(got.auth_type, AuthType::OAuth);
    // 同 UUID 不同认证类型应查不到
    assert!(auths::get(FAKE_UUID_A, AuthType::Offline).is_none());
    assert!(auths::get(FAKE_UUID_B, AuthType::LittleSkin).is_some());
    // 不存在的 UUID
    assert!(auths::get("no-such-uuid", AuthType::Offline).is_none());

    // ---- 2) get_all 按最后登录时间倒序 ----
    let mut latest = fake_login("Latest", FAKE_UUID_C, AuthType::Nide8, "token-c");
    latest.last_login = FixedOffset::east_opt(8 * 3600)
        .unwrap()
        .with_ymd_and_hms(2026, 5, 1, 0, 0, 0)
        .unwrap();
    latest.save();

    let all = auths::get_all();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].user_name, "Latest", "最近使用的账户应排在最前");

    // ---- 3) import 合并导入，同键覆盖 ----
    let import_file = write_json(
        dir,
        "import_auths.json",
        r#"[
        {
            "UserName": "Steve",
            "UUID": "00000000-0000-0000-0000-00000000aaaa",
            "AccessToken": "token-a-new",
            "ClientToken": "fake-client-token",
            "AuthType": 1,
            "Text1": null,
            "Text2": null,
            "LastLogin": "2026-03-01T12:00:00+08:00"
        }
    ]"#,
    );
    auths::import(&import_file).unwrap();

    let merged = auths::get(FAKE_UUID_A, AuthType::OAuth).expect("导入应覆盖同键账户");
    assert_eq!(merged.access_token, "token-a-new");
    assert_eq!(auths::get_all().len(), 3, "导入同键账户不应增加条目");

    // ---- 4) set_current / get_current ----
    assert_eq!(auths::get_current(), None, "初始无当前账户");
    auths::set_current(Some(key_a.clone()));
    assert_eq!(auths::get_current(), Some(key_a.clone()));
    auths::set_current(Some(key_b.clone()));
    assert_eq!(auths::get_current(), Some(key_b.clone()));
    auths::set_current(None);
    assert_eq!(auths::get_current(), None);
    auths::set_current(Some(key_a.clone()));

    // ---- 5) delete 移除账户 ----
    latest.delete();
    assert!(auths::get(FAKE_UUID_C, AuthType::Nide8).is_none());
    assert_eq!(auths::get_all().len(), 2);

    // ---- 6) 停止保存线程，触发最终落盘，验证磁盘文件 ----
    mml_config::config_save::stop();

    let saved: Vec<LoginObj> =
        serialize_tools::json_from_file(inner.join("auth.json")).expect("auth.json 应可解析");
    assert_eq!(saved.len(), 2);
    assert!(
        saved
            .iter()
            .any(|x| x.uuid == FAKE_UUID_A && x.access_token == "token-a-new")
    );
    assert!(saved.iter().any(|x| x.uuid == FAKE_UUID_B));

    // 当前账户选择文件应与最后一次 set_current 一致
    let select: UserKeyObj =
        serialize_tools::json_from_file(inner.join("auth_select.json")).expect("select 应可解析");
    assert_eq!(select, key_a);

    // ---- 7) clear_auths 清空内存与磁盘 ----
    auths::clear_auths();
    assert!(auths::get_all().is_empty());
    assert!(auths::get(FAKE_UUID_A, AuthType::OAuth).is_none());

    // ---- 8) 清理临时目录 ----
    let _ = std::fs::remove_dir_all(dir);
}
