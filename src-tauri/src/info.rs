//! Hardware and system information, read once at startup from WMI, CPUID,
//! NVML (NVIDIA) and a few Win32 calls.
//!
//! Labels are plain English; the page translates the ones it knows.

use crate::winapi;
use nvml_wrapper::enum_wrappers::device::Clock;
use nvml_wrapper::Nvml;
use raw_cpuid::CpuId;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use wmi::{COMLibrary, Variant, WMIConnection};

type Row = HashMap<String, Variant>;

#[derive(Serialize, Default)]
pub struct Group {
    pub title: String,
    pub rows: Vec<[String; 2]>,
}

impl Group {
    fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), rows: Vec::new() }
    }

    /// Adds a row unless the value is empty.
    fn row(&mut self, label: &str, value: impl Into<String>) {
        let value = value.into();
        if !value.trim().is_empty() {
            self.rows.push([label.to_string(), value.trim().to_string()]);
        }
    }
}

#[derive(Serialize)]
pub struct Section {
    pub id: &'static str,
    pub groups: Vec<Group>,
}

#[derive(Serialize)]
pub struct Info {
    pub summary: Value,
    pub sections: Vec<Section>,
}

// ---------- value helpers ----------

fn text(v: Option<&Variant>) -> String {
    match v {
        Some(Variant::String(s)) => s.trim().to_string(),
        Some(Variant::Bool(b)) => (if *b { "Yes" } else { "No" }).to_string(),
        Some(Variant::I1(n)) => n.to_string(),
        Some(Variant::I2(n)) => n.to_string(),
        Some(Variant::I4(n)) => n.to_string(),
        Some(Variant::I8(n)) => n.to_string(),
        Some(Variant::UI1(n)) => n.to_string(),
        Some(Variant::UI2(n)) => n.to_string(),
        Some(Variant::UI4(n)) => n.to_string(),
        Some(Variant::UI8(n)) => n.to_string(),
        Some(Variant::R4(n)) => n.to_string(),
        Some(Variant::R8(n)) => n.to_string(),
        Some(Variant::Array(items)) => items
            .iter()
            .map(|i| text(Some(i)))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

fn s(row: &Row, key: &str) -> String {
    text(row.get(key))
}

fn n(row: &Row, key: &str) -> u64 {
    num(row.get(key))
}

fn num(v: Option<&Variant>) -> u64 {
    opt_num(v).unwrap_or(0)
}

/// For codes where 0 means something (health: 0 is Healthy), so a missing
/// or null value must stay missing instead of becoming 0.
fn code(row: &Row, key: &str) -> Option<u64> {
    opt_num(row.get(key))
}

fn opt_num(v: Option<&Variant>) -> Option<u64> {
    Some(match v? {
        Variant::String(s) => s.trim().parse().ok()?,
        Variant::UI1(v) => *v as u64,
        Variant::UI2(v) => *v as u64,
        Variant::UI4(v) => *v as u64,
        Variant::UI8(v) => *v,
        Variant::I1(v) => (*v).max(0) as u64,
        Variant::I2(v) => (*v).max(0) as u64,
        Variant::I4(v) => (*v).max(0) as u64,
        Variant::I8(v) => (*v).max(0) as u64,
        Variant::R4(v) => v.max(0.0) as u64,
        Variant::R8(v) => v.max(0.0) as u64,
        _ => return None,
    })
}

fn strings(row: &Row, key: &str) -> Vec<String> {
    match row.get(key) {
        Some(Variant::Array(items)) => items.iter().map(|i| text(Some(i))).filter(|s| !s.is_empty()).collect(),
        Some(v) => Some(text(Some(v))).filter(|s| !s.is_empty()).into_iter().collect(),
        None => Vec::new(),
    }
}

/// WMI arrays of UTF-16 code units, as used by WmiMonitorID.
fn chars(row: &Row, key: &str) -> String {
    match row.get(key) {
        Some(Variant::Array(items)) => items
            .iter()
            .map(|i| num(Some(i)) as u32)
            .take_while(|&c| c != 0)
            .filter_map(char::from_u32)
            .collect::<String>()
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

/// Up to two decimals, without trailing zeros: 1.25 MB, 16 GB, 1.82 TB.
fn trimmed(v: f64, unit: &str) -> String {
    let s = format!("{v:.2}");
    format!("{} {unit}", s.trim_end_matches('0').trim_end_matches('.'))
}

/// Binary units, like Windows Explorer.
pub fn bytes(b: u64) -> String {
    let b = b as f64;
    let k = 1024f64;
    if b >= k.powi(4) {
        trimmed(b / k.powi(4), "TB")
    } else if b >= k.powi(3) {
        trimmed(b / k.powi(3), "GB")
    } else if b >= k.powi(2) {
        trimmed(b / k.powi(2), "MB")
    } else if b >= k {
        format!("{:.0} KB", b / k)
    } else {
        format!("{b:.0} B")
    }
}

fn ghz(mhz: u64) -> String {
    format!("{:.2} GHz", mhz as f64 / 1000.0)
}

/// "20231015000000.000000+000" -> "2023-10-15" (or with the time).
pub(crate) fn cim_date(s: &str, with_time: bool) -> String {
    if s.len() < 8 || !s.as_bytes()[..8].iter().all(u8::is_ascii_digit) {
        return String::new();
    }
    let date = format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8]);
    if with_time && s.len() >= 12 {
        format!("{date} {}:{}", &s[8..10], &s[10..12])
    } else {
        date
    }
}

fn link_speed(bits: u64) -> String {
    if bits == 0 || bits >= 9_000_000_000_000_000 {
        String::new()
    } else if bits >= 1_000_000_000 {
        trimmed(bits as f64 / 1e9, "Gbps")
    } else {
        format!("{} Mbps", bits / 1_000_000)
    }
}

fn memory_type(code: u64) -> &'static str {
    match code {
        20 => "DDR",
        21 => "DDR2",
        24 => "DDR3",
        26 => "DDR4",
        29 => "LPDDR3",
        30 => "LPDDR4",
        34 => "DDR5",
        35 => "LPDDR5",
        _ => "",
    }
}

fn bus_type(code: u64) -> &'static str {
    match code {
        1 => "SCSI",
        2 => "ATAPI",
        3 => "ATA",
        4 => "IEEE 1394",
        6 => "Fibre Channel",
        7 => "USB",
        8 => "RAID",
        9 => "iSCSI",
        10 => "SAS",
        11 => "SATA",
        12 => "SD",
        13 => "MMC",
        15 => "Virtual",
        16 => "Storage Spaces",
        17 => "NVMe",
        _ => "",
    }
}

fn media_type(code: u64) -> &'static str {
    match code {
        3 => "HDD",
        4 => "SSD",
        5 => "SCM",
        _ => "",
    }
}

fn health(code: Option<u64>) -> &'static str {
    match code {
        Some(0) => "Healthy",
        Some(1) => "Warning",
        Some(2) => "Unhealthy",
        _ => "Unknown",
    }
}

fn net_status(code: u64) -> &'static str {
    match code {
        0 => "Disconnected",
        1 => "Connecting",
        2 => "Connected",
        3 => "Disconnecting",
        4 => "Hardware not present",
        5 => "Hardware disabled",
        6 => "Hardware malfunction",
        7 => "Media disconnected",
        8 => "Authenticating",
        _ => "",
    }
}

fn vendor_name(id: &str) -> String {
    match id {
        "GenuineIntel" => "Intel".into(),
        "AuthenticAMD" => "AMD".into(),
        other => other.to_string(),
    }
}

// ---------- collection ----------

struct Wmi {
    cimv2: Option<WMIConnection>,
    storage: Option<WMIConnection>,
    wmi: Option<WMIConnection>,
}

impl Wmi {
    fn query(con: &Option<WMIConnection>, sql: &str) -> Vec<Row> {
        con.as_ref().and_then(|c| c.raw_query(sql).ok()).unwrap_or_default()
    }
    fn q(&self, sql: &str) -> Vec<Row> {
        Self::query(&self.cimv2, sql)
    }
}

/// Reads everything. Must run on a thread without COM initialised as STA
/// (i.e. not the UI thread).
pub fn collect() -> Info {
    let com = COMLibrary::new().ok();
    let wmi = Wmi {
        cimv2: com.and_then(|c| WMIConnection::new(c).ok()),
        storage: com.and_then(|c| WMIConnection::with_namespace_path("ROOT\\Microsoft\\Windows\\Storage", c).ok()),
        wmi: com.and_then(|c| WMIConnection::with_namespace_path("ROOT\\WMI", c).ok()),
    };
    let nvml = Nvml::init().ok();

    let (cpu, cpu_sum) = cpu_section(&wmi);
    let (board, board_sum) = board_section(&wmi);
    let (memory, memory_sum) = memory_section(&wmi);
    let (gpu, gpu_sum) = gpu_section(&wmi, nvml.as_ref());
    let (storage, storage_sum) = storage_section(&wmi);
    let (network, network_sum) = network_section(&wmi);
    let (os, os_sum) = os_section(&wmi);

    Info {
        summary: json!({
            "cpu": cpu_sum,
            "board": board_sum,
            "memory": memory_sum,
            "gpu": gpu_sum,
            "storage": storage_sum,
            "network": network_sum,
            "os": os_sum,
        }),
        sections: vec![cpu, board, memory, gpu, storage, network, os],
    }
}

fn cpu_section(wmi: &Wmi) -> (Section, Value) {
    let rows = wmi.q(
        "SELECT Name, MaxClockSpeed, SocketDesignation, VirtualizationFirmwareEnabled FROM Win32_Processor",
    );
    let proc = rows.first();
    let cpuid = CpuId::new();
    let topo = winapi::cpu_topology();

    let name = cpuid
        .get_processor_brand_string()
        .map(|b| b.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| proc.map(|p| s(p, "Name")))
        .unwrap_or_default();
    let base_mhz = proc.map(|p| n(p, "MaxClockSpeed")).unwrap_or(0);

    let total_cores: usize = topo.cores_by_class.values().sum();
    let mut classes = topo.cores_by_class.iter().rev();
    let performance = classes.next().map(|(_, c)| *c).unwrap_or(0);
    let efficient: usize = classes.map(|(_, c)| *c).sum();

    let mut g = Group::new("Processor");
    g.row("Name", &name);
    if let Some(v) = cpuid.get_vendor_info() {
        g.row("Vendor", vendor_name(v.as_str()));
    }
    if let Some(f) = cpuid.get_feature_info() {
        g.row(
            "Family / model / stepping",
            format!("{} / {:#X} / {}", f.family_id(), f.model_id(), f.stepping_id()),
        );
    }
    if efficient > 0 {
        g.row("Cores", format!("{total_cores} ({performance} P + {efficient} E)"));
    } else {
        g.row("Cores", total_cores.to_string());
    }
    g.row("Threads", topo.threads.to_string());
    if base_mhz > 0 {
        g.row("Base clock", ghz(base_mhz));
    }
    if let Some(p) = proc {
        g.row("Socket", s(p, "SocketDesignation"));
        g.row(
            "Virtualization",
            match p.get("VirtualizationFirmwareEnabled") {
                Some(Variant::Bool(true)) => "Enabled",
                Some(Variant::Bool(false)) => "Disabled",
                _ => "",
            },
        );
    }

    let mut caches = Group::new("Caches");
    for (label, level, kind) in [
        ("L1 data", 1u8, 2u32),
        ("L1 instruction", 1, 1),
        ("L2", 2, 0),
        ("L3", 3, 0),
    ] {
        let parts: Vec<String> = topo
            .caches
            .iter()
            .filter(|((l, k, _), _)| *l == level && *k == kind)
            .rev()
            .map(|((_, _, size), count)| {
                let size = bytes(*size as u64);
                if *count > 1 { format!("{count} × {size}") } else { size }
            })
            .collect();
        caches.row(label, parts.join(" + "));
    }

    let mut sets = Vec::new();
    if let Some(f) = cpuid.get_feature_info() {
        for (on, name) in [
            (f.has_mmx(), "MMX"),
            (f.has_sse(), "SSE"),
            (f.has_sse2(), "SSE2"),
            (f.has_sse3(), "SSE3"),
            (f.has_ssse3(), "SSSE3"),
            (f.has_sse41(), "SSE4.1"),
            (f.has_sse42(), "SSE4.2"),
            (f.has_avx(), "AVX"),
            (f.has_fma(), "FMA3"),
            (f.has_f16c(), "F16C"),
            (f.has_aesni(), "AES-NI"),
            (f.has_popcnt(), "POPCNT"),
            (f.has_rdrand(), "RDRAND"),
            (f.has_vmx(), "VT-x"),
        ] {
            if on {
                sets.push(name);
            }
        }
    }
    if let Some(e) = cpuid.get_extended_feature_info() {
        for (on, name) in [
            (e.has_avx2(), "AVX2"),
            (e.has_avx512f(), "AVX-512"),
            (e.has_bmi1(), "BMI1"),
            (e.has_bmi2(), "BMI2"),
            (e.has_sha(), "SHA"),
        ] {
            if on {
                sets.push(name);
            }
        }
    }
    if let Some(x) = cpuid.get_extended_processor_and_feature_identifiers() {
        if x.has_svm() {
            sets.push("AMD-V");
        }
    }
    let mut isa = Group::new("Instruction sets");
    isa.row("Supported", sets.join(", "));

    let summary = json!({
        "name": name,
        "cores": total_cores,
        "performance": performance,
        "efficient": efficient,
        "threads": topo.threads,
        "baseMhz": base_mhz,
    });
    (Section { id: "cpu", groups: vec![g, caches, isa] }, summary)
}

fn board_section(wmi: &Wmi) -> (Section, Value) {
    let board = wmi.q("SELECT Manufacturer, Product, Version FROM Win32_BaseBoard");
    let bios = wmi.q("SELECT Manufacturer, SMBIOSBIOSVersion, ReleaseDate FROM Win32_BIOS");
    let system = wmi.q("SELECT Manufacturer, Model, SystemType, Name FROM Win32_ComputerSystem");

    let mut b = Group::new("Motherboard");
    let mut bi = Group::new("BIOS");
    let mut sy = Group::new("System");
    let mut summary = json!({});
    if let Some(r) = board.first() {
        b.row("Manufacturer", s(r, "Manufacturer"));
        b.row("Model", s(r, "Product"));
        b.row("Version", s(r, "Version"));
        summary["board"] = json!(format!("{} {}", s(r, "Manufacturer"), s(r, "Product")).trim());
    }
    if let Some(r) = bios.first() {
        bi.row("Vendor", s(r, "Manufacturer"));
        bi.row("Version", s(r, "SMBIOSBIOSVersion"));
        bi.row("Release date", cim_date(&s(r, "ReleaseDate"), false));
        summary["bios"] = json!(s(r, "SMBIOSBIOSVersion"));
        summary["biosDate"] = json!(cim_date(&s(r, "ReleaseDate"), false));
    }
    if let Some(r) = system.first() {
        sy.row("Computer name", s(r, "Name"));
        sy.row("Manufacturer", s(r, "Manufacturer"));
        sy.row("Model", s(r, "Model"));
        sy.row("System type", s(r, "SystemType"));
        summary["computer"] = json!(s(r, "Name"));
    }
    (Section { id: "board", groups: vec![b, bi, sy] }, summary)
}

fn memory_section(wmi: &Wmi) -> (Section, Value) {
    let dimms = wmi.q(
        "SELECT BankLabel, DeviceLocator, Capacity, Speed, ConfiguredClockSpeed, Manufacturer, PartNumber, \
         SMBIOSMemoryType, FormFactor, ConfiguredVoltage FROM Win32_PhysicalMemory",
    );
    let slots: u64 = wmi
        .q("SELECT MemoryDevices FROM Win32_PhysicalMemoryArray")
        .iter()
        .map(|r| n(r, "MemoryDevices"))
        .sum();
    let installed: u64 = dimms.iter().map(|d| n(d, "Capacity")).sum();
    let kind = dimms.first().map(|d| memory_type(n(d, "SMBIOSMemoryType"))).unwrap_or("");
    let speed = dimms.iter().map(|d| n(d, "Speed")).max().unwrap_or(0);

    let mut overview = Group::new("Overview");
    overview.row("Installed", bytes(installed));
    overview.row("Usable", bytes(winapi::memory_status().total_phys));
    if slots > 0 {
        overview.row("Slots used", format!("{} / {slots}", dimms.len()));
    }
    overview.row("Type", kind);
    if speed > 0 {
        overview.row("Speed", format!("{speed} MT/s"));
    }

    let mut groups = vec![overview];
    for d in &dimms {
        let mut g = Group::new(s(d, "DeviceLocator"));
        g.row("Size", bytes(n(d, "Capacity")));
        g.row("Type", memory_type(n(d, "SMBIOSMemoryType")));
        if n(d, "Speed") > 0 {
            g.row("Speed", format!("{} MT/s", n(d, "Speed")));
        }
        if n(d, "ConfiguredClockSpeed") > 0 {
            g.row("Configured speed", format!("{} MT/s", n(d, "ConfiguredClockSpeed")));
        }
        g.row("Manufacturer", s(d, "Manufacturer"));
        g.row("Part number", s(d, "PartNumber"));
        if n(d, "ConfiguredVoltage") > 0 {
            g.row("Voltage", format!("{:.2} V", n(d, "ConfiguredVoltage") as f64 / 1000.0));
        }
        g.row(
            "Form factor",
            match n(d, "FormFactor") {
                8 => "DIMM",
                12 => "SO-DIMM",
                _ => "",
            },
        );
        g.row("Bank", s(d, "BankLabel"));
        groups.push(g);
    }

    let summary = json!({
        "total": installed,
        "type": kind,
        "speed": speed,
        "modules": dimms.len(),
        "moduleSize": dimms.first().map(|d| n(d, "Capacity")).unwrap_or(0),
        "slots": slots,
    });
    (Section { id: "memory", groups }, summary)
}

fn gpu_section(wmi: &Wmi, nvml: Option<&Nvml>) -> (Section, Value) {
    let adapters = wmi.q(
        "SELECT Name, DriverVersion, DriverDate, VideoProcessor, AdapterRAM, CurrentHorizontalResolution, \
         CurrentVerticalResolution, CurrentRefreshRate, PNPDeviceID FROM Win32_VideoController",
    );
    let mut groups = Vec::new();
    let mut summary = Vec::new();

    for a in adapters.iter().filter(|a| !s(a, "PNPDeviceID").to_uppercase().starts_with("ROOT\\")) {
        let name = s(a, "Name");
        let mut g = Group::new(&name);
        let mut vram = 0u64;

        let nv = nvml.and_then(|nvml| {
            let count = nvml.device_count().ok()?;
            (0..count)
                .filter_map(|i| nvml.device_by_index(i).ok())
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
        });
        if let Some(d) = &nv {
            if let Ok(m) = d.memory_info() {
                vram = m.total;
                g.row("Video memory", bytes(m.total));
            }
        } else {
            let ram = n(a, "AdapterRAM");
            vram = ram;
            if ram >= 0xFFF0_0000 {
                g.row("Video memory", "≥ 4 GB");
            } else if ram > 0 {
                g.row("Video memory", bytes(ram));
            }
        }
        g.row("GPU", s(a, "VideoProcessor"));
        g.row("Driver version", s(a, "DriverVersion"));
        g.row("Driver date", cim_date(&s(a, "DriverDate"), false));
        let (w, h, hz) = (
            n(a, "CurrentHorizontalResolution"),
            n(a, "CurrentVerticalResolution"),
            n(a, "CurrentRefreshRate"),
        );
        if w > 0 {
            g.row("Current mode", format!("{w} × {h} @ {hz} Hz"));
        }

        if let (Some(d), Some(nvml)) = (&nv, nvml) {
            if let Ok(v) = nvml.sys_driver_version() {
                g.row("NVIDIA driver", v);
            }
            if let Ok(v) = nvml.sys_cuda_driver_version() {
                g.row("CUDA", format!("{}.{}", v / 1000, (v % 1000) / 10));
            }
            if let Ok(v) = d.vbios_version() {
                g.row("VBIOS", v);
            }
            if let (Ok(gen), Ok(width)) = (d.current_pcie_link_gen(), d.current_pcie_link_width()) {
                let max = match (d.max_pcie_link_gen(), d.max_pcie_link_width()) {
                    (Ok(mg), Ok(mw)) => format!(" (max {mg}.0 ×{mw})"),
                    _ => String::new(),
                };
                g.row("PCI Express", format!("{gen}.0 ×{width}{max}"));
            }
            if let Ok(v) = d.max_clock_info(Clock::Graphics) {
                g.row("Max GPU clock", format!("{v} MHz"));
            }
            if let Ok(v) = d.max_clock_info(Clock::Memory) {
                g.row("Max memory clock", format!("{v} MHz"));
            }
            if let Ok(v) = d.enforced_power_limit() {
                g.row("Power limit", format!("{} W", v / 1000));
            }
        }
        summary.push(json!({ "name": name, "vram": vram }));
        groups.push(g);
    }

    // Monitors: model names and physical size from the EDID data Windows keeps.
    let ids = Wmi::query(&wmi.wmi, "SELECT InstanceName, UserFriendlyName, ManufacturerName FROM WmiMonitorID");
    let sizes = Wmi::query(
        &wmi.wmi,
        "SELECT InstanceName, MaxHorizontalImageSize, MaxVerticalImageSize FROM WmiMonitorBasicDisplayParams",
    );
    let mut displays = Group::new("Displays");
    let mut display_names = Vec::new();
    for m in &ids {
        let name = chars(m, "UserFriendlyName");
        let maker = chars(m, "ManufacturerName");
        let label = if name.is_empty() { maker.clone() } else { name.clone() };
        let size = sizes
            .iter()
            .find(|z| s(z, "InstanceName") == s(m, "InstanceName"))
            .map(|z| (n(z, "MaxHorizontalImageSize") as f64, n(z, "MaxVerticalImageSize") as f64))
            .filter(|(w, h)| *w > 0.0 && *h > 0.0)
            .map(|(w, h)| format!("{:.0}\" · ", (w * w + h * h).sqrt() / 2.54))
            .unwrap_or_default();
        displays.row("Monitor", format!("{label} ({size}{maker})").replace("( ", "("));
        display_names.push(label);
    }
    if !displays.rows.is_empty() {
        groups.push(displays);
    }

    (Section { id: "gpu", groups }, json!({ "adapters": summary, "displays": display_names }))
}

fn storage_section(wmi: &Wmi) -> (Section, Value) {
    let mut disks = Wmi::query(
        &wmi.storage,
        "SELECT DeviceId, FriendlyName, BusType, MediaType, Size, FirmwareVersion, HealthStatus FROM MSFT_PhysicalDisk",
    );
    disks.sort_by_key(|d| s(d, "DeviceId").parse::<u32>().unwrap_or(u32::MAX));

    let mut groups = Vec::new();
    let mut summary = Vec::new();
    for d in &disks {
        let mut g = Group::new(s(d, "FriendlyName"));
        g.row("Disk number", s(d, "DeviceId"));
        g.row("Interface", bus_type(n(d, "BusType")));
        g.row("Type", media_type(n(d, "MediaType")));
        g.row("Capacity", bytes(n(d, "Size")));
        g.row("Firmware", s(d, "FirmwareVersion"));
        g.row("Health", health(code(d, "HealthStatus")));
        summary.push(json!({
            "name": s(d, "FriendlyName"),
            "size": n(d, "Size"),
            "bus": bus_type(n(d, "BusType")),
            "media": media_type(n(d, "MediaType")),
        }));
        groups.push(g);
    }

    let volumes = wmi.q(
        "SELECT DeviceID, VolumeName, FileSystem, Size, FreeSpace FROM Win32_LogicalDisk WHERE DriveType = 2 OR DriveType = 3",
    );
    let mut v = Group::new("Volumes");
    for vol in &volumes {
        let size = n(vol, "Size");
        if size == 0 {
            continue;
        }
        let free = n(vol, "FreeSpace");
        let label = format!("{} {}", s(vol, "DeviceID"), s(vol, "VolumeName"));
        v.row(
            label.trim(),
            format!(
                "{} / {} · {} · {:.0}%",
                bytes(free),
                bytes(size),
                s(vol, "FileSystem"),
                100.0 * (size - free) as f64 / size as f64
            ),
        );
    }
    if !v.rows.is_empty() {
        groups.push(v);
    }
    (Section { id: "storage", groups }, json!(summary))
}

fn network_section(wmi: &Wmi) -> (Section, Value) {
    let mut adapters = wmi.q(
        "SELECT Index, Name, NetConnectionID, NetConnectionStatus, MACAddress, Speed FROM Win32_NetworkAdapter \
         WHERE PhysicalAdapter = TRUE",
    );
    let configs = wmi.q(
        "SELECT Index, IPAddress, DefaultIPGateway, DNSServerSearchOrder, DHCPEnabled \
         FROM Win32_NetworkAdapterConfiguration WHERE IPEnabled = TRUE",
    );
    // Connected adapters first.
    adapters.sort_by_key(|a| n(a, "NetConnectionStatus") != 2);

    let mut groups = Vec::new();
    let mut primary = Value::Null;
    for a in &adapters {
        let mut g = Group::new(s(a, "Name"));
        let status = n(a, "NetConnectionStatus");
        g.row("Connection", s(a, "NetConnectionID"));
        g.row("Status", net_status(status));
        if status == 2 {
            g.row("Link speed", link_speed(n(a, "Speed")));
        }
        g.row("MAC address", s(a, "MACAddress"));
        let config = configs.iter().find(|c| n(c, "Index") == n(a, "Index"));
        let mut ipv4 = String::new();
        if let Some(c) = config {
            let ips = strings(c, "IPAddress");
            ipv4 = ips.iter().filter(|ip| ip.contains('.')).cloned().collect::<Vec<_>>().join(", ");
            g.row("IPv4", &ipv4);
            g.row("IPv6", ips.iter().filter(|ip| ip.contains(':')).cloned().collect::<Vec<_>>().join(", "));
            g.row("Gateway", strings(c, "DefaultIPGateway").join(", "));
            g.row("DNS", strings(c, "DNSServerSearchOrder").join(", "));
            g.row("DHCP", s(c, "DHCPEnabled"));
        }
        if status == 2 && primary.is_null() {
            primary = json!({ "name": s(a, "Name"), "ipv4": ipv4, "speed": link_speed(n(a, "Speed")) });
        }
        groups.push(g);
    }
    (Section { id: "network", groups }, primary)
}

fn os_section(wmi: &Wmi) -> (Section, Value) {
    let os = wmi.q(
        "SELECT Caption, Version, BuildNumber, OSArchitecture, InstallDate, LastBootUpTime, WindowsDirectory \
         FROM Win32_OperatingSystem",
    );
    const CURRENT: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let display_version = winapi::registry_string(CURRENT, "DisplayVersion").unwrap_or_default();
    let ubr = winapi::registry_dword(CURRENT, "UBR");

    let mut g = Group::new("Windows");
    let mut summary = json!({});
    if let Some(r) = os.first() {
        let build = match ubr {
            Some(ubr) => format!("{}.{ubr}", s(r, "BuildNumber")),
            None => s(r, "BuildNumber"),
        };
        g.row("Edition", s(r, "Caption"));
        g.row("Version", &display_version);
        g.row("Build", &build);
        g.row("Architecture", s(r, "OSArchitecture"));
        g.row("Install date", cim_date(&s(r, "InstallDate"), true));
        g.row("Last boot", cim_date(&s(r, "LastBootUpTime"), true));
        g.row("Windows folder", s(r, "WindowsDirectory"));
        summary = json!({
            "name": s(r, "Caption"),
            "version": display_version,
            "build": build,
            "arch": s(r, "OSArchitecture"),
        });
    }

    let mut audio = Group::new("Audio devices");
    for d in wmi.q("SELECT Name FROM Win32_SoundDevice") {
        audio.row("Device", s(&d, "Name"));
    }
    let mut groups = vec![g];
    if !audio.rows.is_empty() {
        groups.push(audio);
    }
    (Section { id: "os", groups }, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_disk_health_is_unknown_not_healthy() {
        let mut row = Row::new();
        assert_eq!(health(code(&row, "HealthStatus")), "Unknown");
        row.insert("HealthStatus".into(), Variant::Null);
        assert_eq!(health(code(&row, "HealthStatus")), "Unknown");
        row.insert("HealthStatus".into(), Variant::UI2(0));
        assert_eq!(health(code(&row, "HealthStatus")), "Healthy");
        row.insert("HealthStatus".into(), Variant::UI2(2));
        assert_eq!(health(code(&row, "HealthStatus")), "Unhealthy");
    }
}