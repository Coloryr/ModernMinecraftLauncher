use super::ConfigObj;

/// ColorMC 的 config.json 原样（代理用户名 / 密码写的是 null）
///
/// 迁移后 M²L 要能读它，否则核心初始化直接失败、启动器起不来。
#[test]
fn color_mc_config_with_null_proxy_loads() {
    let json = r#"{
  "Version": "40.20260928",
  "JavaList": [
{
  "Name": "java_1.8.0_301",
  "Local": "C:\\Program Files\\Java\\jre1.8.0_301\\bin\\javaw.exe"
}
  ],
  "Http": {
"Source": 0,
"DownloadThread": 20,
"ProxyIP": "127.0.0.1",
"ProxyPort": 7890,
"ProxyUser": null,
"ProxyPassword": null,
"LoginProxy": true,
"DownloadProxy": true,
"GameProxy": false,
"CheckFile": true,
"AutoDownload": true
  },
  "Dns": {
"Enable": false,
"Https": [
  "https://223.5.5.5/dns-query"
],
"HttpProxy": false
  },
  "DefaultJvmArg": {
"RemoveJvmArg": null,
"RemoveGameArg": null,
"JvmArgs": null,
"GameArgs": null,
"JvmEnv": null,
"GC": 0,
"MaxMemory": 4096,
"MinMemory": 512,
"ColorASM": false,
"LaunchPre": false,
"PreRunSame": false,
"LaunchPreData": null,
"LaunchPost": false,
"LaunchPostData": null
  },
  "Window": {
"FullScreen": false,
"Height": 1080,
"Width": 1920,
"GameTitle": null,
"RandomTitle": false,
"CycTitle": false,
"TitleDelay": 0,
"EditTitle": false
  },
  "GameCheck": {
"CheckCore": true,
"CheckLib": true,
"CheckAssets": true,
"CheckMod": true,
"CheckCoreSha1": true,
"CheckLibSha1": true,
"CheckAssetsSha1": true,
"CheckModSha1": true
  },
  "SafeLog4j": false
}"#;
    let obj: ConfigObj = serde_json::from_str(json).expect("ColorMC 的配置要能读");
    assert_eq!(obj.http.proxy_port, 7890);
    assert!(obj.http.proxy_user.is_empty());
    assert_eq!(obj.java_list.len(), 1);
}

