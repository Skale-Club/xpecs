# Xpecs

Portable system information, live sensors and benchmarks for Windows, in the
spirit of AIDA64. One small exe: no installer, no driver, no administrator
rights. 🇧🇷 [Português](#português) · 🇪🇸 [Español](#español)

Made by **[Skale Club](https://skale.club)**.

## Download

Get the latest `Xpecs.exe` from **[Releases](https://github.com/skaleclub/xpecs/releases/latest)**. It keeps itself up to date: when a new version is out, the app offers it (at the top of the window) and swaps itself in place, keeping your settings. Every update is signed and checked before it is used.

## Run it

Double-click `Xpecs.exe`. It needs WebView2, which comes with Windows 11.
Settings (just the language for now) live in `Data\xpecs.json` next to the exe;
reports go to `Data\reports`. Carry the folder anywhere. The folder follows the PortableApps layout: the exe, `App\AppInfo\` (name,
version, icon) and `Data\` for everything the app saves.

Language: the selector at the bottom left (English, Português, Español).

## What it shows

| Page | Contents |
|---|---|
| Summary | Everything at a glance, plus live CPU, memory, GPU and drive readings |
| Processor | Name, cores (performance + efficient), threads, base clock, caches, instruction sets; live usage and clock per thread |
| Motherboard | Board, BIOS version and date, system model |
| Memory | Installed/usable, slots, and every module: size, type, speed, maker, part number, voltage |
| Graphics | GPU, VRAM, drivers, PCIe link, clocks and power limit (NVIDIA via NVML); monitors with model and size |
| Storage | Every drive (interface, type, capacity, firmware, health) with live temperature, read/write speed and activity; volumes with free space |
| Network | Adapters with status, link speed, MAC, IP addresses, gateway, DNS; live traffic |
| Operating system | Edition, version, build, install date, last boot, audio devices |
| Sensors | Every live reading with current, min, max and a 60-second graph |
| Benchmarks | Memory read, write, copy and latency; CPU integer and floating point, single thread and all threads; stress test |

Click any value to copy it. **Export report** saves everything (including a
sensor snapshot and benchmark results) as one HTML file and opens it.

## About temperatures

Without a kernel driver Windows does not expose the CPU package temperature,
fans or motherboard voltages. AIDA64 installs its own driver for that; Xpecs
stays driver-free so it remains portable. What you get without a driver:

- GPU temperature, load, clocks, power and fan (NVIDIA)
- NVMe and most SATA drive temperatures
- The ACPI thermal zone (on many desktops a fixed board value, not the CPU)

For the rest, open **LibreHardwareMonitor** (free, portable): while it runs,
Xpecs picks up all its sensors automatically and lists them on the Sensors page.

## Benchmarks

- Memory: 512 MB buffers on every thread, larger than any cache. Results in
  GB/s (higher is better); latency is a random pointer chase through 256 MB, in
  nanoseconds (lower is better).
- CPU: about a second of 64-bit integer hashing and FP64 Mandelbrot iterations,
  on one thread and on all threads. "Scaling" is all threads ÷ one thread.
- Stress test: loads every thread at 100% until you stop it, with live usage,
  clocks and temperatures beside it.

Close other heavy programs for steady numbers.

## Build from source

This is a Tauri v2 app (Rust + plain HTML/JS, no bundler).
Needs Rust, Node.js and the Visual C++ Build Tools.

```bash
npm install
npm run build    # src-tauri/target/release/xpecs.exe
```

- `src-tauri/src/info.rs`: hardware information (WMI, CPUID, NVML).
- `src-tauri/src/sensors.rs`: live sensors, sent to the page every second.
- `src-tauri/src/bench.rs`: benchmarks and stress test.
- `src-tauri/src/winapi.rs`: performance counters, CPU topology, drive temperatures, registry.
- `src/app.js`, `src/style.css`: the window; `src/i18n.js`: English, Portuguese, Spanish.
- `make-icon.ps1`: draws the app icon.

## Releasing a new version

```powershell
./scripts/release.ps1 0.2.0
```

It sets the version in `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` and `package.json`, commits, tags `v0.2.0` and pushes. GitHub Actions then builds the exe, signs it with the update key (the `TAURI_SIGNING_PRIVATE_KEY` secret) and publishes the release; installed copies offer it on their next start.

The private key lives outside the repository, in `%USERPROFILE%\.tauri\skale-x-updater.key`. Keep a backup: without it no new update can be signed.

---

## Português

**Download:** [Releases](https://github.com/skaleclub/xpecs/releases/latest). O app avisa quando há versão nova e se atualiza sozinho, mantendo as configurações.

O Xpecs mostra informações do sistema, sensores ao vivo e benchmarks, no
estilo do AIDA64. É um exe pequeno, sem instalação, sem driver e sem precisar
de administrador.

- Abra o `Xpecs.exe`. O idioma fica no canto inferior esquerdo.
- Clique em qualquer valor para copiar. **Exportar relatório** salva tudo num
  HTML na pasta `reports`.
- A temperatura do encapsulamento da CPU, as ventoinhas e as voltagens exigem um
  driver. Abra o **LibreHardwareMonitor** (gratuito e portátil) e o Xpecs mostra
  os sensores dele automaticamente. Sem ele, você já tem as temperaturas da GPU
  e dos discos NVMe/SATA.
- Benchmarks: leitura, escrita, cópia e latência da memória; CPU inteiro e
  ponto flutuante em 1 thread e em todas; teste de estresse.

## Español

**Descarga:** [Releases](https://github.com/skaleclub/xpecs/releases/latest). La app avisa cuando hay una versión nueva y se actualiza sola, conservando la configuración.

Xpecs muestra información del sistema, sensores en vivo y benchmarks, al
estilo de AIDA64. Es un exe pequeño, sin instalación, sin controlador y sin
necesidad de administrador.

- Abre `Xpecs.exe`. El idioma está en la esquina inferior izquierda.
- Haz clic en cualquier valor para copiarlo. **Exportar informe** guarda todo
  en un HTML en la carpeta `reports`.
- La temperatura del encapsulado de la CPU, los ventiladores y los voltajes
  necesitan un controlador. Abre **LibreHardwareMonitor** (gratuito y portátil)
  y Xpecs mostrará sus sensores automáticamente. Sin él ya tienes las
  temperaturas de la GPU y de los discos NVMe/SATA.
- Benchmarks: lectura, escritura, copia y latencia de memoria; CPU entero y
  coma flotante en 1 hilo y en todos; prueba de estrés.

---

© 2026 [Skale Club](https://skale.club)
