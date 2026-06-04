use std::path::PathBuf;

use serde::Serialize;
use specta::Type;

use crate::error::IpcError;

const SYMLINK_PATH: &str = "/usr/local/bin/mango";
const SIDECAR_NAME: &str = "mango-cli";

#[derive(Debug, Serialize, Type)]
pub struct CliStatus {
    pub installed: bool,
    pub symlink_target: Option<String>,
    pub points_to_current_app: bool,
}

fn sidecar_bin_path() -> Result<PathBuf, IpcError> {
    let exe = std::env::current_exe()
        .map_err(|e| IpcError::internal(format!("cannot resolve current exe: {e}")))?;
    let dir = exe
        .parent()
        .ok_or_else(|| IpcError::internal("current exe has no parent directory"))?;
    Ok(dir.join(SIDECAR_NAME))
}

#[tauri::command]
#[specta::specta]
pub async fn check_cli_installed() -> Result<CliStatus, IpcError> {
    let expected = sidecar_bin_path()?;
    let symlink = PathBuf::from(SYMLINK_PATH);

    let meta = match std::fs::symlink_metadata(&symlink) {
        Ok(m) => m,
        Err(_) => {
            return Ok(CliStatus {
                installed: false,
                symlink_target: None,
                points_to_current_app: false,
            });
        }
    };

    if !meta.file_type().is_symlink() {
        return Ok(CliStatus {
            installed: true,
            symlink_target: Some(SYMLINK_PATH.to_string()),
            points_to_current_app: false,
        });
    }

    let target = std::fs::read_link(&symlink)
        .map_err(|e| IpcError::internal(format!("cannot read symlink: {e}")))?;

    let points_to_current = target == expected;

    Ok(CliStatus {
        installed: true,
        symlink_target: Some(target.to_string_lossy().into_owned()),
        points_to_current_app: points_to_current,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn install_cli() -> Result<(), IpcError> {
    let source = sidecar_bin_path()?;
    if !source.exists() {
        return Err(IpcError::internal(format!(
            "CLI binary not found at {}",
            source.display()
        )));
    }

    let source_str = source.to_string_lossy();
    let cmd = format!(
        "ln -sf '{}' '{}'",
        source_str.replace('\'', "'\\''"),
        SYMLINK_PATH
    );

    // Try direct symlink first (works if /usr/local/bin is user-writable).
    if try_direct_symlink(&source).is_ok() {
        return Ok(());
    }

    // Fall back to osascript for admin escalation.
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            &format!(
                "do shell script \"{}\" with administrator privileges",
                cmd.replace('\\', "\\\\").replace('"', "\\\"")
            ),
        ])
        .output()
        .map_err(|e| IpcError::internal(format!("failed to launch osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            return Err(IpcError {
                message: "user cancelled authorization".into(),
                code: "USER_CANCELLED".into(),
                request_id: None,
                http_status: None,
                kind: None,
            });
        }
        return Err(IpcError::internal(format!(
            "osascript failed: {}",
            stderr.trim()
        )));
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn uninstall_cli() -> Result<(), IpcError> {
    let symlink = PathBuf::from(SYMLINK_PATH);
    if !symlink.exists() && std::fs::symlink_metadata(&symlink).is_err() {
        return Ok(());
    }

    // Try direct removal first.
    if std::fs::remove_file(&symlink).is_ok() {
        return Ok(());
    }

    let cmd = format!("rm -f '{}'", SYMLINK_PATH);
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            &format!(
                "do shell script \"{}\" with administrator privileges",
                cmd.replace('\\', "\\\\").replace('"', "\\\"")
            ),
        ])
        .output()
        .map_err(|e| IpcError::internal(format!("failed to launch osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            return Err(IpcError {
                message: "user cancelled authorization".into(),
                code: "USER_CANCELLED".into(),
                request_id: None,
                http_status: None,
                kind: None,
            });
        }
        return Err(IpcError::internal(format!(
            "osascript failed: {}",
            stderr.trim()
        )));
    }

    Ok(())
}

fn try_direct_symlink(source: &std::path::Path) -> Result<(), std::io::Error> {
    let symlink = PathBuf::from(SYMLINK_PATH);
    // Remove existing symlink/file if present.
    if symlink.exists() || std::fs::symlink_metadata(&symlink).is_ok() {
        std::fs::remove_file(&symlink)?;
    }
    std::os::unix::fs::symlink(source, &symlink)
}
