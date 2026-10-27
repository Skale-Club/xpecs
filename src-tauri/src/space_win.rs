//! Windows bits for Disk space (once Xpace): the drive list, the folder
//! picker, the user's folders, the Recycle Bin and showing a path in Explorer.

use serde::Serialize;
use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::ptr::null;

#[link(name = "kernel32")]
extern "system" {
    fn GetLogicalDrives() -> u32;
    fn GetDriveTypeW(root: *const u16) -> u32;
    fn GetDiskFreeSpaceExW(root: *const u16, available: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    fn GetVolumeInformationW(
        root: *const u16,
        name: *mut u16,
        name_len: u32,
        serial: *mut u32,
        max_component: *mut u32,
        flags: *mut u32,
        fs_name: *mut u16,
        fs_name_len: u32,
    ) -> i32;
    fn SetErrorMode(mode: u32) -> u32;
}

#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);

/// SHFILEOPSTRUCTW (not packed on 64-bit Windows).
#[repr(C)]
struct FileOperation {
    owner: isize,
    function: u32,
    from: *const u16,
    to: *const u16,
    flags: u16,
    aborted: i32,
    mappings: *mut c_void,
    title: *const u16,
}

#[link(name = "shell32")]
extern "system" {
    fn SHFileOperationW(operation: *mut FileOperation) -> i32;
    fn SHCreateItemFromParsingName(path: *const u16, context: *mut c_void, iid: *const Guid, item: *mut *mut c_void) -> i32;
    fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: isize, path: *mut *mut u16) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoCreateInstance(class: *const Guid, outer: *mut c_void, context: u32, iid: *const Guid, object: *mut *mut c_void) -> i32;
    fn CoInitializeEx(reserved: *const c_void, flags: u32) -> i32;
    fn CoUninitialize();
    fn CoTaskMemFree(data: *const c_void);
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn text(buffer: &[u16]) -> String {
    String::from_utf16_lossy(&buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())])
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub path: String,
    label: String,
    file_system: String,
    /// "fixed", "removable", "network" or "other".
    pub kind: &'static str,
    total: u64,
    free: u64,
}

/// Drives with media in them; empty card readers and DVD drives are left out.
pub fn drives() -> Vec<Drive> {
    // No "insert a disk" dialogs for empty drives.
    unsafe { SetErrorMode(0x0001 | 0x8000) };
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .filter_map(|i| {
            let path = format!("{}:\\", (b'A' + i) as char);
            let root = wide(&path);
            let kind = match unsafe { GetDriveTypeW(root.as_ptr()) } {
                2 => "removable",
                3 => "fixed",
                4 => "network",
                _ => "other",
            };
            let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
            if unsafe { GetDiskFreeSpaceExW(root.as_ptr(), &mut available, &mut total, &mut free) } == 0 || total == 0 {
                return None;
            }
            let (mut name, mut fs_name) = ([0u16; 261], [0u16; 261]);
            let (mut serial, mut max_component, mut flags) = (0u32, 0u32, 0u32);
            unsafe {
                GetVolumeInformationW(
                    root.as_ptr(),
                    name.as_mut_ptr(),
                    name.len() as u32,
                    &mut serial,
                    &mut max_component,
                    &mut flags,
                    fs_name.as_mut_ptr(),
                    fs_name.len() as u32,
                )
            };
            Some(Drive { path, label: text(&name), file_system: text(&fs_name), kind, total, free })
        })
        .collect()
}

const FILE_OPEN_DIALOG: Guid = Guid(0xDC1C_5A9C, 0xE88A, 0x4DDE, [0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7]);
const FILE_OPEN_DIALOG_IID: Guid = Guid(0xD57C_7288, 0xD4AD, 0x4768, [0xBE, 0x02, 0x9D, 0x96, 0x95, 0x32, 0xD9, 0x60]);
const SHELL_ITEM_IID: Guid = Guid(0x4382_6D1E, 0xE718, 0x42EE, [0xBC, 0x55, 0xA1, 0xE2, 0x61, 0xC3, 0x7B, 0xFE]);
/// Pick folders, file system only, must exist.
const PICKER_OPTIONS: u32 = 0x20 | 0x40 | 0x800;
type SetOptions = unsafe extern "system" fn(*mut c_void, u32) -> i32;

/// The method at `index` in a COM object's table.
unsafe fn method<F: Copy>(object: *mut c_void, index: usize) -> F {
    let table = *(object as *const *const F);
    *table.add(index)
}

unsafe fn release(object: *mut c_void) {
    method::<unsafe extern "system" fn(*mut c_void) -> u32>(object, 2)(object);
}

/// A path that Windows hands over and the caller must free.
unsafe fn take_text(ptr: *mut u16) -> String {
    let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
    let value = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
    CoTaskMemFree(ptr as *const c_void);
    value
}

/// Windows' own folder picker (IFileOpenDialog), with the address bar,
/// search and favorites; None when the user cancels.
pub fn pick_folder(owner: isize, start: Option<&str>) -> Result<Option<String>, String> {
    type Show = unsafe extern "system" fn(*mut c_void, isize) -> i32;
    type SetFolder = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
    type GetResult = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
    type GetDisplayName = unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16) -> i32;

    const FILE_SYSTEM_PATH: u32 = 0x8005_8000;
    const CANCELLED: i32 = 0x8007_04C7_u32 as i32;
    let initialized = unsafe { CoInitializeEx(null(), 2) };
    if initialized < 0 {
        return Err(format!("COM: {initialized:#x}"));
    }
    let result = unsafe {
        let mut dialog: *mut c_void = std::ptr::null_mut();
        let created = CoCreateInstance(&FILE_OPEN_DIALOG, std::ptr::null_mut(), 1, &FILE_OPEN_DIALOG_IID, &mut dialog);
        if created < 0 {
            Err(format!("COM: {created:#x}"))
        } else {
            method::<SetOptions>(dialog, 9)(dialog, PICKER_OPTIONS);
            // Start where the box points, when it points at a folder.
            if let Some(start) = start.filter(|s| Path::new(s).is_dir()) {
                let mut folder: *mut c_void = std::ptr::null_mut();
                let start = wide(start);
                if SHCreateItemFromParsingName(start.as_ptr(), std::ptr::null_mut(), &SHELL_ITEM_IID, &mut folder) >= 0 {
                    method::<SetFolder>(dialog, 12)(dialog, folder);
                    release(folder);
                }
            }
            let shown = method::<Show>(dialog, 3)(dialog, owner);
            let picked = if shown == CANCELLED {
                Ok(None)
            } else if shown < 0 {
                Err(format!("Windows error {shown:#x}"))
            } else {
                let mut item: *mut c_void = std::ptr::null_mut();
                let got = method::<GetResult>(dialog, 20)(dialog, &mut item);
                if got < 0 {
                    Err(format!("Windows error {got:#x}"))
                } else {
                    let mut name: *mut u16 = std::ptr::null_mut();
                    let named = method::<GetDisplayName>(item, 5)(item, FILE_SYSTEM_PATH, &mut name);
                    release(item);
                    if named < 0 { Err("This place is not a folder on disk.".into()) } else { Ok(Some(take_text(name))) }
                }
            };
            release(dialog);
            picked
        }
    };
    unsafe { CoUninitialize() };
    result
}

/// The user's own folders, where Windows really keeps them (OneDrive may
/// have moved Documents or Desktop): id and path, only those that exist.
pub fn known_folders() -> Vec<(&'static str, String)> {
    const FOLDERS: [(&str, Guid); 9] = [
        ("profile", Guid(0x5E6C_858F, 0x0E22, 0x4760, [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x71, 0x73])),
        ("desktop", Guid(0xB4BF_CC3A, 0xDB2C, 0x424C, [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41])),
        ("documents", Guid(0xFDD3_9AD0, 0x238F, 0x46AF, [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7])),
        ("downloads", Guid(0x374D_E290, 0x123F, 0x4565, [0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B])),
        ("pictures", Guid(0x33E2_8130, 0x4E1E, 0x4676, [0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB])),
        ("videos", Guid(0x1898_9B1D, 0x99B5, 0x455B, [0x84, 0x1C, 0xAB, 0x7C, 0x74, 0xE4, 0xDD, 0xFC])),
        ("music", Guid(0x4BD8_D571, 0x6D19, 0x48D3, [0xBE, 0x97, 0x42, 0x22, 0x20, 0x08, 0x0E, 0x43])),
        ("appdata", Guid(0xF1B3_2785, 0x6FBA, 0x4FCF, [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91])),
        ("programs", Guid(0x905E_63B6, 0xC1BF, 0x494E, [0xB2, 0x9C, 0x65, 0xB7, 0x32, 0xD3, 0xD2, 0x1A])),
    ];
    FOLDERS
        .iter()
        .filter_map(|(id, guid)| {
            let mut ptr: *mut u16 = std::ptr::null_mut();
            let found = unsafe { SHGetKnownFolderPath(guid, 0, 0, &mut ptr) };
            // Windows asks for the buffer to be freed even when it fails.
            let path = if ptr.is_null() { String::new() } else { unsafe { take_text(ptr) } };
            (found >= 0 && Path::new(&path).is_dir()).then_some((*id, path))
        })
        .collect()
}

/// Moves `paths` to the Recycle Bin with Windows' own progress window. Windows
/// still asks before deleting for good anything too big for the Bin, and
/// the user can stop it; check which paths are gone afterwards.
pub fn recycle(owner: isize, paths: &[std::path::PathBuf]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let mut from: Vec<u16> = Vec::new();
    for path in paths {
        from.extend(path.as_os_str().encode_wide());
        from.push(0);
    }
    from.push(0);
    const DELETE: u32 = 3;
    // allow undo (Recycle Bin), no "are you sure" (Xpace asked), but warn
    // before anything would be destroyed instead of recycled
    const FLAGS: u16 = 0x40 | 0x10 | 0x4000;
    let mut operation = FileOperation {
        owner,
        function: DELETE,
        from: from.as_ptr(),
        to: null(),
        flags: FLAGS,
        aborted: 0,
        mappings: std::ptr::null_mut(),
        title: null(),
    };
    let initialized = unsafe { CoInitializeEx(null(), 2) };
    let result = unsafe { SHFileOperationW(&mut operation) };
    if initialized >= 0 {
        unsafe { CoUninitialize() };
    }
    match result {
        0 => Ok(()),
        // the user stopped it, or said no to deleting for good
        0x4C7 => Err("aborted".into()),
        code => Err(format!("Windows error {code:#x}")),
    }
}

/// Opens Explorer with `path` selected (or, for a folder, opened).
pub fn reveal(path: &str, open: bool) -> Result<(), String> {
    let path = Path::new(path);
    // A quote can't be in a Windows name, and would break the argument.
    if !path.is_absolute() || path.to_string_lossy().contains('"') || !path.exists() {
        return Err("notFound".into());
    }
    let shown = path.to_string_lossy();
    let mut command = Command::new("explorer.exe");
    if open {
        command.raw_arg(format!("\"{shown}\""));
    } else {
        command.raw_arg(format!("/select,\"{shown}\""));
    }
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates the folder picker without showing it, and reads its options
    /// back: the ids and method slots are right.
    #[test]
    fn folder_picker_is_set_up() {
        type GetOptions = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        unsafe {
            assert!(CoInitializeEx(null(), 2) >= 0);
            let mut dialog: *mut c_void = std::ptr::null_mut();
            assert_eq!(CoCreateInstance(&FILE_OPEN_DIALOG, std::ptr::null_mut(), 1, &FILE_OPEN_DIALOG_IID, &mut dialog), 0);
            assert_eq!(method::<SetOptions>(dialog, 9)(dialog, PICKER_OPTIONS), 0);
            let mut options = 0;
            assert_eq!(method::<GetOptions>(dialog, 10)(dialog, &mut options), 0);
            assert_eq!(options & PICKER_OPTIONS, PICKER_OPTIONS);
            let mut item: *mut c_void = std::ptr::null_mut();
            let windows = wide(&std::env::var("SystemRoot").unwrap());
            assert_eq!(SHCreateItemFromParsingName(windows.as_ptr(), std::ptr::null_mut(), &SHELL_ITEM_IID, &mut item), 0);
            release(item);
            release(dialog);
            CoUninitialize();
        }
    }

    #[test]
    fn finds_the_user_folders() {
        let folders = known_folders();
        let profile = folders.iter().find(|(id, _)| *id == "profile").map(|(_, p)| p.clone());
        assert_eq!(profile, std::env::var("USERPROFILE").ok());
        assert!(folders.iter().any(|(id, _)| *id == "downloads"));
    }

    #[test]
    fn file_operation_layout_matches_windows_x64() {
        assert_eq!(std::mem::size_of::<FileOperation>(), 56);
    }

    /// Puts a small temporary file in the real Recycle Bin, so it only runs
    /// when asked: cargo test -- --ignored recycles
    #[test]
    #[ignore]
    fn recycles_a_file() {
        let file = std::env::temp_dir().join(format!("xpace-recycle-test-{}.txt", std::process::id()));
        std::fs::write(&file, "Xpace test file, safe to delete.").unwrap();
        recycle(0, &[file.clone()]).unwrap();
        assert!(!file.exists());
    }
}
