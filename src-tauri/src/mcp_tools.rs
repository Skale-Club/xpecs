//! What Xpecs offers over MCP (`Xpecs.exe --mcp`): the PC's state and what
//! cleaning would free, disk space, drivers, and the hardware with its
//! sensors and benchmarks. Only reading: cleaning, startup changes,
//! uninstalling, deleting and installing drivers stay in the window, where
//! the user reviews them.

use crate::mcp::{self, Tool};
use crate::{apps, bench, care, cleaner, drivers, info, power, scan, sensors, space_win, startup, win};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const INSTRUCTIONS: &str = "Xpecs looks after this Windows PC and reports its hardware. PC care: status \
(system drive space, uptime, memory load, Recycle Bin, unhealthy drives), junk_files (what each cleaning \
category would free), startup_items, installed_apps and power_plans. Disk space (like TreeSize): \
list_drives, scan_folder, then list_folder, largest_files and file_types read the kept scan. Drivers: \
drivers (every device's driver and the devices with a problem) and driver_updates (what Windows Update \
offers, 10 to 60 seconds). Hardware (like AIDA64): system_info, sensors and benchmark. Sizes are bytes \
and times Unix seconds. Nothing is cleaned, deleted, installed or changed from here: tell the user what \
to do in Xpecs.";

/// Runs WMI and COM work on a thread of its own, with its own COM setup.
fn apart<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    std::thread::spawn(work).join().map_err(|_| "Stopped unexpectedly.".to_string())
}

// ---------- PC care ----------

fn care_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "status",
            "The system drive's free and total bytes, uptime in seconds, memory load in percent, \
             Recycle Bin bytes, unhealthy drives, and whether this runs as administrator.",
            mcp::no_input(),
            |_| Ok(json!(care::read_status(apart(care::unhealthy_disks)?))),
        ),
        Tool::new(
            "junk_files",
            "Scans the cleaning categories (temporary files, caches, logs, crash dumps, Recycle Bin...) \
             and returns, per category id, the bytes and files it would free. Deletes nothing.",
            mcp::no_input(),
            |_| Ok(json!(cleaner::scan(win::is_admin(), &mut |_| {}).0)),
        ),
        Tool::new(
            "startup_items",
            "Programs that start with Windows: name, command, where it is registered, enabled, and \
             whether changing it needs administrator rights.",
            mcp::no_input(),
            |_| Ok(json!(startup::list())),
        ),
        Tool::new(
            "installed_apps",
            "Installed programs: name, publisher, version, estimated size in bytes and install date.",
            mcp::no_input(),
            |_| Ok(json!(apps::list())),
        ),
        Tool::new("power_plans", "Windows power plans and which one is active.", mcp::no_input(), |_| Ok(json!(power::list()))),
    ]
}

// ---------- disk space ----------

type Shared = Rc<RefCell<Option<scan::Tree>>>;

fn scan_path(tree: &Shared, path: &Path) -> Result<Value, String> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(format!("{} is not a folder.", path.display()));
    }
    let result = scan::scan(path, &scan::Progress::default());
    let summary = serde_json::to_value(&result.summary).unwrap_or_default();
    *tree.borrow_mut() = Some(result);
    Ok(summary)
}

/// The listing of `path` from the kept scan, scanning `path` first when the
/// scan doesn't cover it.
fn listing(tree: &Shared, path: &Path, limit: usize) -> Result<Value, String> {
    let covered = tree.borrow().as_ref().is_some_and(|t| t.find(path).is_some());
    if !covered {
        scan_path(tree, path)?;
    }
    let tree = tree.borrow();
    let tree = tree.as_ref().ok_or("No scan.")?;
    let id = tree.find(path).ok_or_else(|| format!("{} was not found in the scan.", path.display()))?;
    let mut value = serde_json::to_value(tree.listing(id)).unwrap_or_default();
    mcp::trim_list(&mut value, "folders", limit);
    mcp::trim_list(&mut value, "files", limit);
    Ok(value)
}

fn with_tree(tree: &Shared, read: impl FnOnce(&scan::Tree) -> Value) -> Result<Value, String> {
    tree.borrow().as_ref().map(read).ok_or_else(|| "Nothing scanned yet: call scan_folder first.".into())
}

fn path_input(what: &str) -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": what },
            "limit": { "type": "integer", "description": "Most folders and files to list (default 50)." },
        },
        "required": ["path"],
    })
}

fn limit_input() -> Value {
    json!({
        "type": "object",
        "properties": { "limit": { "type": "integer", "description": "Most items to return (default 50)." } },
    })
}

fn space_tools() -> Vec<Tool> {
    let tree: Shared = Rc::new(RefCell::new(None));
    vec![
        Tool::new(
            "list_drives",
            "Drives with media in them: letter, label, file system, kind, total and free bytes.",
            mcp::no_input(),
            |_| Ok(json!(space_win::drives())),
        ),
        Tool::new(
            "scan_folder",
            "Scans a folder or drive (e.g. C:\\) and keeps the result for the other disk space tools. \
             Returns the totals (size, files, folders, unreadable folders, how long it took) and the \
             largest subfolders. A whole drive can take a few minutes.",
            path_input("Absolute folder or drive path."),
            {
                let tree = tree.clone();
                move |args| {
                    let mut summary = scan_path(&tree, &PathBuf::from(mcp::text(args, "path")?))?;
                    let root = tree.borrow().as_ref().map(|t| t.root().to_path_buf()).unwrap_or_default();
                    summary["top"] = listing(&tree, &root, mcp::number(args, "limit", 20) as usize)?["folders"].take();
                    Ok(summary)
                }
            },
        ),
        Tool::new(
            "list_folder",
            "A folder's subfolders (largest first, with size, file and folder counts) and its files. \
             Uses the kept scan when it covers the folder, otherwise scans the folder first.",
            path_input("Absolute folder path."),
            {
                let tree = tree.clone();
                move |args| listing(&tree, &PathBuf::from(mcp::text(args, "path")?), mcp::number(args, "limit", 50) as usize)
            },
        ),
        Tool::new(
            "largest_files",
            "The largest files in the kept scan, largest first, with full paths.",
            limit_input(),
            {
                let tree = tree.clone();
                move |args| {
                    let limit = mcp::number(args, "limit", 50) as usize;
                    with_tree(&tree, |t| json!(t.top_files().iter().take(limit).collect::<Vec<_>>()))
                }
            },
        ),
        Tool::new("file_types", "Space taken per file extension in the kept scan, largest first.", limit_input(), move |args| {
            let limit = mcp::number(args, "limit", 50) as usize;
            with_tree(&tree, |t| json!(t.types().iter().take(limit).collect::<Vec<_>>()))
        }),
    ]
}

// ---------- drivers ----------

fn driver_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "drivers",
            "Every device's driver (device, class, provider, version, date, INF, signed, and whether it \
             is a Windows inbox driver) and the devices Windows reports a problem with (with Device \
             Manager's problem code: 28 = no driver, 22 = disabled, 43 = stopped after a problem...). \
             Set all to include inbox drivers; by default only manufacturers' drivers are listed.",
            json!({
                "type": "object",
                "properties": { "all": { "type": "boolean", "description": "Include Windows' own inbox drivers." } },
            }),
            |args| {
                let all = mcp::flag(args, "all");
                let mut value = json!(apart(drivers::inventory)??);
                if !all {
                    if let Some(list) = value["drivers"].as_array_mut() {
                        list.retain(|d| d["inbox"] == false);
                    }
                }
                Ok(value)
            },
        ),
        Tool::new(
            "driver_updates",
            "Driver updates Windows Update offers for this PC (the signed drivers of Optional updates): \
             title, device model, provider, class, date and download size. Takes 10 to 60 seconds. \
             Installing them is done in Xpecs, which creates a restore point first.",
            mcp::no_input(),
            |_| Ok(json!(drivers::search()?)),
        ),
    ]
}

// ---------- hardware ----------

const TESTS: [(&str, &str, &str); 8] = [
    ("mem-read", "GB/s", "Memory read bandwidth."),
    ("mem-write", "GB/s", "Memory write bandwidth."),
    ("mem-copy", "GB/s", "Memory copy bandwidth."),
    ("mem-latency", "ns", "Random-access memory latency (lower is better)."),
    ("cpu-int-1", "M rounds/s", "Integer throughput, one thread."),
    ("cpu-int-n", "M rounds/s", "Integer throughput, all threads."),
    ("cpu-fp-1", "M iterations/s", "Floating point throughput, one thread."),
    ("cpu-fp-n", "M iterations/s", "Floating point throughput, all threads."),
];

fn hardware_tools() -> Vec<Tool> {
    // Collecting takes a few seconds, and the hardware doesn't change while we run.
    let collected: RefCell<Option<Value>> = RefCell::new(None);
    vec![
        Tool::new(
            "system_info",
            "This PC's hardware and system. Without a section: the summary of every part. With a \
             section: its detailed groups of label/value rows.",
            json!({
                "type": "object",
                "properties": {
                    "section": {
                        "type": "string",
                        "enum": ["cpu", "board", "memory", "gpu", "storage", "network", "os"],
                        "description": "One section in detail.",
                    },
                },
            }),
            move |args| {
                if collected.borrow().is_none() {
                    *collected.borrow_mut() = Some(json!(apart(info::collect)?));
                }
                let collected = collected.borrow();
                let all = collected.as_ref().ok_or("No information.")?;
                match mcp::optional_text(args, "section") {
                    None => Ok(all["summary"].clone()),
                    Some(id) => all["sections"]
                        .as_array()
                        .and_then(|list| list.iter().find(|s| s["id"] == id.as_str()))
                        .map(|s| s["groups"].clone())
                        .ok_or_else(|| format!("No section {id}.")),
                }
            },
        ),
        Tool::new(
            "sensors",
            "One set of live readings, taken over about a second: id, group, name, value, unit and kind \
             (load, clock, temperature, power, fan, voltage, throughput...).",
            mcp::no_input(),
            |_| Ok(json!(apart(sensors::sample)?)),
        ),
        Tool::new(
            "benchmark",
            "Runs one benchmark (a few seconds, best of several passes) and returns its score and unit. \
             It loads the CPU or memory while it runs.",
            json!({
                "type": "object",
                "properties": {
                    "test": {
                        "type": "string",
                        "enum": TESTS.map(|t| t.0),
                        "description": TESTS.map(|t| format!("{}: {}", t.0, t.2)).join(" "),
                    },
                },
                "required": ["test"],
            }),
            |args| {
                let test = mcp::text(args, "test")?;
                let unit = TESTS.iter().find(|t| t.0 == test).map(|t| t.1).ok_or_else(|| format!("Unknown test {test}."))?;
                let score = bench::run(&test)?;
                Ok(json!({ "test": test, "score": (score * 100.0).round() / 100.0, "unit": unit }))
            },
        ),
    ]
}

pub fn serve() {
    let tools = [care_tools(), space_tools(), driver_tools(), hardware_tools()].into_iter().flatten().collect();
    mcp::serve("Xpecs", "xpecs.json", INSTRUCTIONS, tools);
}
