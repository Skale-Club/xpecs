//! Drivers: what each device uses, the devices Windows reports a problem
//! with, and driver updates from Windows Update, the same signed drivers
//! Windows offers as optional updates. Drivers never come from anywhere else.
//!
//! Listing needs no administrator rights. Installing does, and creates a
//! restore point first; backing up all drivers does too (pnputil).

use crate::{info::cim_date, portable, win};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use wmi::{COMLibrary, Variant, WMIConnection};

const NO_WINDOW: u32 = 0x0800_0000;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Driver {
    device: String,
    class: String,
    provider: String,
    version: String,
    /// YYYY-MM-DD
    date: String,
    inf: String,
    signed: bool,
    /// Windows' own inbox driver, not one a manufacturer installed.
    inbox: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    name: String,
    class: String,
    /// Device Manager's problem code (28 = no driver, 22 = disabled...).
    code: u32,
}

#[derive(Serialize)]
pub struct Inventory {
    drivers: Vec<Driver>,
    problems: Vec<Problem>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub id: String,
    pub title: String,
    pub model: String,
    pub provider: String,
    pub class: String,
    pub date: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub id: String,
    pub title: String,
    /// "installed", "failed" or "skipped" (it needs a license accepted in Windows Update).
    pub result: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    /// "created", "recent" (Windows made one in the last day) or "skipped".
    restore_point: String,
    results: Vec<Installed>,
    reboot: bool,
}

fn text(row: &HashMap<String, Variant>, key: &str) -> String {
    match row.get(key) {
        Some(Variant::String(s)) => s.trim().to_string(),
        _ => String::new(),
    }
}

fn number(row: &HashMap<String, Variant>, key: &str) -> u32 {
    match row.get(key) {
        Some(Variant::UI4(n)) => *n,
        Some(Variant::I4(n)) => *n as u32,
        Some(Variant::UI2(n)) => *n as u32,
        Some(Variant::I2(n)) => *n as u32,
        _ => 0,
    }
}

/// Every device's driver and the devices with a problem. WMI needs COM, so
/// call this on a thread of its own.
pub fn inventory() -> Result<Inventory, String> {
    let com = COMLibrary::new().map_err(|e| e.to_string())?;
    let con = WMIConnection::new(com).map_err(|e| e.to_string())?;
    let rows: Vec<HashMap<String, Variant>> = con
        .raw_query("SELECT DeviceName, DeviceClass, DriverProviderName, DriverVersion, DriverDate, InfName, IsSigned FROM Win32_PnPSignedDriver")
        .map_err(|e| e.to_string())?;
    let mut drivers: Vec<Driver> = rows
        .iter()
        .filter(|r| !text(r, "DeviceName").is_empty())
        .map(|r| {
            let provider = text(r, "DriverProviderName");
            let inf = text(r, "InfName");
            Driver {
                device: text(r, "DeviceName"),
                class: text(r, "DeviceClass"),
                // Only third-party packages are copied in as oemNN.inf.
                inbox: !inf.to_ascii_lowercase().starts_with("oem") || provider.starts_with("Microsoft"),
                provider,
                version: text(r, "DriverVersion"),
                date: cim_date(&text(r, "DriverDate"), false),
                inf,
                signed: matches!(r.get("IsSigned"), Some(Variant::Bool(true))),
            }
        })
        .collect();
    drivers.sort_by(|a, b| a.class.cmp(&b.class).then_with(|| a.device.cmp(&b.device)));
    let rows: Vec<HashMap<String, Variant>> = con
        .raw_query("SELECT Name, PNPClass, ConfigManagerErrorCode FROM Win32_PnPEntity WHERE ConfigManagerErrorCode <> 0")
        .map_err(|e| e.to_string())?;
    let problems = rows
        .iter()
        .map(|r| Problem {
            name: Some(text(r, "Name")).filter(|n| !n.is_empty()).unwrap_or_else(|| "?".into()),
            class: text(r, "PNPClass"),
            code: number(r, "ConfigManagerErrorCode"),
        })
        .collect();
    Ok(Inventory { drivers, problems })
}

/// Runs a PowerShell script with no window and returns what it printed. The
/// script uses only single quotes, so it travels as one argument untouched.
fn powershell(script: &str) -> Result<String, String> {
    debug_assert!(!script.contains('"'));
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!("[Console]::OutputEncoding = [Text.Encoding]::UTF8\n{script}"))
        .creation_flags(NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    let out = String::from_utf8_lossy(&output.stdout).trim_start_matches('\u{feff}').trim().to_string();
    if !output.status.success() && out.is_empty() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.lines().find(|l| !l.trim().is_empty()).unwrap_or("PowerShell failed").trim().to_string());
    }
    Ok(out)
}

/// The last line of the output that is JSON, past any warnings.
fn json_line(out: &str) -> &str {
    out.lines().rev().find(|l| l.starts_with('[') || l.starts_with('{')).unwrap_or("[]")
}

const SEARCH: &str = r#"$ErrorActionPreference = 'Stop'
$session = New-Object -ComObject Microsoft.Update.Session
$found = $session.CreateUpdateSearcher().Search('IsInstalled=0 and Type=''Driver''')
$list = foreach ($u in $found.Updates) {
  $date = ''
  if ($u.DriverVerDate) { $date = $u.DriverVerDate.ToString('yyyy-MM-dd') }
  [pscustomobject]@{ id = $u.Identity.UpdateID; title = $u.Title; model = [string]$u.DriverModel; provider = [string]$u.DriverProvider; class = [string]$u.DriverClass; date = $date; size = [uint64]$u.MaxDownloadSize }
}
ConvertTo-Json -InputObject @($list) -Compress"#;

/// Driver updates Windows Update has for this PC. Takes 10 to 60 seconds.
pub fn search() -> Result<Vec<Update>, String> {
    let out = powershell(SEARCH)?;
    serde_json::from_str(json_line(&out)).map_err(|_| out.lines().last().unwrap_or("").to_string())
}

const RESTORE_POINT: &str = r#"$w = $null
try {
  Checkpoint-Computer -Description 'Xpecs: driver updates' -RestorePointType DEVICE_DRIVER_INSTALL -WarningVariable w -WarningAction SilentlyContinue -ErrorAction Stop
  if ($w) { 'recent' } else { 'created' }
} catch { 'failed: ' + $_.Exception.Message }"#;

fn restore_point() -> Result<String, String> {
    let out = powershell(RESTORE_POINT)?;
    let last = out.lines().last().unwrap_or("").trim().to_string();
    match last.as_str() {
        "created" | "recent" => Ok(last),
        other => Err(format!("restorePoint:{}", other.trim_start_matches("failed: "))),
    }
}

const INSTALL: &str = r#"$ErrorActionPreference = 'Stop'
$session = New-Object -ComObject Microsoft.Update.Session
$found = $session.CreateUpdateSearcher().Search('IsInstalled=0 and Type=''Driver''')
$todo = New-Object -ComObject Microsoft.Update.UpdateColl
$results = @()
foreach ($u in $found.Updates) {
  if ($ids -notcontains $u.Identity.UpdateID) { continue }
  if (-not $u.EulaAccepted) { $results += [pscustomobject]@{ id = $u.Identity.UpdateID; title = $u.Title; result = 'skipped' }; continue }
  [void]$todo.Add($u)
}
$reboot = $false
if ($todo.Count -gt 0) {
  $downloader = $session.CreateUpdateDownloader()
  $downloader.Updates = $todo
  [void]$downloader.Download()
  $installer = $session.CreateUpdateInstaller()
  $installer.Updates = $todo
  $done = $installer.Install()
  $reboot = [bool]$done.RebootRequired
  for ($i = 0; $i -lt $todo.Count; $i++) {
    $code = $done.GetUpdateResult($i).ResultCode
    $state = 'failed'
    if ($code -eq 2 -or $code -eq 3) { $state = 'installed' }
    $results += [pscustomobject]@{ id = $todo.Item($i).Identity.UpdateID; title = $todo.Item($i).Title; result = $state }
  }
}
ConvertTo-Json -InputObject @{ results = @($results); reboot = $reboot } -Compress -Depth 3"#;

#[derive(Deserialize)]
struct InstallReply {
    results: Vec<Installed>,
    reboot: bool,
}

/// An update id: a GUID, so it can go into the script as it is.
fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Downloads and installs the chosen updates from Windows Update, after a
/// restore point unless `skip_restore_point`. Needs administrator rights.
pub fn install(ids: &[String], skip_restore_point: bool) -> Result<InstallOutcome, String> {
    if !win::is_admin() {
        return Err("needsAdmin".into());
    }
    if ids.is_empty() || !ids.iter().all(|id| valid_id(id)) {
        return Err("noUpdates".into());
    }
    let restore_point = if skip_restore_point { "skipped".to_string() } else { restore_point()? };
    let list = ids.iter().map(|id| format!("'{id}'")).collect::<Vec<_>>().join(",");
    let out = powershell(&format!("$ids = @({list})\n{INSTALL}"))?;
    let reply: InstallReply =
        serde_json::from_str(json_line(&out)).map_err(|_| out.lines().last().unwrap_or("").to_string())?;
    Ok(InstallOutcome { restore_point, results: reply.results, reboot: reply.reboot })
}

#[repr(C)]
#[derive(Default)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(time: *mut SystemTime);
}

/// Copies every third-party driver package to Data\driver-backup\<date-time>
/// with pnputil, so they can be put back after reinstalling Windows.
pub fn backup() -> Result<String, String> {
    if !win::is_admin() {
        return Err("needsAdmin".into());
    }
    let mut now = SystemTime::default();
    unsafe { GetLocalTime(&mut now) };
    let name = format!("{:04}-{:02}-{:02} {:02}{:02}", now.year, now.month, now.day, now.hour, now.minute);
    let dir: PathBuf = portable::data_dir().join("driver-backup").join(name);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let status = Command::new("pnputil.exe")
        .arg("/export-driver")
        .arg("*")
        .arg(&dir)
        .creation_flags(NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    let copied = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
    if !status.status.success() && copied == 0 {
        let _ = std::fs::remove_dir(&dir);
        return Err("backupFailed".into());
    }
    Ok(dir.display().to_string())
}

/// The driver page of a graphics vendor's own app. Only these.
pub fn vendor_page(vendor: &str) -> Option<&'static str> {
    match vendor {
        "nvidia" => Some("https://www.nvidia.com/en-us/software/nvidia-app/"),
        "amd" => Some("https://www.amd.com/en/support/download/drivers.html"),
        "intel" => Some("https://www.intel.com/content/www/us/en/support/detect.html"),
        _ => None,
    }
}

// ---------- commands ----------

#[tauri::command]
pub async fn driver_inventory() -> Result<Inventory, String> {
    tauri::async_runtime::spawn_blocking(inventory).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn search_driver_updates() -> Result<Vec<Update>, String> {
    tauri::async_runtime::spawn_blocking(search).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_driver_updates(ids: Vec<String>, skip_restore_point: bool) -> Result<InstallOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || install(&ids, skip_restore_point))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn backup_drivers() -> Result<String, String> {
    let dir = tauri::async_runtime::spawn_blocking(backup).await.map_err(|e| e.to_string())??;
    win::shell("open", &dir, "");
    Ok(dir)
}

#[tauri::command]
pub fn open_vendor_drivers(vendor: String) -> Result<(), String> {
    let page = vendor_page(&vendor).ok_or("unknown vendor")?;
    win::shell("open", page, "");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_travel_as_one_argument() {
        for script in [SEARCH, RESTORE_POINT, INSTALL] {
            assert!(!script.contains('"'));
        }
    }

    #[test]
    fn only_guids_reach_the_install_script() {
        assert!(valid_id("d2090f44-1445-423f-b574-5fb25bee43e6"));
        assert!(!valid_id("d2090f44-1445-423f-b574-5fb25bee43e'"));
        assert!(!valid_id("'); Remove-Item C:\\ -Recurse; ('"));
    }

    #[test]
    fn finds_the_json_past_warnings() {
        assert_eq!(json_line("WARNING: x\n[{\"a\":1}]"), "[{\"a\":1}]");
        assert_eq!(json_line("nothing"), "[]");
    }
}
