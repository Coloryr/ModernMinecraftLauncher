use std::path::PathBuf;

use uuid::Uuid;

use crate::game_mods::jar::{forge_logo_file, mcmod_logo_file_in};
use crate::game_mods::*;
use crate::launcher::instance_setting_obj::InstanceSettingObj;
use crate::loader::LoaderType;
use crate::test_support::{TextEntry, text_entry};
use mml_base::range_checker::{RangeSyntax, VersionRange};
use mml_names::names;
use std::io::Write;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

// ==================== 元数据解析 ====================
//
// 单个元数据文件坏了不该影响整包：真实案例 `mekanism_lasers-1.1.10.3-a.jar` 里两个 toml
// 坏了一个，旧实现用 `?` 抛出后整包判为"读取失败"、连名字都看不到；正确行为是坏的那个
// 跳过并记一条日志，好的照常读。

/// 起日志系统：解析失败时**故意**要记一条日志，而 `mml_log` 没 start 过就会 panic，
/// 那是"日志系统还没起来"的前提问题，不是被测逻辑的问题。
///
/// **走 crate 级的共用 boot**，见 [`crate::test_support::boot`]：`mml_log::STREAM` 是
/// `OnceLock`，各测试模块各写一份就会第二次 `start()` → panic，本模块与
/// `game_group::tests` 曾经互相踩挂就是这个原因。
fn boot_log() {
    crate::test_support::boot();
}

/// 造一个只含指定条目的 jar
fn make_jar(entries: &[TextEntry]) -> PathBuf {
    boot_log();
    let path = mml_testutil::temp_dir().join(format!("mml-mod-{}.jar", Uuid::new_v4()));
    let file = std::fs::File::create(&path).unwrap();
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for e in entries {
        writer.start_file(e.name, options).unwrap();
        writer.write_all(e.text.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
    path
}

/// 两个 toml、其中一个非法：整包仍应读出**合法那个**的信息，而不是判失败
///
/// 两个路径都必须是**会被扫描的**那三个之一，即 `META-INF/mods.toml` /
/// `META-INF/neoforge.mods.toml` / `neoforge.mods.toml`；
/// 放在别处的 toml 根本不进解析，拿它当样本是测不出问题的。
#[test]
fn broken_toml_does_not_fail_whole_jar() {
    let path = make_jar(&[
        text_entry(
            names::NEO_TOML_FILE,
            "modLoader=\"javafml\"\nloaderVersion=\"[1,)\"\nlicense=\"MIT\"\n\
             [[mods]]\nmodId=\"mekanism_lasers\"\ndisplayName=\"Mekanism Lasers\"\nversion=\"1.1.10.3\"\n",
        ),
        // 非法样本用**根目录的 neoforge.mods.toml**，即第三个扫描路径，`[[mods]` 少一个方括号；
        // 这正是"一个包里两个 toml、坏的那个把整包带崩"的真实形态
        text_entry(names::NEO_TOML1_FILE, "[[mods]\nmodId=\"whatever\"\n"),
    ]);

    let info = read_mod_info(&path).expect("坏 toml 不该让整包解析失败");
    assert!(!info.fail, "不该被判为读取失败");
    let first = info.info.first().expect("应读出合法 toml 的条目");
    assert_eq!(first.mod_id, "mekanism_lasers");
    assert_eq!(first.name, "Mekanism Lasers");
    assert_eq!(first.version.as_deref(), Some("1.1.10.3"));
}

/// 只有非法 toml：当作"读不出元数据"，但不 panic、也不报错
#[test]
fn only_broken_toml_yields_empty_info() {
    let path = make_jar(&[text_entry(names::MC_MOD_TOML_FILE, "[[mods]\n")]);

    let info = read_mod_info(&path).expect("坏 toml 不该让整包解析失败");
    assert!(info.info.is_empty(), "解析不出条目时 info 应为空");
}

/// MANIFEST.MF 里**没有** core mod 标记时，不该多出一条空 info
///
/// `META-INF/MANIFEST.MF` 几乎每个 jar 都有，而 `read_core_mod` 以前无条件 push 一条
/// info，那条的加载器是枚举默认值 `Normal`；界面改成"汇总**所有**条目的加载器"之后，
/// 用户就会看到**每个模组都多出一个「原版」**。
#[test]
fn manifest_without_core_marker_adds_no_info() {
    let path = make_jar(&[
        text_entry(
            names::FABRIC_MOD_FILE,
            r#"{"id":"demo","name":"Demo","version":"1.0"}"#,
        ),
        text_entry(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\nCreated-By: Gradle\n",
        ),
    ]);

    let info = read_mod_info(&path).expect("应能解析");
    assert_eq!(info.info.len(), 1, "只该有 fabric 那一条，不该多出空条目");
    assert_eq!(info.info[0].loaders.id(), "fabric", "不该是默认的 normal");
    assert!(!info.core, "没有 core mod 标记就不该判成 core");
}

/// 真的带 core mod 标记时，那条 info 仍要保留，别把功能一起改掉
#[test]
fn manifest_with_core_marker_keeps_info() {
    let path = make_jar(&[text_entry(
        "META-INF/MANIFEST.MF",
        "Manifest-Version: 1.0\nFMLCorePlugin: com.example.CorePlugin\n",
    )]);

    let info = read_mod_info(&path).expect("应能解析");
    assert!(info.core, "有 FMLCorePlugin 就该判成 core");
    let first = info.info.first().expect("core mod 的 info 不该被丢掉");
    assert_eq!(first.loaders.id(), "forge");
}

// ==================== 依赖范围与依赖检测 ====================

/// 造一个装着元数据的模组条目，加载器记为 **Forge**（与 `forge_game` 配套）
///
/// 依赖检测只看与实例加载器一致的元数据，所以换加载器的用例要自己改
/// `item.info[0].loaders`，见 `fabric_api_is_not_treated_as_loader`。
///
/// - `file`: 文件名，重复安装那一档按它区分
/// - `modid`: 模组标识
/// - `version`: 版本号
/// - `deps`: 前置列表
fn mod_item(file: &str, modid: &str, version: &str, deps: Vec<DependantType>) -> ModObj {
    ModObj {
        file: PathBuf::from(file),
        info: vec![ModItemObj {
            mod_id: modid.to_string(),
            name: modid.to_string(),
            version: Some(version.to_string()),
            dependants: deps,
            loaders: LoaderType::Forge,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// 造一条强制前置，按 1.12.2 的 `modid@版本范围` 写法
///
/// - `spec`: 形如 `b@[1.0,)`；不带 `@` 时整串就是 modid
fn need(spec: &str) -> DependantType {
    DependantType::Required(DependantInfo::parse_required_name(spec))
}

/// 造一个 Forge 实例设置；版本留空，用来验证"版本未知时不比版本"
fn forge_game() -> InstanceSettingObj {
    InstanceSettingObj {
        loader: LoaderType::Forge,
        ..Default::default()
    }
}

/// 带游戏版本与加载器版本的 Forge 实例，用来验证游戏 / 加载器版本检查
fn forge_game_at(version: &str, loader_version: &str) -> InstanceSettingObj {
    InstanceSettingObj {
        loader: LoaderType::Forge,
        version: version.to_string(),
        loader_version: Some(loader_version.to_string()),
        ..Default::default()
    }
}

/// 带 `modid@` 前缀的解析：modid 取出来、范围可用
#[test]
fn parse_with_modid_prefix() {
    let d = DependantInfo::parse_required_name("spongeapi@[7.0.0,)");
    assert_eq!(d.modid, "spongeapi");
    assert!(d.version.contains("7.0.0"));
    assert!(!d.version.contains("6.9.9"));

    let d = DependantInfo::parse_required_name("Forge@[9.10.1.850,)");
    assert_eq!(d.modid, "Forge");
    assert!(d.version.contains("9.10.1.850"));

    let d = DependantInfo::parse_required_version("modid", ">=0.19.5", RangeSyntax::Semver);
    assert_eq!(d.modid, "modid");
    assert!(d.version.contains("0.20.0"));
}

/// 裸名字整串就是 modid、范围不限：当版本范围解析会让 modid 变空、依赖被丢掉
#[test]
fn parse_bare_name_is_modid() {
    let d = DependantInfo::parse_required_name("spongeapi");
    assert_eq!(d.modid, "spongeapi");
    assert_eq!(d.version, VersionRange::Any);
}

/// 缺前置：被依赖的包不在列表里 → Missing，并带上"谁要求它"
#[test]
fn missing_dep_is_reported() {
    let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("b@[1.0,)")])];

    let test = mods_dep_test(&forge_game(), &mods).unwrap();
    assert_eq!(test.problems.len(), 1);
    assert_eq!(test.problems[0].modid, "b");
    assert_eq!(test.problems[0].kind, DependantProblemKind::Missing);
    assert_eq!(test.problems[0].required_by, vec!["a".to_string()]);
    assert!(test.problems[0].installed.is_empty());
}

/// 版本落在范围内 → 不报
#[test]
fn satisfied_dep_is_silent() {
    let mods = vec![
        mod_item("a.jar", "a", "1.0", vec![need("b@[1.0,)")]),
        mod_item("b.jar", "b", "1.4", vec![]),
    ];

    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 装着了但版本不在范围内 → Mismatch，并报出实际版本
#[test]
fn version_mismatch_is_reported() {
    let mods = vec![
        mod_item("a.jar", "a", "1.0", vec![need("b@[2.0,)")]),
        mod_item("b.jar", "b", "1.4", vec![]),
    ];

    let test = mods_dep_test(&forge_game(), &mods).unwrap();
    assert_eq!(test.problems.len(), 1);
    assert_eq!(test.problems[0].kind, DependantProblemKind::Mismatch);
    assert_eq!(test.problems[0].installed, "1.4");
}

/// 实例没记版本时，游戏 / 加载器自身的前置只当"有"，不比版本
#[test]
fn loader_provided_dep_without_version_is_silent() {
    let mods = vec![mod_item(
        "a.jar",
        "a",
        "1.0",
        vec![need("minecraft@[1.20,)"), need("forge@[47,)")],
    )];

    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 游戏版本落在要求范围内 → 不报
#[test]
fn minecraft_version_satisfied_is_silent() {
    let mods = vec![mod_item(
        "a.jar",
        "a",
        "1.0",
        vec![need("minecraft@[1.20,1.21)")],
    )];

    let test = mods_dep_test(&forge_game_at("1.20.1", "47.2.0"), &mods).unwrap();
    assert!(test.problems.is_empty());
}

/// 模组要求 1.20，实例是 1.21 → minecraft 版本不符，并报出实例的实际版本
#[test]
fn minecraft_version_mismatch_is_reported() {
    let mods = vec![mod_item(
        "a.jar",
        "a",
        "1.0",
        vec![need("minecraft@[1.20,1.21)")],
    )];

    let test = mods_dep_test(&forge_game_at("1.21", "47.2.0"), &mods).unwrap();
    assert_eq!(test.problems.len(), 1);
    assert_eq!(test.problems[0].modid, "minecraft");
    assert_eq!(test.problems[0].kind, DependantProblemKind::Mismatch);
    assert_eq!(test.problems[0].installed, "1.21");
}

/// 加载器版本落在要求范围内 → 不报
#[test]
fn loader_version_satisfied_is_silent() {
    let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("forge@[47,)")])];

    let test = mods_dep_test(&forge_game_at("1.20.1", "47.2.0"), &mods).unwrap();
    assert!(test.problems.is_empty());
}

/// 加载器版本低于要求 → 报加载器版本不符
#[test]
fn loader_version_mismatch_is_reported() {
    let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("forge@[47,)")])];

    let test = mods_dep_test(&forge_game_at("1.20.1", "46.0.14"), &mods).unwrap();
    assert_eq!(test.problems.len(), 1);
    assert_eq!(test.problems[0].modid, "forge");
    assert_eq!(test.problems[0].kind, DependantProblemKind::Mismatch);
    assert_eq!(test.problems[0].installed, "46.0.14");
}

/// 带构建元数据的版本（`9.5.1+1.21.1`）能顶 `[9.5.1,)`
#[test]
fn build_metadata_version_satisfies_min_range() {
    let mods = vec![
        mod_item("a.jar", "a", "1.0", vec![need("b@[9.5.1,)")]),
        mod_item("b.jar", "b", "9.5.1+1.21.1", vec![]),
    ];

    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 没替换的版本占位符（`${file.jarVersion}`）当"任何版本"，不比版本
#[test]
fn placeholder_version_is_satisfied() {
    let mods = vec![
        mod_item("a.jar", "a", "1.0", vec![need("b@[2.0,)")]),
        mod_item("b.jar", "b", "${file.jarVersion}", vec![]),
    ];

    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 1.12.2 的裸版本要求（`b@4.7.5.1`）是 Maven 的"推荐版本"，装了更新版本不算不符
#[test]
fn maven_bare_requirement_is_silent() {
    let mods = vec![
        mod_item("a.jar", "a", "1.0", vec![need("b@4.7.5.1")]),
        mod_item("b.jar", "b", "4.9.3", vec![]),
    ];

    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 闭区间上界：游戏版本 1.21.1 落在 `[1.21,1.21.1]` 里
#[test]
fn inclusive_upper_bound_is_satisfied() {
    let mods = vec![mod_item(
        "a.jar",
        "a",
        "1.0",
        vec![need("minecraft@[1.21,1.21.1]")],
    )];

    let test = mods_dep_test(&forge_game_at("1.21.1", "47.2.0"), &mods).unwrap();
    assert!(test.problems.is_empty());
}

/// 裸版本要求（fabric 的 `depends` 写法）按"这一档"匹配：要求 `1.21`、实例 `1.21.1` 算符合
#[test]
fn bare_version_requirement_matches_next_patch() {
    let mods = vec![mod_item(
        "a.jar",
        "a",
        "1.0",
        vec![DependantType::Required(
            DependantInfo::parse_required_version("minecraft", "1.21", RangeSyntax::Semver),
        )],
    )];

    let test = mods_dep_test(&forge_game_at("1.21.1", "47.2.0"), &mods).unwrap();
    assert!(test.problems.is_empty());
}

/// Forge 的裸版本要求（`minecraft@1.21`）按 Maven 的"推荐版本"处理，更宽松
#[test]
fn maven_bare_minecraft_requirement_is_silent() {
    let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("minecraft@1.21")])];

    let test = mods_dep_test(&forge_game_at("1.21.1", "47.2.0"), &mods).unwrap();
    assert!(test.problems.is_empty());
}

/// 游戏版本按"这一档"：minecraft 要求 `[1.21]`，装 1.21.1 算符合
#[test]
fn minecraft_exact_line_matches_next_patch() {
    let mods = vec![mod_item(
        "a.jar",
        "a",
        "1.0",
        vec![need("minecraft@[1.21]")],
    )];

    let test = mods_dep_test(&forge_game_at("1.21.1", "47.2.0"), &mods).unwrap();
    assert!(test.problems.is_empty());
}

/// 模组之间不放宽：`b@[1.21]` 装了 1.21.1 照样报版本不符
#[test]
fn mod_exact_line_is_strict() {
    let mods = vec![
        mod_item("a.jar", "a", "1.0", vec![need("b@[1.21]")]),
        mod_item("b.jar", "b", "1.21.1", vec![]),
    ];

    let test = mods_dep_test(&forge_game(), &mods).unwrap();
    assert_eq!(test.problems.len(), 1);
    assert_eq!(test.problems[0].kind, DependantProblemKind::Mismatch);
}

/// 上界写成不包含、装的正是上界那个版本：minecraft 这类虚拟前置按"符合"处理
#[test]
fn exclusive_upper_bound_of_game_is_satisfied() {
    let mods = vec![mod_item(
        "a.jar",
        "a",
        "1.0",
        vec![need("minecraft@[1.21,1.21.1)")],
    )];

    let test = mods_dep_test(&forge_game_at("1.21.1", "47.2.0"), &mods).unwrap();
    assert!(test.problems.is_empty());
}

/// 模组之间不宽容：`[1.0,2.0)` 是真的排除了 2.0
#[test]
fn exclusive_upper_bound_of_mod_is_reported() {
    let mods = vec![
        mod_item("a.jar", "a", "1.0", vec![need("b@[1.0,2.0)")]),
        mod_item("b.jar", "b", "2.0", vec![]),
    ];

    let test = mods_dep_test(&forge_game(), &mods).unwrap();
    assert_eq!(test.problems.len(), 1);
    assert_eq!(test.problems[0].kind, DependantProblemKind::Mismatch);
}

/// Fabric 的 `fabric` 不排：Fabric API 的 modid 就是它，缺了要报出来
#[test]
fn fabric_api_is_not_treated_as_loader() {
    let game = InstanceSettingObj {
        loader: LoaderType::Fabric,
        ..Default::default()
    };
    let fabric_mod = |file: &str, modid: &str, dep: &str| {
        let mut item = mod_item(file, modid, "1.0", vec![need(dep)]);
        item.info[0].loaders = LoaderType::Fabric;
        item
    };
    let mods = vec![
        fabric_mod("a.jar", "a", "fabric"),
        // 加载器自己的 id 则要排掉
        fabric_mod("b.jar", "b", "fabricloader"),
    ];

    let test = mods_dep_test(&game, &mods).unwrap();
    assert_eq!(test.problems.len(), 1);
    assert_eq!(test.problems[0].modid, "fabric");
}

/// 只有禁用状态的包提供它 → Disabled，与"缺失"分开
#[test]
fn disabled_dep_is_reported() {
    let mut dep = mod_item("b.jar", "b", "1.4", vec![]);
    dep.disable = true;
    let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("b")]), dep];

    let test = mods_dep_test(&forge_game(), &mods).unwrap();
    assert_eq!(test.problems.len(), 1);
    assert_eq!(test.problems[0].kind, DependantProblemKind::Disabled);
}

/// 内置 jar-in-jar 里的模组能顶上依赖，不算缺失
#[test]
fn bundled_dep_is_satisfied() {
    let mut a = mod_item("a.jar", "a", "1.0", vec![need("lib@[2.0,)")]);
    a.jar_in_jar = vec![mod_item("lib.jar", "lib", "2.1", vec![])];

    assert!(
        mods_dep_test(&forge_game(), &vec![a])
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 同一个 modid 被多个包内置、版本不同时，**任一份符合**就算满足
///
/// 只留最后一份的话，这里能顶上的 2.1 会被后面那份 1.5 覆盖掉，报出假的版本不符。
#[test]
fn any_bundled_version_can_satisfy() {
    let mut a = mod_item("a.jar", "a", "1.0", vec![need("lib@[2.0,)")]);
    a.jar_in_jar = vec![mod_item("lib-new.jar", "lib", "2.1", vec![])];
    let mut c = mod_item("c.jar", "c", "1.0", vec![]);
    c.jar_in_jar = vec![mod_item("lib-old.jar", "lib", "1.5", vec![])];

    let mods = vec![a, c];
    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 依赖检测不再管重复：同一个 modid 装两份也不出问题
#[test]
fn dep_test_ignores_duplicates() {
    let mods = vec![
        mod_item("sodium-1.jar", "sodium", "0.5", vec![]),
        mod_item("sodium-2.jar", "sodium", "0.6", vec![]),
    ];

    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 同一个 modid 装了两份启用的包 → 重复，列出文件名与各自版本
#[test]
fn duplicate_modids_are_reported() {
    let mods = vec![
        mod_item("sodium-1.jar", "sodium", "0.5", vec![]),
        mod_item("sodium-2.jar", "sodium", "0.6", vec![]),
    ];

    let list = mods_duplicate_test(&mods);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].modid, "sodium");
    assert_eq!(
        list[0].files,
        vec![
            ModDuplicateFile {
                file: "sodium-1.jar".to_string(),
                version: "0.5".to_string(),
            },
            ModDuplicateFile {
                file: "sodium-2.jar".to_string(),
                version: "0.6".to_string(),
            },
        ]
    );
}

/// 禁用的包不算重复
#[test]
fn disabled_duplicate_is_silent() {
    let mut disabled = mod_item("sodium-2.jar", "sodium", "0.6", vec![]);
    disabled.disable = true;
    let mods = vec![mod_item("sodium-1.jar", "sodium", "0.5", vec![]), disabled];

    assert!(mods_duplicate_test(&mods).is_empty());
}

/// 同一个包里多份元数据声明同一个 modid 不算重复，只有不同文件才算
#[test]
fn same_file_metadata_is_not_duplicate() {
    let mut item = mod_item("sodium.jar", "sodium", "0.5", vec![]);
    item.info.push(ModItemObj {
        mod_id: "sodium".to_string(),
        name: "sodium".to_string(),
        version: Some("0.5".to_string()),
        ..Default::default()
    });

    assert!(mods_duplicate_test(&vec![item]).is_empty());
}

/// 推荐前置不报，禁用包声明的依赖也不报
#[test]
fn recommend_and_disabled_requirer_are_silent() {
    let mut disabled = mod_item(
        "c.jar",
        "c",
        "1.0",
        vec![DependantType::Required(DependantInfo::parse_required_name(
            "d",
        ))],
    );
    disabled.disable = true;
    let mods = vec![
        mod_item(
            "a.jar",
            "a",
            "1.0",
            vec![DependantType::Recommend(
                DependantInfo::parse_required_name("z"),
            )],
        ),
        disabled,
    ];

    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

/// 装了但没写版本号时不判版本不符，避免把"元数据没写"报成问题
#[test]
fn empty_installed_version_is_not_mismatch() {
    let mut b = mod_item("b.jar", "b", "", vec![]);
    b.info[0].version = None;
    let mods = vec![mod_item("a.jar", "a", "1.0", vec![need("b@[2.0,)")]), b];

    assert!(
        mods_dep_test(&forge_game(), &mods)
            .unwrap()
            .problems
            .is_empty()
    );
}

// ==================== 图标 ====================

/// 模板里被注释掉的 logoFile 必须跳过，这是最常见的形态
#[test]
fn logo_file_ignores_comments() {
    let text = "#logoFile=\"examplemod.png\"\nmodId=\"demo\"\n";
    assert_eq!(forge_logo_file(text), None);
}

/// 真写上的 logoFile 取得到，引号与行内注释都去掉
#[test]
fn logo_file_reads_value() {
    assert_eq!(
        forge_logo_file("modId=\"demo\"\nlogoFile=\"icon/logo.png\"\n").unwrap(),
        "icon/logo.png"
    );
    assert_eq!(
        forge_logo_file("logoFile = 'a.png' # 图标").unwrap(),
        "a.png"
    );
    assert_eq!(forge_logo_file("  logoFile=\"b.png\"  ").unwrap(), "b.png");
}

/// mcmod.info 顶层数组形态：取第一条写了 logoFile 的
#[test]
fn mcmod_logo_file_reads_array() {
    let json = r#"[
        {"modid": "demo", "name": "Demo"},
        {"modid": "other", "logoFile": "assets/logo.png"}
    ]"#;
    assert_eq!(mcmod_logo_file_in(json).unwrap(), "assets/logo.png");
}

/// 带 `modList` 的对象形态，部分老包这么写，空值跳过
#[test]
fn mcmod_logo_file_reads_mod_list() {
    let json = r#"{"modListVersion": 2, "modList": [
        {"modid": "demo", "logoFile": ""},
        {"modid": "other", "logoFile": "icon.png"}
    ]}"#;
    assert_eq!(mcmod_logo_file_in(json).unwrap(), "icon.png");
}

/// 非法的单引号 JSON 也能读，走 sanitize_mcmod_json 那条容错路径
#[test]
fn mcmod_logo_file_sanitizes_single_quotes() {
    let json = "[{'modid': 'demo', 'logoFile': 'logo.png'}]";
    assert_eq!(mcmod_logo_file_in(json).unwrap(), "logo.png");
}

/// 没写 logoFile / 结构不认识 / 文本不是 JSON 时返回 None，不 panic
#[test]
fn mcmod_logo_file_absent() {
    assert_eq!(mcmod_logo_file_in(r#"[{"modid": "demo"}]"#), None);
    assert_eq!(mcmod_logo_file_in("{}"), None);
    assert_eq!(mcmod_logo_file_in("not a json"), None);
}
