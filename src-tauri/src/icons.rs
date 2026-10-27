//! Real icons for the lists: a program's own icon from its .exe, shortcut or
//! .ico, and Windows' stock icons for the cleaner categories. Each comes back
//! as a PNG data URL the window can show directly.

use base64::Engine;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::null_mut;
use std::sync::{Mutex, OnceLock};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use crate::win::wide;

/// Pixels per side; the lists draw them at 28 px, so this stays sharp at 150 %.
const SIZE: u32 = 48;

#[repr(C)]
struct ShFileInfo {
    icon: isize,
    index: i32,
    attributes: u32,
    display_name: [u16; 260],
    type_name: [u16; 80],
}

#[repr(C)]
struct StockIconInfo {
    size: u32,
    icon: isize,
    system_index: i32,
    index: i32,
    path: [u16; 260],
}

#[repr(C)]
struct IconInfo {
    is_icon: i32,
    hotspot_x: u32,
    hotspot_y: u32,
    mask: isize,
    color: isize,
}

#[repr(C)]
struct Bitmap {
    kind: i32,
    width: i32,
    height: i32,
    width_bytes: i32,
    planes: u16,
    bits_pixel: u16,
    bits: *mut c_void,
}

#[repr(C)]
struct BitmapInfo {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    used: u32,
    important: u32,
    colors: [u32; 4],
}

#[link(name = "shell32")]
extern "system" {
    fn SHDefExtractIconW(file: *const u16, index: i32, flags: u32, large: *mut isize, small: *mut isize, size: u32) -> i32;
    fn SHGetFileInfoW(path: *const u16, attributes: u32, info: *mut ShFileInfo, size: u32, flags: u32) -> usize;
    fn SHGetStockIconInfo(id: u32, flags: u32, info: *mut StockIconInfo) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn GetIconInfo(icon: isize, info: *mut IconInfo) -> i32;
    fn DestroyIcon(icon: isize) -> i32;
    fn GetDC(hwnd: isize) -> isize;
    fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn GetObjectW(object: isize, size: i32, out: *mut c_void) -> i32;
    fn GetDIBits(dc: isize, bitmap: isize, start: u32, lines: u32, bits: *mut c_void, info: *mut BitmapInfo, usage: u32) -> i32;
    fn DeleteObject(object: isize) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn ExpandEnvironmentStringsW(source: *const u16, out: *mut u16, size: u32) -> u32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *const c_void, flags: u32) -> i32;
}

/// The shell calls below want COM on the calling thread.
fn com() {
    thread_local!(static READY: () = unsafe { CoInitializeEx(std::ptr::null(), 2); });
    READY.with(|_| ());
}

/// Reads a 32-bit top-down copy of a bitmap as BGRA.
unsafe fn pixels(dc: isize, bitmap: isize, width: i32, height: i32) -> Option<Vec<u8>> {
    let mut info = BitmapInfo {
        size: 40,
        width,
        height: -height,
        planes: 1,
        bit_count: 32,
        compression: 0,
        size_image: 0,
        x_ppm: 0,
        y_ppm: 0,
        used: 0,
        important: 0,
        colors: [0; 4],
    };
    let mut out = vec![0u8; (width * height * 4) as usize];
    let lines = GetDIBits(dc, bitmap, 0, height as u32, out.as_mut_ptr().cast(), &mut info, 0);
    (lines == height).then_some(out)
}

/// An icon handle as a PNG data URL. Frees the handle.
fn png(icon: isize) -> Option<String> {
    if icon == 0 {
        return None;
    }
    let rgba = unsafe {
        let mut info: IconInfo = std::mem::zeroed();
        let ok = GetIconInfo(icon, &mut info) != 0;
        DestroyIcon(icon);
        if !ok {
            return None;
        }
        let mut result = None;
        let mut bitmap: Bitmap = std::mem::zeroed();
        // Monochrome icons (no colour bitmap) are left out: none of the lists need them.
        if info.color != 0 && GetObjectW(info.color, std::mem::size_of::<Bitmap>() as i32, (&mut bitmap as *mut Bitmap).cast()) != 0 {
            let (w, h) = (bitmap.width, bitmap.height);
            let dc = GetDC(0);
            if let Some(mut bgra) = pixels(dc, info.color, w, h) {
                // Old icons carry no alpha; their mask says which pixels show.
                if bgra.chunks(4).all(|p| p[3] == 0) {
                    if let Some(mask) = pixels(dc, info.mask, w, h) {
                        for (p, m) in bgra.chunks_mut(4).zip(mask.chunks(4)) {
                            p[3] = if m[0] == 0 { 255 } else { 0 };
                        }
                    }
                }
                for p in bgra.chunks_mut(4) {
                    p.swap(0, 2);
                }
                result = Some((w as u32, h as u32, bgra));
            }
            ReleaseDC(0, dc);
        }
        DeleteObject(info.color);
        DeleteObject(info.mask);
        result
    };
    let (width, height, data) = rgba?;
    let mut bytes = Vec::new();
    {
        let mut encoder = ::png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(::png::ColorType::Rgba);
        encoder.set_depth(::png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&data).ok()?;
    }
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn expand(path: &str) -> String {
    let source = wide(path);
    let mut out = vec![0u16; 1024];
    let n = unsafe { ExpandEnvironmentStringsW(source.as_ptr(), out.as_mut_ptr(), out.len() as u32) } as usize;
    if n == 0 || n > out.len() {
        return path.to_string();
    }
    String::from_utf16_lossy(&out[..n - 1])
}

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Splits an icon location like `"C:\App\app.exe",0` into path and index.
pub fn location(spec: &str) -> (String, i32) {
    let spec = spec.trim();
    let (path, index) = match spec.rsplit_once(',') {
        Some((path, index)) if index.trim().parse::<i32>().is_ok() => (path, index.trim().parse().unwrap_or(0)),
        _ => (spec, 0),
    };
    (path.trim().trim_matches('"').to_string(), index)
}

/// The program a command line starts: `"C:\a b\app.exe" -x` gives `C:\a b\app.exe`.
pub fn program(command: &str) -> String {
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        return rest.split('"').next().unwrap_or_default().to_string();
    }
    // ASCII lowercasing keeps byte offsets, so the index is valid in `command`.
    match command.to_ascii_lowercase().find(".exe") {
        Some(i) => command[..i + 4].to_string(),
        None => command.split_whitespace().next().unwrap_or_default().to_string(),
    }
}

/// The icon of a file: an .exe, .dll or .ico (at `index`), or any other file
/// such as a shortcut, as Explorer shows it.
pub fn file(path: &str, index: i32) -> Option<String> {
    let mut path = expand(path);
    if !path.is_empty() && !Path::new(&path).is_absolute() {
        // "rundll32.exe …" and the like live in System32.
        path = format!(r"{}\System32\{}", expand("%SystemRoot%"), path);
    }
    if path.is_empty() || !Path::new(&path).is_file() {
        return None;
    }
    let key = format!("{path}|{index}");
    if let Some(hit) = cache().lock().unwrap().get(&key) {
        return hit.clone();
    }
    com();
    let name = wide(&path);
    let mut icon = 0isize;
    let extracted = unsafe { SHDefExtractIconW(name.as_ptr(), index, 0, &mut icon, null_mut(), SIZE) } == 0;
    let url = if extracted {
        png(icon)
    } else {
        let mut info: ShFileInfo = unsafe { std::mem::zeroed() };
        const SHGFI_ICON: u32 = 0x100;
        let found = unsafe { SHGetFileInfoW(name.as_ptr(), 0, &mut info, std::mem::size_of::<ShFileInfo>() as u32, SHGFI_ICON) };
        if found != 0 { png(info.icon) } else { None }
    };
    cache().lock().unwrap().insert(key, url.clone());
    url
}

/// The icon of what a startup command runs: a shortcut or file as a whole,
/// otherwise the program at the start of the command line.
pub fn command(command: &str) -> Option<String> {
    let whole = command.trim().trim_matches('"');
    if Path::new(&expand(whole)).is_file() {
        return file(whole, 0);
    }
    file(&program(command), 0)
}

/// One of Windows' stock shell icons (SHSTOCKICONID).
pub fn stock(id: u32) -> Option<String> {
    com();
    let mut info: StockIconInfo = unsafe { std::mem::zeroed() };
    info.size = std::mem::size_of::<StockIconInfo>() as u32;
    // Flags 0 = SHGSI_ICONLOCATION: where the icon lives, extracted below at our size.
    if unsafe { SHGetStockIconInfo(id, 0, &mut info) } != 0 {
        return None;
    }
    let end = info.path.iter().position(|&c| c == 0).unwrap_or(info.path.len());
    file(&String::from_utf16_lossy(&info.path[..end]), info.index)
}

/// Where an installed browser's program is, from "App Paths" or its usual folder.
fn browser(exe: &str, usual: &[&str]) -> Option<String> {
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let path: Option<String> = RegKey::predef(hive)
            .open_subkey(format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}"))
            .and_then(|k| k.get_value(""))
            .ok();
        if let Some(path) = path {
            let path = path.trim().trim_matches('"').to_string();
            if Path::new(&expand(&path)).is_file() {
                return Some(path);
            }
        }
    }
    usual.iter().map(|p| expand(p)).find(|p| Path::new(p).is_file())
}

// SHSTOCKICONID values.
const FOLDER: u32 = 3;
const DRIVE: u32 = 8;
const WORLD: u32 = 13;
const RECYCLER_FULL: u32 = 32;
const IMAGES: u32 = 72;
const WARNING: u32 = 78;
const SOFTWARE: u32 = 82;
const NETWORK: u32 = 103;

/// Icons for the cleaner's categories: the browsers' own, Windows' for the rest.
pub fn categories() -> HashMap<&'static str, String> {
    let browsers: [(&str, &str, &[&str]); 5] = [
        ("chrome", "chrome.exe", &[r"%ProgramFiles%\Google\Chrome\Application\chrome.exe", r"%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe"]),
        ("edge", "msedge.exe", &[r"%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe", r"%ProgramFiles%\Microsoft\Edge\Application\msedge.exe"]),
        ("brave", "brave.exe", &[r"%ProgramFiles%\BraveSoftware\Brave-Browser\Application\brave.exe", r"%LOCALAPPDATA%\BraveSoftware\Brave-Browser\Application\brave.exe"]),
        ("opera", "opera.exe", &[r"%LOCALAPPDATA%\Programs\Opera\opera.exe", r"%LOCALAPPDATA%\Programs\Opera\launcher.exe", r"%ProgramFiles%\Opera\launcher.exe"]),
        ("firefox", "firefox.exe", &[r"%ProgramFiles%\Mozilla Firefox\firefox.exe", r"%ProgramFiles(x86)%\Mozilla Firefox\firefox.exe"]),
    ];
    let mut icons = HashMap::new();
    for (id, exe, usual) in browsers {
        if let Some(url) = browser(exe, usual).and_then(|p| file(&p, 0)) {
            icons.insert(id, url);
        }
    }
    let system: [(&str, u32); 10] = [
        ("temp", FOLDER),
        ("wintemp", FOLDER),
        ("thumbs", IMAGES),
        ("inetcache", WORLD),
        ("reports", WARNING),
        ("sysreports", WARNING),
        ("update", SOFTWARE),
        ("delivery", NETWORK),
        ("dumps", DRIVE),
        ("recycle", RECYCLER_FULL),
    ];
    for (id, stock_id) in system {
        if let Some(url) = stock(stock_id) {
            icons.insert(id, url);
        }
    }
    // Shader caches belong to DirectX, whose diagnostic tool carries its logo.
    if let Some(url) = file(r"%SystemRoot%\System32\dxdiag.exe", 0) {
        icons.insert("shaders", url);
    }
    icons
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_program_in_a_command_line() {
        assert_eq!(program(r#""C:\Program Files\App\app.exe" --min"#), r"C:\Program Files\App\app.exe");
        assert_eq!(program(r"C:\Program Files\App\App.EXE /background"), r"C:\Program Files\App\App.EXE");
        assert_eq!(program("rundll32.exe shell32.dll,Control_RunDLL"), "rundll32.exe");
        assert_eq!(program(r"C:\Tools\run.bat now"), r"C:\Tools\run.bat");
    }

    #[test]
    fn reads_icon_locations() {
        assert_eq!(location(r#""C:\App\app.exe",0"#), (r"C:\App\app.exe".into(), 0));
        assert_eq!(location(r"C:\App\app.exe,-101"), (r"C:\App\app.exe".into(), -101));
        assert_eq!(location(r"C:\App\icon.ico"), (r"C:\App\icon.ico".into(), 0));
        assert_eq!(location(r"C:\Odd, Name\app.exe"), (r"C:\Odd, Name\app.exe".into(), 0));
    }

    #[test]
    fn extracts_real_icons_as_png() {
        let explorer = file(r"%SystemRoot%\explorer.exe", 0).expect("explorer icon");
        assert!(explorer.starts_with("data:image/png;base64,"));
        assert!(stock(RECYCLER_FULL).is_some(), "recycle bin stock icon");
        assert!(file(r"C:\no\such\program.exe", 0).is_none());
    }
}
