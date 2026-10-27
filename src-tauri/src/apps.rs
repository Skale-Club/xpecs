//! Installed programs, read from the same registry keys as "Apps & features".

use serde::Serialize;
use std::collections::HashSet;
use std::os::windows::process::CommandExt;
use std::process::Command;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

const SOURCES: [(bool, &str); 3] = [
    (true, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    (true, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    (false, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
];

#[derive(Serialize)]
pub struct App {
    pub id: String,
    name: String,
    publisher: String,
    version: String,
    /// Bytes, as estimated by the installer (0 = unknown).
    size: u64,
    date: String,
    /// Where its icon is ("DisplayIcon"), sent separately by `icons`.
    #[serde(skip)]
    pub icon: String,
}

fn hive(machine: bool) -> RegKey {
    RegKey::predef(if machine { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
}

pub fn list() -> Vec<App> {
    let mut apps = Vec::new();
    let mut seen = HashSet::new();
    for (index, (machine, path)) in SOURCES.iter().enumerate() {
        let Ok(root) = hive(*machine).open_subkey_with_flags(path, KEY_READ) else { continue };
        for sub in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey_with_flags(&sub, KEY_READ) else { continue };
            let text = |name: &str| key.get_value::<String, _>(name).unwrap_or_default().trim().to_string();
            let name = text("DisplayName");
            // Hidden parts, updates and entries with no way to uninstall.
            if name.is_empty()
                || text("UninstallString").is_empty()
                || key.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1
                || !text("ParentKeyName").is_empty()
                || ["Update", "Hotfix", "Security Update"].contains(&text("ReleaseType").as_str())
            {
                continue;
            }
            let version = text("DisplayVersion");
            if !seen.insert((name.to_lowercase(), version.clone())) {
                continue;
            }
            let date = text("InstallDate");
            let date = if date.len() == 8 && date.chars().all(|c| c.is_ascii_digit()) {
                format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8])
            } else {
                String::new()
            };
            apps.push(App {
                id: format!("{index}|{sub}"),
                name,
                publisher: text("Publisher"),
                version,
                size: key.get_value::<u32, _>("EstimatedSize").unwrap_or(0) as u64 * 1024,
                date,
                icon: text("DisplayIcon"),
            });
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Starts the program's own uninstaller (it shows its own window and, for
/// programs installed for all users, its own UAC prompt).
pub fn uninstall(id: &str) -> Result<(), String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let (index, sub) = id.split_once('|').ok_or("bad id")?;
    let (machine, path) = SOURCES.get(index.parse::<usize>().map_err(|_| "bad id")?).ok_or("bad id")?;
    let key = hive(*machine)
        .open_subkey_with_flags(format!(r"{path}\{sub}"), KEY_READ)
        .map_err(|e| e.to_string())?;
    let mut command: String = key.get_value("UninstallString").map_err(|e| e.to_string())?;
    // Windows Installer: /I opens the "modify" screen, /X uninstalls.
    if command.to_lowercase().contains("msiexec") {
        command = command.replace("/I{", "/X{").replace("/i{", "/X{");
    }
    // /S strips only the outer quotes we add, keeping the command intact.
    Command::new("cmd.exe")
        .args(["/S", "/C"])
        .raw_arg(format!("\"{command}\""))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
