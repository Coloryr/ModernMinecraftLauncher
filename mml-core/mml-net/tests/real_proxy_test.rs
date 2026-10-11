//! 实测：用**已配置好的用户代理**发起真实加载器查询，看能否成功
//!
//! 本测试直接调用 `get_support_loaders`，用真实网络加真实代理配置，看每一路的结果
//! 与错误，定位到底哪一路失败。

use mml_config::config_obj::{ProxyState, ProxyType};
use std::sync::Arc;
use std::time::Duration;

/// 初始化日志、配置与 HTTP 客户端，进程内只跑一次
fn init_once() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let exe = std::env::current_exe().unwrap();
        let run_dir = exe.parent().unwrap().parent().unwrap().to_path_buf();
        let _ = mml_log::start(&run_dir);
        let _ = mml_config::init(&run_dir);
        mml_net::init();
    });
}

/// 直接用配置里的代理客户端访问 Forge 官方源，那是唯一没有镜像回退的路径
#[test]
fn forge_official_with_user_proxy() {
    init_once();

    let proxy_ip = std::env::var("MML_TEST_PROXY_IP").unwrap_or_else(|_| "127.0.0.1".to_string());
    let proxy_port: u16 = std::env::var("MML_TEST_PROXY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7890);

    let url = "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
    println!("目标：{url}");
    println!("代理：{proxy_ip}:{proxy_port}");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // 两种客户端对比：跟随系统 / 用户代理
    let cases: Vec<(&str, mml_net::Client)> = vec![
        ("Auto（跟随系统）", mml_net::Client::new(ProxyState::Auto)),
        (
            "User（用户代理）",
            mml_net::Client::new_proxy(
                ProxyType::Http,
                &proxy_ip.to_string(),
                proxy_port,
                &String::new(),
                &String::new(),
            ),
        ),
    ];

    for (name, client) in cases {
        let url = url.to_string();
        let started = std::time::Instant::now();
        let client = Arc::new(client);
        let result = rt.block_on(async move {
            tokio::time::timeout(Duration::from_secs(15), client.get_text(&url)).await
        });
        match result {
            Ok(Ok(text)) => {
                let versions = text.matches("<version>").count();
                println!(
                    "  {name}: ✓ {:?} 拿到 {} 字节，{} 个 <version>",
                    started.elapsed(),
                    text.len(),
                    versions
                );
            }
            Ok(Err(err)) => {
                println!("  {name}: ✗ {:?} 错误：{err:?}", started.elapsed());
            }
            Err(_) => {
                println!("  {name}: ✗ 15 秒超时（{:?}）", started.elapsed());
            }
        }
    }
}
