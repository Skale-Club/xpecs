# Development and releases — Xpecs

This guide is for maintainers building, checking and publishing Xpecs for Windows. For everyday use,
return to the [README](../README.md).

## Build locally

Use Windows with Microsoft Edge WebView2 Runtime, Node.js/npm, Rust's MSVC toolchain and Microsoft
C++ Build Tools with the Windows SDK. Install the **Desktop development with C++** workload. The
repository's release workflow currently selects Node.js 20 and stable Rust. See the [Tauri Windows
prerequisites](https://v2.tauri.app/start/prerequisites/#windows) for setup details.

From a directory where you keep source repositories:

~~~powershell
git clone https://github.com/Skale-Club/xpecs.git
cd xpecs
npm ci
npm run dev
~~~

Private repositories require an authenticated GitHub account with access. The frontend is plain
HTML, CSS and JavaScript served by Tauri 2; opening the HTML directly in a browser does not provide
the native commands.

To check Rust code, run the included tests and build the executable:

~~~powershell
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --release --locked
npm run build
~~~

The build produces `src-tauri/target/release/xpecs.exe`. It does not create an installer or a
PortableApps package. Copy the executable to a writable folder for manual testing. Its `Data` folder
belongs beside that executable, including when running a development build.

## Architecture

Xpecs is one window with a page per area. Each area has its own Rust module, with its commands and
state, and its own page file; `main.rs` keeps the settings, updates and the list of commands.

| Area | Rust | Page files |
|---|---|---|
| PC care: overview, cleaner, startup, apps, optimize | `care.rs` (commands), `cleaner.rs`, `startup.rs`, `apps.rs`, `icons.rs`, `power.rs`, `win.rs` | `care.js`, `care-i18n.js`, `care.css` |
| Force Delete | `force_delete.rs` | `force-delete.js`, `force-delete.css` |
| Disk space | `space.rs` (commands), `scan.rs`, `find.rs`, `space_win.rs` | `space.js`, `space-i18n.js`, `space.css` |
| Drivers | `drivers.rs` | `drivers.js` (texts inside), `drivers.css` |
| Hardware, sensors, benchmarks | `info.rs`, `sensors.rs`, `bench.rs`, `winapi.rs` | `hardware.js`, `brands.js` |
| Window, menu, texts | `main.rs`, `portable.rs`, `updater.rs` | `app.js`, `common.js`, `i18n.js`, `style.css`, `skale.css` |

`common.js` holds what pages share (Tauri calls, formatting, the toast, the confirm box, icons) and
the page registry `PAGES`: each page file adds `{ render, actions, show, nav, cls }` entries, and
`app.js` draws the menu groups, the header and the chosen page. Page files load in the order
`index.html` lists them. `addTexts()` merges each area's texts into `I18N`; Disk space keeps its
keys under the `sp_` prefix, and its stylesheet only applies under `#page.space`.

PC care: Rust handles scan snapshots, deletion boundaries, registry startup flags, installed-program
entries and Windows maintenance commands. JavaScript renders scan progress, user confirmations and
the heuristic health score. Keep scan and deletion separate; do not turn viewing a page into a
destructive action.

Disk space: Rust walks the chosen folder in parallel (rayon) and keeps every folder, with its size
and counts, in memory; files are only counted, plus the largest files and per-extension totals. The
page asks for one folder at a time (`open_folder`), and that folder's files are read from disk when
it is opened. Junctions and directory symbolic links are listed but never followed. A scan emits
`progress` events while it runs and can be cancelled. `find.rs` answers the search box
(`find_folders`): a typed path is completed from its parent folder on disk; anything else searches
an index of folder names, built on a background thread the first time someone searches, walking the
profile and the local drives together breadth first (network drives are left out; at most 250,000
folders, 7 levels or 45 seconds). `known_folders` reads the user's folders with
`SHGetKnownFolderPath`; `pick_scan_folder` is `IFileOpenDialog` in folder mode. The last eight
scanned folders are kept in `recent` in the settings. `recycle` sends a selection to the Recycle Bin
with `SHFileOperationW`; permanent deletion goes through the same `force_delete.rs` as Force Delete
(one copy now), and only accepts items inside the scanned folder. Afterwards `Tree::forget` takes
what is gone out of the totals, so the page does not need a rescan.

Drivers: `drivers.rs` reads `Win32_PnPSignedDriver` and the devices with a
`ConfigManagerErrorCode` through WMI (a few seconds; the page loads it in the background at start so
the overview can count problems). Windows Update is reached through PowerShell and the
`Microsoft.Update.Session` COM object: the scripts use only single quotes, so each travels as one
`-Command` argument with no execution-policy change. Install takes update ids (GUIDs only reach the
script), creates a restore point with `Checkpoint-Computer` first unless the user accepted going
without one, downloads and installs, and skips updates that need a license accepted. Backup runs
`pnputil /export-driver *`. Installing and backup need administrator rights.

Hardware: Rust gathers information through WMI, CPUID and Windows APIs, with NVML for supported
NVIDIA readings. The sensor loop starts the first time a hardware page is opened (`start_sensors`)
and emits live values once a second. Benchmarks run separately from the UI; the page renders the
graphs and the HTML report.

All the Skale X apps use the same palette: navy surfaces, primary blue `#5173D6` and soft blue
`#8FA9EE`. Keep English, Portuguese and Spanish UI strings consistent. Settings and application data
use the portable data directory; do not commit runtime data, API credentials or private signing
keys.

## Local MCP server

`Xpecs.exe --mcp` makes `main()` call `mcp_tools::serve()` before Tauri starts, so no window opens.
`mcp.rs` is the MCP server over stdio (one JSON-RPC 2.0 message per line: `initialize`, `ping`,
`tools/list`, `tools/call`, plus the shared `app_info` tool). It is the same file in every Skale
app: change it in one and copy it to the others. `mcp_tools.rs` lists this app's tools and calls the
same modules the window uses, grouped by area (PC care, disk space, drivers, hardware). Keep stdout
for protocol messages only (log with `eprintln!`), and return `Err` from a tool instead of
panicking: release builds abort on panic. Release builds use the GUI subsystem and still talk over
the pipes an MCP client opens.

To check by hand, write these lines to the exe's stdin; each request gets one JSON line back:

```text
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"app_info","arguments":{}}}
```

Any MCP client can drive it the same way, for example the MCP Inspector:
`npx @modelcontextprotocol/inspector "<path>\Xpecs.exe" --mcp`.
`cargo test mcp` runs the protocol tests.

## Validate a change

Run the checks above when changing application code. Unit tests do not validate real Windows
devices, capture software or external APIs. For documentation-only changes, compare claims with the
implementation and check links and commands.

- Go through every page in the three languages, as a standard user and as administrator.
- PC care: review the overview, scan categories and cancellation/confirmation flows. Test deletion
  only with disposable files in a controlled Windows environment. Confirm recent temporary files,
  reparse points and files outside category roots are protected. Verify a disposable startup entry
  can be disabled and re-enabled. Check that uninstall and maintenance buttons open the intended
  Windows tools; do not test them on valuable installed software.
- Disk space: scan a whole drive and compare totals with Explorer's folder properties (expect small
  differences where folders can't be read). Scan a folder containing a junction and check it is
  listed as a link and not counted. Cancel a scan midway, then rescan; open folders, the treemap and
  both lists; export a CSV. In the search box: type `C:\Us`, use Tab to go down a few levels and
  Enter to scan; search a name and watch results fill in; try `%LOCALAPPDATA%`; scan from a known
  folder and a recent one, remove a recent one, and drop a folder from Explorer on the window.
  Delete a file and a folder to the Recycle Bin and restore them; delete a file that an app has open
  permanently and with "close the apps"; try a protected folder (Documents itself) and check it is
  refused; check the totals after each. `cargo test -- --ignored recycles` sends a small temporary
  file to the real Recycle Bin.
- Drivers: compare the problem list with Device Manager. Search Windows Update and compare with
  Settings › Windows Update › Optional updates. On a test machine, install one update as
  administrator and check the restore point in System Restore; turn System Restore off and check the
  "install without a restore point" question. Back up drivers and look at the folder.
- Hardware: compare the summary with Windows' reported hardware and verify missing values stay
  understandable. Check live sensors and export an HTML report; inspect the saved report. Run an
  appropriate CPU and memory benchmark on a test machine. Start and stop stress testing, then check
  that closing the app also stops it. Check the behavior with and without LibreHardwareMonitor and
  supported NVIDIA hardware.

To look at the window without the native side, copy `src` somewhere, add a script that defines
`window.__TAURI__` with fake answers before the others in `index.html`, and serve the folder.

Keep documentation aligned with observed behavior. Update the README's English instructions and
Portuguese/Spanish summaries together. A feature listed in the README should have an implemented
workflow; a release link should not imply that a binary exists before publication succeeds.

See [Force Delete implementation and validation](FORCE_DELETE.md) for its path/process boundaries,
fixture tests, delayed-registration semantics and remaining manual acceptance checks.

## Publish a release

The **Release** GitHub Actions workflow runs when a `v*` tag is pushed. A normal branch push does
not build or publish an executable. Publication needs repository write access, GitHub Actions
enabled and an Actions secret named `TAURI_SIGNING_PRIVATE_KEY` available to this repository. An
organization secret must grant this repository access.

The secret must contain the private signing key matching the app's embedded public update key. Keep
the key outside Git and back it up securely. The current workflow supplies an empty signing-key
password; using a password-protected key requires updating the workflow's secret configuration
first.

Before tagging a release, confirm that the repository has a signing key available through a
repository or permitted organization secret. A normal source push does not publish a binary.

For a reviewed release:

1. Start from a clean checkout of `main`, fetch the remote and confirm `origin`    points to
   `https://github.com/Skale-Club/xpecs.git`.
2. Choose an unused semantic version. Set the same version in the npm package,    Rust package and
   Tauri configuration. Refresh the npm lockfile with    `npm install --package-lock-only` and the
   Rust lockfile with    `cargo check --manifest-path src-tauri/Cargo.toml`. Review those changes.
3. Run the tests and build above, then perform the relevant manual checks.
4. Commit the version changes and lockfiles. Create the corresponding `vMAJOR.MINOR.PATCH`    tag on
   that commit, then push `main` and that exact tag to `origin`.
5. Watch the [Actions run](https://github.com/Skale-Club/xpecs/actions).    The tag must match the
   Rust package version. The workflow builds the app,    signs it and publishes `Xpecs.exe` plus
   `Xpecs.exe.sig` in    [Releases](https://github.com/Skale-Club/xpecs/releases).
6. Download and smoke-test the release. Confirm both assets are present and    readable by the
   intended users before announcing a download or update.    Replace the dated pre-release notice in
   the README after the first release.

The existing `scripts/release.ps1` helper changes the three version manifests, commits, tags and
pushes immediately. It does not refresh lockfiles, run tests or check that signing secrets exist.
Use the reviewed process above when preparing lockfiles or validating a first release.

The updater queries GitHub without authentication, finds a newer version, verifies its signature and
replaces the executable after the user selects the update. It retains `Data`. Release assets must be
publicly readable for this updater to work; GitHub login in a browser does not authenticate the app.

The update signature is separate from Windows Authenticode signing; this workflow does not configure
an Authenticode certificate.

## Portable distribution

The app creates `Data` next to its executable. Optional PortableApps-style metadata can be supplied
under `App/AppInfo`; the app only refreshes the version when that metadata already exists. Neither
the build nor the release workflow creates a complete PortableApps package.

Close the app before moving or backing up its folder.

## Verify the Windows executable icon

After a normal `npm run build`, the postbuild step checks the icon resources
embedded in the resulting executable against `src-tauri/icons/icon.ico`. You can
repeat the data-only check with `npm run verify:icon`. This uses Node.js, does not
launch the app and fails if any icon frame is stale or missing. Cargo tracks icon
inputs through `build.rs`, so updating artwork rebuilds the Windows resource library.
See [branding](BRANDING.md) for the icon source and regeneration instructions.
