//! "Start with Windows" through the current user's `Run` registry key.
//!
//! Recent Windows builds also require an "enabled" entry under `StartupApproved\Run` (the state
//! shown in Task Manager's Startup apps); without it the `Run` entry is skipped at sign-in.

use crate::config::APP_NAME;
use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::{RegKey, RegValue};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// Task Manager's "enabled" value: first byte 2, followed by 11 zero bytes.
const APPROVED_ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub fn is_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .and_then(|k| k.get_value::<String, _>(APP_NAME))
        .is_ok()
        && approval() != Some(false)
}

/// Approval state: None when missing, Some(true) when enabled, Some(false) when disabled
/// in Task Manager (first byte odd).
fn approval() -> Option<bool> {
    let value = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(APPROVED_KEY, KEY_READ)
        .and_then(|k| k.get_raw_value(APP_NAME))
        .ok()?;
    Some(value.bytes.first().is_some_and(|b| b % 2 == 0))
}

/// Enables or disables start with Windows. With `override_task_manager` false, an entry the user
/// disabled in Task Manager stays disabled.
pub fn set_enabled(enabled: bool, override_task_manager: bool) -> std::io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)?;
    if enabled {
        let exe = std::env::current_exe()?;
        run.set_value(APP_NAME, &format!("\"{}\"", exe.display()))?;
        if override_task_manager || approval().is_none() {
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
