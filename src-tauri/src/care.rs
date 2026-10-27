//! The PC care pages (from Xcare): health overview, junk cleaning, startup
//! programs, installed apps, power plans, Windows' own tools and Force Delete.
//! It only does what actually helps, and leaves out registry cleaners and
//! "RAM boosters".

use crate::{apps, cleaner, force_delete, icons, power, startup, win};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use wmi::{COMLibrary, Variant, WMIConnection};

#[derive(Default)]
pub struct Care {
    force_delete: force_delete::Service,
    /// Files found by the last scan, per category; cleaning deletes only these.
    scanned: Mutex<HashMap<String, cleaner::Files>>,
    /// Drives Windows reports as not healthy; None until the startup check ends.
    unhealthy_disks: Mutex<Option<Vec<String>>>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    admin: bool,
    drive: String,
    free: u64,
    total: u64,
    uptime: u64,
    memory_load: u32,
    recycle_bin: u64,
    unhealthy_disks: Vec<String>,
}

pub fn read_status(unhealthy_disks: Vec<String>) -> Status {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let (free, total) = win::disk_space(&format!("{drive}\\")).unwrap_or((0, 0));
    Status {
        admin: win::is_admin(),
        drive,
        free,
        total,
        uptime: win::uptime_seconds(),
        memory_load: win::memory_load(),
        recycle_bin: win::recycle_bin().0,
        unhealthy_disks,
    }
}

#[tauri::command]
pub async fn get_status(app: AppHandle) -> Status {
    tauri::async_runtime::spawn_blocking(move || {
        let care = app.state::<Care>();
        // The drive check starts with the app; wait for it (up to 5 s) so the
        // first overview does not call a failing drive healthy.
        for _ in 0..50 {
            if care.unhealthy_disks.lock().unwrap().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let unhealthy_disks = care.unhealthy_disks.lock().unwrap().clone().unwrap_or_default();
        read_status(unhealthy_disks)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn scan_junk(app: AppHandle) -> Vec<cleaner::Found> {
    tauri::async_runtime::spawn_blocking(move || {
        let (found, lists) = cleaner::scan(win::is_admin(), &mut |p| {
            let _ = app.emit("scan-progress", p);
        });
        *app.state::<Care>().scanned.lock().unwrap() = lists;
        found
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn clean_junk(app: AppHandle, ids: Vec<String>) -> cleaner::Cleaned {
    tauri::async_runtime::spawn_blocking(move || {
        let care = app.state::<Care>();
        let mut scanned = care.scanned.lock().unwrap();
        let result = cleaner::clean(&ids, &scanned, win::is_admin(), &mut |p| {
            let _ = app.emit("clean-progress", p);
        });
        // The lists are spent; a new scan is needed before cleaning again.
        scanned.clear();
        result
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub fn list_startup() -> Vec<startup::Item> {
    startup::list()
}

#[tauri::command]
pub fn set_startup(id: String, enabled: bool) -> Result<(), String> {
    startup::set(&id, enabled)
}

#[tauri::command]
pub async fn list_apps() -> Vec<apps::App> {
    tauri::async_runtime::spawn_blocking(apps::list).await.unwrap_or_default()
}

/// Real icons for a list ("startup", "apps" or "cleaner"), by item id. Sent
/// after the list itself, so the list shows without waiting for them.
#[tauri::command]
pub async fn list_icons(kind: String) -> HashMap<String, String> {
    tauri::async_runtime::spawn_blocking(move || match kind.as_str() {
        "startup" => startup::list().into_iter().filter_map(|i| Some((i.id, icons::command(&i.command)?))).collect(),
        "apps" => apps::list()
            .into_iter()
            .filter_map(|a| {
                let (path, index) = icons::location(&a.icon);
                Some((a.id, icons::file(&path, index)?))
            })
            .collect(),
        "cleaner" => icons::categories().into_iter().map(|(id, url)| (id.to_string(), url)).collect(),
        _ => HashMap::new(),
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub fn uninstall_app(id: String) -> Result<(), String> {
    apps::uninstall(&id)
}

#[tauri::command]
pub fn list_power_plans() -> Vec<power::Plan> {
    power::list()
}

#[tauri::command]
pub fn set_power_plan(guid: String) -> Result<(), String> {
    power::set_active(&guid)
}

#[tauri::command]
pub fn add_ultimate_plan() -> Result<(), String> {
    power::add_ultimate()
}

#[tauri::command]
pub async fn pick_force_delete(window: tauri::WebviewWindow, folder: bool) -> Result<Vec<String>, String> {
    let owner = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    tauri::async_runtime::spawn_blocking(move || force_delete::pick(folder, owner))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn prepare_force_delete(app: AppHandle, paths: Vec<String>) -> Result<force_delete::Preview, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Care>().force_delete.prepare(paths))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn execute_force_delete(app: AppHandle, token: String, mode: String) -> Result<force_delete::Outcome, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Care>().force_delete.execute(&token, &mode))
        .await
        .map_err(|e| e.to_string())?
}

/// Windows' own tools. Only these ids are accepted.
#[tauri::command]
pub fn run_tool(tool: String) -> Result<(), String> {
    let (verb, file, params) = match tool.as_str() {
        "repair" => ("runas", "cmd.exe", "/k DISM /Online /Cleanup-Image /RestoreHealth && sfc /scannow"),
        "restore" => (
            "runas",
            "powershell.exe",
            "-NoProfile -NoExit -Command \"Checkpoint-Computer -Description 'Xpecs' -RestorePointType MODIFY_SETTINGS; 'Restore point created.'\"",
        ),
        "defrag" => ("open", "dfrgui.exe", ""),
        "visual" => ("open", "SystemPropertiesPerformance.exe", ""),
        "gamemode" => ("open", "ms-settings:gaming-gamemode", ""),
        "storage" => ("open", "ms-settings:storagesense", ""),
        "update" => ("open", "ms-settings:windowsupdate", ""),
        "taskmgr" => ("open", "taskmgr.exe", ""),
        "cleanmgr" => ("open", "cleanmgr.exe", ""),
        "devmgmt" => ("open", "devmgmt.msc", ""),
        _ => return Err("unknown tool".into()),
    };
    if win::shell(verb, file, params) {
        Ok(())
    } else {
        Err("cancelled".into())
    }
}

/// Relaunches Xpecs with a UAC prompt; this copy closes if the user accepts.
#[tauri::command]
pub fn restart_as_admin(app: AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if win::shell("runas", &exe.display().to_string(), "") {
        app.exit(0);
        Ok(())
    } else {
        Err("cancelled".into())
    }
}

#[tauri::command]
pub fn is_admin() -> bool {
    win::is_admin()
}

/// Drives Windows' storage management reports as Warning (1) or Unhealthy (2).
pub fn unhealthy_disks() -> Vec<String> {
    let Ok(com) = COMLibrary::new() else { return Vec::new() };
    let Ok(con) = WMIConnection::with_namespace_path("ROOT\\Microsoft\\Windows\\Storage", com) else {
        return Vec::new();
    };
    let rows: Vec<HashMap<String, Variant>> =
        con.raw_query("SELECT FriendlyName, HealthStatus FROM MSFT_PhysicalDisk").unwrap_or_default();
    rows.iter()
        .filter(|r| {
            let code = match r.get("HealthStatus") {
                Some(Variant::UI1(v)) => *v as i64,
                Some(Variant::UI2(v)) => *v as i64,
                Some(Variant::UI4(v)) => *v as i64,
                Some(Variant::I2(v)) => *v as i64,
                Some(Variant::I4(v)) => *v as i64,
                _ => 0,
            };
            code == 1 || code == 2
        })
        .filter_map(|r| match r.get("FriendlyName") {
            Some(Variant::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// WMI needs its own COM thread, so the drive check runs once in the
/// background and always records a result, even an empty one.
pub fn check_disks(app: AppHandle) {
    std::thread::spawn(move || {
        let result = unhealthy_disks();
        *app.state::<Care>().unhealthy_disks.lock().unwrap() = Some(result);
    });
}
