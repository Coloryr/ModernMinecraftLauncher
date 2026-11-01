use super::*;
use crate::{
    i18_items::{
        error_type::ErrorType, info_type::InfoType, panic_type::PanicType,
        thread_type::ThreadType,
    },
    names,
};
use std::fs;

/// i18 文案与语言加载生命周期测试
///
/// `LANG` 与 `i18::I18` 都是进程级全局状态，所有涉及全局状态的操作
/// 统一放在这一个测试里顺序执行，避免与其他测试并发时互相干扰。
#[test]
fn lang_and_i18_lifecycle() {
    // ---------- i18 文案：强制使用中文实现，不依赖系统语言 ----------
    i18::set(Box::new(i18::zh_cn::ZhCn));

    // 线程名
    assert_eq!(
        i18::get_thread(ThreadType::LogThread),
        String::from("日志线程")
    );
    assert_eq!(
        i18::get_thread(ThreadType::ConfigSaveThread),
        String::from("配置保存线程")
    );

    // 信息文案：CoreStart 的文案里带版本号
    assert_eq!(
        i18::get_info(InfoType::CoreStart),
        format!("M²L启动，版本：{}", *VERSION)
    );
    assert_eq!(i18::get_info(InfoType::TempFile), String::from("临时文件"));

    // 错误文案：带参数的格式化
    let err = ErrorType::ConfigError(FileSystemErrorData {
        path: PathBuf::from("a.json"),
        error: String::from("boom"),
    });
    assert_eq!(
        i18::get_error(err.clone()),
        String::from("配置文件 a.json 处理失败：boom")
    );

    // Display 实现走同一条 i18 路径
    assert_eq!(err.to_string(), "配置文件 a.json 处理失败：boom");
    assert_eq!(ErrorType::TaskCancel.to_string(), "任务已取消");
    assert_eq!(ErrorType::TaskTimeout.to_string(), "任务执行超时");

    // 严重错误文案
    assert_eq!(
        i18::get_panic(PanicType::CoreArgLocalEmpty),
        String::from("运行路径为空")
    );
    assert_eq!(
        i18::get_panic(PanicType::LogOpenFail(
            String::from("C:/logs"),
            String::from("denied")
        )),
        String::from("日志系统初始化失败：denied 路径：C:/logs")
    );

    // ---------- 语言文件加载 ----------
    // 使用临时目录，测完清理
    let dir = mml_testutil::temp_dir().join(format!("mml_names_test_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let lang_file = dir.join(names::LANG_FILE);
    // 清掉上次运行可能残留的语言文件
    let _ = fs::remove_file(&lang_file);

    // init 传入的是一个文件路径，语言文件取其同目录下的 lang.txt
    let init_path = dir.join("notexist.json");
    init(&init_path).unwrap();
    // 语言文件不存在时不应被创建
    assert!(!lang_file.exists());

    // 写入 en_US 后重新 init，应切换到英文
    fs::write(&lang_file, names::LANG_EN_US).unwrap();
    init(&init_path).unwrap();
    assert!(matches!(get_lang_type(), Lang::en_us));

    // set_lang 切回中文，同时应把语言写回文件
    set_lang(Lang::zh_cn).unwrap();
    assert!(matches!(get_lang_type(), Lang::zh_cn));
    assert_eq!(fs::read_to_string(&lang_file).unwrap(), names::LANG_ZH_CN);

    // 未知语言字符串应回退为中文
    fs::write(&lang_file, "fr_FR").unwrap();
    init(&init_path).unwrap();
    assert!(matches!(get_lang_type(), Lang::zh_cn));

    // 清理临时目录
    let _ = fs::remove_dir_all(&dir);
}

/// 语言字符串判断，不依赖全局状态
#[test]
fn check_lang_strings() {
    assert!(matches!(check_lang(&String::from("zh_CN")), Lang::zh_cn));
    assert!(matches!(check_lang(&String::from("en_US")), Lang::en_us));
    // 不认识的语言回退为中文
    assert!(matches!(check_lang(&String::from("fr_FR")), Lang::zh_cn));
}

/// get_lang 与语言常量一一对应
#[test]
fn get_lang_map() {
    assert_eq!(get_lang(Lang::zh_cn), names::LANG_ZH_CN);
    assert_eq!(get_lang(Lang::en_us), names::LANG_EN_US);
}

/// Lang 的默认值是中文
#[test]
fn lang_default() {
    assert!(matches!(Lang::default(), Lang::zh_cn));
}

/// 获取本地语言：只读环境变量，仅检查返回值非空
#[test]
fn current_locale_non_empty() {
    assert!(!get_current_locale().is_empty());
}

/// 换行符应与编译目标平台一致
#[test]
fn line_ending_matches_platform() {
    let expected = if cfg!(windows) { "\r\n" } else { "\n" };
    assert_eq!(get_line_ending(), expected);
}

/// 配置版本号 = 主版本号 + 编译日期
#[test]
fn version_format() {
    let base = format!("{VERSION_NUM}.{VERSION_SEC_NUM}.{VERSION_PATCH_NUM}");
    println!("配置版本号：{}", *VERSION);

    #[cfg(debug_assertions)]
    {
        assert_eq!(*VERSION, format!("{base}.{BUILD_DATE}"));
        assert_eq!(BUILD_DATE.len(), 8, "日期应为 yyyymmdd：{BUILD_DATE}");
        assert!(
            BUILD_DATE.chars().all(|c| c.is_ascii_digit()),
            "日期只能是数字：{BUILD_DATE}"
        );
    }

    #[cfg(not(debug_assertions))]
    assert_eq!(*VERSION, base);
}

