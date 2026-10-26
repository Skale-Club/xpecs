//! Thin wrappers over the few Win32 calls Xpecs needs. All of them work
//! without administrator rights and without a kernel driver.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::ptr::{null, null_mut};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

unsafe fn from_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

// ---------- performance counters (PDH) ----------

#[link(name = "pdh")]
extern "system" {
    fn PdhOpenQueryW(source: *const u16, user: usize, query: *mut isize) -> u32;
    fn PdhAddEnglishCounterW(query: isize, path: *const u16, user: usize, counter: *mut isize) -> u32;
    fn PdhCollectQueryData(query: isize) -> u32;
    fn PdhGetFormattedCounterArrayW(
        counter: isize,
        format: u32,
        size: *mut u32,
        count: *mut u32,
        items: *mut c_void,
    ) -> u32;
    fn PdhCloseQuery(query: isize) -> u32;
}

const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
const PDH_MORE_DATA: u32 = 0x8000_07D2;

#[repr(C)]
struct PdhValue {
    status: u32,
    value: f64,
}

#[repr(C)]
struct PdhItem {
    name: *const u16,
    value: PdhValue,
}

/// A PDH query holding wildcard counters such as `\PhysicalDisk(*)\Disk Read Bytes/sec`.
pub struct Pdh {
    query: isize,
    counters: Vec<isize>,
}

impl Pdh {
    pub fn new() -> Option<Self> {
        let mut query = 0;
        let status = unsafe { PdhOpenQueryW(null(), 0, &mut query) };
        (status == 0).then_some(Self { query, counters: Vec::new() })
    }

    /// Adds a counter by its English path; returns its index, or None if
    /// this machine does not have it.
    pub fn add(&mut self, path: &str) -> Option<usize> {
        let mut counter = 0;
        let path = wide(path);
        let status = unsafe { PdhAddEnglishCounterW(self.query, path.as_ptr(), 0, &mut counter) };
        (status == 0).then(|| {
            self.counters.push(counter);
            self.counters.len() - 1
        })
    }

    pub fn collect(&self) {
        unsafe { PdhCollectQueryData(self.query) };
    }

    /// Instance name and value of every instance with valid data.
    pub fn read(&self, index: Option<usize>) -> Vec<(String, f64)> {
        let Some(&counter) = index.and_then(|i| self.counters.get(i)) else { return Vec::new() };
        let format = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;
        let (mut size, mut count) = (0u32, 0u32);
        unsafe {
            let status = PdhGetFormattedCounterArrayW(counter, format, &mut size, &mut count, null_mut());
            if status != PDH_MORE_DATA {
                return Vec::new();
            }
            // u64 storage keeps the item array 8-byte aligned.
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            let status = PdhGetFormattedCounterArrayW(
                counter,
                format,
                &mut size,
                &mut count,
                buf.as_mut_ptr() as *mut c_void,
            );
            if status != 0 {
                return Vec::new();
            }
            let items = std::slice::from_raw_parts(buf.as_ptr() as *const PdhItem, count as usize);
            items
                .iter()
                .filter(|item| item.value.status <= 1) // valid or new data
                .map(|item| (from_wide(item.name), item.value.value))
                .collect()
        }
    }
}

impl Drop for Pdh {
    fn drop(&mut self) {
        unsafe { PdhCloseQuery(self.query) };
    }
}

// ---------- kernel32 ----------

#[repr(C)]
#[derive(Default)]
pub struct MemoryStatus {
    length: u32,
    pub load: u32,
    pub total_phys: u64,
    pub avail_phys: u64,
    pub total_page_file: u64,
    pub avail_page_file: u64,
    total_virtual: u64,
    avail_virtual: u64,
    avail_ext_virtual: u64,
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    fn GetTickCount64() -> u64;
    fn GetLogicalProcessorInformationEx(relation: u32, buffer: *mut u8, length: *mut u32) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        disposition: u32,
        flags: u32,
        template: isize,
    ) -> isize;
    fn DeviceIoControl(
        device: isize,
        code: u32,
        input: *const c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(handle: isize) -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        key: isize,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> i32;
}

const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;

/// A REG_SZ value under HKEY_LOCAL_MACHINE.
pub fn registry_string(subkey: &str, value: &str) -> Option<String> {
    const RRF_RT_REG_SZ: u32 = 0x2;
    let (subkey, value) = (wide(subkey), wide(value));
    let mut buf = [0u16; 512];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buf.as_mut_ptr() as *mut c_void,
            &mut size,
        )
    };
    (status == 0).then(|| unsafe { from_wide(buf.as_ptr()) })
}

/// A REG_DWORD value under HKEY_LOCAL_MACHINE.
pub fn registry_dword(subkey: &str, value: &str) -> Option<u32> {
    const RRF_RT_REG_DWORD: u32 = 0x10;
    let (subkey, value) = (wide(subkey), wide(value));
    let mut data = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            &mut data as *mut u32 as *mut c_void,
            &mut size,
        )
    };
    (status == 0).then_some(data)
}

pub fn memory_status() -> MemoryStatus {
    let mut status = MemoryStatus { length: std::mem::size_of::<MemoryStatus>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut status) };
    status
}

pub fn uptime_seconds() -> u64 {
    unsafe { GetTickCount64() / 1000 }
}

/// Cores grouped by efficiency class (higher = faster cores on hybrid CPUs),
/// and caches as (level, kind, size in bytes) -> how many.
#[derive(Default)]
pub struct Topology {
    pub cores_by_class: BTreeMap<u8, usize>,
    pub threads: usize,
    pub caches: BTreeMap<(u8, u32, u32), usize>,
}

pub fn cpu_topology() -> Topology {
    const RELATION_ALL: u32 = 0xFFFF;
    const RELATION_CORE: u32 = 0;
    const RELATION_CACHE: u32 = 2;
    let mut topo = Topology::default();
    let mut len = 0u32;
    unsafe {
        GetLogicalProcessorInformationEx(RELATION_ALL, null_mut(), &mut len);
        if len == 0 {
            return topo;
        }
        let mut buf = vec![0u8; len as usize];
        if GetLogicalProcessorInformationEx(RELATION_ALL, buf.as_mut_ptr(), &mut len) == 0 {
            return topo;
        }
        let u32_at = |off: usize| u32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
        let mut off = 0usize;
        while off + 8 <= len as usize {
            let relation = u32_at(off);
            let size = u32_at(off + 4) as usize;
            if size == 0 {
                break;
            }
            match relation {
                RELATION_CORE => {
                    let smt = buf[off + 8] & 1 == 1;
                    *topo.cores_by_class.entry(buf[off + 9]).or_default() += 1;
                    topo.threads += if smt { 2 } else { 1 };
                }
                RELATION_CACHE => {
                    let level = buf[off + 8];
                    let bytes = u32_at(off + 12);
                    let kind = u32_at(off + 16); // 0 unified, 1 instruction, 2 data
                    *topo.caches.entry((level, kind, bytes)).or_default() += 1;
                }
                _ => {}
            }
            off += size;
        }
    }
    topo
}

/// Drive temperature in °C straight from the storage driver (works for most
/// NVMe and many SATA drives). Opening the disk with no access rights is
/// enough for this query, so no administrator rights are needed.
pub fn disk_temperature(disk: u32) -> Option<f64> {
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
    const STORAGE_DEVICE_TEMPERATURE_PROPERTY: u32 = 52;
    const SHARE_READ_WRITE: u32 = 3;
    const OPEN_EXISTING: u32 = 3;

    #[repr(C)]
    struct Query {
        property: u32,
        query_type: u32,
        extra: [u8; 4],
    }

    let path = wide(&format!(r"\\.\PhysicalDrive{disk}"));
    unsafe {
        let handle = CreateFileW(path.as_ptr(), 0, SHARE_READ_WRITE, null_mut(), OPEN_EXISTING, 0, 0);
        if handle == -1 {
            return None;
        }
        let query = Query { property: STORAGE_DEVICE_TEMPERATURE_PROPERTY, query_type: 0, extra: [0; 4] };
        let mut out = [0u8; 512];
        let mut returned = 0u32;
        let ok = DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const Query as *const c_void,
            std::mem::size_of::<Query>() as u32,
            out.as_mut_ptr() as *mut c_void,
            out.len() as u32,
            &mut returned,
            null_mut(),
        );
        CloseHandle(handle);
        // Descriptor: 24-byte header, InfoCount at 12, then 16-byte entries
        // with the temperature (i16, °C) at offset 2.
        if ok == 0 || returned < 40 {
            return None;
        }
        let count = u16::from_le_bytes([out[12], out[13]]);
        if count == 0 {
            return None;
        }
        let temp = i16::from_le_bytes([out[26], out[27]]) as f64;
        (temp > -40.0 && temp < 150.0 && temp != 0.0).then_some(temp)
    }
}
