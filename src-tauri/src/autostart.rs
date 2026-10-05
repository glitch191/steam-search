//! "Start with Windows" through the current user's `Run` registry key, so the program appears
//! in Settings > Apps > Startup and in Task Manager's Startup apps.
//!
//! Windows only launches a `Run` entry whose `StartupApproved\Run` value is enabled (or missing);
//! Settings and Task Manager toggle that value. Version 0.2.1 used a Task Scheduler logon task
//! instead; it is removed here so the program does not start twice.
//!
//! Note: when this program is started from a packaged (MSIX) app, Windows redirects its registry
//! writes to a private copy that Explorer never reads. Start it normally (from Explorer, a
//! shortcut or at sign-in) for these entries to take effect.

use crate::config::APP_NAME;
use std::os::windows::process::CommandExt;
use std::process::Command;
use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::{RegKey, RegValue};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// "Enabled" as written by Settings and Task Manager: first byte 2, then 11 zero bytes.
const APPROVED_ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
/// Prevents a console window from flashing when schtasks.exe runs.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// True when the `Run` entry exists and is not disabled in Settings or Task Manager.
pub fn is_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .and_then(|k| k.get_value::<String, _>(APP_NAME))
        .is_ok()
        && approval() != Some(false)
}

/// Approval state: None when missing, Some(true) when enabled, Some(false) when disabled
/// (odd first byte).
fn approval() -> Option<bool> {
    let value = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(APPROVED_KEY, KEY_READ)
        .and_then(|k| k.get_raw_value(APP_NAME))
        .ok()?;
    Some(value.bytes.first().is_some_and(|b| b % 2 == 0))
}

/// Enables or disables start with Windows. With `override_disabled` false, an entry the user
/// disabled in Settings or Task Manager stays disabled.
pub fn set_enabled(enabled: bool, override_disabled: bool) -> std::io::Result<()> {
    remove_logon_task();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)?;
    if enabled {
        let exe = std::env::current_exe()?;
        run.set_value(APP_NAME, &format!("\"{}\"", exe.display()))?;
        if override_disabled || approval().is_none() {
            let (approved, _) = hkcu.create_subkey(APPROVED_KEY)?;
            approved.set_raw_value(
                APP_NAME,
                &RegValue { vtype: RegType::REG_BINARY, bytes: APPROVED_ENABLED.to_vec().into() },
            )?;
        }
        Ok(())
    } else {
        if let Ok(approved) = hkcu.open_subkey_with_flags(APPROVED_KEY, KEY_SET_VALUE) {
            let _ = approved.delete_value(APP_NAME);
        }
        match run.delete_value(APP_NAME) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

/// Deletes the logon task created by version 0.2.1, if present.
fn remove_logon_task() {
    let exists = Command::new("schtasks.exe")
        .args(["/query", "/tn", APP_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|o| o.status.success());
    if exists {
        let _ = Command::new("schtasks.exe")
            .args(["/delete", "/tn", APP_NAME, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}
