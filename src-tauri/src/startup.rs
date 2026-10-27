//! Programs that start with Windows. Turning one off writes the same
//! "StartupApproved" flag the Task Manager uses, so it is reversible and the
//! program's own entry is left alone.

use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_BINARY};
use winreg::{RegKey, RegValue};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

#[derive(Serialize)]
pub struct Item {
    pub id: String,
    name: String,
    pub command: String,
    /// "user", "machine", "machine32", "folder" or "folderAll".
    location: &'static str,
    enabled: bool,
    /// Changing it needs administrator rights.
    admin: bool,
}

fn hive(machine: bool) -> RegKey {
    RegKey::predef(if machine { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
}

/// Missing flag = enabled; otherwise an even first byte means enabled.
fn approved(machine: bool, sub: &str, name: &str) -> bool {
    hive(machine)
        .open_subkey(format!(r"{APPROVED}\{sub}"))
        .and_then(|k| k.get_raw_value(name))
        .map(|v| v.bytes.first().map_or(true, |b| b & 1 == 0))
        .unwrap_or(true)
}

fn startup_folder(machine: bool) -> PathBuf {
    let (var, tail) = if machine {
        ("ProgramData", r"Microsoft\Windows\Start Menu\Programs\StartUp")
    } else {
        ("APPDATA", r"Microsoft\Windows\Start Menu\Programs\Startup")
    };
    std::env::var_os(var).map(PathBuf::from).unwrap_or_default().join(tail)
}

pub fn list() -> Vec<Item> {
    let mut items = Vec::new();
    for (machine, path, sub, location) in
        [(false, RUN, "Run", "user"), (true, RUN, "Run", "machine"), (true, RUN32, "Run32", "machine32")]
    {
        let Ok(key) = hive(machine).open_subkey_with_flags(path, KEY_READ) else { continue };
        for (name, _) in key.enum_values().flatten() {
            let command: String = key.get_value(&name).unwrap_or_default();
            // The key's unnamed "(Default)" value is not a program.
            if name.is_empty() || command.trim().is_empty() {
                continue;
            }
            items.push(Item {
                id: format!("{}|{sub}|{name}", machine as u8),
                enabled: approved(machine, sub, &name),
                name,
                command,
                location,
                admin: machine,
            });
        }
    }
    for (machine, location) in [(false, "folder"), (true, "folderAll")] {
        let dir = startup_folder(machine);
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case("desktop.ini") || !entry.path().is_file() {
                continue;
            }
            items.push(Item {
                id: format!("{}|StartupFolder|{name}", machine as u8),
                enabled: approved(machine, "StartupFolder", &name),
                command: entry.path().display().to_string(),
                name,
                location,
                admin: machine,
            });
        }
    }
    items.sort_by_key(|i| i.name.to_lowercase());
    items
}

/// 12 bytes: 2 = enabled, 3 = disabled followed by the time it was disabled.
fn flag(enabled: bool) -> Vec<u8> {
    let mut bytes = vec![0u8; 12];
    if enabled {
        bytes[0] = 2;
    } else {
        bytes[0] = 3;
        // FILETIME: 100 ns ticks since 1601.
        let since_unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let ticks = since_unix.as_nanos() as u64 / 100 + 116_444_736_000_000_000;
        bytes[4..].copy_from_slice(&ticks.to_le_bytes());
    }
    bytes
}

pub fn set(id: &str, enabled: bool) -> Result<(), String> {
    let mut parts = id.splitn(3, '|');
    let machine = parts.next() == Some("1");
    let (Some(sub), Some(name)) = (parts.next(), parts.next()) else {
        return Err("bad id".into());
    };
    if !["Run", "Run32", "StartupFolder"].contains(&sub) {
        return Err("bad id".into());
    }
    let (key, _) = hive(machine).create_subkey(format!(r"{APPROVED}\{sub}")).map_err(|e| e.to_string())?;
    key.set_raw_value(name, &RegValue { bytes: flag(enabled), vtype: REG_BINARY })
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::flag;

    #[test]
    fn flag_layout_matches_task_manager() {
        let on = flag(true);
        assert_eq!(on, vec![2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let off = flag(false);
        assert_eq!(off.len(), 12);
        assert_eq!(off[0] & 1, 1, "odd first byte = disabled");
        assert!(u64::from_le_bytes(off[4..].try_into().unwrap()) > 130_000_000_000_000_000, "FILETIME after 2012");
    }
}
