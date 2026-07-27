use crate::{AppError, AppResult};
use serde::Serialize;
use tokio::process::Command;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenChromeResult {
    application: &'static str,
    opened: bool,
}

#[cfg(target_os = "macos")]
fn chrome_command() -> Command {
    let mut command = Command::new("open");
    command.args(["-a", "Google Chrome"]);
    command
}

#[cfg(target_os = "windows")]
fn chrome_command() -> Command {
    let mut command = Command::new("cmd");
    command.args(["/C", "start", "", "chrome"]);
    command
}

#[cfg(target_os = "linux")]
fn chrome_command() -> Command {
    Command::new("google-chrome")
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn chrome_command() -> Command {
    // 保持移动端等目标可以编译；调用时会由操作系统返回找不到程序。
    Command::new("google-chrome")
}

/// Mock MCP 最终调用的本地能力。
///
/// 这里只负责操作系统调用，不包含 Tool、Agent 或 MCP 协议逻辑。
#[tauri::command]
pub async fn open_chrome() -> AppResult<OpenChromeResult> {
    let status = chrome_command().status().await?;

    if !status.success() {
        return Err(AppError::Shell(format!(
            "failed to open Google Chrome: {status}"
        )));
    }

    Ok(OpenChromeResult {
        application: "Google Chrome",
        opened: true,
    })
}
