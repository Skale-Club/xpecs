// Xpecs: portable PC care and system information for Windows. PC health,
// junk cleaning, startup programs, apps, drivers and Force Delete (once
// Xcare), disk space analysis (once Xpace), plus hardware information, live
// sensors and benchmarks in the spirit of AIDA64. No installer and no driver; administrator rights only for
// the actions that need them.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod apps;
mod bench;
mod care;
mod cleaner;
mod drivers;
mod find;
mod force_delete;
mod icons;
mod info;
mod mcp;
mod mcp_tools;
mod portable;
mod power;
mod scan;
mod sensors;
mod space;
mod space_win;
mod startup;
mod updater;
mod win;
mod winapi;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, process::Command, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

/// Settings kept in Data/xpecs.json beside the executable.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    language: String,
    /// Folders and drives scanned lately in Disk space, newest first.
    recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { language: "en".into(), recent: Vec::new() }
    }
}

const RECENT: usize = 8;

/// Changes the recent scans, saves them and returns them.
fn change_recent(app: &AppHandle, change: impl FnOnce(&mut Vec<String>)) -> Vec<String> {
    let state = app.state::<AppState>();
    let mut settings = state.settings.lock().unwrap();
    change(&mut settings.recent);
    settings.recent.truncate(RECENT);
    save_settings(&settings);
    settings.recent.clone()
}

struct AppState {
    settings: Mutex<Settings>,
    /// Hardware information, filled in by a background thread at startup.
    info: Mutex<Option<Value>>,
    /// A newer release found on GitHub, waiting for the user to install it.
    update: Mutex<Option<updater::Update>>,
}

fn save_settings(settings: &Settings) {
    if let Ok(text) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(portable::data_dir().join("xpecs.json"), text);
    }
}

fn load_settings() -> Settings {
    match fs::read_to_string(portable::data_dir().join("xpecs.json")) {
        // Some editors save UTF-8 with a BOM, which serde_json rejects.
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default(),
        Err(_) => {
            let settings = Settings::default();
            save_settings(&settings);
            settings
        }
    }
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_language(state: State<AppState>, language: String) {
    let mut settings = state.settings.lock().unwrap();
    settings.language = language;
    save_settings(&settings);
}

/// None until the startup scan has finished; the page then gets an "info" event.
#[tauri::command]
fn get_info(state: State<AppState>) -> Option<Value> {
    state.info.lock().unwrap().clone()
}

/// Live sensors run only once a page that shows them has been opened.
#[tauri::command]
fn start_sensors(app: AppHandle) {
    sensors::spawn(app);
}

/// Runs one benchmark off the UI thread and returns its result.
#[tauri::command]
async fn run_benchmark(test: String) -> Result<f64, String> {
    tauri::async_runtime::spawn_blocking(move || bench::run(&test))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn set_stress(running: bool) {
    if running {
        bench::start_stress();
    } else {
        bench::stop_stress();
    }
}

/// Opens the Skale Club website in the default browser.
#[tauri::command]
fn open_site() {
    let _ = Command::new("explorer.exe").arg("https://skale.club").spawn();
}
/// Writes the HTML report into Data/reports beside the executable and
/// opens it in the default browser.
#[tauri::command]
fn save_report(name: String, html: String) -> Result<String, String> {
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let dir = portable::data_dir().join("reports");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{safe}.html"));
    fs::write(&path, html).map_err(|e| e.to_string())?;
    let _ = Command::new("explorer.exe").arg(&path).spawn();
    Ok(path.display().to_string())
}

/// Looks for a newer release; quietly None when offline.
#[tauri::command]
async fn check_update(app: AppHandle) -> Option<updater::Update> {
    tauri::async_runtime::spawn_blocking(move || {
        let update = updater::check().ok().flatten();
        *app.state::<AppState>().update.lock().unwrap() = update.clone();
        update
    })
    .await
    .ok()
    .flatten()
}

/// Downloads and verifies the update, swaps the exe and restarts.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app.state::<AppState>().update.lock().unwrap().clone().ok_or("no update")?;
    tauri::async_runtime::spawn_blocking(move || updater::install(&update))
        .await
        .map_err(|e| e.to_string())??;
    bench::stop_stress();
    app.exit(0);
    Ok(())
}

fn start_info_scan(app: AppHandle) {
    std::thread::spawn(move || {
        let info = serde_json::to_value(info::collect()).unwrap_or(Value::Null);
        *app.state::<AppState>().info.lock().unwrap() = Some(info.clone());
        let _ = app.emit("info", info);
    });
}

fn main() {
    updater::finish_previous();
    portable::migrate(&["xpecs.json", "reports"]);
    portable::sync_appinfo();
    if mcp::requested() {
        return mcp_tools::serve();
    }
    tauri::Builder::default()
        .manage(AppState { settings: Mutex::new(load_settings()), info: Mutex::new(None), update: Mutex::new(None) })
        .manage(care::Care::default())
        .manage(space::Space::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_language,
            get_info,
            start_sensors,
            run_benchmark,
            set_stress,
            save_report,
            open_site,
            check_update,
            install_update,
            care::get_status,
            care::scan_junk,
            care::clean_junk,
            care::list_startup,
            care::set_startup,
            care::list_apps,
            care::list_icons,
            care::uninstall_app,
            care::list_power_plans,
            care::set_power_plan,
            care::add_ultimate_plan,
            care::pick_force_delete,
            care::prepare_force_delete,
            care::execute_force_delete,
            care::run_tool,
            care::restart_as_admin,
            care::is_admin,
            drivers::driver_inventory,
            drivers::search_driver_updates,
            drivers::install_driver_updates,
            drivers::backup_drivers,
            drivers::open_vendor_drivers,
            space::list_drives,
            space::pick_scan_folder,
            space::find_folders,
            space::known_folders,
            space::forget_recent,
            space::start_scan,
            space::cancel_scan,
            space::open_folder,
            space::largest_files,
            space::file_types,
            space::recycle,
            space::prepare_delete,
            space::execute_delete,
            space::reveal,
            space::save_export
        ])
        .setup(|app| {
            start_info_scan(app.handle().clone());
            care::check_disks(app.handle().clone());
            Ok(())
        })
        .on_window_event(|_, event| {
            if let tauri::WindowEvent::Destroyed = event {
                bench::stop_stress();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to start Xpecs");
}
