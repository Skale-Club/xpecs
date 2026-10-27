//! Explicit, snapshot-based deletion. Never called by the automatic cleaner.
//! Win32 handles pin ancestors and identify targets; no shell commands, ACL
//! takeover, driver tricks or arbitrary PID termination are used.
use serde::Serialize;
use std::{
    ffi::c_void,
    fs, io,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    sync::Mutex,
    time::{Duration, Instant},
};

const MAX_ENTRIES: usize = 5000;
const READ_ATTR: u32 = 0x80;
const WRITE_ATTR: u32 = 0x100;
const DELETE: u32 = 0x10000;
const REPARSE: u32 = 0x400;
const DIRECTORY: u32 = 0x10;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct FileTime {
    low: u32,
    high: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FileInfo {
    attributes: u32,
    created: FileTime,
    accessed: FileTime,
    written: FileTime,
    volume: u32,
    size_high: u32,
    size_low: u32,
    links: u32,
    index_high: u32,
    index_low: u32,
}
#[repr(C)]
#[derive(Default)]
struct BasicInfo {
    created: i64,
    accessed: i64,
    written: i64,
    changed: i64,
    attributes: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct RmProcess {
    pid: u32,
    started: FileTime,
    name: [u16; 256],
    service: [u16; 64],
    kind: u32,
    status: u32,
    session: u32,
    restartable: i32,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        creation: u32,
        flags: u32,
        template: isize,
    ) -> isize;
    fn CloseHandle(handle: isize) -> i32;
    fn GetFileInformationByHandle(handle: isize, info: *mut FileInfo) -> i32;
    fn SetFileInformationByHandle(handle: isize, class: i32, info: *const c_void, size: u32)
        -> i32;
    fn GetFinalPathNameByHandleW(handle: isize, name: *mut u16, len: u32, flags: u32) -> u32;
    fn MoveFileExW(old: *const u16, new: *const u16, flags: u32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn GetProcessTimes(
        handle: isize,
        created: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn IsProcessCritical(handle: isize, critical: *mut i32) -> i32;
    fn QueryFullProcessImageNameW(handle: isize, flags: u32, name: *mut u16, len: *mut u32) -> i32;
    fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    fn TerminateProcess(handle: isize, code: u32) -> i32;
    fn WaitForSingleObject(handle: isize, millis: u32) -> u32;
}
#[link(name = "rstrtmgr")]
extern "system" {
    fn RmStartSession(handle: *mut u32, flags: u32, key: *mut u16) -> u32;
    fn RmEndSession(handle: u32) -> u32;
    fn RmRegisterResources(
        handle: u32,
        count: u32,
        files: *const *const u16,
        apps: u32,
        processes: *const c_void,
        services: u32,
        names: *const c_void,
    ) -> u32;
    fn RmGetList(
        handle: u32,
        needed: *mut u32,
        count: *mut u32,
        processes: *mut RmProcess,
        reasons: *mut u32,
    ) -> u32;
}
#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(algorithm: isize, buffer: *mut u8, len: u32, flags: u32) -> i32;
}

struct Handle(isize);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn os_error() -> String {
    io::Error::last_os_error().to_string()
}
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
fn label(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string()
}
fn lower(path: &Path) -> String {
    label(path).replace('/', "\\").to_lowercase()
}
pub(crate) fn under(path: &Path, root: &Path) -> bool {
    let (p, r) = (lower(path), lower(root));
    p == r || p.starts_with(&(r.trim_end_matches('\\').to_string() + "\\"))
}

// Restrict to unambiguous local drive paths. Reject ADS, device namespaces,
// reserved DOS names and spelling aliases before Windows normalizes them.
pub(crate) fn parse_path(raw: &str) -> Result<PathBuf, String> {
    let raw = raw.replace('/', "\\");
    let bytes = raw.as_bytes();
    if bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || &bytes[1..3] != b":\\"
        || raw.contains('\0')
    {
        return Err("invalidPath".into());
    }
    for part in raw[3..].trim_end_matches('\\').split('\\') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || part.contains([':', '*', '?', '"', '<', '>', '|'])
            || part.chars().any(|c| c.is_control())
            || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err("invalidPath".into());
        }
    }
    Ok(PathBuf::from(raw.trim_end_matches('\\')))
}

pub(crate) fn protected(path: &Path) -> bool {
    let mut trees: Vec<PathBuf> = [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "ProgramData",
    ]
    .iter()
    .filter_map(std::env::var_os)
    .map(PathBuf::from)
    .collect();
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    trees.extend(
        [
            "Windows",
            "Program Files",
            "Program Files (x86)",
            "ProgramData",
            "System Volume Information",
            "$Recycle.Bin",
            "Recovery",
            "Boot",
            "EFI",
            "$WINDOWS.~BT",
            "$WinREAgent",
        ]
        .iter()
        .map(|p| PathBuf::from(format!("{drive}\\{p}"))),
    );
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            trees.push(p.to_path_buf());
        }
    }
    // Every volume has reserved metadata directories, not just the system disk.
    let s = lower(path);
    if s.len() <= 3
        || s[3..]
            .split('\\')
            .any(|p| ["system volume information", "$recycle.bin"].contains(&p))
    {
        return true;
    }
    if trees
        .iter()
        .any(|root| under(path, root) || under(root, path))
    {
        return true;
    }
    let mut containers: Vec<PathBuf> = ["USERPROFILE", "PUBLIC", "APPDATA", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    if let Some(home) = std::env::var_os("USERPROFILE") {
        containers.extend(
            [
                "Desktop",
                "Documents",
                "Downloads",
                "Pictures",
                "Videos",
                "Music",
                "OneDrive",
                "AppData",
            ]
            .iter()
            .map(|p| PathBuf::from(&home).join(p)),
        );
    }
    if containers.iter().any(|p| under(p, path)) {
        return true;
    }
    matches!(
        path.file_name()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "ntuser.dat"
            | "ntuser.dat.log1"
            | "ntuser.dat.log2"
            | "usrclass.dat"
            | "pagefile.sys"
            | "swapfile.sys"
            | "hiberfil.sys"
            | "bootmgr"
            | "bootnxt"
    )
}

fn open(path: &Path, access: u32, share: u32) -> Result<Handle, String> {
    let name = wide(path);
    let h = unsafe { CreateFileW(name.as_ptr(), access, share, null(), 3, 0x02200000, 0) }; // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
    if h == -1 {
        Err(os_error())
    } else {
        Ok(Handle(h))
    }
}
fn info(handle: &Handle) -> Result<FileInfo, String> {
    let mut out = FileInfo::default();
    if unsafe { GetFileInformationByHandle(handle.0, &mut out) } == 0 {
        Err(os_error())
    } else {
        Ok(out)
    }
}
fn final_path(handle: &Handle) -> Result<PathBuf, String> {
    let mut out = vec![0u16; 32768];
    let n = unsafe { GetFinalPathNameByHandleW(handle.0, out.as_mut_ptr(), out.len() as u32, 0) }
        as usize;
    if n == 0 || n >= out.len() {
        return Err("invalidPath".into());
    }
    Ok(PathBuf::from(String::from_utf16_lossy(&out[..n])))
}
// Ancestors stay open without FILE_SHARE_DELETE through each operation. A
// junction or a replacement directory cannot be swapped into the path meanwhile.
fn pin_parents(path: &Path) -> Result<Vec<Handle>, String> {
    let mut pinned = Vec::new();
    let mut ancestors: Vec<_> = path.ancestors().skip(1).collect();
    ancestors.reverse();
    for ancestor in ancestors {
        let handle = open(ancestor, READ_ATTR | 1, 3)?;
        if info(&handle)?.attributes & REPARSE != 0 {
            return Err("reparsePoint".into());
        }
        pinned.push(handle);
    }
    Ok(pinned)
}

#[derive(Clone)]
struct Entry {
    path: PathBuf,
    info: FileInfo,
}
impl Entry {
    fn directory(&self) -> bool {
        self.info.attributes & DIRECTORY != 0
    }
    fn bytes(&self) -> u64 {
        ((self.info.size_high as u64) << 32) | self.info.size_low as u64
    }
}
fn same_entry(a: &FileInfo, b: &FileInfo) -> bool {
    a.volume == b.volume
        && a.index_high == b.index_high
        && a.index_low == b.index_low
        && a.created == b.created
        && (a.attributes & DIRECTORY) == (b.attributes & DIRECTORY)
        && (a.attributes & DIRECTORY != 0
            || (a.written == b.written && a.size_high == b.size_high && a.size_low == b.size_low))
}
fn snapshot(path: &Path, entries: &mut Vec<Entry>, depth: usize) -> Result<(), String> {
    if entries.len() >= MAX_ENTRIES || depth > 64 {
        return Err("tooManyFiles".into());
    }
    if protected(path) {
        return Err("protectedPath".into());
    }
    let _pins = pin_parents(path)?;
    let h = open(path, READ_ATTR, 3)?;
    let metadata = info(&h)?;
    if metadata.attributes & REPARSE != 0 {
        return Err("reparsePoint".into());
    }
    if metadata.links > 1 && metadata.attributes & DIRECTORY == 0 {
        return Err("hardLink".into());
    }
    let physical = final_path(&h)?;
    if protected(&physical) {
        return Err("protectedPath".into());
    }
    if metadata.attributes & DIRECTORY != 0 {
        // Keep the selected directory pinned while enumerating its children.
        for child in fs::read_dir(&physical).map_err(|e| e.to_string())? {
            snapshot(
                &child.map_err(|e| e.to_string())?.path(),
                entries,
                depth + 1,
            )?;
        }
    }
    if entries.len() >= MAX_ENTRIES {
        return Err("tooManyFiles".into());
    }
    entries.push(Entry {
        path: physical,
        info: metadata,
    }); // children before parents
    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocker {
    pid: u32,
    name: String,
    can_terminate: bool,
    #[serde(skip)]
    raw: RmProcess,
}
struct Session(u32);
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            RmEndSession(self.0);
        }
    }
}
fn query_blockers(entries: &[Entry]) -> Result<Vec<Blocker>, String> {
    let (mut handle, mut key) = (0, [0u16; 33]);
    let code = unsafe { RmStartSession(&mut handle, 0, key.as_mut_ptr()) };
    if code != 0 {
        return Err(format!("Restart Manager: {code}"));
    }
    let session = Session(handle);
    let names: Vec<_> = entries
        .iter()
        .filter(|e| !e.directory())
        .map(|e| wide(&e.path))
        .collect();
    if names.is_empty() {
        return Ok(Vec::new());
    }
    for batch in names.chunks(128) {
        let ptrs: Vec<_> = batch.iter().map(|n| n.as_ptr()).collect();
        let code = unsafe {
            RmRegisterResources(
                session.0,
                ptrs.len() as u32,
                ptrs.as_ptr(),
                0,
                null(),
                0,
                null(),
            )
        };
        if code != 0 {
            return Err(format!("Restart Manager: {code}"));
        }
    }
    let mut raw = Vec::<RmProcess>::new();
    for _ in 0..5 {
        let (mut needed, mut count, mut reasons) = (0, raw.len() as u32, 0);
        let code = unsafe {
            RmGetList(
                session.0,
                &mut needed,
                &mut count,
                if raw.is_empty() {
                    null_mut()
                } else {
                    raw.as_mut_ptr()
                },
                &mut reasons,
            )
        };
        if code == 0 {
            raw.truncate(count as usize);
            return Ok(raw
                .into_iter()
                .map(|r| {
                    let name = String::from_utf16_lossy(
                        &r.name[..r.name.iter().position(|v| *v == 0).unwrap_or(r.name.len())],
                    );
                    Blocker {
                        pid: r.pid,
                        name,
                        can_terminate: process_handle(&r, true).is_ok(),
                        raw: r,
                    }
                })
                .collect());
        }
        if code != 234 || needed > 4096 {
            return Err(format!("Restart Manager: {code}"));
        }
        raw.resize_with(needed.max(1) as usize, || unsafe { std::mem::zeroed() });
    }
    Err("blockersChanged".into())
}

fn process_handle(process: &RmProcess, terminate: bool) -> Result<Handle, String> {
    if process.pid <= 4
        || process.pid == std::process::id()
        || !matches!(process.kind, 0 | 1 | 2 | 5)
    {
        return Err("protectedProcess".into());
    }
    let (mut own_session, mut other_session) = (0, 0);
    if unsafe { ProcessIdToSessionId(std::process::id(), &mut own_session) } == 0
        || unsafe { ProcessIdToSessionId(process.pid, &mut other_session) } == 0
        || own_session != other_session
    {
        return Err("protectedProcess".into());
    }
    let h = unsafe {
        OpenProcess(
            0x1000 | if terminate { 0x100001 } else { 0 },
            0,
            process.pid,
        )
    };
    if h == 0 {
        return Err(os_error());
    }
    let handle = Handle(h);
    let (mut created, mut exit, mut kernel, mut user) = (
        FileTime::default(),
        FileTime::default(),
        FileTime::default(),
        FileTime::default(),
    );
    let mut critical = 1;
    if unsafe { GetProcessTimes(h, &mut created, &mut exit, &mut kernel, &mut user) } == 0
        || created != process.started
        || unsafe { IsProcessCritical(h, &mut critical) } == 0
        || critical != 0
    {
        return Err("protectedProcess".into());
    }
    let mut name = vec![0u16; 32768];
    let mut len = name.len() as u32;
    if unsafe { QueryFullProcessImageNameW(h, 0, name.as_mut_ptr(), &mut len) } == 0 {
        return Err(os_error());
    }
    let path = PathBuf::from(String::from_utf16_lossy(&name[..len as usize]));
    let windows =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()));
    if under(&path, &windows) {
        return Err("protectedProcess".into());
    }
    Ok(handle)
}

struct Job {
    token: String,
    created: Instant,
    roots: Vec<String>,
    entries: Vec<Entry>,
    blockers: Vec<Blocker>,
}
#[derive(Default)]
pub struct Service {
    job: Mutex<Option<Job>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    token: String,
    paths: Vec<String>,
    files: usize,
    folders: usize,
    bytes: u64,
    blockers: Vec<Blocker>,
    blocker_error: Option<String>,
}
#[derive(Serialize)]
pub struct Failure {
    path: String,
    error: String,
}
#[derive(Default, Serialize)]
pub struct Outcome {
    deleted: usize,
    scheduled: usize,
    failed: usize,
    closed: usize,
    errors: Vec<Failure>,
}

impl Service {
    pub fn prepare(&self, paths: Vec<String>) -> Result<Preview, String> {
        let mut slot = self.job.lock().map_err(|_| "busy")?;
        *slot = None;
        if paths.is_empty() || paths.len() > 100 {
            return Err("invalidSelection".into());
        }
        let mut parsed = Vec::<PathBuf>::new();
        for path in paths {
            let path = parse_path(&path)?;
            if !parsed.iter().any(|p| lower(p) == lower(&path)) {
                parsed.push(path);
            }
        }
        // A parent selection already includes its selected descendants.
        let roots: Vec<_> = parsed
            .iter()
            .filter(|p| {
                !parsed
                    .iter()
                    .any(|other| lower(p) != lower(other) && under(p, other))
            })
            .cloned()
            .collect();
        let mut entries = Vec::new();
        for root in &roots {
            snapshot(root, &mut entries, 0)?;
        }
        let roots = roots.iter().map(|p| label(p)).collect::<Vec<_>>();
        let (blockers, blocker_error) = match query_blockers(&entries) {
            Ok(b) => (b, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let mut random = [0u8; 16];
        if unsafe { BCryptGenRandom(0, random.as_mut_ptr(), 16, 2) } < 0 {
            return Err("randomFailed".into());
        }
        let token = random
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        let preview = Preview {
            token: token.clone(),
            paths: roots.clone(),
            files: entries.iter().filter(|e| !e.directory()).count(),
            folders: entries.iter().filter(|e| e.directory()).count(),
            bytes: entries
                .iter()
                .filter(|e| !e.directory())
                .map(Entry::bytes)
                .sum(),
            blockers: blockers.clone(),
            blocker_error,
        };
        *slot = Some(Job {
            token,
            created: Instant::now(),
            roots,
            entries,
            blockers,
        });
        Ok(preview)
    }

    pub fn execute(&self, token: &str, mode: &str) -> Result<Outcome, String> {
        if !["delete", "terminate", "reboot"].contains(&mode) {
            return Err("invalidMode".into());
        }
        if mode == "reboot" && !crate::win::is_admin() {
            return Err("adminRequired".into());
        }
        let mut slot = self.job.lock().map_err(|_| "busy")?;
        let job = slot.as_ref().ok_or("expiredSelection")?;
        if job.token != token || job.created.elapsed() > Duration::from_secs(600) {
            return Err("expiredSelection".into());
        }
        // Revalidate the complete selection BEFORE stopping apps or deleting any
        // file. An altered subtree never gets silently added to the snapshot.
        for entry in &job.entries {
            checked_handle(entry, READ_ATTR, 7)?;
        }
        if mode == "terminate" {
            let now = query_blockers(&job.entries)?;
            if now.iter().any(|b| {
                !b.can_terminate
                    || !job
                        .blockers
                        .iter()
                        .any(|old| old.pid == b.pid && old.raw.started == b.raw.started)
            }) {
                return Err("blockersChanged".into());
            }
            if now.is_empty() { /* Already closed: deletion can proceed. */ }
        }
        let job = slot.take().unwrap(); // one-shot token, even on partial failure
        let mut closed = 0;
        if mode == "terminate" {
            let blockers = query_blockers(&job.entries)?;
            // Pin every process handle before terminating any process. PID reuse
            // and unreviewed lockers cannot expand the user's confirmation.
            let mut handles = Vec::new();
            for b in &blockers {
                if !job
                    .blockers
                    .iter()
                    .any(|old| old.pid == b.pid && old.raw.started == b.raw.started)
                {
                    return Err("blockersChanged".into());
                }
                handles.push(process_handle(&b.raw, true)?);
            }
            for h in handles {
                let error = if unsafe { TerminateProcess(h.0, 1) } == 0 {
                    Some(os_error())
                } else {
                    closed += 1;
                    if unsafe { WaitForSingleObject(h.0, 5000) } != 0 {
                        Some("processTimeout".into())
                    } else {
                        None
                    }
                };
                if let Some(error) = error {
                    return Ok(Outcome {
                        closed,
                        failed: job.entries.len(),
                        errors: vec![Failure {
                            path: job.roots.join("\n"),
                            error,
                        }],
                        ..Default::default()
                    });
                }
            }
        }
        let mut out = run_entries(&job.entries, mode == "reboot", &mut native_schedule);
        out.closed = closed;
        Ok(out)
    }
}

fn checked_handle(entry: &Entry, access: u32, share: u32) -> Result<(Vec<Handle>, Handle), String> {
    if protected(&entry.path) {
        return Err("protectedPath".into());
    }
    let pins = pin_parents(&entry.path)?;
    let h = open(&entry.path, access, share)?;
    let current = info(&h)?;
    if current.attributes & REPARSE != 0 {
        return Err("reparsePoint".into());
    }
    if current.links > 1 && current.attributes & DIRECTORY == 0 {
        return Err("hardLink".into());
    }
    if !same_entry(&entry.info, &current) {
        return Err("selectionChanged".into());
    }
    if protected(&final_path(&h)?) {
        return Err("protectedPath".into());
    }
    Ok((pins, h))
}

fn delete_entry(entry: &Entry) -> Result<(), String> {
    let (_pins, h) = checked_handle(entry, DELETE | READ_ATTR | WRITE_ATTR, 1)?;
    let metadata = info(&h)?;
    if metadata.links > 1 {
        return Err("hardLink".into());
    }
    let attributes = metadata.attributes;
    let readonly = attributes & 1 != 0;
    if readonly {
        let writable = attributes & !1;
        let basic = BasicInfo {
            attributes: if writable == 0 { 0x80 } else { writable },
            ..Default::default()
        };
        // Only FILE_ATTRIBUTE_READONLY is removed. Never change ownership/ACLs.
        if unsafe {
            SetFileInformationByHandle(
                h.0,
                0,
                &basic as *const _ as *const c_void,
                std::mem::size_of::<BasicInfo>() as u32,
            )
        } == 0
        {
            return Err(os_error());
        }
    }
    let delete: u8 = 1;
    if unsafe { SetFileInformationByHandle(h.0, 4, &delete as *const _ as *const c_void, 1) } != 0 {
        return Ok(());
    }
    let error = os_error();
    if readonly {
        let basic = BasicInfo {
            attributes,
            ..Default::default()
        };
        unsafe {
            SetFileInformationByHandle(
                h.0,
                0,
                &basic as *const _ as *const c_void,
                std::mem::size_of::<BasicInfo>() as u32,
            );
        }
    }
    Err(error)
}
fn native_schedule(path: &Path) -> Result<(), String> {
    let path = wide(path);
    if unsafe { MoveFileExW(path.as_ptr(), null(), 4) } == 0 {
        Err(os_error())
    } else {
        Ok(())
    }
}
fn run_entries(
    entries: &[Entry],
    reboot: bool,
    schedule: &mut impl FnMut(&Path) -> Result<(), String>,
) -> Outcome {
    let mut out = Outcome::default();
    // Failed descendants prevent scheduling their ancestors. Successful entries
    // stay explicit: a partially registered reboot job is never called deleted.
    let mut failed_paths = Vec::<PathBuf>::new();
    for entry in entries {
        let result = if reboot {
            if failed_paths.iter().any(|p| under(p, &entry.path)) {
                Err("childFailed".into())
            } else {
                checked_handle(entry, READ_ATTR | 1, 3)
                    .and_then(|(_pins, _h)| schedule(&entry.path))
            }
        } else {
            delete_entry(entry)
        };
        match result {
            Ok(()) => {
                if reboot {
                    out.scheduled += 1;
                } else {
                    out.deleted += 1;
                }
            }
            Err(error) => {
                out.failed += 1;
                failed_paths.push(entry.path.clone());
                if out.errors.len() < 100 {
                    out.errors.push(Failure {
                        path: label(&entry.path),
                        error,
                    });
                }
            }
        }
    }
    out
}

// Native pickers keep the application portable, with no dialog plugin/runtime.
#[repr(C)]
#[derive(Default)]
struct OpenFileName {
    size: u32,
    owner: isize,
    instance: isize,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    title_file: *mut u16,
    max_title: u32,
    initial: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    extension_offset: u16,
    default_extension: *const u16,
    data: isize,
    hook: isize,
    template: *const u16,
    reserved: *mut c_void,
    reserved2: u32,
    flags_ex: u32,
}
#[repr(C)]
struct BrowseInfo {
    owner: isize,
    root: *const c_void,
    display: *mut u16,
    title: *const u16,
    flags: u32,
    callback: isize,
    data: isize,
    image: i32,
}
#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(data: *mut OpenFileName) -> i32;
    fn CommDlgExtendedError() -> u32;
}
#[link(name = "shell32")]
extern "system" {
    fn SHBrowseForFolderW(data: *mut BrowseInfo) -> *mut c_void;
    fn SHGetPathFromIDListEx(id: *const c_void, path: *mut u16, len: u32, flags: u32) -> i32;
}
#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *const c_void, flags: u32) -> i32;
    fn CoUninitialize();
    fn CoTaskMemFree(data: *const c_void);
}

pub fn pick(folder: bool, owner: isize) -> Result<Vec<String>, String> {
    if folder {
        let initialized = unsafe { CoInitializeEx(null(), 2) };
        if initialized < 0 {
            return Err(format!("COM: {initialized}"));
        }
        let mut display = [0u16; 260];
        let mut data = BrowseInfo {
            owner,
            root: null(),
            display: display.as_mut_ptr(),
            title: null(),
            flags: 0x41,
            callback: 0,
            data: 0,
            image: 0,
        };
        let id = unsafe { SHBrowseForFolderW(&mut data) };
        let mut path = vec![0u16; 32768];
        let result = if id.is_null() {
            Ok(Vec::new())
        } else {
            let ok = unsafe { SHGetPathFromIDListEx(id, path.as_mut_ptr(), path.len() as u32, 0) };
            unsafe {
                CoTaskMemFree(id);
            }
            if ok == 0 {
                Err("invalidPath".into())
            } else {
                Ok(vec![String::from_utf16_lossy(
                    &path[..path.iter().position(|v| *v == 0).unwrap_or(0)],
                )])
            }
        };
        unsafe {
            CoUninitialize();
        }
        result
    } else {
        let mut buffer = vec![0u16; 65536];
        let mut data = OpenFileName {
            size: std::mem::size_of::<OpenFileName>() as u32,
            owner,
            file: buffer.as_mut_ptr(),
            max_file: buffer.len() as u32,
            flags: 0x80000 | 0x1000 | 0x800 | 0x200 | 8,
            ..Default::default()
        }; // Explorer, file/path exist, multi-select, no chdir
        if unsafe { GetOpenFileNameW(&mut data) } == 0 {
            let code = unsafe { CommDlgExtendedError() };
            return if code == 0 {
                Ok(Vec::new())
            } else {
                Err(format!("File dialog: {code}"))
            };
        }
        let parts: Vec<_> = buffer
            .split(|v| *v == 0)
            .take_while(|v| !v.is_empty())
            .map(String::from_utf16_lossy)
            .collect();
        if parts.len() == 1 {
            Ok(parts)
        } else {
            Ok(parts[1..]
                .iter()
                .map(|p| Path::new(&parts[0]).join(p).to_string_lossy().into_owned())
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            // Tests run in parallel and the clock can repeat, so count too.
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "xpecs-force-test-{}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn entries(&self) -> Vec<Entry> {
            let mut e = Vec::new();
            snapshot(&self.0, &mut e, 0).unwrap();
            e
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn rejects_ambiguous_paths() {
        for p in [
            r"C:\",
            r"C:relative",
            r"\\server\file",
            r"\\?\C:\file",
            r"C:\temp\..\file",
            r"C:\temp\file:stream",
            r"C:\temp\NUL.txt",
            "C:\\temp\\file.",
            "C:\\temp\\file ",
        ] {
            assert!(parse_path(p).is_err(), "{p}");
        }
        assert!(parse_path(r"C:\temp\file.txt").is_ok());
    }
    #[test]
    fn protects_system_and_profile_containers() {
        assert!(protected(Path::new(r"C:\Windows\System32\kernel32.dll")));
        assert!(protected(Path::new(r"C:\Program Files\app\app.exe")));
        let profile = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
        assert!(protected(&profile));
        assert!(protected(&profile.join("Documents")));
        assert!(!protected(&profile.join("Documents\\disposable.txt")));
    }
    #[test]
    fn deletes_snapshot_children_then_directories_and_readonly() {
        let f = Fixture::new();
        fs::create_dir(f.0.join("nested")).unwrap();
        let path = f.0.join("nested\\read only.txt");
        fs::write(&path, "fixture").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        let out = run_entries(&f.entries(), false, &mut |_| panic!("no reboot scheduling"));
        assert_eq!(out.deleted, 3);
        assert_eq!(out.failed, 0, "{}", serde_json::to_string(&out).unwrap());
        assert!(!f.0.exists());
    }
    #[test]
    fn changed_files_are_kept() {
        let f = Fixture::new();
        let path = f.0.join("keep.txt");
        fs::write(&path, "old").unwrap();
        let entries = f.entries();
        fs::write(&path, "changed content").unwrap();
        let out = run_entries(&entries, false, &mut |_| unreachable!());
        assert_eq!(out.deleted, 0);
        assert_eq!(out.failed, 2);
        assert!(path.exists());
    }
    #[test]
    fn new_files_are_not_added_to_the_snapshot() {
        let f = Fixture::new();
        fs::write(f.0.join("old.txt"), "old").unwrap();
        let entries = f.entries();
        fs::write(f.0.join("new.txt"), "new").unwrap();
        let out = run_entries(&entries, false, &mut |_| unreachable!());
        assert_eq!(out.deleted, 1);
        assert_eq!(out.failed, 1);
        assert!(f.0.join("new.txt").exists());
    }
    #[test]
    fn locked_file_is_reported_and_preserved() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        let path = f.0.join("locked.txt");
        fs::write(&path, "locked").unwrap();
        let entries = f.entries();
        let _lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        let out = run_entries(&entries, false, &mut |_| unreachable!());
        assert_eq!(out.deleted, 0);
        assert_eq!(out.failed, 2);
        assert!(path.exists());
    }
    #[test]
    fn mock_reboot_registration_is_postorder_and_reports_partial_failures() {
        let f = Fixture::new();
        fs::write(f.0.join("a.txt"), "a").unwrap();
        let entries = f.entries();
        let mut calls = Vec::new();
        let out = run_entries(&entries, true, &mut |p| {
            calls.push(p.to_path_buf());
            Ok(())
        });
        assert_eq!(out.scheduled, 2);
        assert_eq!(out.deleted, 0);
        assert_eq!(calls.last().unwrap(), &entries.last().unwrap().path);
        assert!(f.0.join("a.txt").exists());
        let mut calls = 0;
        let out = run_entries(&entries, true, &mut |_| {
            calls += 1;
            Err("fixture registration failure".into())
        });
        assert_eq!(calls, 1);
        assert_eq!(out.failed, 2);
        assert_eq!(out.scheduled, 0);
    }
    #[test]
    fn rejects_root_and_nested_junctions() {
        use std::os::windows::process::CommandExt;
        let f = Fixture::new();
        let outside = Fixture::new();
        fs::write(outside.0.join("keep.txt"), "keep").unwrap();
        let link = f.0.join("junction");
        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside.0)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(status.status.success());
        assert_eq!(
            snapshot(&link, &mut Vec::new(), 0).unwrap_err(),
            "reparsePoint"
        );
        assert_eq!(
            snapshot(&f.0, &mut Vec::new(), 0).unwrap_err(),
            "reparsePoint"
        );
        fs::remove_dir(&link).unwrap();
        assert!(outside.0.join("keep.txt").exists());
    }
    #[test]
    fn detects_junction_inserted_after_review() {
        use std::os::windows::process::CommandExt;
        let f = Fixture::new();
        let outside = Fixture::new();
        fs::create_dir(f.0.join("nested")).unwrap();
        fs::write(f.0.join("nested\\keep.txt"), "original").unwrap();
        fs::write(outside.0.join("keep.txt"), "outside").unwrap();
        let entries = f.entries();
        fs::remove_file(f.0.join("nested\\keep.txt")).unwrap();
        fs::remove_dir(f.0.join("nested")).unwrap();
        assert!(std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(f.0.join("nested"))
            .arg(&outside.0)
            .creation_flags(0x08000000)
            .output()
            .unwrap()
            .status
            .success());
        let out = run_entries(&entries, false, &mut |_| unreachable!());
        assert_eq!(out.deleted, 0);
        assert!(outside.0.join("keep.txt").exists());
        fs::remove_dir(f.0.join("nested")).unwrap();
    }
    #[test]
    fn ffi_layouts_match_windows_x64() {
        assert_eq!(std::mem::size_of::<RmProcess>(), 668);
        assert_eq!(std::mem::size_of::<FileInfo>(), 52);
        assert_eq!(std::mem::size_of::<BasicInfo>(), 40);
        assert_eq!(std::mem::size_of::<OpenFileName>(), 152);
        assert_eq!(std::mem::size_of::<BrowseInfo>(), 64);
    }
    #[test]
    fn cannot_terminate_self_or_critical_categories() {
        let mut p: RmProcess = unsafe { std::mem::zeroed() };
        p.pid = std::process::id();
        assert!(process_handle(&p, true).is_err());
        p.pid = 4;
        assert!(process_handle(&p, true).is_err());
        p.pid = 123456;
        for kind in [3, 4, 1000] {
            p.kind = kind;
            assert!(process_handle(&p, true).is_err());
        }
    }

    #[test]
    fn file_replaced_after_review_is_kept() {
        let f = Fixture::new();
        let path = f.0.join("same.txt");
        fs::write(&path, "original").unwrap();
        let entries = f.entries();
        fs::rename(&path, f.0.join("old.txt")).unwrap();
        fs::write(&path, "replacement").unwrap();
        let out = run_entries(&entries, false, &mut |_| unreachable!());
        assert_eq!(out.deleted, 0);
        assert_eq!(fs::read_to_string(path).unwrap(), "replacement");
    }
    #[test]
    fn hard_links_are_refused_without_attribute_changes() {
        let f = Fixture::new();
        fs::write(f.0.join("original.txt"), "keep").unwrap();
        fs::hard_link(f.0.join("original.txt"), f.0.join("alias.txt")).unwrap();
        assert_eq!(snapshot(&f.0, &mut Vec::new(), 0).unwrap_err(), "hardLink");
        assert!(f.0.join("original.txt").exists());
    }
    #[test]
    fn parent_handles_prevent_rename_during_operation() {
        let f = Fixture::new();
        let dir = f.0.join("pinned");
        fs::create_dir(&dir).unwrap();
        let path = dir.join("file.txt");
        fs::write(&path, "keep").unwrap();
        let pins = pin_parents(&path).unwrap();
        assert!(fs::rename(&dir, f.0.join("moved")).is_err());
        drop(pins);
        fs::rename(&dir, f.0.join("moved")).unwrap();
    }
    #[test]
    fn review_token_is_single_use_and_expires() {
        let f = Fixture::new();
        let path = f.0.join("file.txt");
        fs::write(&path, "disposable").unwrap();
        let service = Service::default();
        let preview = service.prepare(vec![label(&path)]).unwrap();
        assert!(service.execute("wrong", "delete").is_err());
        assert!(path.exists());
        service.job.lock().unwrap().as_mut().unwrap().created =
            Instant::now() - Duration::from_secs(601);
        assert!(service.execute(&preview.token, "delete").is_err());
        assert!(path.exists());
        let preview = service.prepare(vec![label(&path)]).unwrap();
        let out = service.execute(&preview.token, "delete").unwrap();
        assert_eq!(out.deleted, 1);
        assert!(service.execute(&preview.token, "delete").is_err());
    }
    #[test]
    fn service_revalidates_all_files_before_deleting_any() {
        let f = Fixture::new();
        let a = f.0.join("a.txt");
        let b = f.0.join("b.txt");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let service = Service::default();
        let p = service.prepare(vec![label(&a), label(&b)]).unwrap();
        fs::write(&b, "changed").unwrap();
        assert!(service.execute(&p.token, "delete").is_err());
        assert!(a.exists());
        assert!(b.exists());
    }
    #[test]
    fn overlapping_and_duplicate_selections_are_counted_once() {
        let f = Fixture::new();
        let path = f.0.join("a.txt");
        fs::write(&path, "a").unwrap();
        let service = Service::default();
        let p = service
            .prepare(vec![label(&f.0), label(&path), label(&f.0)])
            .unwrap();
        assert_eq!(p.paths.len(), 1);
        assert_eq!(p.files, 1);
        assert_eq!(p.folders, 1);
    }
    #[test]
    fn entry_limit_is_enforced_before_any_deletion() {
        let f = Fixture::new();
        fs::write(f.0.join("keep.txt"), "keep").unwrap();
        let entry = f.entries().pop().unwrap();
        let mut entries = vec![entry; MAX_ENTRIES];
        assert_eq!(snapshot(&f.0, &mut entries, 0).unwrap_err(), "tooManyFiles");
        assert!(f.0.join("keep.txt").exists());
    }
    // This test executable is also a disposable lock-holder process. The normal
    // suite never sleeps here; only the parent test sets the fixture environment.
    #[test]
    fn fixture_lock_holder() {
        use std::os::windows::fs::OpenOptionsExt;
        let Some(path) = std::env::var_os("XCARE_TEST_LOCK_FILE") else {
            return;
        };
        let ready = std::env::var_os("XCARE_TEST_LOCK_READY").unwrap();
        let _file = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(path)
            .unwrap();
        fs::write(ready, "ready").unwrap();
        std::thread::sleep(Duration::from_secs(30));
    }
    #[test]
    fn detects_and_terminates_only_the_reviewed_fixture_locker() {
        use std::{
            os::windows::process::CommandExt,
            process::{Child, Command, Stdio},
        };
        struct ChildGuard(Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let f = Fixture::new();
        let path = f.0.join("locked.txt");
        let ready = f.0.join("ready");
        fs::write(&path, "fixture").unwrap();
        let unreviewed_service = Service::default();
        let before_lock = unreviewed_service.prepare(vec![label(&path)]).unwrap();
        assert!(before_lock.blockers.is_empty());
        let mut child = ChildGuard(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "force_delete::tests::fixture_lock_holder",
                    "--nocapture",
                ])
                .env("XCARE_TEST_LOCK_FILE", &path)
                .env("XCARE_TEST_LOCK_READY", &ready)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
                .unwrap(),
        );
        for _ in 0..100 {
            if ready.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(ready.exists(), "fixture child did not acquire the lock");
        assert!(unreviewed_service
            .execute(&before_lock.token, "terminate")
            .is_err());
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "unreviewed blocker must remain running"
        );
        assert!(path.exists());
        let service = Service::default();
        let p = service.prepare(vec![label(&path)]).unwrap();
        assert!(p.blocker_error.is_none(), "{:?}", p.blocker_error);
        assert!(
            p.blockers
                .iter()
                .any(|b| b.pid == child.0.id() && b.can_terminate),
            "fixture PID {} not detected as eligible: {:?}",
            child.0.id(),
            p.blockers
                .iter()
                .map(|b| (
                    b.pid,
                    b.can_terminate,
                    b.raw.kind,
                    process_handle(&b.raw, true).err()
                ))
                .collect::<Vec<_>>()
        );
        assert!(
            p.blockers.iter().all(|b| b.pid == child.0.id()),
            "unexpected process: refusing test termination"
        );
        // PID reuse protection: a mismatched creation time is never authorized.
        let mut fake = p.blockers[0].raw;
        fake.started.high ^= 1;
        assert!(process_handle(&fake, true).is_err());
        let out = service.execute(&p.token, "terminate").unwrap();
        assert_eq!(out.closed, 1);
        assert_eq!(out.deleted, 1);
        assert_eq!(out.failed, 0);
        assert!(!path.exists());
        assert!(child.0.try_wait().unwrap().is_some());
    }

    #[test]
    fn failed_delete_restores_readonly_attribute() {
        let f = Fixture::new();
        let path = f.0.join("nonempty");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("new.txt"), "keep").unwrap();
        let mut entries = Vec::new();
        snapshot(&path, &mut entries, 0).unwrap();
        let directory = entries.pop().unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        assert!(delete_entry(&directory).is_err());
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions).unwrap();
    }
}
