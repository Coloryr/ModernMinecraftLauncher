//! Tauri 命令：只做参数转发、任务开始 / 结束与错误本地化
//!
//! 命令名 = 函数名；前端只通过这些命令拿数据，不在窗口之间共享状态。

use crate::{
    files::{SaveInput, SourceFile},
    messages::StudioError,
    project::{CreateProjectInput, Project},
    studio::{Studio, StudioState},
};
use std::sync::Arc;
use tauri::State;

/// 命令失败时把文案写进输出面板并回给前端
async fn finish<T>(studio: &Studio, result: Result<T, StudioError>) -> Result<T, String> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            let text = error.text(studio.locale().await);
            studio.log_raw(&format!("{text}\n"));
            Err(text)
        }
    }
}

/// 启动状态：项目、语言、配色、最近项目、外部环境
#[tauri::command]
pub async fn studio_state(studio: State<'_, Arc<Studio>>) -> Result<StudioState, String> {
    Ok(studio.inner().state().await)
}

/// 选择目录，返回空表示用户取消
#[tauri::command]
pub async fn studio_pick(
    studio: State<'_, Arc<Studio>>,
    input: String,
) -> Result<Option<String>, String> {
    let studio = studio.inner().clone();
    finish(&studio, studio.pick(input).await).await
}

/// 新建项目
#[tauri::command]
pub async fn studio_create(
    studio: State<'_, Arc<Studio>>,
    input: CreateProjectInput,
) -> Result<Project, String> {
    let studio = studio.inner().clone();
    let cancel = finish(&studio, studio.begin_task().await).await?;
    let result = studio.create(input, &cancel).await;
    studio.end_task().await;
    finish(&studio, result).await
}

/// 打开项目
#[tauri::command]
pub async fn studio_open(studio: State<'_, Arc<Studio>>, input: String) -> Result<Project, String> {
    let studio = studio.inner().clone();
    finish(&studio, studio.open(&input).await).await
}

/// 项目文件树
#[tauri::command]
pub async fn studio_files(studio: State<'_, Arc<Studio>>) -> Result<Vec<String>, String> {
    let studio = studio.inner().clone();
    finish(&studio, studio.files().await).await
}

/// 读取文件
#[tauri::command]
pub async fn studio_read(
    studio: State<'_, Arc<Studio>>,
    input: String,
) -> Result<SourceFile, String> {
    let studio = studio.inner().clone();
    finish(&studio, studio.read(&input).await).await
}

/// 保存文件
#[tauri::command]
pub async fn studio_save(
    studio: State<'_, Arc<Studio>>,
    input: SaveInput,
) -> Result<SourceFile, String> {
    let studio = studio.inner().clone();
    finish(&studio, studio.save(input).await).await
}

/// 读取项目 SDK 的控件与变量清单
#[tauri::command]
pub async fn studio_catalog(studio: State<'_, Arc<Studio>>) -> Result<serde_json::Value, String> {
    let studio = studio.inner().clone();
    finish(&studio, studio.catalog().await).await
}

/// 安装依赖
#[tauri::command]
pub async fn studio_install(studio: State<'_, Arc<Studio>>) -> Result<(), String> {
    let studio = studio.inner().clone();
    let cancel = finish(&studio, studio.begin_task().await).await?;
    let result = studio.install(&cancel).await;
    studio.end_task().await;
    finish(&studio, result).await
}

/// 构建 ZIP
#[tauri::command]
pub async fn studio_build(studio: State<'_, Arc<Studio>>) -> Result<Vec<String>, String> {
    let studio = studio.inner().clone();
    let cancel = finish(&studio, studio.begin_task().await).await?;
    let result = studio.build(&cancel).await;
    studio.end_task().await;
    finish(&studio, result).await
}

/// 启动预览
#[tauri::command]
pub async fn studio_preview(studio: State<'_, Arc<Studio>>) -> Result<String, String> {
    let studio = studio.inner().clone();
    let cancel = finish(&studio, studio.begin_task().await).await?;
    let result = studio.start_preview(&cancel).await;
    studio.end_task().await;
    finish(&studio, result).await
}

/// 停止预览
#[tauri::command]
pub async fn studio_stop(studio: State<'_, Arc<Studio>>) -> Result<(), String> {
    let studio = studio.inner().clone();
    studio.stop_preview().await;
    Ok(())
}

/// 取消当前任务
#[tauri::command]
pub async fn studio_cancel(studio: State<'_, Arc<Studio>>) -> Result<(), String> {
    studio.inner().cancel().await;
    Ok(())
}

/// 打开开发者工具；预览 iframe 在同一套 DevTools 里
#[tauri::command]
pub async fn studio_devtools(studio: State<'_, Arc<Studio>>) -> Result<(), String> {
    studio.inner().open_devtools();
    Ok(())
}

/// 切换界面语言
#[tauri::command]
pub async fn studio_locale(studio: State<'_, Arc<Studio>>, input: String) -> Result<(), String> {
    let studio = studio.inner().clone();
    finish(&studio, studio.set_locale(&input).await).await?;
    Ok(())
}

/// 切换界面配色，返回实际生效的值
#[tauri::command]
pub async fn studio_theme(studio: State<'_, Arc<Studio>>, input: String) -> Result<String, String> {
    let studio = studio.inner().clone();
    let theme = finish(&studio, studio.set_theme(&input).await).await?;
    Ok(theme.as_str().to_string())
}

/// 同步未保存状态，关闭窗口时用来决定要不要确认
#[tauri::command]
pub async fn studio_dirty(studio: State<'_, Arc<Studio>>, input: bool) -> Result<(), String> {
    studio.inner().set_dirty(input);
    Ok(())
}

/// 在文件管理器里打开项目目录
#[tauri::command]
pub async fn studio_reveal(studio: State<'_, Arc<Studio>>) -> Result<String, String> {
    let studio = studio.inner().clone();
    finish(&studio, studio.reveal().await).await
}
