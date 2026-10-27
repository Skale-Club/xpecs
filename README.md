# Xpecs

Portable PC care and system information for Windows: health check, junk cleaning, disk space,
drivers, startup programs and apps, in the spirit of CCleaner, TreeSize and Driver Booster, plus
hardware information, live sensors and benchmarks in the spirit of AIDA64. One small exe: no
installer and no kernel driver; administrator rights only for the actions that need them. 🇧🇷
[Português](#português) · 🇪🇸 [Español](#español)

Made by **[Skale Club](https://skale.club)**. Xpecs now includes what used to be
[Xcare](https://github.com/Skale-Club/xcare) and [Xpace](https://github.com/Skale-Club/xpace).

## Availability

**Status checked October 27, 2026:** source version 0.1.1. No GitHub Release or
publicly downloadable executable has been published. Build from source using the
[development guide](docs/DEVELOPMENT.md). Future release binaries belong in
[Releases](https://github.com/Skale-Club/xpecs/releases).

The app can offer signed updates when a newer release contains both an executable and its signature.
Updates require internet access and are installed when you select the update action; your `Data`
folder is retained.

## Run it

Double-click `Xpecs.exe`. It requires Microsoft Edge WebView2 Runtime. Settings (the language and
recent disk scans) live in `Data\xpecs.json` next to the exe; reports go to `Data\reports`, CSV
exports to `Data\exports` and driver backups to `Data\driver-backup`. Carry the folder anywhere.
Keep the executable in a writable folder. The app creates `Data` beside it. PortableApps-style
metadata is optional; the build and release workflow do not create a PortableApps package.

Language: the selector at the bottom left (English, Português, Español). The badge at the top right
says whether Xpecs runs as administrator; pages with actions that need it offer **Restart as
administrator**.

## Pages

| Group | Page | What it does |
|---|---|---|
| PC care | Overview | Checks the PC on open and gives a 0–100 score: junk files, startup programs, free space, time since restart, power plan, drive health, memory, devices with driver problems. Action buttons are shown where a supported action exists. |
| | Cleaner | Scans and shows the size of each category, then deletes only what you ticked, after a confirmation. |
| | Disk space | Which folders and files take the space on a drive: search box, your folders, recent scans and drives; folder tree with a treemap, largest files, file types; Recycle Bin or reviewed permanent deletion. |
| | Startup | Turns programs that start with Windows on and off. |
| | Apps | Programs found in supported uninstall registry entries, with size/date when provided; opens the program's own uninstaller. |
| | Drivers | Devices with problems, driver updates from Windows Update, the graphics driver with a link to its maker's app, every installed driver, and a backup of all of them. |
| | Optimize | Power plan (including adding Ultimate Performance) and shortcuts to Windows' own tools: repair system files (DISM + SFC), restore point, optimize drives, visual effects, Game Mode, Storage Sense, Windows Update, Task Manager, Disk Cleanup, Device Manager. |
| Hardware | Hardware | Summary and one tab per part: processor, motherboard, memory, graphics, storage, network, operating system. |
| | Sensors | Available live readings with current, min, max and a 60-second graph. |
| | Benchmarks | Memory read, write, copy and latency; CPU integer and floating point, single thread and all threads; stress test. |
| Tools | Force Delete | Reviews explicitly selected local files/folders, identifies blocking apps, deletes reviewed items, optionally terminates eligible apps or schedules removal at restart. |

## Cleaning behavior

- Categories cover caches, temporary files and diagnostic leftovers: temp folders, thumbnail cache,
  error reports, Windows Update downloads, Delivery Optimization, memory dumps and browser caches
  (Chrome, Edge, Brave, Opera, Firefox). Browser categories target cache directories, not password,
  cookie or history databases.
- Temp folders keep anything from the last 24 hours, to reduce interference with current work.
- File-based categories delete the scanned files within their allowed roots, skipping
  links/junctions and files they cannot delete. Close relevant apps before scanning so caches are
  less likely to change or stay locked.
- Shader caches and the Recycle Bin are off by default. Rebuilding shaders may delay a game's next
  launch. The Recycle Bin action empties the bin through Windows at execution time, including items
  added after the scan; it is not a per-file scan snapshot. Deletion is permanent.
- Clearing browser caches can remove offline cached website resources. The next visit may need to
  download them again.

## Disk space

To pick what to scan, type or paste a path in the search box (it completes folder names like
Explorer's address bar; `~` and `%VARIABLES%` work), or type part of a folder's name to find it
anywhere on your local drives. ↑ ↓ choose, **Enter** scans, **Tab** goes into the folder, and
**Ctrl+L** brings the box back from any page. **Browse…** opens Windows' folder picker, and you can
also click one of your folders (Desktop, Documents, Downloads…), a recent scan, a drive, or drop a
folder from Explorer on the window. The first name search builds a list of folder names in the
background (up to 250,000 folders or 45 seconds); results appear right away and fill in as it goes.

The tabs move between **Scan**, **Folders**, **Largest files** (the 250 largest of the scan) and
**File types**. Click a folder (in the tree or in the treemap) to open it; the trail above the
treemap leads back up. Double-click a row, or use its arrow button, to show it in Explorer.
**Export CSV** saves the current list. **Rescan** reads the same place again.

How it counts:

- Sizes are file sizes, as Explorer's "Size" shows them, not the space allocated on disk.
  Compressed files and OneDrive files that are online-only count at their full size.
- Junctions and symbolic links to folders are listed but not followed, so nothing is counted twice
  and a link can't take the scan outside the chosen folder. OneDrive and other cloud folders are
  scanned.
- Folders Windows doesn't let a normal user read (for example `System Volume Information`) are
  marked and count as empty. Running Xpecs as administrator reads more of them.
- Hard links count once per name, as in Explorer.

Scanning only reads. Deleting happens only for what you select and confirm, in **Folders** and
**Largest files**:

- Tick the box on a row (or several), or use a row's trash button. A bar shows how many items and
  how much space are selected.
- **Move to Recycle Bin** is the default and can be undone from the Bin. If something is too big
  for the Bin, Windows asks before deleting it for good.
- **Delete permanently…** first reviews the selection: files, folders, total size and which apps
  have the files open. Then you choose to delete, to **close those apps and delete** (only apps that
  are safe to close), or to **delete at the next restart** (needs administrator). This is the same
  reviewed engine as [Force Delete](#force-delete): every item is checked again just before it is
  deleted, and anything that changed since the review is kept.
- Protected: the scanned folder itself, drive roots, Windows, Program Files, ProgramData, your
  profile folders themselves (Desktop, Documents, Downloads… but not what is inside them), system
  files such as `pagefile.sys`, junctions and links, and hard-linked files. Only items inside the
  scanned folder can be deleted.
- After deleting, the tree, the treemap and the largest files update without a rescan, and the
  summary shows the space freed. File types only drop deleted files until the next scan.

## Drivers

Drivers come only from **Windows Update**: the same signed drivers Windows offers in *Optional
updates*. Xpecs never downloads drivers from other sites.

- **Devices with problems** lists what Device Manager marks with a warning, with the reason (no
  driver installed, disabled, stopped after a problem…). Disabled and disconnected devices are
  listed but don't lower the Overview score.
- **Search for updates** asks Windows Update (10 to 60 seconds, no administrator needed). Tick the
  updates and **Install**: Xpecs, running as administrator, first creates a restore point, then
  downloads and installs them and shows the result of each. If Windows can't create a restore point
  (System Restore turned off, for example), it asks before installing without one. If Windows made
  one in the last day, that one covers the install. Devices may flicker or reconnect, and Windows
  may ask to restart. Updates that need a license accepted are left to Windows Update.
- **Graphics driver** shows the installed version. Graphics drivers reach Windows Update late, so
  there is a button for the maker's own app (NVIDIA, AMD or Intel).
- **Installed drivers** lists manufacturers' drivers by class, or all of them including Windows'
  own, with provider, version and date, and a search box. An old date doesn't mean a driver is
  outdated: many are final versions.
- **Back up drivers** (administrator) copies every manufacturer driver with Windows' `pnputil` to
  `Data\driver-backup\<date>`, to put back after reinstalling Windows (Device Manager › Update
  driver › Browse my computer).

## Force Delete

Choose files, choose a folder, drop local files/folders into the page, or paste an absolute local
path. Select **Review selection** before choosing an action. The tool supports up to 100 selections
and 5,000 total items per review.

- **Delete now** permanently removes reviewed items, bypassing the Recycle Bin. Read-only attributes
  can be cleared; ownership and access-control permissions are not rewritten.
- **Terminate apps & delete** explicitly confirms the blocking apps reported by Windows Restart
  Manager. Eligible apps in the current user session can be terminated; unsaved work may be lost.
  Windows processes, services, Explorer, critical processes and Xpecs itself are protected. Apps
  are not automatically reopened. Restart Manager does not detect every possible lock.
- **Delete on restart** requires administrator access and registers paths with Windows. It does not
  restart the PC. Registration persists after Xpecs closes and does not guarantee removal. Windows
  acts on paths at restart, so a replacement at the same path may also be affected. Folder entries
  are registered after their children and removed only if empty at restart.

Reviews expire after 10 minutes and are consumed by an execution attempt. Immediate deletion checks
file identity and changes since review; new files are not added automatically. Partial results show
deleted, scheduled and failed counts separately. Refresh the review before retrying.

The tool refuses drive roots, network/device paths, symbolic links/junctions, Windows and installed
program trees, and main profile containers. It is intended for stubborn user files and disposable
folders. It does not bypass every Windows restriction. See [implementation and validation
notes](docs/FORCE_DELETE.md).

## Startup switch

Turning a program off writes the same "StartupApproved" flag the Task Manager uses. The program's
own entry stays, so you can turn it back on anytime, here or in Task Manager. The list covers
supported Run registry entries and Startup folders; it does not enumerate every service or scheduled
task.

## What the health score means

The 0–100 score is a heuristic based on the checks in the overview. It is not a benchmark, antivirus
result or guarantee that hardware is healthy. Some findings need manual review rather than a
one-click fix. Power plans and cleanup can change system behavior; the app does not promise a
particular speed increase.

## Hardware

| Tab | Contents |
|---|---|
| Summary | Everything at a glance, plus live CPU, memory, GPU and drive readings |
| Processor | Name, cores (performance + efficient), threads, base clock, caches, instruction sets; live usage and clock per thread |
| Motherboard | Board, BIOS version and date, system model |
| Memory | Installed/usable, slots, and every module: size, type, speed, maker, part number, voltage |
| Graphics | GPU, VRAM, drivers, PCIe link, clocks and power limit (NVIDIA via NVML); monitors with model and size |
| Storage | Every drive (interface, type, capacity, firmware, health) with live temperature, read/write speed and activity; volumes with free space |
| Network | Adapters with status, link speed, MAC, IP addresses, gateway, DNS; live traffic |
| Operating system | Edition, version, build, install date, last boot, audio devices |

Click any value to copy it. **Export report** saves everything (including a sensor snapshot and
benchmark results) as one HTML file and opens it. Live sensors start the first time a hardware page
is opened.

### About temperatures

Without a kernel driver Xpecs cannot generally read CPU package temperature, fans or motherboard
voltages directly. Xpecs does not ship a kernel driver. What you get without a driver:

- GPU temperature, load, clocks, power and fan (NVIDIA)
- Drive temperatures when exposed by the device and Windows storage APIs
- The ACPI thermal zone (on many desktops a fixed board value, not the CPU)

For the rest, open **LibreHardwareMonitor** (free, portable): while it runs, Xpecs reads the
supported sensors it exposes through WMI and lists them on the Sensors page.

### Benchmarks

- Memory read/write: a 512 MiB buffer partitioned among workers. Copy uses separate 256 MiB source
  and destination buffers. Bandwidth is reported in GB/s. Latency uses a single random pointer chase
  through 256 MiB and is reported in nanoseconds. These are Xpecs tests, not directly comparable to
  another tool's scores.
- CPU: about a second of 64-bit integer hashing and FP64 Mandelbrot iterations, on one thread and on
  all threads. "Scaling" is all threads ÷ one thread.
- Stress test: loads every thread at 100% until you stop it, with live usage, clocks and
  temperatures beside it.

Close other heavy programs for steadier numbers. Results depend on workload, thermal conditions and
system activity. Sensors vary by device, driver and permissions; an unavailable reading is not a
zero value. LibreHardwareMonitor may need elevation to expose hardware sensors.

## Left out on purpose

Registry cleaning, forced RAM clearing, automatic network tuning and drivers from outside Windows
Update are not implemented. The Optimize page changes supported power settings or opens Windows
tools; availability and elevation requirements depend on the system.

## Use with Claude Code and Codex

`Xpecs.exe --mcp` runs Xpecs as a local [MCP](https://modelcontextprotocol.io) server, with no
window, so AI assistants such as Claude Code and Codex can use its engine on this PC. Xpecs itself
sends nothing over the network; what the assistant reads goes to its AI provider, as with anything
else you share with it. It only reads: cleaning, startup changes, uninstalling, deleting and
installing drivers stay in the window. `benchmark` loads the CPU or memory for a few seconds while
it runs. Run the assistant as administrator to measure the cleaning categories that need it.

Claude Code (available in every project):

```bash
claude mcp add --scope user xpecs -- "C:\Apps\Xpecs\Xpecs.exe" --mcp
```

Codex, in `%USERPROFILE%\.codex\config.toml`:

```toml
[mcp_servers.xpecs]
command = 'C:\Apps\Xpecs\Xpecs.exe'
args = ["--mcp"]
```

Use the path where your `Xpecs.exe` is. Tools:

- `app_info`: version, where the exe and `Data` folder are, and the saved settings.
- PC care: `status` (system drive space, uptime, memory load, Recycle Bin size, unhealthy drives,
  administrator or not), `junk_files` (what each cleaning category would free; deletes nothing),
  `startup_items`, `installed_apps`, `power_plans`.
- Disk space: `list_drives`; `scan_folder` (`path`, `limit`) scans and keeps the result;
  `list_folder` (`path`, `limit`), `largest_files` (`limit`) and `file_types` (`limit`) read it.
- Drivers: `drivers` (`all`): every device's driver and the devices with a problem;
  `driver_updates`: what Windows Update offers (10 to 60 seconds).
- Hardware: `system_info` (`section`), `sensors` (one set of live readings), `benchmark` (`test`).

## Development

Tauri 2, Rust and plain HTML/CSS/JavaScript. See the [development and release
guide](docs/DEVELOPMENT.md) for prerequisites, clone, build, tests and signed-release instructions.
A push to `main` alone does not publish an executable.

---

## Português

**Disponibilidade:** em 27/10/2026, ainda não há release nem executável publicado. Consulte
[Releases](https://github.com/Skale-Club/xpecs/releases) ou as [instruções de
compilação](docs/DEVELOPMENT.md). As atualizações dependem de uma release assinada e da ação do
usuário.

O Xpecs cuida do PC e mostra o hardware: verificação de saúde, limpeza, espaço em disco, drivers,
programas de inicialização e apps, no estilo do CCleaner, TreeSize e Driver Booster, mais
informações do sistema, sensores ao vivo e benchmarks, no estilo do AIDA64. É um exe pequeno, sem
instalação e sem driver; só pede administrador nas ações que precisam. Ele junta o que eram o Xcare
e o Xpace.

- Abra o `Xpecs.exe`. O idioma fica no canto inferior esquerdo; o selo no alto à direita mostra se
  ele roda como administrador.
- **Visão geral** dá uma nota de 0 a 100 e aponta o que resolver. **Limpeza** mede cada categoria e
  apaga só o que você marcar, depois de confirmar.
- **Espaço em disco**: digite um caminho ou parte do nome de uma pasta (Ctrl+L), clique nas suas
  pastas, numa análise recente ou num disco. Árvore com treemap, maiores arquivos e tipos; mover
  para a Lixeira ou excluir definitivamente após revisão. Pastas do sistema e do perfil em si são
  protegidas.
- **Drivers**: dispositivos com problema, atualizações do Windows Update (os mesmos drivers
  assinados das Atualizações opcionais), instaladas como administrador depois de um ponto de
  restauração; driver de vídeo com botão para o app do fabricante; lista de todos os drivers e
  backup. O Xpecs nunca baixa drivers de outros sites.
- **Hardware**, **Sensores** e **Benchmarks**: clique em qualquer valor para copiar; **Exportar
  relatório** salva tudo num HTML em `Data/reports`. Temperatura da CPU, ventoinhas e voltagens
  aparecem com o **LibreHardwareMonitor** (gratuito e portátil) aberto.
- **Exclusão forçada** apaga arquivos teimosos depois de revisar quais programas os usam, com opção
  de fechar esses programas ou excluir ao reiniciar.
- **Claude Code e Codex**: `Xpecs.exe --mcp` roda o Xpecs como servidor MCP local, sem janela, para
  assistentes de IA lerem o estado do PC, o espaço em disco, os drivers e o hardware (só leitura).
  Veja [Use with Claude Code and Codex](#use-with-claude-code-and-codex).

## Español

**Disponibilidad:** al 27/10/2026, aún no hay release ni ejecutable publicado. Consulte
[Releases](https://github.com/Skale-Club/xpecs/releases) o las [instrucciones de
compilación](docs/DEVELOPMENT.md). Las actualizaciones requieren una release firmada y la acción del
usuario.

Xpecs cuida el PC y muestra el hardware: revisión de salud, limpieza, espacio en disco,
controladores, programas de inicio y apps, al estilo de CCleaner, TreeSize y Driver Booster, más
información del sistema, sensores en vivo y benchmarks, al estilo de AIDA64. Es un exe pequeño, sin
instalación y sin controlador propio; solo pide administrador en las acciones que lo necesitan.
Reúne lo que eran Xcare y Xpace.

- Abre `Xpecs.exe`. El idioma está en la esquina inferior izquierda; la etiqueta arriba a la
  derecha indica si se ejecuta como administrador.
- **Vista general** da una nota de 0 a 100 y señala qué resolver. **Limpieza** mide cada categoría y
  borra solo lo que marques, tras confirmar.
- **Espacio en disco**: escribe una ruta o parte del nombre de una carpeta (Ctrl+L), o haz clic en
  tus carpetas, un análisis reciente o una unidad. Árbol con treemap, archivos más grandes y tipos;
  mover a la Papelera o eliminar definitivamente tras revisar. Las carpetas del sistema y del perfil
  en sí están protegidas.
- **Controladores**: dispositivos con problemas, actualizaciones de Windows Update (los mismos
  controladores firmados de Actualizaciones opcionales), instaladas como administrador tras un punto
  de restauración; controlador de gráficos con botón a la app del fabricante; lista de todos los
  controladores y copia de seguridad. Xpecs nunca descarga controladores de otros sitios.
- **Hardware**, **Sensores** y **Benchmarks**: haz clic en cualquier valor para copiarlo; **Exportar
  informe** guarda todo en un HTML en `Data/reports`. La temperatura de la CPU, los ventiladores y
  los voltajes aparecen con **LibreHardwareMonitor** (gratuito y portátil) abierto.
- **Eliminación forzada** borra archivos rebeldes tras revisar qué programas los usan, con opción de
  cerrarlos o eliminar al reiniciar.
- **Claude Code y Codex**: `Xpecs.exe --mcp` ejecuta Xpecs como servidor MCP local, sin ventana,
  para que asistentes de IA lean el estado del PC, el espacio en disco, los controladores y el
  hardware (solo lectura). Consulta [Use with Claude Code and Codex](#use-with-claude-code-and-codex).

## Skale Club apps

These Windows apps belong to the [Skale-Club organization](https://github.com/Skale-Club).
They share navy interfaces, blue accents, transparent product symbols, and English,
Portuguese and Spanish UI text. Each app has its own executable and portable Data folder.

| App | Purpose |
| --- | --- |
| [Xpecs](https://github.com/Skale-Club/xpecs) | Care for the PC (cleaning, disk space, drivers, startup, apps) and inspect hardware, live sensors and benchmarks |
| [Xink](https://github.com/Skale-Club/xink) | Draw over the screen during presentations or recordings |
| [Xnote](https://github.com/Skale-Club/xnote) | Transcribe meetings and turn notes into summaries and tasks |
| [Xvert](https://github.com/Skale-Club/xvert) | Convert videos, music and pictures with FFmpeg |

Repository access may be required depending on project visibility. Report problems through the
relevant repository, including app version, Windows version and reproduction steps. Remove
personal information from reports and transcripts before sharing.


---

© 2026 [Skale Club](https://skale.club)
