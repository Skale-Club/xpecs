//! Folder scanner. Walks a folder tree in parallel and keeps every folder in
//! memory (files are only counted), so a whole drive fits and any folder can
//! be listed again instantly. Files of one folder are read from disk when
//! that folder is opened.
//!
//! Rules:
//! - Never follows junctions or directory symbolic links: they are listed,
//!   marked as links and counted as empty, so nothing is counted twice and
//!   a link can't lead the scan out of the folder.
//! - Cloud folders (OneDrive and similar) are reparse points too, but they
//!   hold real content, so they are walked.
//! - Folders that can't be read are marked and counted as empty.
//! - Sizes are file sizes (what Explorer's "Size" shows), not space on disk.

use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DIRECTORY: u32 = 0x10;
const REPARSE_POINT: u32 = 0x400;
/// Reparse tags with this bit are links to another name (junctions,
/// symbolic links); the rest (cloud files, dedup…) hold their own content.
const NAME_SURROGATE: u32 = 0x2000_0000;
const TOP_FILES: usize = 250;
const MAX_DEPTH: usize = 512;
/// Files listed for one folder; the rest are summed in one line.
const MAX_LISTED: usize = 500;
const NO_PARENT: u32 = u32::MAX;

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(name: *const u16, access: u32, share: u32, security: *const c_void, disposition: u32, flags: u32, template: isize) -> isize;
    fn GetFileInformationByHandleEx(handle: isize, class: i32, info: *mut c_void, size: u32) -> i32;
    fn CloseHandle(handle: isize) -> i32;
}

#[repr(C)]
#[derive(Default)]
struct AttributeTagInfo {
    attributes: u32,
    reparse_tag: u32,
}

fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let text = path.as_os_str();
    // Long paths need the \\?\ prefix for CreateFileW.
    let prefixed: std::ffi::OsString = if text.len() >= 248 && !text.to_string_lossy().starts_with(r"\\") {
        [std::ffi::OsStr::new(r"\\?\"), text].iter().copied().collect()
    } else {
        text.to_os_string()
    };
    prefixed.encode_wide().chain(Some(0)).collect()
}

/// True for junctions and symbolic links, and for any reparse point whose
/// tag can't be read: when unsure, don't follow.
fn is_link(path: &Path) -> bool {
    let name = wide(path);
    // FILE_READ_ATTRIBUTES, share all, OPEN_EXISTING, BACKUP_SEMANTICS | OPEN_REPARSE_POINT
    let handle = unsafe { CreateFileW(name.as_ptr(), 0x80, 7, std::ptr::null(), 3, 0x0220_0000, 0) };
    if handle == -1 {
        return true;
    }
    let mut info = AttributeTagInfo::default();
    let ok = unsafe {
        GetFileInformationByHandleEx(handle, 9, &mut info as *mut _ as *mut c_void, std::mem::size_of::<AttributeTagInfo>() as u32)
    };
    unsafe { CloseHandle(handle) };
    ok == 0 || info.reparse_tag & NAME_SURROGATE != 0
}

fn seconds(time: std::io::Result<SystemTime>) -> u64 {
    time.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

/// Lower-case extension with the dot, or "" for none.
fn extension(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => name[i..].to_lowercase(),
        _ => String::new(),
    }
}

// ---------- scanning ----------

/// Live counters a scan updates and the window polls.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub folders: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub current: Mutex<String>,
    cancel: AtomicBool,
}

impl Progress {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BigFile {
    pub path: String,
    pub size: u64,
    pub modified: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TypeStat {
    pub ext: String,
    pub size: u64,
    pub files: u64,
}

/// What a scan shares between its threads besides the progress counters.
struct Shared<'a> {
    progress: &'a Progress,
    top: Mutex<Vec<BigFile>>,
    /// Smallest size that can still enter `top`; skips the lock for small files.
    top_floor: AtomicU64,
    types: Mutex<HashMap<String, (u64, u64)>>,
}

impl Shared<'_> {
    fn offer(&self, path: PathBuf, size: u64, modified: u64) {
        if size <= self.top_floor.load(Ordering::Relaxed) {
            return;
        }
        let mut top = self.top.lock().unwrap();
        top.push(BigFile { path: path.to_string_lossy().into_owned(), size, modified });
        if top.len() >= TOP_FILES * 2 {
            top.sort_unstable_by(|a, b| b.size.cmp(&a.size));
            top.truncate(TOP_FILES);
            self.top_floor.store(top[TOP_FILES - 1].size, Ordering::Relaxed);
        }
    }
}

/// A folder while the scan builds the tree.
#[derive(Default)]
struct Walked {
    name: String,
    size: u64,
    files: u64,
    folders: u64,
    modified: u64,
    denied: bool,
    link: bool,
    children: Vec<Walked>,
}

fn walk(path: &Path, name: String, depth: usize, shared: &Shared) -> Walked {
    let progress = shared.progress;
    let mut node = Walked { name, ..Default::default() };
    if progress.cancelled() {
        return node;
    }
    if let Ok(mut current) = progress.current.try_lock() {
        *current = path.to_string_lossy().into_owned();
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) if depth < MAX_DEPTH => entries,
        _ => {
            node.denied = true;
            progress.errors.fetch_add(1, Ordering::Relaxed);
            return node;
        }
    };
    let mut subfolders = Vec::new();
    let mut types: HashMap<String, (u64, u64)> = HashMap::new();
    for entry in entries {
        let Ok(entry) = entry else {
            progress.errors.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        // DirEntry::metadata comes from the directory listing and does not
        // follow links.
        let Ok(meta) = entry.metadata() else {
            progress.errors.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let modified = seconds(meta.modified());
        // std reports a junction as a symlink, not a directory, so ask the
        // attributes.
        let attributes = meta.file_attributes();
        if attributes & DIRECTORY != 0 {
            if attributes & REPARSE_POINT != 0 && is_link(&entry.path()) {
                node.children.push(Walked { name, modified, link: true, ..Default::default() });
            } else {
                subfolders.push((entry.path(), name));
            }
            continue;
        }
        let size = meta.len();
        node.size += size;
        node.files += 1;
        node.modified = node.modified.max(modified);
        let stat = types.entry(extension(&name)).or_default();
        stat.0 += size;
        stat.1 += 1;
        shared.offer(entry.path(), size, modified);
    }
    progress.files.fetch_add(node.files, Ordering::Relaxed);
    progress.bytes.fetch_add(node.size, Ordering::Relaxed);
    progress.folders.fetch_add(subfolders.len() as u64, Ordering::Relaxed);
    {
        let mut all = shared.types.lock().unwrap();
        for (ext, (size, files)) in types {
            let stat = all.entry(ext).or_default();
            stat.0 += size;
            stat.1 += files;
        }
    }
    let walked: Vec<Walked> = subfolders
        .into_par_iter()
        .map(|(path, name)| walk(&path, name, depth + 1, shared))
        .collect();
    for child in walked {
        node.size += child.size;
        node.files += child.files;
        node.folders += child.folders + 1;
        node.modified = node.modified.max(child.modified);
        node.children.push(child);
    }
    node.folders += node.children.iter().filter(|c| c.link).count() as u64;
    node
}

// ---------- the scanned tree ----------

struct Node {
    name: String,
    parent: u32,
    size: u64,
    files: u64,
    folders: u64,
    modified: u64,
    denied: bool,
    link: bool,
    /// Largest first.
    children: Vec<u32>,
}

pub struct Tree {
    root: PathBuf,
    nodes: Vec<Node>,
    top: Vec<BigFile>,
    types: Vec<TypeStat>,
    pub summary: Summary,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub path: String,
    pub size: u64,
    pub files: u64,
    pub folders: u64,
    pub errors: u64,
    pub elapsed_ms: u64,
    pub cancelled: bool,
    pub scanned_at: u64,
    /// Bytes deleted from inside Xpace since the scan.
    pub freed: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    id: u32,
    name: String,
    size: u64,
    files: u64,
    folders: u64,
    modified: u64,
    denied: bool,
    link: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    name: String,
    size: u64,
    modified: u64,
}

/// One folder opened in the window: its subfolders from the scan and its
/// files as they are on disk now.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    folder: Folder,
    path: String,
    /// Up to the root, root first.
    trail: Vec<Folder>,
    folders: Vec<Folder>,
    files: Vec<FileRow>,
    /// Files beyond the listed ones.
    more_files: u64,
    more_bytes: u64,
}

/// Scans `root`. A cancelled scan still returns what it saw so far.
pub fn scan(root: &Path, progress: &Progress) -> Tree {
    let started = Instant::now();
    let shared = Shared {
        progress,
        top: Mutex::new(Vec::new()),
        top_floor: AtomicU64::new(0),
        types: Mutex::new(HashMap::new()),
    };
    let name = root.to_string_lossy().into_owned();
    let walked = walk(root, name, 0, &shared);

    let mut nodes: Vec<Node> = Vec::new();
    // Iterative, so deep trees can't exhaust the stack.
    let mut stack = vec![(walked, NO_PARENT)];
    while let Some((mut w, parent)) = stack.pop() {
        let id = nodes.len() as u32;
        if parent != NO_PARENT {
            nodes[parent as usize].children.push(id);
        }
        w.children.sort_unstable_by(|a, b| a.size.cmp(&b.size).then_with(|| b.name.cmp(&a.name)));
        let children = std::mem::take(&mut w.children);
        nodes.push(Node {
            name: w.name,
            parent,
            size: w.size,
            files: w.files,
            folders: w.folders,
            modified: w.modified,
            denied: w.denied,
            link: w.link,
            children: Vec::with_capacity(children.len()),
        });
        // Smallest pushed first, so the largest is popped (and numbered) first.
        stack.extend(children.into_iter().map(|c| (c, id)));
    }

    let mut top = shared.top.into_inner().unwrap();
    top.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    top.truncate(TOP_FILES);
    let mut types: Vec<TypeStat> = shared
        .types
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|(ext, (size, files))| TypeStat { ext, size, files })
        .collect();
    types.sort_unstable_by(|a, b| b.size.cmp(&a.size));

    let summary = Summary {
        path: root.to_string_lossy().into_owned(),
        size: nodes[0].size,
        files: nodes[0].files,
        folders: nodes[0].folders,
        errors: progress.errors.load(Ordering::Relaxed),
        elapsed_ms: started.elapsed().as_millis() as u64,
        cancelled: progress.cancelled(),
        scanned_at: seconds(Ok(SystemTime::now())),
        freed: 0,
    };
    Tree { root: root.to_path_buf(), nodes, top, types, summary }
}

impl Tree {
    fn folder(&self, id: u32) -> Folder {
        let n = &self.nodes[id as usize];
        Folder {
            id,
            name: n.name.clone(),
            size: n.size,
            files: n.files,
            folders: n.folders,
            modified: n.modified,
            denied: n.denied,
            link: n.link,
        }
    }

    pub fn path(&self, id: u32) -> Option<PathBuf> {
        let mut names = Vec::new();
        let mut at = id;
        while at != 0 {
            let node = self.nodes.get(at as usize)?;
            names.push(node.name.as_str());
            at = node.parent;
        }
        let mut path = self.root.clone();
        path.extend(names.iter().rev());
        Some(path)
    }

    pub fn listing(&self, id: u32) -> Option<Listing> {
        let node = self.nodes.get(id as usize)?;
        let path = self.path(id)?;
        let mut trail = vec![self.folder(id)];
        let mut at = node.parent;
        while at != NO_PARENT {
            trail.push(self.folder(at));
            at = self.nodes[at as usize].parent;
        }
        trail.reverse();
        let mut files = Vec::new();
        if !node.link && !node.denied {
            if let Ok(entries) = fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let Ok(meta) = entry.metadata() else { continue };
                    if meta.file_attributes() & DIRECTORY == 0 {
                        files.push(FileRow {
                            name: entry.file_name().to_string_lossy().into_owned(),
                            size: meta.len(),
                            modified: seconds(meta.modified()),
                        });
                    }
                }
            }
        }
        files.sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
        let rest = files.split_off(files.len().min(MAX_LISTED));
        Some(Listing {
            folder: self.folder(id),
            path: path.to_string_lossy().into_owned(),
            trail,
            folders: node.children.iter().map(|&c| self.folder(c)).collect(),
            files,
            more_files: rest.len() as u64,
            more_bytes: rest.iter().map(|f| f.size).sum(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder at `path`, if the scan saw it.
    pub fn find(&self, path: &Path) -> Option<u32> {
        let rest = path.strip_prefix(&self.root).ok()?;
        let mut at = 0u32;
        for part in rest.components() {
            let name = part.as_os_str().to_string_lossy();
            at = *self.nodes[at as usize].children.iter().find(|&&c| self.nodes[c as usize].name.eq_ignore_ascii_case(&name))?;
        }
        Some(at)
    }

    /// Takes a deleted file or folder out of the totals, without a rescan.
    /// Folders are known to the scan; for a file, `file_size` is its size.
    /// File types only lose deleted files, since a folder's mix of types
    /// isn't kept.
    pub fn forget(&mut self, path: &Path, file_size: u64) {
        let folder = self.find(path).filter(|&id| id != 0);
        let (parent, size, files, folders) = match folder {
            Some(id) => {
                let node = &self.nodes[id as usize];
                let removed = (node.parent, node.size, node.files, node.folders + 1);
                self.nodes[removed.0 as usize].children.retain(|&c| c != id);
                // Detached: its path can no longer be built, so it can't be opened.
                self.nodes[id as usize].parent = NO_PARENT;
                removed
            }
            None => match path.parent().and_then(|p| self.find(p)) {
                Some(parent) => (parent, file_size, 1, 0),
                None => return,
            },
        };
        let mut at = parent;
        while at != NO_PARENT {
            let node = &mut self.nodes[at as usize];
            node.size = node.size.saturating_sub(size);
            node.files = node.files.saturating_sub(files);
            node.folders = node.folders.saturating_sub(folders);
            let up = node.parent;
            if up != NO_PARENT {
                let mut siblings = std::mem::take(&mut self.nodes[up as usize].children);
                siblings.sort_by(|&a, &b| self.nodes[b as usize].size.cmp(&self.nodes[a as usize].size));
                self.nodes[up as usize].children = siblings;
            }
            at = up;
        }
        let inside = |file: &str| {
            let file = Path::new(file);
            file == path || (folder.is_some() && file.starts_with(path))
        };
        self.top.retain(|f| !inside(&f.path));
        if folder.is_none() {
            let ext = extension(&path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            if let Some(stat) = self.types.iter_mut().find(|t| t.ext == ext) {
                stat.size = stat.size.saturating_sub(size);
                stat.files = stat.files.saturating_sub(1);
            }
            self.types.retain(|t| t.files > 0);
            self.types.sort_unstable_by(|a, b| b.size.cmp(&a.size));
        }
        let root = &self.nodes[0];
        self.summary.size = root.size;
        self.summary.files = root.files;
        self.summary.folders = root.folders;
        self.summary.freed += size;
    }

    pub fn top_files(&self) -> &[BigFile] {
        &self.top
    }

    pub fn types(&self) -> &[TypeStat] {
        &self.types
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("xpace-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
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
    fn sums_sizes_and_sorts_largest_first() {
        let f = Fixture::new("sums");
        fs::create_dir_all(f.0.join("big/inner")).unwrap();
        fs::create_dir_all(f.0.join("small")).unwrap();
        fs::write(f.0.join("root.txt"), [0u8; 10]).unwrap();
        fs::write(f.0.join("big/a.bin"), [0u8; 1000]).unwrap();
        fs::write(f.0.join("big/inner/b.BIN"), [0u8; 500]).unwrap();
        fs::write(f.0.join("small/c.txt"), [0u8; 20]).unwrap();

        let tree = scan(&f.0, &Progress::default());
        assert_eq!((tree.summary.size, tree.summary.files, tree.summary.folders), (1530, 4, 3));
        let root = tree.listing(0).unwrap();
        let names: Vec<_> = root.folders.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["big", "small"]);
        assert_eq!(root.folders[0].size, 1500);
        assert_eq!(root.files.len(), 1);
        let big = tree.listing(root.folders[0].id).unwrap();
        assert_eq!(big.path, f.0.join("big").to_string_lossy());
        assert_eq!(big.trail.len(), 2);
        assert_eq!(tree.top_files()[0].size, 1000);
        let bin = tree.types().iter().find(|t| t.ext == ".bin").unwrap();
        assert_eq!((bin.size, bin.files), (1500, 2));
    }

    #[test]
    fn junctions_are_listed_but_never_followed() {
        let f = Fixture::new("links");
        let outside = Fixture::new("links-outside");
        fs::write(outside.0.join("huge.bin"), [0u8; 4096]).unwrap();
        fs::create_dir_all(f.0.join("real")).unwrap();
        fs::write(f.0.join("real/x.txt"), [0u8; 8]).unwrap();
        junction(&f.0.join("link"), &outside.0);

        let tree = scan(&f.0, &Progress::default());
        assert_eq!((tree.summary.size, tree.summary.files), (8, 1), "counted through a junction");
        let root = tree.listing(0).unwrap();
        let link = root.folders.iter().find(|d| d.name == "link").unwrap();
        assert!(link.link && link.size == 0);
        assert!(tree.listing(link.id).unwrap().files.is_empty(), "listed files through a junction");
        assert!(tree.top_files().iter().all(|b| !b.path.ends_with("huge.bin")));
        fs::remove_dir(f.0.join("link")).unwrap();
    }

    #[test]
    fn deleted_items_leave_the_totals() {
        let f = Fixture::new("forget");
        fs::create_dir_all(f.0.join("big/inner")).unwrap();
        fs::create_dir_all(f.0.join("small")).unwrap();
        fs::write(f.0.join("big/a.bin"), [0u8; 1000]).unwrap();
        fs::write(f.0.join("big/inner/b.bin"), [0u8; 500]).unwrap();
        fs::write(f.0.join("small/c.txt"), [0u8; 20]).unwrap();
        let mut tree = scan(&f.0, &Progress::default());

        // A file: its folder and the root shrink, and it leaves the lists.
        tree.forget(&f.0.join("big/a.bin"), 1000);
        assert_eq!((tree.summary.size, tree.summary.files, tree.summary.freed), (520, 2, 1000));
        let root = tree.listing(0).unwrap();
        assert_eq!(root.folders[0].name, "big", "{:?}", root.folders.iter().map(|d| (&d.name, d.size)).collect::<Vec<_>>());
        assert_eq!(root.folders[0].size, 500);
        assert!(tree.top_files().iter().all(|b| !b.path.ends_with("a.bin")));
        assert_eq!(tree.types().iter().find(|t| t.ext == ".bin").map(|t| (t.size, t.files)), Some((500, 1)));

        // A folder: gone from its parent with everything under it.
        let big = root.folders[0].id;
        tree.forget(&f.0.join("big"), 0);
        assert_eq!((tree.summary.size, tree.summary.files, tree.summary.folders), (20, 1, 1));
        let root = tree.listing(0).unwrap();
        assert_eq!(root.folders.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["small"]);
        assert!(tree.listing(big).is_none());
        assert!(tree.top_files().iter().all(|b| !b.path.contains("inner")));

        // Something the scan never saw changes nothing.
        tree.forget(&f.0.join("elsewhere/x"), 5);
        assert_eq!(tree.summary.size, 20);
    }

    #[test]
    fn a_cancelled_scan_stops_early() {
        let f = Fixture::new("cancel");
        fs::create_dir_all(f.0.join("a/b")).unwrap();
        fs::write(f.0.join("a/b/c.txt"), b"c").unwrap();
        let progress = Progress::default();
        progress.cancel();
        let tree = scan(&f.0, &progress);
        assert!(tree.summary.cancelled);
        assert_eq!(tree.summary.files, 0);
    }

    #[test]
    fn extensions_ignore_leading_dots_and_case() {
        assert_eq!(extension("Movie.MKV"), ".mkv");
        assert_eq!(extension(".gitignore"), "");
        assert_eq!(extension("README"), "");
        assert_eq!(extension("archive.tar.gz"), ".gz");
    }
}
