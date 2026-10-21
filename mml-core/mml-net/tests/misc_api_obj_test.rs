//! 其余 API 数据结构的离线反序列化测试。
//!
//! 覆盖：Nide8、OpenFrp、SakuraFrp、Authlib-Injector、OptiFine 的 BMCLAPI 镜像
//! 返回 JSON，以及 Adoptium。样例 JSON 均为对应 API 真实响应结构的内嵌字符串，
//! 不联网。

use mml_net::adoptium_api::{AdoptiumJavaVersionObj, AdoptiumObj};
use mml_net::authlib_api::{ArtifactsObj, AuthlibInjectorMetaObj, AuthlibInjectorObj};
use mml_net::nide8_api::Nide8Obj;
use mml_net::openfrp_api::{OpenFrpChannelInfoObj, OpenFrpChannelObj, OpenFrpDownloadObj};
use mml_net::optifine_api::OptifineListObj;
use mml_net::sakurafrp_api::SakuraFrpDownloadObj;

/// 统一通行证 JAR 信息接口的响应
const NIDE8_JSON: &str = r#"{
    "jarVersion": "1.2.10",
    "jarHash": "5f1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c"
}"#;

/// OpenFrp 通道列表接口 `?action=getallproxies` 的响应
const OPENFRP_CHANNEL_JSON: &str = r#"{
    "state": "success",
    "data": [
        {
            "name": "香港",
            "proxies": [
                { "name": "mc-server", "id": 10086, "type": "tcp", "remote": "hk-1.openfrp.net:12345" }
            ]
        }
    ]
}"#;

/// OpenFrp 通道配置接口 `?action=getproxy` 的响应
const OPENFRP_CONFIG_JSON: &str = r#"{
    "proxies": {
        "1.2.3.4": "节点说明",
        "hk-1.openfrp.net": "香港节点"
    }
}"#;

/// OpenFrp 下载列表接口的响应
const OPENFRP_DOWNLOAD_JSON: &str = r#"{
    "state": "success",
    "key": "software",
    "data": {
        "latest": "0.51.3",
        "latest_full": "0.51.3",
        "source": [
            { "value": "https://ghproxy.net/https://github.com/openfrp/frpc/releases/download/" }
        ]
    }
}"#;

/// SakuraFrp 客户端下载接口 `system/clients` 的响应
const SAKURA_DOWNLOAD_JSON: &str = r#"{
    "frpc": {
        "ver": "1.0.3",
        "archs": {
            "windows_amd64": { "title": "Windows x64", "url": "https://nya.globalslb.net/natfrp/client/frpc/1.0.3/frpc_windows_amd64.zip", "hash": "abc123" },
            "windows_arm64": { "title": "Windows ARM64", "url": "https://nya.globalslb.net/natfrp/client/frpc/1.0.3/frpc_windows_arm64.zip", "hash": "def456" },
            "linux_amd64": { "title": "Linux x64", "url": "https://nya.globalslb.net/natfrp/client/frpc/1.0.3/frpc_linux_amd64.tar.gz", "hash": "aaa111" },
            "linux_arm64": { "title": "Linux ARM64", "url": "https://nya.globalslb.net/natfrp/client/frpc/1.0.3/frpc_linux_arm64.tar.gz", "hash": "bbb222" },
            "darwin_amd64": { "title": "macOS x64", "url": "https://nya.globalslb.net/natfrp/client/frpc/1.0.3/frpc_darwin_amd64.tar.gz", "hash": "ccc333" },
            "darwin_arm64": { "title": "macOS ARM64", "url": "https://nya.globalslb.net/natfrp/client/frpc/1.0.3/frpc_darwin_arm64.tar.gz", "hash": "ddd444" }
        }
    }
}"#;

/// Authlib-Injector 元数据接口 `artifacts.json` 的响应
const AUTHLIB_META_JSON: &str = r#"{
    "meta": {
        "version": 1,
        "release_time": "2026-01-01T00:00:00Z"
    },
    "latest_build_number": 64,
    "artifacts": [
        {
            "build_number": 62,
            "created": "2025-06-01T00:00:00Z",
            "version": "1.2.7",
            "download_url": "https://authlib-injector.yushi.moe/artifact/62/authlib-injector-1.2.7.jar",
            "checksums": { "sha256": "eaf14bc5acffc7d885bd5bd5942b99f36d6299302beae356b2fc5807fe42652b" }
        },
        {
            "build_number": 64,
            "created": "2026-01-01T00:00:00Z",
            "version": "1.3.0",
            "download_url": "https://authlib-injector.yushi.moe/artifact/64/authlib-injector-1.3.0.jar",
            "checksums": { "sha256": "ff14bc5acffc7d885bd5bd5942b99f36d6299302beae356b2fc5807fe42652b" }
        }
    ]
}"#;

/// Authlib-Injector 单个 artifact 详情接口的响应
const AUTHLIB_OBJ_JSON: &str = r#"{
    "build_number": 64,
    "version": "1.3.0",
    "download_url": "https://authlib-injector.yushi.moe/artifact/64/authlib-injector-1.3.0.jar",
    "checksums": { "sha256": "ff14bc5acffc7d885bd5bd5942b99f36d6299302beae356b2fc5807fe42652bc" }
}"#;

/// BMCLAPI 的 OptiFine 版本列表接口 `optifine/versionList` 的响应
const OPTIFINE_LIST_JSON: &str = r#"[
    {
        "mcversion": "1.20.4",
        "patch": "L5",
        "type": "HD_U",
        "filename": "OptiFine_1.20.4_HD_U_L5.jar",
        "forge": "unavailable"
    },
    {
        "mcversion": "1.21.4",
        "patch": "I7",
        "type": "HD_U",
        "filename": "preview_OptiFine_1.21.4_HD_U_I7.jar",
        "forge": "N/A"
    }
]"#;

/// Adoptium Java 版本列表接口 `v3/info/available_releases` 的响应
const ADOPTIUM_RELEASES_JSON: &str = r#"{
    "available_releases": [8, 11, 17, 21, 25]
}"#;

/// Adoptium 资产接口 `v3/assets/latest/{ver}/hotspot` 的响应
const ADOPTIUM_ASSETS_JSON: &str = r#"[
    {
        "download_count": 1000000,
        "id": "abc123",
        "release_name": "jdk-21.0.5+11",
        "release_type": "ga",
        "binary": {
            "architecture": "x64",
            "image_type": "jdk",
            "os": "windows",
            "scm_ref": "jdk-21.0.5+11_adopt",
            "package": {
                "checksum": "sha256-checksum-value",
                "link": "https://github.com/adoptium/temurin21-binaries/releases/download/x/OpenJDK21U-jdk_x64_windows_hotspot_21.0.5_11.zip",
                "name": "OpenJDK21U-jdk_x64_windows_hotspot_21.0.5_11.zip",
                "size": 190000000
            }
        },
        "version": {
            "openjdk_version": "21.0.5+11",
            "semver": "21.0.5+11.0.LTS"
        }
    }
]"#;

/// Nide8 JAR 信息反序列化：camelCase 字段映射
#[test]
fn deserialize_nide8_obj() {
    let obj: Nide8Obj = serde_json::from_str(NIDE8_JSON).unwrap();
    assert_eq!(obj.jar_version, "1.2.10");
    assert_eq!(obj.jar_hash, "5f1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c");
}

/// OpenFrp 通道列表反序列化
#[test]
fn deserialize_openfrp_channel() {
    let obj: OpenFrpChannelObj = serde_json::from_str(OPENFRP_CHANNEL_JSON).unwrap();
    assert_eq!(obj.data.len(), 1);
    assert_eq!(obj.data[0].name, "香港");
    assert_eq!(obj.data[0].proxies.len(), 1);
    let proxy = &obj.data[0].proxies[0];
    assert_eq!(proxy.id, 10086);
    assert_eq!(proxy.p_type, "tcp");
    assert_eq!(proxy.remote, "hk-1.openfrp.net:12345");
}

/// OpenFrp 通道配置反序列化
#[test]
fn deserialize_openfrp_config() {
    let obj: OpenFrpChannelInfoObj = serde_json::from_str(OPENFRP_CONFIG_JSON).unwrap();
    assert_eq!(obj.proxies.len(), 2);
    assert_eq!(
        obj.proxies.get("hk-1.openfrp.net").map(String::as_str),
        Some("香港节点")
    );
}

/// OpenFrp 下载列表反序列化
#[test]
fn deserialize_openfrp_download() {
    let obj: OpenFrpDownloadObj = serde_json::from_str(OPENFRP_DOWNLOAD_JSON).unwrap();
    assert_eq!(obj.data.latest, "0.51.3");
    assert_eq!(obj.data.source.len(), 1);
    assert!(obj.data.source[0].value.starts_with("https://"));
}

/// SakuraFrp 客户端下载信息反序列化
#[test]
fn deserialize_sakura_download() {
    let obj: SakuraFrpDownloadObj = serde_json::from_str(SAKURA_DOWNLOAD_JSON).unwrap();
    assert_eq!(obj.frpc.ver, "1.0.3");
    assert_eq!(obj.frpc.archs.windows_amd64.hash, "abc123");
    assert_eq!(obj.frpc.archs.darwin_arm64.title, "macOS ARM64");
}

/// Authlib-Injector 元数据反序列化：应能按 latest_build_number 定位最新 artifact
#[test]
fn deserialize_authlib_meta() {
    let obj: AuthlibInjectorMetaObj = serde_json::from_str(AUTHLIB_META_JSON).unwrap();
    assert_eq!(obj.latest_build_number, 64);
    assert_eq!(obj.artifacts.len(), 2);

    // 与 authlib_api::get_obj 相同的定位逻辑
    let latest = obj
        .artifacts
        .iter()
        .find(|item| item.build_number == obj.latest_build_number)
        .expect("应能定位到最新构建");
    assert_eq!(latest.build_number, 64);
}

/// Authlib-Injector artifact 详情反序列化
#[test]
fn deserialize_authlib_obj() {
    let obj: AuthlibInjectorObj = serde_json::from_str(AUTHLIB_OBJ_JSON).unwrap();
    assert_eq!(obj.version, "1.3.0");
    assert_eq!(obj.checksums.sha256.len(), 64);

    // ArtifactsObj 只有 build_number 一个字段
    let artifacts: ArtifactsObj = serde_json::from_str(r#"{ "build_number": 64 }"#).unwrap();
    assert_eq!(artifacts.build_number, 64);
}

/// BMCLAPI OptiFine 版本列表反序列化
#[test]
fn deserialize_optifine_list() {
    let obj: Vec<OptifineListObj> = serde_json::from_str(OPTIFINE_LIST_JSON).unwrap();
    assert_eq!(obj.len(), 2);
    assert_eq!(obj[0].mcversion, "1.20.4");
    assert_eq!(obj[0].rtype, "HD_U");
    assert_eq!(obj[0].patch, "L5");
    assert_eq!(obj[1].filename, "preview_OptiFine_1.21.4_HD_U_I7.jar");
}

/// Adoptium 支持版本反序列化
#[test]
fn deserialize_adoptium_releases() {
    let obj: AdoptiumJavaVersionObj = serde_json::from_str(ADOPTIUM_RELEASES_JSON).unwrap();
    assert!(obj.available_releases.contains(&8));
    assert!(obj.available_releases.contains(&25));
}

/// Adoptium 资产反序列化：嵌套 binary/package/version 结构
#[test]
fn deserialize_adoptium_assets() {
    let obj: Vec<AdoptiumObj> = serde_json::from_str(ADOPTIUM_ASSETS_JSON).unwrap();
    assert_eq!(obj.len(), 1);
    let item = &obj[0];
    assert_eq!(item.binary.architecture, "x64");
    assert_eq!(item.binary.image_type, "jdk");
    assert_eq!(
        item.binary.package.name,
        "OpenJDK21U-jdk_x64_windows_hotspot_21.0.5_11.zip"
    );
    // assert_eq!(item.version.openjdk_version, "21.0.5+11");
}
