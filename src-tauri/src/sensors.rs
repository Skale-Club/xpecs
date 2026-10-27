//! Live sensors, sampled once a second on a background thread and sent to the
//! page as a "sensors" event (a flat list of readings).
//!
//! Sources, none of which need a driver or administrator rights:
//! - Windows performance counters: CPU usage and effective clocks, disks,
//!   network, ACPI thermal zones.
//! - NVML (ships with the NVIDIA driver): GPU temperature, load, clocks, power.
//! - The storage driver: NVMe/SATA drive temperatures.
//! - LibreHardwareMonitor, when it is running: CPU package temperature, fans,
//!   voltages and everything else it reads with its own driver.

use crate::winapi::{self, Pdh};
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use nvml_wrapper::Nvml;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use wmi::{COMLibrary, Variant, WMIConnection};

#[derive(Serialize)]
pub struct Reading {
    id: String,
    group: String,
    name: String,
    value: f64,
    unit: &'static str,
    kind: &'static str,
}

#[derive(Default)]
struct Out(Vec<Reading>);

impl Out {
    fn push(
        &mut self,
        id: impl Into<String>,
        group: &str,
        name: impl Into<String>,
        value: f64,
        unit: &'static str,
        kind: &'static str,
    ) {
        if value.is_finite() {
            self.0.push(Reading { id: id.into(), group: group.to_string(), name: name.into(), value, unit, kind });
        }
    }
}

struct Counters {
    utility: Option<usize>,
    performance: Option<usize>,
    frequency: Option<usize>,
    thermal: Option<usize>,
    disk_read: Option<usize>,
    disk_write: Option<usize>,
    disk_idle: Option<usize>,
    net_in: Option<usize>,
    net_out: Option<usize>,
}

/// Starts sampling once; later calls do nothing.
pub fn spawn(app: AppHandle) {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new().name("sensors".into()).spawn(move || {
        run(|readings| {
            let _ = app.emit("sensors", &readings);
            true
        })
    });
}

/// One set of readings, taken over about a second (rate counters need two
/// samples).
pub fn sample() -> Vec<Reading> {
    let mut taken = Vec::new();
    run(|readings| {
        taken = readings;
        false
    });
    taken
}

/// Samples every second and hands each set to `sink` until it returns false.
fn run(mut sink: impl FnMut(Vec<Reading>) -> bool) {
    let com = COMLibrary::new().ok();
    let disk_names = disk_names(com);

    let mut pdh = Pdh::new();
    let counters = {
        let mut add = |path: &str| pdh.as_mut().and_then(|p| p.add(path));
        Counters {
            utility: add(r"\Processor Information(*)\% Processor Utility"),
            performance: add(r"\Processor Information(*)\% Processor Performance"),
            frequency: add(r"\Processor Information(*)\Processor Frequency"),
            thermal: add(r"\Thermal Zone Information(*)\High Precision Temperature"),
            disk_read: add(r"\PhysicalDisk(*)\Disk Read Bytes/sec"),
            disk_write: add(r"\PhysicalDisk(*)\Disk Write Bytes/sec"),
            disk_idle: add(r"\PhysicalDisk(*)\% Idle Time"),
            net_in: add(r"\Network Interface(*)\Bytes Received/sec"),
            net_out: add(r"\Network Interface(*)\Bytes Sent/sec"),
        }
    };
    let nvml = Nvml::init().ok();

    let mut lhm: Option<WMIConnection> = None;
    let mut lhm_retry = Instant::now();
    let mut disk_temps: HashMap<u32, f64> = HashMap::new();
    let mut active_nics: HashSet<String> = HashSet::new();

    // Rate counters need a first sample to compare against.
    if let Some(p) = &pdh {
        p.collect();
    }
    std::thread::sleep(Duration::from_millis(500));

    for tick in 0u64.. {
        let started = Instant::now();
        let mut out = Out::default();

        if let Some(p) = &pdh {
            p.collect();
            cpu(p, &counters, &mut out);
            thermal(p, &counters, &mut out);
            if tick % 5 == 0 {
                for &disk in disk_names.keys() {
                    match winapi::disk_temperature(disk) {
                        Some(t) => disk_temps.insert(disk, t),
                        None => disk_temps.remove(&disk),
                    };
                }
            }
            disks(p, &counters, &disk_names, &disk_temps, &mut out);
            network(p, &counters, &mut active_nics, &mut out);
        }
        memory(&mut out);
        if let Some(nvml) = &nvml {
            gpus(nvml, &mut out);
        }

        if lhm.is_none() && Instant::now() >= lhm_retry {
            lhm = com.and_then(|c| WMIConnection::with_namespace_path("ROOT\\LibreHardwareMonitor", c).ok());
            lhm_retry = Instant::now() + Duration::from_secs(10);
        }
        if let Some(con) = &lhm {
            if !librehardwaremonitor(con, &mut out) {
                lhm = None;
            }
        }

        if !sink(out.0) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1000).saturating_sub(started.elapsed()));
    }
}

fn find(list: &[(String, f64)], name: &str) -> Option<f64> {
    list.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
}

fn cpu(p: &Pdh, c: &Counters, out: &mut Out) {
    let utility = p.read(c.utility);
    let performance = p.read(c.performance);
    let frequency = p.read(c.frequency);

    if let Some(v) = find(&utility, "_Total") {
        out.push("cpu.usage", "CPU", "Total usage", v.min(100.0), "%", "usage");
    }
    if let (Some(perf), Some(freq)) = (find(&performance, "_Total"), find(&frequency, "_Total")) {
        out.push("cpu.clock", "CPU", "Effective clock", perf / 100.0 * freq, "MHz", "clock");
    }

    // Logical processors are named "group,index".
    let mut threads: Vec<(u32, u32, f64)> = utility
        .iter()
        .filter(|(n, _)| !n.contains("_Total"))
        .filter_map(|(n, v)| {
            let (g, i) = n.split_once(',')?;
            Some((g.parse().ok()?, i.parse().ok()?, *v))
        })
        .collect();
    threads.sort_by_key(|(g, i, _)| (*g, *i));

    let mut fastest = 0f64;
    for (k, (g, i, usage)) in threads.iter().enumerate() {
        out.push(format!("cpu.t{k}.usage"), "CPU threads", format!("#{k}"), usage.min(100.0), "%", "thread");
        let name = format!("{g},{i}");
        if let (Some(perf), Some(freq)) = (find(&performance, &name), find(&frequency, &name)) {
            let mhz = perf / 100.0 * freq;
            fastest = fastest.max(mhz);
            out.push(format!("cpu.t{k}.clock"), "CPU thread clocks", format!("#{k}"), mhz, "MHz", "threadclock");
        }
    }
    if fastest > 0.0 {
        out.push("cpu.clock.max", "CPU", "Fastest thread clock", fastest, "MHz", "clock");
    }
}

fn thermal(p: &Pdh, c: &Counters, out: &mut Out) {
    // Tenths of a kelvin. These ACPI zones are what Windows exposes without a
    // driver; on many desktops they are a fixed board value, not the CPU die.
    for (name, v) in p.read(c.thermal) {
        let zone = name.trim_start_matches("\\_TZ.").to_uppercase();
        out.push(format!("acpi.{zone}"), "Motherboard", format!("ACPI zone {zone}"), v / 10.0 - 273.15, "°C", "temp");
    }
}

fn disk_names(com: Option<COMLibrary>) -> HashMap<u32, String> {
    let Some(con) = com.and_then(|c| WMIConnection::with_namespace_path("ROOT\\Microsoft\\Windows\\Storage", c).ok())
    else {
        return HashMap::new();
    };
    let rows: Vec<HashMap<String, Variant>> =
        con.raw_query("SELECT DeviceId, FriendlyName FROM MSFT_PhysicalDisk").unwrap_or_default();
    rows.iter()
        .filter_map(|r| {
            let id = match r.get("DeviceId") {
                Some(Variant::String(s)) => s.parse().ok()?,
                _ => return None,
            };
            let name = match r.get("FriendlyName") {
                Some(Variant::String(s)) => s.trim().to_string(),
                _ => String::new(),
            };
            Some((id, name))
        })
        .collect()
}

fn disks(p: &Pdh, c: &Counters, names: &HashMap<u32, String>, temps: &HashMap<u32, f64>, out: &mut Out) {
    let writes = p.read(c.disk_write);
    let idle = p.read(c.disk_idle);
    let mut reads = p.read(c.disk_read);
    reads.sort_by(|a, b| a.0.cmp(&b.0));
    // Instances look like "1 C:" (disk number, then its drive letters).
    for (instance, read) in reads.iter().filter(|(n, _)| n != "_Total") {
        let Some(disk) = instance.split_whitespace().next().and_then(|d| d.parse::<u32>().ok()) else { continue };
        let letters = instance.split_whitespace().skip(1).collect::<Vec<_>>().join(" ").to_uppercase();
        let model = names.get(&disk).cloned().unwrap_or_default();
        let group = match (model.is_empty(), letters.is_empty()) {
            (false, false) => format!("{model} ({letters})"),
            (false, true) => model,
            _ => format!("Disk {disk} ({letters})"),
        };
        out.push(format!("disk{disk}.read"), &group, "Read speed", *read, "B/s", "rate");
        if let Some(w) = find(&writes, instance) {
            out.push(format!("disk{disk}.write"), &group, "Write speed", w, "B/s", "rate");
        }
        if let Some(i) = find(&idle, instance) {
            out.push(format!("disk{disk}.active"), &group, "Activity", (100.0 - i).clamp(0.0, 100.0), "%", "usage");
        }
        if let Some(t) = temps.get(&disk) {
            out.push(format!("disk{disk}.temp"), &group, "Temperature", *t, "°C", "temp");
        }
    }
}

fn network(p: &Pdh, c: &Counters, active: &mut HashSet<String>, out: &mut Out) {
    const SKIP: [&str; 6] = ["isatap", "teredo", "loopback", "6to4", "wan miniport", "kernel debug"];
    let sent = p.read(c.net_out);
    let mut received = p.read(c.net_in);
    received.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, down) in received {
        let lower = name.to_lowercase();
        if SKIP.iter().any(|s| lower.contains(s)) {
            continue;
        }
        let up = find(&sent, &name).unwrap_or(0.0);
        // Only adapters that have carried traffic, so idle virtual ones stay hidden.
        if down + up > 0.0 {
            active.insert(name.clone());
        }
        if !active.contains(&name) {
            continue;
        }
        let id: String = lower.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        out.push(format!("net.{id}.down"), &name, "Download", down, "B/s", "rate");
        out.push(format!("net.{id}.up"), &name, "Upload", up, "B/s", "rate");
    }
}

fn memory(out: &mut Out) {
    let m = winapi::memory_status();
    let used = m.total_phys.saturating_sub(m.avail_phys) as f64;
    if m.total_phys > 0 {
        out.push("mem.usage", "Memory", "Usage", 100.0 * used / m.total_phys as f64, "%", "usage");
    }
    out.push("mem.used", "Memory", "Used", used, "B", "bytes");
    out.push("mem.free", "Memory", "Available", m.avail_phys as f64, "B", "bytes");
    let committed = m.total_page_file.saturating_sub(m.avail_page_file) as f64;
    out.push("mem.commit", "Memory", "Committed", committed, "B", "bytes");
    out.push("sys.uptime", "System", "Uptime", winapi::uptime_seconds() as f64, "s", "time");
}

fn gpus(nvml: &Nvml, out: &mut Out) {
    for i in 0..nvml.device_count().unwrap_or(0) {
        let Ok(d) = nvml.device_by_index(i) else { continue };
        let group = d.name().unwrap_or_else(|_| format!("GPU {i}"));
        let id = format!("gpu{i}");
        if let Ok(t) = d.temperature(TemperatureSensor::Gpu) {
            out.push(format!("{id}.temp"), &group, "Temperature", t as f64, "°C", "temp");
        }
        if let Ok(u) = d.utilization_rates() {
            out.push(format!("{id}.usage"), &group, "Usage", u.gpu as f64, "%", "usage");
            out.push(format!("{id}.memload"), &group, "Memory controller", u.memory as f64, "%", "usage");
        }
        if let Ok(v) = d.clock_info(Clock::Graphics) {
            out.push(format!("{id}.clock"), &group, "Core clock", v as f64, "MHz", "clock");
        }
        if let Ok(v) = d.clock_info(Clock::Memory) {
            out.push(format!("{id}.memclock"), &group, "Memory clock", v as f64, "MHz", "clock");
        }
        if let Ok(v) = d.power_usage() {
            out.push(format!("{id}.power"), &group, "Power", v as f64 / 1000.0, "W", "power");
        }
        if let Ok(v) = d.fan_speed(0) {
            out.push(format!("{id}.fan"), &group, "Fan", v as f64, "%", "usage");
        }
        if let Ok(m) = d.memory_info() {
            out.push(format!("{id}.vram"), &group, "Memory used", m.used as f64, "B", "bytes");
        }
    }
}

fn float(v: Option<&Variant>) -> Option<f64> {
    match v? {
        Variant::R4(x) => Some(*x as f64),
        Variant::R8(x) => Some(*x),
        Variant::I4(x) => Some(*x as f64),
        Variant::UI4(x) => Some(*x as f64),
        _ => None,
    }
}

fn string(v: Option<&Variant>) -> String {
    match v {
        Some(Variant::String(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Reads LibreHardwareMonitor's WMI sensors. Returns false once it stops
/// answering (closed), so the caller reconnects later.
fn librehardwaremonitor(con: &WMIConnection, out: &mut Out) -> bool {
    let Ok(hardware) = con.raw_query::<HashMap<String, Variant>>("SELECT Identifier, Name FROM Hardware") else {
        return false;
    };
    let names: HashMap<String, String> = hardware
        .iter()
        .map(|h| (string(h.get("Identifier")), string(h.get("Name"))))
        .collect();
    let Ok(sensors) =
        con.raw_query::<HashMap<String, Variant>>("SELECT Identifier, Name, SensorType, Value, Parent FROM Sensor")
    else {
        return false;
    };
    for s in &sensors {
        let Some(value) = float(s.get("Value")) else { continue };
        let (unit, kind, scale) = match string(s.get("SensorType")).as_str() {
            "Temperature" => ("°C", "temp", 1.0),
            "Load" | "Control" | "Level" => ("%", "usage", 1.0),
            "Clock" => ("MHz", "clock", 1.0),
            "Voltage" => ("V", "voltage", 1.0),
            "Fan" => ("RPM", "fan", 1.0),
            "Power" => ("W", "power", 1.0),
            "Data" => ("B", "bytes", 1024.0 * 1024.0 * 1024.0),
            "SmallData" => ("B", "bytes", 1024.0 * 1024.0),
            "Throughput" => ("B/s", "rate", 1.0),
            _ => continue,
        };
        let parent = string(s.get("Parent"));
        let hardware = names.get(&parent).cloned().unwrap_or(parent);
        out.push(
            format!("lhm{}", string(s.get("Identifier"))),
            &format!("{hardware} · LHM"),
            string(s.get("Name")),
            value * scale,
            unit,
            kind,
        );
    }
    true
}
