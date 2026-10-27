//! Disk space (once Xpace): which folders and files take the space on a
//! drive, in the spirit of TreeSize. Scanning only reads; deleting what the
//! user picks goes to the Recycle Bin, or for good through the reviewed
//! Force Delete flow.

use crate::{find, force_delete, scan, space_win};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
pub struct Space {
    /// The last finished scan; deleting from it updates its totals.
    tree: Mutex<Option<Arc<RwLock<scan::Tree>>>>,
    /// The reviewed permanent deletion, and the size of each selected file.
    force: force_delete::Service,
    pending: Mutex<Option<(String, Vec<(PathBuf, u64)>)>>,
    /// The scan in progress, if any.
    scanning: Mutex<Option<Arc<scan::Progress>>>,
    /// Folder names for the search box, indexed on the first search.
    finder: find::Index,
}

#[tauri::command]
pub async fn list_drives() -> Vec<space_win::Drive> {
    tauri::async_runtime::spawn_blocking(space_win::drives).await.unwrap_or_default()
}

/// Windows' folder picker, opened at `start` when that is a folder.
#[tauri::command]
pub async fn pick_scan_folder(window: tauri::WebviewWindow, start: Option<String>) -> Result<Option<String>, String> {
    let owner = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    let start = start.map(|s| find::expand(&s));
    tauri::async_runtime::spawn_blocking(move || space_win::pick_folder(owner, start.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

/// What the search box offers for `text`: subfolders of a typed path, or
/// folders whose names match.
#[tauri::command]
pub async fn find_folders(app: AppHandle, text: String) -> Result<find::Found, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Space>().finder.find(&text, || {
            // The profile first, so the user's own folders turn up first.
            let profile = std::env::var("USERPROFILE").map(PathBuf::from).into_iter();
            let drives = space_win::drives().into_iter().filter(|d| d.kind != "network").map(|d| PathBuf::from(d.path));
            profile.chain(drives).collect()
        })
    })
    .await
    .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct Place {
    id: &'static str,
    path: String,
}

#[tauri::command]
pub async fn known_folders() -> Vec<Place> {
    tauri::async_runtime::spawn_blocking(|| space_win::known_folders().into_iter().map(|(id, path)| Place { id, path }).collect())
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn forget_recent(app: AppHandle, path: String) -> Vec<String> {
    crate::change_recent(&app, |recent| recent.retain(|p| !p.eq_ignore_ascii_case(&path)))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    files: u64,
    folders: u64,
    bytes: u64,
    current: String,
}

/// Scans `path` (a previous scan in progress is cancelled) and sends
/// "progress" events while it runs.
#[tauri::command]
pub async fn start_scan(app: AppHandle, path: String) -> Result<scan::Summary, String> {
    let root = find::folder(&path);
    if !root.is_absolute() || !root.is_dir() {
        return Err("notFolder".into());
    }
    let progress = Arc::new(scan::Progress::default());
    {
        let space = app.state::<Space>();
        let mut scanning = space.scanning.lock().unwrap();
        if let Some(previous) = scanning.replace(progress.clone()) {
            previous.cancel();
        }
    }
    let done = Arc::new(AtomicBool::new(false));
    let ticker = {
        let (app, progress, done) = (app.clone(), progress.clone(), done.clone());
        std::thread::spawn(move || {
            // A cancelled scan goes quiet, so it can't mix with the next one.
            while !done.load(Ordering::Relaxed) && !progress.cancelled() {
                let _ = app.emit("progress", ProgressEvent {
                    files: progress.files.load(Ordering::Relaxed),
                    folders: progress.folders.load(Ordering::Relaxed),
                    bytes: progress.bytes.load(Ordering::Relaxed),
                    current: progress.current.lock().unwrap().clone(),
                });
                std::thread::sleep(Duration::from_millis(150));
            }
        })
    };
    let worker = progress.clone();
    let tree = tauri::async_runtime::spawn_blocking(move || scan::scan(&root, &worker)).await;
    done.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    let tree = tree.map_err(|e| e.to_string())?;
    let summary = tree.summary.clone();
    {
        let space = app.state::<Space>();
        let mut scanning = space.scanning.lock().unwrap();
        // A newer scan replaced this one: leave the field to it.
        if scanning.as_ref().is_some_and(|p| Arc::ptr_eq(p, &progress)) {
            *scanning = None;
            *space.tree.lock().unwrap() = Some(Arc::new(RwLock::new(tree)));
        }
    }
    if !summary.cancelled {
        let path = summary.path.clone();
        crate::change_recent(&app, |recent| {
            recent.retain(|p| !p.eq_ignore_ascii_case(&path));
            recent.insert(0, path.clone());
        });
    }
    Ok(summary)
}

#[tauri::command]
pub fn cancel_scan(space: State<Space>) {
    if let Some(progress) = space.scanning.lock().unwrap().as_ref() {
        progress.cancel();
    }
}

fn current_tree(space: &Space) -> Result<Arc<RwLock<scan::Tree>>, String> {
    space.tree.lock().unwrap().clone().ok_or_else(|| "noScan".into())
}

#[tauri::command]
pub async fn open_folder(space: State<'_, Space>, id: u32) -> Result<scan::Listing, String> {
    let tree = current_tree(&space)?;
    tauri::async_runtime::spawn_blocking(move || tree.read().unwrap().listing(id).ok_or_else(|| "noFolder".to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn largest_files(space: State<Space>) -> Result<Vec<scan::BigFile>, String> {
    Ok(current_tree(&space)?.read().unwrap().top_files().to_vec())
}

#[tauri::command]
pub fn file_types(space: State<Space>) -> Result<Vec<scan::TypeStat>, String> {
    Ok(current_tree(&space)?.read().unwrap().types().to_vec())
}

// ---------- deleting ----------

/// Only things inside the scanned folder can be deleted, never the folder
/// itself, nothing protected (Windows, programs, profile folders) and
/// nothing missing. Returns each path with its size if it is a file.
fn checked(tree: &scan::Tree, paths: &[String]) -> Result<Vec<(PathBuf, u64)>, String> {
    if paths.is_empty() {
        return Err("invalidSelection".into());
    }
    paths
        .iter()
        .map(|raw| {
            let path = force_delete::parse_path(raw)?;
            let root = tree.root();
            if !force_delete::under(&path, root) || force_delete::under(root, &path) {
                return Err("outsideScan".to_string());
            }
            if force_delete::protected(&path) {
                return Err("protectedPath".into());
            }
            let meta = fs::symlink_metadata(&path).map_err(|_| "notFound")?;
            Ok((path, if meta.is_dir() { 0 } else { meta.len() }))
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deleted {
    /// Paths that are gone now.
    removed: Vec<String>,
    /// Paths still there (stopped, failed or scheduled for restart).
    kept: Vec<String>,
    summary: scan::Summary,
    outcome: Option<force_delete::Outcome>,
    /// Why the Recycle Bin stopped, if it did.
    error: Option<String>,
}

/// Takes what is gone out of the scan and says what is gone and what isn't.
fn settle(tree: &RwLock<scan::Tree>, paths: Vec<(PathBuf, u64)>, outcome: Option<force_delete::Outcome>, error: Option<String>) -> Deleted {
    let mut tree = tree.write().unwrap();
    let (mut removed, mut kept) = (Vec::new(), Vec::new());
    for (path, size) in paths {
        if fs::symlink_metadata(&path).is_err() {
            tree.forget(&path, size);
            removed.push(path.display().to_string());
        } else {
            kept.push(path.display().to_string());
        }
    }
    Deleted { removed, kept, summary: tree.summary.clone(), outcome, error }
}

/// Moves the chosen files and folders to the Recycle Bin.
#[tauri::command]
pub async fn recycle(app: AppHandle, window: tauri::WebviewWindow, paths: Vec<String>) -> Result<Deleted, String> {
    let owner = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    let tree = current_tree(&app.state::<Space>())?;
    let checked = checked(&tree.read().unwrap(), &paths)?;
    tauri::async_runtime::spawn_blocking(move || {
        let list: Vec<PathBuf> = checked.iter().map(|(p, _)| p.clone()).collect();
        let error = space_win::recycle(owner, &list).err();
        settle(&tree, checked, None, error)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Reviews a permanent deletion: counts, size and the apps holding files.
#[tauri::command]
pub async fn prepare_delete(app: AppHandle, paths: Vec<String>) -> Result<force_delete::Preview, String> {
    let tree = current_tree(&app.state::<Space>())?;
    let checked = checked(&tree.read().unwrap(), &paths)?;
    tauri::async_runtime::spawn_blocking(move || {
        let space = app.state::<Space>();
        let preview = space.force.prepare(paths)?;
        // The token is private to the shared module; read it from the reply.
        let token = serde_json::to_value(&preview).ok().and_then(|v| v["token"].as_str().map(String::from)).unwrap_or_default();
        *space.pending.lock().unwrap() = Some((token, checked));
        Ok(preview)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Deletes the reviewed selection for good. `mode` is "delete", "terminate"
/// (close the apps holding files first) or "reboot" (at the next restart,
/// administrator only).
#[tauri::command]
pub async fn execute_delete(app: AppHandle, token: String, mode: String) -> Result<Deleted, String> {
    let tree = current_tree(&app.state::<Space>())?;
    tauri::async_runtime::spawn_blocking(move || {
        let space = app.state::<Space>();
        let paths = match space.pending.lock().unwrap().take() {
            Some((pending, paths)) if pending == token => paths,
            _ => return Err("expiredSelection".to_string()),
        };
        let (outcome, error) = match space.force.execute(&token, &mode) {
            Ok(outcome) => (Some(outcome), None),
            Err(error) => (None, Some(error)),
        };
        Ok(settle(&tree, paths, outcome, error))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Shows a file or folder in Explorer; `open` opens a folder instead.
#[tauri::command]
pub fn reveal(path: String, open: bool) -> Result<(), String> {
    space_win::reveal(&path, open)
}

/// Writes the CSV export into Data/exports beside the executable and shows it.
#[tauri::command]
pub fn save_export(name: String, csv: String) -> Result<String, String> {
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let dir = crate::portable::data_dir().join("exports");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{safe}.csv"));
    // BOM so Excel reads the names as UTF-8.
    fs::write(&path, format!("\u{feff}{csv}")).map_err(|e| e.to_string())?;
    let shown = path.display().to_string();
    let _ = space_win::reveal(&shown, false);
    Ok(shown)
}
