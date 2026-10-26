// Xpecs: portable system information, live sensors and benchmarks for Windows,
// in the spirit of AIDA64. No installer, no driver, no administrator rights.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bench;
mod info;
mod sensors;
mod updater;
mod winapi;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

/// Settings kept in xpecs.json next to the executable.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { language: "en".into() }
    }
}

struct AppState {
    settings: Mutex<Settings>,
    /// Hardware information, filled in by a background thread at startup.
    info: Mutex<Option<Value>>,
    /// A newer release found on GitHub, waiting for the user to install it.
    update: Mutex<Option<updater::Update>>,
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn save_settings(settings: &Settings) {
    if let Ok(text) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(exe_dir().join("xpecs.json"), text);
    }
}

fn load_settings() -> Settings {
    match fs::read_to_string(exe_dir().join("xpecs.json")) {
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

/// Runs one benchmark off the UI thread and returns its result.
#[tauri::command]
async fn run_benchmark(test: String) -> Result<f64, String> {
    tauri::async_runtime::spawn_blocking(move || match test.as_str() {
        "mem-read" => Ok(bench::memory_read()),
        "mem-write" => Ok(bench::memory_write()),
        "mem-copy" => Ok(bench::memory_copy()),
        "mem-latency" => Ok(bench::memory_latency()),
        "cpu-int-1" => Ok(bench::cpu_integer(false)),
        "cpu-int-n" => Ok(bench::cpu_integer(true)),
        "cpu-fp-1" => Ok(bench::cpu_float(false)),
        "cpu-fp-n" => Ok(bench::cpu_float(true)),
        other => Err(format!("unknown test {other}")),
    })
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

/// Writes the HTML report into a "reports" folder next to the executable and
/// opens it in the default browser.
#[tauri::command]
fn save_report(name: String, html: String) -> Result<String, String> {
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let dir = exe_dir().join("reports");
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
    tauri::Builder::default()
        .manage(AppState { settings: Mutex::new(load_settings()), info: Mutex::new(None), update: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_language,
            get_info,
            run_benchmark,
            set_stress,
            save_report,
            check_update,
            install_update
        ])
        .setup(|app| {
            start_info_scan(app.handle().clone());
            sensors::spawn(app.handle().clone());
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
