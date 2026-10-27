//! The few Win32 calls Xcare needs, declared directly.

use std::ptr::{null, null_mut};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

#[repr(C)]
struct RecycleInfo {
    size: u32,
    bytes: i64,
    items: i64,
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> isize;
    fn SHQueryRecycleBinW(root: *const u16, info: *mut RecycleInfo) -> i32;
    fn SHEmptyRecycleBinW(hwnd: isize, root: *const u16, flags: u32) -> i32;
    fn IsUserAnAdmin() -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetDiskFreeSpaceExW(dir: *const u16, available: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    fn MultiByteToWideChar(page: u32, flags: u32, src: *const u8, len: i32, dst: *mut u16, dst_len: i32) -> i32;
}

/// Total size and item count of the Recycle Bin on every drive.
pub fn recycle_bin() -> (u64, u64) {
    let mut info = RecycleInfo { size: std::mem::size_of::<RecycleInfo>() as u32, bytes: 0, items: 0 };
    let status = unsafe { SHQueryRecycleBinW(null(), &mut info) };
    if status < 0 {
        return (0, 0);
    }
    (info.bytes.max(0) as u64, info.items.max(0) as u64)
}

/// Empties the Recycle Bin on every drive, without Explorer's dialog.
pub fn empty_recycle_bin() -> bool {
    const NO_CONFIRMATION: u32 = 0x1;
    const NO_PROGRESS_UI: u32 = 0x2;
    const NO_SOUND: u32 = 0x4;
    unsafe { SHEmptyRecycleBinW(0, null(), NO_CONFIRMATION | NO_PROGRESS_UI | NO_SOUND) >= 0 }
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

/// ShellExecute with a verb ("open", or "runas" for a UAC prompt).
pub fn shell(verb: &str, file: &str, params: &str) -> bool {
    const SW_SHOWNORMAL: i32 = 1;
    let (verb, file, params) = (wide(verb), wide(file), wide(params));
    let result = unsafe { ShellExecuteW(0, verb.as_ptr(), file.as_ptr(), params.as_ptr(), null(), SW_SHOWNORMAL) };
    result > 32
}

/// Physical memory in use, in percent.
pub fn memory_load() -> u32 {
    crate::winapi::memory_status().load
}

pub fn uptime_seconds() -> u64 {
    crate::winapi::uptime_seconds()
}

/// (free, total) bytes of the volume holding `root`, e.g. "C:\\".
pub fn disk_space(root: &str) -> Option<(u64, u64)> {
    let root = wide(root);
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(root.as_ptr(), &mut available, &mut total, &mut free) };
    (ok != 0).then_some((available, total))
}

/// Console programs such as powercfg print in the OEM code page.
pub fn from_oem(bytes: &[u8]) -> String {
    const CP_OEMCP: u32 = 1;
    if bytes.is_empty() {
        return String::new();
    }
    unsafe {
        let len = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, null_mut(), 0);
        if len <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut out = vec![0u16; len as usize];
        MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, out.as_mut_ptr(), len);
        String::from_utf16_lossy(&out)
    }
}
