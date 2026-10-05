//! "Start with Windows" through a Task Scheduler task that runs at the current user's sign-in.
//!
//! An earlier version used the `HKCU\...\Run` registry key. When the program is started from a
//! packaged (MSIX) app, Windows redirects its registry writes to a private copy, so that entry was
//! never seen at sign-in. The task is registered by the Task Scheduler service, which is not
//! affected, and needs no administrator rights. Leftover `Run` entries are removed.

use crate::config::APP_NAME;
use std::os::windows::process::CommandExt;
use std::process::{Command, Output};
use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// Prevents a console window from flashing when schtasks.exe runs.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn schtasks(args: &[&str]) -> std::io::Result<Output> {
    Command::new("schtasks.exe").args(args).creation_flags(CREATE_NO_WINDOW).output()
}

/// The task definition as XML, or None when the task does not exist.
fn task_xml() -> Option<String> {
    let out = schtasks(&["/query", "/tn", APP_NAME, "/xml"]).ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// True when the task exists and was not disabled in Task Scheduler.
pub fn is_enabled() -> bool {
    task_xml().is_some_and(|xml| !xml.contains("<Enabled>false</Enabled>"))
}

/// Enables or disables start with Windows. With `override_disabled` false, a task the user
/// disabled in Task Scheduler stays disabled.
pub fn set_enabled(enabled: bool, override_disabled: bool) -> std::io::Result<()> {
    remove_run_entry();
    if !enabled {
        if task_xml().is_some() {
            check(schtasks(&["/delete", "/tn", APP_NAME, "/f"])?)?;
        }
        return Ok(());
    }
    let exe = std::env::current_exe()?.display().to_string();
    let escaped = escape_xml(&exe);
    match task_xml() {
        Some(xml) if xml.contains("<Enabled>false</Enabled>") && !override_disabled => Ok(()),
        Some(xml) if xml.contains(&escaped) && !xml.contains("<Enabled>false</Enabled>") => Ok(()),
        _ => create_task(&escaped),
    }
}

fn create_task(exe_escaped: &str) -> std::io::Result<()> {
    let user = escape_xml(&format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default()
    ));
    // Priority 5 keeps the normal process priority (the Task Scheduler default, 7, is below normal).
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Starts {APP_NAME} at sign-in.</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author"><Exec><Command>"{exe_escaped}"</Command></Exec></Actions>
</Task>
"#
    );
    let file = std::env::temp_dir().join(format!("{APP_NAME}-task.xml"));
    // schtasks expects UTF-16 with a byte order mark for this declaration.
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    std::fs::write(&file, bytes)?;
    let result = schtasks(&["/create", "/tn", APP_NAME, "/xml", &file.to_string_lossy(), "/f"]);
    let _ = std::fs::remove_file(&file);
    check(result?)
}

fn check(out: Output) -> std::io::Result<()> {
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

/// Removes the registry entries written by versions up to 0.2.0.
fn remove_run_entry() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for key in [RUN_KEY, APPROVED_KEY] {
        if let Ok(k) = hkcu.open_subkey_with_flags(key, KEY_SET_VALUE) {
            let _ = k.delete_value(APP_NAME);
        }
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_xml_special_characters() {
        assert_eq!(escape_xml(r#"C:\A&B\<x>"y".exe"#), r#"C:\A&amp;B\&lt;x&gt;&quot;y&quot;.exe"#);
    }
}
