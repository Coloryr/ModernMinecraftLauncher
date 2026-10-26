//! 配置文件路径由唯一的位置参数指定。

use mml_names::{
    i18,
    i18_items::{
        error_type::{ErrorData, ErrorType},
        info_type::InfoType,
    },
};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let _ = mml_names::get_lang_type();
    match serve().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "{}",
                i18::get_error(ErrorType::CloudServerFailed(ErrorData::from_error(error)))
            );
            std::process::ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "cloud-server.json".into());
    let config = mml_cloud_server::read_config(std::path::Path::new(&path))?;
    let listener = tokio::net::TcpListener::bind(&config.listen).await?;
    let app = mml_cloud_server::router(config)?;
    eprintln!(
        "{}",
        i18::get_info(InfoType::CloudServerStart {
            address: listener.local_addr()?.to_string(),
        })
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
