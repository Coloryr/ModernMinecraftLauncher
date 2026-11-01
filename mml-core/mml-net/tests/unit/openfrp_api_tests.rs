use super::download_url;

/// 下载源与版本目录带不带斜杠都要拼出单斜杠
#[test]
fn download_url_joins_slashes() {
    assert_eq!(
        download_url(
            "https://staticassets.naids.com/client",
            "/OF_0.61.1_xxx/",
            "OF_0.61.1_xxx",
            "frpc_windows_amd64.zip"
        ),
        "https://staticassets.naids.com/client/OF_0.61.1_xxx/frpc_windows_amd64.zip"
    );
    assert_eq!(
        download_url(
            "https://r.zyghit.cn/download/client/",
            "OF_0.61.1_xxx",
            "OF_0.61.1_xxx",
            "frpc_linux_amd64.tar.gz"
        ),
        "https://r.zyghit.cn/download/client/OF_0.61.1_xxx/frpc_linux_amd64.tar.gz"
    );
    // latest 为空时退回 latest_full
    assert_eq!(
        download_url("https://x/client", "", "OF_1", "frpc_darwin_arm64.tar.gz"),
        "https://x/client/OF_1/frpc_darwin_arm64.tar.gz"
    );
}

