//! Finding a folder to scan from the search box: a typed path is completed
//! like Explorer's address bar, and anything else searches folder names in an
//! index built in the background the first time the user searches.

use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HIDDEN: u32 = 0x2;
const SYSTEM: u32 = 0x4;
const DIRECTORY: u32 = 0x10;
const REPARSE_POINT: u32 = 0x400;

/// How many results the box shows.
const SHOWN: usize = 30;
/// The index stops here, or after this long, whichever comes first.
const MAX_FOLDERS: usize = 250_000;
const MAX_TIME: Duration = Duration::from_secs(45);
const MAX_DEPTH: usize = 7;
/// Folders full of tooling that nobody wants to find by name.
const SKIPPED: [&str; 6] = ["node_modules", ".git", "$recycle.bin", "system volume information", "winsxs", "__pycache__"];

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Hit {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    hits: Vec<Hit>,
    /// The index is still being built: asking again later may find more.
    searching: bool,
}

#[derive(Default)]
struct Folders {
    started: bool,
    done: bool,
    /// Lowercase name and full path.
    list: Vec<(String, PathBuf)>,
}

#[derive(Default)]
pub struct Index(Arc<Mutex<Folders>>);

/// `~` and `%VARIABLES%` expanded, `/` turned into `\`.
pub fn expand(text: &str) -> String {
    let mut text = text.trim().trim_matches('"').replace('/', "\\");
    if text == "~" || text.starts_with("~\\") {
        text = format!("{}{}", std::env::var("USERPROFILE").unwrap_or_default(), &text[1..]);
    }
    while let Some(start) = text.find('%') {
        let Some(len) = text[start + 1..].find('%') else { break };
        let name = &text[start + 1..start + 1 + len];
        let Ok(value) = std::env::var(name) else { break };
        text.replace_range(start..start + len + 2, &value);
    }
    text
}

/// A folder to scan as typed: expanded, without a trailing backslash except
/// on a drive ("C:" becomes "C:\").
pub fn folder(text: &str) -> PathBuf {
    let text = expand(text);
    let trimmed = text.trim_end_matches('\\');
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        PathBuf::from(format!("{trimmed}\\"))
    } else {
        PathBuf::from(trimmed)
    }
}

/// "C:", "C:\..." or "\\server\share".
fn is_path(text: &str) -> bool {
    let b = text.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') || text.starts_with("\\\\")
}

fn listable(meta: &fs::Metadata) -> bool {
    let a = meta.file_attributes();
    a & DIRECTORY != 0 && a & REPARSE_POINT == 0 && a & (HIDDEN | SYSTEM) != (HIDDEN | SYSTEM)
}

/// Subfolders of the folder being typed whose names start with (then
/// contain) what follows the last backslash.
fn complete(text: &str) -> Vec<Hit> {
    let (dir, part) = match text.rfind('\\') {
        Some(i) => (text[..=i].to_string(), &text[i + 1..]),
        None => (format!("{text}\\"), ""),
    };
    let part = part.to_lowercase();
    let Ok(entries) = fs::read_dir(&dir) else { return Vec::new() };
    let mut found: Vec<(bool, Hit)> = entries
        .flatten()
        .filter(|e| e.metadata().is_ok_and(|m| listable(&m)))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            let starts = lower.starts_with(&part);
            (starts || lower.contains(&part)).then(|| (!starts, Hit { path: e.path().to_string_lossy().into_owned(), name }))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())));
    found.into_iter().take(SHOWN).map(|(_, hit)| hit).collect()
}

/// Ranks a folder: 0 its name is the query, 1 starts with it, 2 contains
/// it, 3 every word is in the name or the path; None otherwise.
fn rank(name: &str, path: &Path, query: &str, words: &[&str]) -> Option<u8> {
    if !words.iter().all(|w| name.contains(w)) {
        let path = path.to_string_lossy().to_lowercase();
        if !words.iter().all(|w| name.contains(w) || path.contains(w)) {
            return None;
        }
    }
    Some(if name == query { 0 } else if name.starts_with(query) { 1 } else if name.contains(query) { 2 } else { 3 })
}

fn search(list: &[(String, PathBuf)], query: &str) -> Vec<Hit> {
    let query = query.to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut found: Vec<(u8, usize, &PathBuf)> = list
        .iter()
        .filter_map(|(name, path)| Some((rank(name, path, &query, &words)?, path.components().count(), path)))
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then_with(|| a.2.cmp(b.2)));
    found
        .into_iter()
        .take(SHOWN)
        .map(|(_, _, path)| Hit {
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path: path.to_string_lossy().into_owned(),
        })
        .collect()
}

/// Walks all `roots` together, breadth first, so the top levels of every
/// drive are found within seconds, and adds folder names in batches.
fn build(folders: Arc<Mutex<Folders>>, roots: Vec<PathBuf>) {
    let started = Instant::now();
    // Roots are walked once, even when one is inside another (the profile).
    let seen: HashSet<String> = roots.iter().map(|r| r.to_string_lossy().to_lowercase()).collect();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    let mut queued = HashSet::new();
    for root in roots {
        if queued.insert(root.to_string_lossy().to_lowercase()) {
            queue.push_back((root, 0));
        }
    }
    let mut batch = Vec::new();
    let mut total = 0;
    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !listable(&meta) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let path = entry.path();
            if SKIPPED.contains(&name.as_str()) || seen.contains(&path.to_string_lossy().to_lowercase()) {
                continue;
            }
            if depth + 1 < MAX_DEPTH {
                queue.push_back((path.clone(), depth + 1));
            }
            batch.push((name, path));
            total += 1;
        }
        if batch.len() >= 2000 {
            folders.lock().unwrap().list.append(&mut batch);
        }
        if total >= MAX_FOLDERS || started.elapsed() > MAX_TIME {
            break;
        }
    }
    let mut folders = folders.lock().unwrap();
    folders.list.append(&mut batch);
    folders.done = true;
}

impl Index {
    /// Completes a path, or searches folder names, starting the index the
    /// first time; `roots` are where to look (profile first, then drives).
    pub fn find(&self, text: &str, roots: impl FnOnce() -> Vec<PathBuf>) -> Found {
        let text = expand(text);
        if text.is_empty() {
            return Found { hits: Vec::new(), searching: false };
        }
        if is_path(&text) {
            return Found { hits: complete(&text), searching: false };
        }
        let mut folders = self.0.lock().unwrap();
        if !folders.started {
            folders.started = true;
            let (index, roots) = (self.0.clone(), roots());
            std::thread::spawn(move || build(index, roots));
        }
        Found { hits: search(&folders.list, &text), searching: !folders.done }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_and_variables() {
        std::env::set_var("XPACE_TEST", "D:\\Games");
        assert_eq!(expand(" %XPACE_TEST%/Steam "), "D:\\Games\\Steam");
        assert_eq!(expand("\"C:\\Users\""), "C:\\Users");
        assert!(expand("~\\Downloads").ends_with("\\Downloads"));
        assert_eq!(expand("100%"), "100%");
    }

    #[test]
    fn tidies_a_typed_folder() {
        assert_eq!(folder("C:\\Users\\"), PathBuf::from("C:\\Users"));
        assert_eq!(folder("d:"), PathBuf::from("d:\\"));
        assert_eq!(folder("C:\\\\"), PathBuf::from("C:\\"));
    }

    #[test]
    fn tells_paths_from_names() {
        assert!(is_path("C:") && is_path("c:\\win") && is_path("\\\\nas\\share"));
        assert!(!is_path("downloads") && !is_path("1:"));
    }

    #[test]
    fn completes_a_typed_path() {
        let root = std::env::temp_dir().join(format!("xpace-find-{}", std::process::id()));
        for dir in ["Games", "Music", "My Games"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(root.join("games.txt"), "").unwrap();
        let base = format!("{}\\", root.display());
        let names = |text: &str| complete(text).into_iter().map(|h| h.name).collect::<Vec<_>>();
        assert_eq!(names(&format!("{base}ga")), ["Games", "My Games"]);
        assert_eq!(names(&base), ["Games", "Music", "My Games"]);
        assert!(names(&format!("{base}nope\\")).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ranks_names_then_depth() {
        let list: Vec<(String, PathBuf)> = ["C:\\a\\b\\Steam Library", "C:\\Steam", "C:\\Games\\steamapps", "C:\\x\\Music"]
            .iter()
            .map(|p| (Path::new(p).file_name().unwrap().to_string_lossy().to_lowercase(), PathBuf::from(p)))
            .collect();
        let paths = |q: &str| search(&list, q).into_iter().map(|h| h.path).collect::<Vec<_>>();
        assert_eq!(paths("steam"), ["C:\\Steam", "C:\\Games\\steamapps", "C:\\a\\b\\Steam Library"]);
        assert_eq!(paths("games steam"), ["C:\\Games\\steamapps"]);
        assert!(paths("video").is_empty());
    }
}
