use mml_names::Lang;

use super::LogText;

/// 两套文案都得有内容，参数要真的落到文案里
#[test]
fn both_languages_render() {
    let failed = LogText::ColormcMigrateFailed {
        error: String::from("boom"),
    };
    assert_eq!(
        failed.text_in(&Lang::zh_cn),
        String::from("[colormc] 迁移任务失败：boom")
    );
    assert_eq!(
        failed.text_in(&Lang::en_us),
        String::from("[colormc] migration task failed: boom")
    );

    let retry = LogText::LoaderQueryRetry {
        attempt: 2,
        mc: String::from("1.20.1"),
    };
    assert!(retry.text_in(&Lang::zh_cn).contains("第 2 次"));
    assert!(retry.text_in(&Lang::en_us).contains("attempt 2"));
    assert!(retry.text_in(&Lang::en_us).contains("mc=1.20.1"));

    let net = LogText::NetworkSettingsSaved {
        work_proxy: String::from("Auto"),
        work_proxy_type: String::from("http"),
        proxy_ip: String::from("127.0.0.1"),
        proxy_port: String::from("7890"),
        login_proxy: String::from("None"),
        login_proxy_type: String::from("http"),
    };
    assert!(net.text_in(&Lang::zh_cn).starts_with("保存网络设置："));
    assert!(
        net.text_in(&Lang::en_us)
            .starts_with("saving network settings:")
    );
    assert!(net.text_in(&Lang::en_us).contains("127.0.0.1:7890"));

    // 致命弹窗标题也要两边都不一样
    assert_ne!(
        LogText::FatalInitFailed.text_in(&Lang::zh_cn),
        LogText::FatalInitFailed.text_in(&Lang::en_us)
    );
    assert!(
        LogText::FatalInitFailed
            .text_in(&Lang::en_us)
            .contains("M²L")
    );

    // 日志窗口那几条：时间要落进文案
    let login = LogText::LogLoginTime { secs: 1.25 };
    assert_eq!(login.text_in(&Lang::zh_cn), String::from("登录用时：1.25s"));
    assert_eq!(
        login.text_in(&Lang::en_us),
        String::from("login took 1.25s")
    );

    let java = LogText::LaunchNoJava { java: 17 };
    assert!(java.text_in(&Lang::zh_cn).contains("Java 17"));
    assert!(java.text_in(&Lang::en_us).contains("Java 17"));

    // 带数量与项目号的条目，两个语言都要把参数落进去
    let miss = LogText::ModUpdateItemFailed {
        project_id: String::from("sodium"),
        error: String::from("timeout"),
    };
    assert_eq!(
        miss.text_in(&Lang::zh_cn),
        String::from("模组更新失败 sodium：timeout")
    );
    assert_eq!(
        miss.text_in(&Lang::en_us),
        String::from("mod update failed for sodium: timeout")
    );

    let done = LogText::ModUpdateFinished { ok: 3, failed: 1 };
    assert!(done.text_in(&Lang::zh_cn).contains("ok=3 failed=1"));
    assert!(done.text_in(&Lang::en_us).contains("ok=3 failed=1"));

    // 带缩放与来源的条目：参数要落进两种文案
    let set = LogText::BgSet {
        percent: 50,
        source: String::from("a.png"),
    };
    assert!(set.text_in(&Lang::zh_cn).contains("缩放 50%"));
    assert!(set.text_in(&Lang::zh_cn).contains("a.png"));
    assert!(set.text_in(&Lang::en_us).contains("50%"));
    assert!(set.text_in(&Lang::en_us).contains("a.png"));

    // 耗时条目：一秒以内用毫秒；没有改名那一段时不要留空壳
    let detail = LogText::ExportPackDetail {
        open: 0.012,
        write: 2.5,
        finish: 0.3,
        commit: None,
        entries: 7,
    };
    assert!(detail.text_in(&Lang::zh_cn).contains("12 ms"));
    assert!(detail.text_in(&Lang::zh_cn).contains("2.50 s"));
    assert!(detail.text_in(&Lang::zh_cn).contains("共 7 个条目"));
    assert!(!detail.text_in(&Lang::zh_cn).contains("改名"));
    assert!(detail.text_in(&Lang::en_us).contains("12 ms"));
    assert!(detail.text_in(&Lang::en_us).contains("(7 entries)"));

    let overview = LogText::ExportTime {
        level: String::from("fast"),
        prepare: 0.05,
        pack: 12.34,
        total: 12.5,
        file: String::from("a.zip"),
    };
    assert!(overview.text_in(&Lang::zh_cn).contains("50 ms"));
    assert!(overview.text_in(&Lang::zh_cn).contains("12.34 s"));
    assert!(overview.text_in(&Lang::zh_cn).contains("a.zip"));
    assert!(overview.text_in(&Lang::en_us).contains("fast"));

    // 没有参数的条目也两边都有字
    assert!(
        !LogText::WebviewSettingsFailed
            .text_in(&Lang::zh_cn)
            .is_empty()
    );
    assert_ne!(
        LogText::MainWindowModelMissing.text_in(&Lang::zh_cn),
        LogText::MainWindowModelMissing.text_in(&Lang::en_us)
    );
}

