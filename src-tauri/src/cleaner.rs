//! Junk cleaner. A scan lists the files of each category; cleaning deletes
//! exactly those files and nothing else.
//!
//! Safety rules:
//! - Only folders that hold caches or leftovers that Windows and apps rebuild.
//! - Never follows junctions or symbolic links (reparse points), and never
//!   uses one as a cleaning root.
//! - Refuses relative paths, drive roots and the big parent folders
//!   (profile, Windows, Program Files…) as cleaning roots.
//! - A file is only deleted if it sits inside its category's own folders.
//! - Temp folders keep anything modified in the last 24 hours, so running
//!   installers and apps keep their working files.
//! - Files in use are skipped, not forced.

use crate::win;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const REPARSE_POINT: u32 = 0x400;
const DAY: Duration = Duration::from_secs(24 * 3600);

#[derive(Clone, Copy)]
enum Filter {
    All,
    /// File names starting with one of these (case-insensitive).
    Prefixes(&'static [&'static str]),
}

pub struct Root {
    dir: PathBuf,
    recursive: bool,
    filter: Filter,
}

struct Category {
    id: &'static str,
    group: &'static str,
    admin: bool,
    default: bool,
    older_than: Option<Duration>,
    roots: Vec<Root>,
}

fn env(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).unwrap_or_default()
}

fn tree(dir: PathBuf) -> Root {
    Root { dir, recursive: true, filter: Filter::All }
}

/// A root is safe when it is absolute, below the drive root (three path
/// components or more; "C:\" alone has two) and, if everything in it may go,
/// not one of the big parent folders. A missing environment variable leaves a
/// relative path, which this refuses.
fn safe(root: &Root) -> bool {
    let dir = &root.dir;
    if !dir.is_absolute() || dir.components().count() < 3 {
        return false;
    }
    if matches!(root.filter, Filter::Prefixes(_)) {
        return true;
    }
    let norm = |p: &Path| p.to_string_lossy().trim_end_matches('\\').to_lowercase();
    let target = norm(dir);
    !["USERPROFILE", "WINDIR", "ProgramData", "LOCALAPPDATA", "APPDATA", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .any(|v| norm(Path::new(&v)) == target)
}

/// A junction or symbolic link could point anywhere, so it is never a root.
fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).map_or(false, |m| m.file_attributes() & REPARSE_POINT != 0)
}

fn usable(root: &Root) -> bool {
    safe(root) && !is_link(&root.dir)
}

/// The user's temp folder, only if it really is a temp folder: a TEMP
/// variable pointed at "C:\" or a profile must never be swept.
fn temp_dir() -> Vec<Root> {
    let dir = env("TEMP");
    let named_temp = dir
        .file_name()
        .map(|n| matches!(n.to_string_lossy().to_lowercase().as_str(), "temp" | "tmp"))
        .unwrap_or(false);
    if named_temp { vec![tree(dir)] } else { Vec::new() }
}

/// Cache folders of every profile of a Chromium browser. Cookies, history,
/// passwords and settings live elsewhere and are never touched.
fn chromium(user_data: PathBuf) -> Vec<Root> {
    let mut roots = Vec::new();
    let Ok(entries) = fs::read_dir(&user_data) else { return roots };
    for profile in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        for sub in ["Cache", "Code Cache", "GPUCache", r"Service Worker\CacheStorage", r"Service Worker\ScriptCache"] {
            let dir = profile.join(sub);
            if dir.is_dir() {
                roots.push(tree(dir));
            }
        }
    }
    roots
}

fn firefox(profiles: PathBuf) -> Vec<Root> {
    let Ok(entries) = fs::read_dir(&profiles) else { return Vec::new() };
    entries
        .flatten()
        .map(|e| e.path().join("cache2"))
        .filter(|d| d.is_dir())
        .map(tree)
        .collect()
}

fn categories() -> Vec<Category> {
    let local = env("LOCALAPPDATA");
    let windir = env("WINDIR");
    let data = env("ProgramData");
    let cat = |id, group, admin, default, older_than, roots| Category { id, group, admin, default, older_than, roots };
    vec![
        cat("temp", "windows", false, true, Some(DAY), temp_dir()),
        cat("thumbs", "windows", false, true, None, vec![Root {
            dir: local.join(r"Microsoft\Windows\Explorer"),
            recursive: false,
            filter: Filter::Prefixes(&["thumbcache_", "iconcache_"]),
        }]),
        cat("inetcache", "windows", false, true, None, vec![tree(local.join(r"Microsoft\Windows\INetCache"))]),
        cat("reports", "windows", false, true, None, vec![
            tree(local.join(r"Microsoft\Windows\WER")),
            tree(local.join("CrashDumps")),
        ]),
        cat("wintemp", "windows", true, true, Some(DAY), vec![tree(windir.join("Temp"))]),
        cat("update", "windows", true, true, None, vec![tree(windir.join(r"SoftwareDistribution\Download"))]),
        cat("delivery", "windows", true, true, None, vec![tree(
            windir.join(r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache"),
        )]),
        cat("sysreports", "windows", true, true, None, vec![tree(data.join(r"Microsoft\Windows\WER"))]),
        cat("dumps", "windows", true, true, None, vec![
            Root { dir: windir.clone(), recursive: false, filter: Filter::Prefixes(&["memory.dmp"]) },
            tree(windir.join("Minidump")),
            tree(windir.join("LiveKernelReports")),
        ]),
        cat("chrome", "browsers", false, true, None, chromium(local.join(r"Google\Chrome\User Data"))),
        cat("edge", "browsers", false, true, None, chromium(local.join(r"Microsoft\Edge\User Data"))),
        cat("brave", "browsers", false, true, None, chromium(local.join(r"BraveSoftware\Brave-Browser\User Data"))),
        cat("opera", "browsers", false, true, None, vec![
            tree(local.join(r"Opera Software\Opera Stable\Cache")),
            tree(local.join(r"Opera Software\Opera GX Stable\Cache")),
        ]),
        cat("firefox", "browsers", false, true, None, firefox(local.join(r"Mozilla\Firefox\Profiles"))),
        // Off by default: games rebuild shaders on first launch (brief stutter).
        cat("shaders", "optional", false, false, None, vec![
            tree(local.join("D3DSCache")),
            tree(local.join(r"NVIDIA\DXCache")),
            tree(local.join(r"NVIDIA\GLCache")),
            tree(local.join(r"AMD\DxCache")),
            tree(local.join(r"AMD\DxcCache")),
            tree(local.join(r"AMD\GLCache")),
        ]),
        // Off by default: emptying it is permanent.
        cat("recycle", "optional", false, false, None, Vec::new()),
    ]
}

pub type Files = Vec<(PathBuf, u64)>;

#[derive(Serialize)]
pub struct Found {
    id: &'static str,
    group: &'static str,
    admin: bool,
    default: bool,
    /// Needs administrator rights that this run does not have.
    locked: bool,
    bytes: u64,
    files: u64,
}

fn walk(dir: &Path, root: &Root, cutoff: Option<SystemTime>, out: &mut Files) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        // DirEntry::metadata does not follow links, so junctions show up here.
        let Ok(meta) = entry.metadata() else { continue };
        if meta.file_attributes() & REPARSE_POINT != 0 {
            continue;
        }
        if meta.is_dir() {
            if root.recursive {
                walk(&entry.path(), root, cutoff, out);
            }
            continue;
        }
        if let Filter::Prefixes(prefixes) = root.filter {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !prefixes.iter().any(|p| name.starts_with(p)) {
                continue;
            }
        }
        if let Some(cutoff) = cutoff {
            match meta.modified() {
                Ok(modified) if modified < cutoff => {}
                _ => continue,
            }
        }
        out.push((entry.path(), meta.len()));
    }
}

fn collect(roots: &[Root], cutoff: Option<SystemTime>) -> Files {
    let mut files = Vec::new();
    for root in roots.iter().filter(|r| usable(r)) {
        walk(&root.dir, root, cutoff, &mut files);
    }
    files
}

/// Scans every category. Admin-only folders are still measured where they
/// can be read, so the user sees what running as administrator would free.
pub fn scan(elevated: bool, on_progress: &mut dyn FnMut(Progress)) -> (Vec<Found>, HashMap<String, Files>) {
    let now = SystemTime::now();
    let mut found = Vec::new();
    let mut lists = HashMap::new();
    let all = categories();
    let count = all.len() as u64;
    for (i, c) in all.into_iter().enumerate() {
        on_progress(Progress { category: c.id, done: i as u64, total: count, ..Default::default() });
        let (bytes, files) = if c.id == "recycle" {
            win::recycle_bin()
        } else {
            let files = collect(&c.roots, c.older_than.map(|age| now - age));
            let total = (files.iter().map(|(_, size)| size).sum(), files.len() as u64);
            lists.insert(c.id.to_string(), files);
            total
        };
        found.push(Found {
            id: c.id,
            group: c.group,
            admin: c.admin,
            default: c.default,
            locked: c.admin && !elevated,
            bytes,
            files,
        });
    }
    (found, lists)
}

/// Where a scan (categories) or a clean (files and bytes) has got to.
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub category: &'static str,
    pub done: u64,
    pub total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

#[derive(Serialize, Default, Debug)]
pub struct Cleaned {
    pub freed: u64,
    pub deleted: u64,
    /// In use, protected, or already gone.
    pub skipped: u64,
}

fn remove(path: &Path) -> bool {
    if fs::remove_file(path).is_ok() {
        return true;
    }
    // Read-only files: clear the flag and try once more.
    match fs::metadata(path) {
        Ok(meta) if meta.permissions().readonly() => {
            let mut perms = meta.permissions();
            perms.set_readonly(false);
            fs::set_permissions(path, perms).is_ok() && fs::remove_file(path).is_ok()
        }
        _ => false,
    }
}

/// Removes empty sub-folders (never `dir` itself), skipping recent ones when
/// a cutoff applies.
fn prune(dir: &Path, cutoff: Option<SystemTime>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() || meta.file_attributes() & REPARSE_POINT != 0 {
            continue;
        }
        prune(&entry.path(), cutoff);
        let old_enough = cutoff.map_or(true, |c| meta.modified().map_or(false, |m| m < c));
        if old_enough {
            let _ = fs::remove_dir(entry.path()); // fails, as intended, if not empty
        }
    }
}

/// Deletes `files`, but only those inside one of `roots`, both by path and by
/// where the folder really is on disk: a folder swapped for a junction after
/// the scan does not lead the delete elsewhere. `tick` gets the size of each
/// file handled, deleted or not.
fn delete(roots: &[Root], files: &Files, cutoff: Option<SystemTime>, out: &mut Cleaned, tick: &mut dyn FnMut(u64)) {
    let homes: Vec<(&Path, PathBuf)> = roots
        .iter()
        .filter(|r| usable(r))
        .filter_map(|r| fs::canonicalize(&r.dir).ok().map(|real| (r.dir.as_path(), real)))
        .collect();
    let mut real_dirs: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    for (path, size) in files {
        let real_dir = path
            .parent()
            .and_then(|dir| real_dirs.entry(dir.to_path_buf()).or_insert_with(|| fs::canonicalize(dir).ok()).clone());
        let inside = real_dir.map_or(false, |real_dir| {
            homes.iter().any(|(dir, real)| path.starts_with(dir) && real_dir.starts_with(real))
        });
        if inside && remove(path) {
            out.freed += size;
            out.deleted += 1;
        } else {
            out.skipped += 1;
        }
        tick(*size);
    }
    for root in roots.iter().filter(|r| r.recursive && usable(r)) {
        prune(&root.dir, cutoff);
    }
}

pub fn clean(
    ids: &[String],
    scanned: &HashMap<String, Files>,
    elevated: bool,
    on_progress: &mut dyn FnMut(Progress),
) -> Cleaned {
    let now = SystemTime::now();
    let plan: Vec<Category> = categories()
        .into_iter()
        .filter(|c| ids.iter().any(|id| id == c.id) && !(c.admin && !elevated))
        .collect();
    let recycle = plan.iter().any(|c| c.id == "recycle").then(win::recycle_bin).unwrap_or((0, 0));
    let mut progress = Progress::default();
    for c in &plan {
        let (bytes, files) = match scanned.get(c.id) {
            Some(list) => (list.iter().map(|(_, s)| s).sum(), list.len() as u64),
            None if c.id == "recycle" => recycle,
            None => (0, 0),
        };
        progress.total += files;
        progress.bytes_total += bytes;
    }

    let mut out = Cleaned::default();
    let mut last = Instant::now();
    for c in &plan {
        progress.category = c.id;
        on_progress(progress.clone());
        if c.id == "recycle" {
            if win::empty_recycle_bin() {
                out.freed += recycle.0;
                out.deleted += recycle.1;
            }
            progress.done += recycle.1;
            progress.bytes_done += recycle.0;
            continue;
        }
        if let Some(files) = scanned.get(c.id) {
            // At most ~10 updates a second, however many files there are.
            let mut tick = |size: u64| {
                progress.done += 1;
                progress.bytes_done += size;
                if last.elapsed() >= Duration::from_millis(100) {
                    last = Instant::now();
                    on_progress(progress.clone());
                }
            };
            delete(&c.roots, files, c.older_than.map(|age| now - age), &mut out, &mut tick);
        }
    }
    on_progress(progress);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletes_only_inside_roots_and_keeps_recent_files() {
        let base = std::env::temp_dir().join(format!("xcare-test-{}", std::process::id()));
        let inside = base.join("cache");
        let outside = base.join("keep");
        fs::create_dir_all(inside.join("sub")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(inside.join("a.tmp"), b"aaaa").unwrap();
        fs::write(inside.join("sub").join("b.tmp"), b"bb").unwrap();
        fs::write(outside.join("c.txt"), b"c").unwrap();

        let roots = vec![tree(inside.clone())];
        // Nothing is old enough yet: a 1-day cutoff keeps everything.
        assert!(collect(&roots, Some(SystemTime::now() - DAY)).is_empty());

        let mut files = collect(&roots, None);
        assert_eq!(files.len(), 2);
        // A path smuggled in from outside the root must be refused.
        files.push((outside.join("c.txt"), 1));

        let mut out = Cleaned::default();
        let mut ticks = 0;
        delete(&roots, &files, None, &mut out, &mut |_| ticks += 1);
        assert_eq!(ticks, 3, "progress counts every file handled");
        assert_eq!((out.deleted, out.freed, out.skipped), (2, 6, 1));
        assert!(outside.join("c.txt").exists(), "file outside the root was deleted");
        assert!(inside.exists(), "the root folder itself must stay");
        assert!(!inside.join("sub").exists(), "empty sub-folder should be pruned");

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn refuses_dangerous_roots() {
        assert!(!safe(&tree(PathBuf::from(r"C:\"))), "drive root");
        assert!(!safe(&tree(PathBuf::from(r"Windows\Temp"))), "relative path from a missing variable");
        assert!(!safe(&tree(PathBuf::from(""))), "empty path");
        let windir = PathBuf::from(std::env::var_os("WINDIR").unwrap());
        assert!(!safe(&tree(windir.clone())), "whole Windows folder");
        let profile = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
        assert!(!safe(&tree(profile.join(""))), "profile, even with a trailing slash");
        // A prefix-filtered, non-recursive root may sit in a big folder.
        assert!(safe(&Root { dir: windir.clone(), recursive: false, filter: Filter::Prefixes(&["memory.dmp"]) }));
        assert!(safe(&tree(windir.join("Temp"))));
    }

    #[test]
    fn refused_root_is_neither_scanned_nor_cleaned() {
        let roots = vec![tree(PathBuf::from(r"relative\folder"))];
        assert!(collect(&roots, None).is_empty());
        let mut out = Cleaned::default();
        delete(&roots, &vec![(PathBuf::from(r"relative\folder\x.tmp"), 1)], None, &mut out, &mut |_| {});
        assert_eq!((out.deleted, out.skipped), (0, 1));
    }

    fn junction(link: &Path, target: &Path) {
        let ok = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "could not create a junction for the test");
    }

    #[test]
    fn junction_as_root_is_neither_scanned_nor_cleaned() {
        let base = std::env::temp_dir().join(format!("xcare-rootlink-{}", std::process::id()));
        let elsewhere = base.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(elsewhere.join("keep.txt"), b"k").unwrap();
        let link = base.join("cache");
        junction(&link, &elsewhere);

        let roots = vec![tree(link.clone())];
        assert!(collect(&roots, None).is_empty(), "scanned through a junction root");
        let mut out = Cleaned::default();
        delete(&roots, &vec![(link.join("keep.txt"), 1)], None, &mut out, &mut |_| {});
        assert_eq!((out.deleted, out.skipped), (0, 1));
        assert!(elsewhere.join("keep.txt").exists());

        fs::remove_dir(&link).unwrap();
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn folder_swapped_for_a_junction_after_the_scan_is_not_cleaned() {
        let base = std::env::temp_dir().join(format!("xcare-swap-{}", std::process::id()));
        let root = base.join("cache");
        let elsewhere = base.join("elsewhere");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(root.join("sub").join("a.tmp"), b"a").unwrap();
        let roots = vec![tree(root.clone())];
        let files = collect(&roots, None);
        assert_eq!(files.len(), 1);

        // Between scan and clean, "sub" becomes a junction to a folder with a same-named file.
        fs::remove_dir_all(root.join("sub")).unwrap();
        fs::write(elsewhere.join("a.tmp"), b"a").unwrap();
        junction(&root.join("sub"), &elsewhere);

        let mut out = Cleaned::default();
        delete(&roots, &files, None, &mut out, &mut |_| {});
        assert_eq!((out.deleted, out.skipped), (0, 1));
        assert!(elsewhere.join("a.tmp").exists(), "deleted through a junction");

        fs::remove_dir(root.join("sub")).unwrap();
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn prefix_filter_matches_case_insensitively() {
        let base = std::env::temp_dir().join(format!("xcare-prefix-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        fs::write(base.join("THUMBCACHE_256.db"), b"x").unwrap();
        fs::write(base.join("other.db"), b"x").unwrap();
        let roots = vec![Root { dir: base.clone(), recursive: false, filter: Filter::Prefixes(&["thumbcache_"]) }];
        let files = collect(&roots, None);
        assert_eq!(files.len(), 1);
        fs::remove_dir_all(&base).unwrap();
    }
}
